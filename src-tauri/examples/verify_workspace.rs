//! Isolated release-core acceptance. Never reads the user's app data or models.
use liubai_lib::{art_ai, imaging, layout, model, source_io};
// Exercise the private desktop backend directly without widening its production API.
#[allow(dead_code)]
#[path = "../src/app_models.rs"]
mod app_models;
#[allow(dead_code)]
#[path = "../src/jobs.rs"]
mod jobs;
#[allow(dead_code)]
#[path = "../src/storage.rs"]
mod storage;
use app_models::BatchSnapshot;
use jobs::{Backend, ProgressSink};
use model::RenderSettings;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn inventory(root: &Path) -> Vec<(String, u64)> {
    let mut result = Vec::new();
    fn walk(base: &Path, here: &Path, out: &mut Vec<(String, u64)>) {
        for e in fs::read_dir(here).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((
                    p.strip_prefix(base).unwrap().to_string_lossy().into(),
                    p.metadata().unwrap().len(),
                ));
            }
        }
    }
    walk(root, root, &mut result);
    result.sort();
    result
}
fn finished(backend: &Backend) -> BatchSnapshot {
    let started = Instant::now();
    loop {
        let b = backend.snapshot().unwrap().batch.unwrap();
        if b.status != "running" {
            return b;
        }
        assert!(started.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn retry_start(action: impl Fn() -> Result<BatchSnapshot, String>) {
    let start = Instant::now();
    loop {
        match action() {
            Ok(_) => return,
            Err(e) if e == "已有导出任务正在进行。" && start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => panic!("{e}"),
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let a: Vec<_> = std::env::args().collect();
    let fixture = PathBuf::from(a.get(1).ok_or("fixture directory")?);
    let output = PathBuf::from(a.get(2).ok_or("new acceptance directory")?);
    if output.exists() {
        return Err("Use a new output directory to preserve prior evidence".into());
    }
    fs::create_dir_all(output.join("sources"))?;
    let mut files = Vec::new();
    for sub in ["照片", "另一文件夹"] {
        let dest = output.join("sources").join(sub);
        fs::create_dir(&dest)?;
        for entry in fs::read_dir(fixture.join(sub))? {
            let p = entry?.path();
            if p.is_file() {
                let to = dest.join(p.file_name().unwrap());
                fs::copy(&p, &to)?;
                files.push(to);
            }
        }
    }
    let heic = output.join("sources/静态测试.heic");
    fs::copy(fixture.join("apple-synthetic.heic"), &heic)?;
    files.push(heic);
    files.sort();
    let originals: Vec<_> = files
        .iter()
        .map(|p| (p.clone(), fs::read(p).unwrap()))
        .collect();
    let paths: Vec<_> = files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let state = output.join("state");
    let backend = Backend::new(state.clone())?;
    let mut imports = paths.clone();
    imports.push(paths[0].clone());
    imports.push(output.join("sources").to_string_lossy().into_owned());
    let imported = backend.import_photos(imports)?;
    assert_eq!(imported.photos.len(), 53);
    assert_eq!(imported.errors.len(), 1);
    assert!(backend.import_photos(paths)?.photos.is_empty());
    let before = inventory(&state);
    for i in 0..80 {
        let mut s = RenderSettings::default();
        s.border_percent = 1.0 + (i % 20) as f64;
        backend.preview(&imported.photos[i as usize % 2].id, &s)?;
    }
    assert_eq!(
        before,
        inventory(&state),
        "Repeated previews must not write growing disk caches"
    );
    let failing = &imported.photos[7];
    let saved = fs::read(&failing.path)?;
    fs::write(&failing.path, b"QA corruption after import")?;
    let weak = Arc::downgrade(&backend);
    let sink: ProgressSink = Arc::new(move |b| {
        if b.items.iter().any(|i| i.status == "completed") {
            if let Some(b) = weak.upgrade() {
                b.cancel_batch().unwrap();
            }
        }
    });
    let mut s = RenderSettings::default();
    s.export.long_edge = Some(1000);
    let exports = output.join("exports");
    fs::create_dir(&exports)?;
    let sentinel = exports.join("照片_01-framed.jpg");
    fs::write(&sentinel, b"KEEP EXISTING")?;
    backend.start_batch(
        exports.to_string_lossy().into_owned(),
        s.clone(),
        imported.photos.iter().map(|p| p.id.clone()).collect(),
        sink,
    )?;
    assert!(backend
        .start_batch(
            exports.to_string_lossy().into_owned(),
            s,
            imported.photos.iter().map(|p| p.id.clone()).collect(),
            Arc::new(|_| {})
        )
        .is_err());
    let cancelled = finished(&backend);
    assert_eq!(cancelled.status, "cancelled");
    let completed_before = cancelled
        .items
        .iter()
        .filter(|i| i.status == "completed")
        .count();
    assert_eq!(completed_before, 1);
    // Give the worker's final guard release a bounded opportunity before simulating relaunch.
    std::thread::sleep(Duration::from_millis(20));
    drop(backend);
    let backend = Backend::new(state.clone())?;
    retry_start(|| backend.resume_batch(false, Arc::new(|_| {})));
    let resumed = finished(&backend);
    assert_eq!(
        resumed
            .items
            .iter()
            .filter(|i| i.status == "completed")
            .count(),
        52
    );
    assert_eq!(
        resumed
            .items
            .iter()
            .filter(|i| i.status == "failed")
            .count(),
        1
    );
    fs::write(&failing.path, saved)?;
    retry_start(|| backend.resume_batch(true, Arc::new(|_| {})));
    let retried = finished(&backend);
    assert!(retried.items.iter().all(|i| i.status == "completed"));
    assert_eq!(fs::read(&sentinel)?, b"KEEP EXISTING");
    assert_eq!(fs::read_dir(&exports)?.count(), 54);
    assert!(!fs::read_dir(&exports)?.any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".liubai-")));
    for (path, bytes) in originals {
        assert_eq!(fs::read(path)?, bytes);
    }
    let report = serde_json::json!({"validImports":53,"corruptImportErrors":1,"duplicateReimportAdded":0,"previewsWithoutStorageGrowth":80,"completedBeforeCancel":completed_before,"resumedWithoutDuplicateOutputs":true,"failedThenRetried":1,"completedExports":53,"existingOutputPreserved":true,"temporaryOutputsRemaining":0,"sourceCopiesUnchanged":true,"note":"Isolated release core; not a native UI or real process-crash test"});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
