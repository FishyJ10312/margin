//! Parent kills only this isolated runner during its private temporary export.
use liubai_lib::{art_ai, imaging, layout, model, source_io};
#[allow(dead_code)]
#[path = "../src/app_models.rs"]
mod app_models;
#[allow(dead_code)]
#[path = "../src/jobs.rs"]
mod jobs;
#[allow(dead_code)]
#[path = "../src/storage.rs"]
mod storage;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<_> = std::env::args().collect();
    let stage = args.get(1).ok_or("init/start/resume")?;
    let root = PathBuf::from(args.get(2).ok_or("isolated QA directory")?);
    if stage == "init" {
        if root.exists() {
            return Err("Refusing an existing directory".into());
        }
        fs::create_dir_all(&root)?;
        let mut seed = 92345u32;
        image::RgbImage::from_fn(1800, 1200, |_, _| {
            let mut c = [0; 3];
            for v in &mut c {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *v = (seed >> 24) as u8;
            }
            image::Rgb(c)
        })
        .save(root.join("one.png"))?;
        image::RgbImage::from_pixel(16, 16, image::Rgb([40, 80, 120]))
            .save(root.join("two.png"))?;
        let b = jobs::Backend::new(root.join("state"))?;
        b.import_photos(
            ["one.png", "two.png"]
                .map(|n| root.join(n).to_string_lossy().into_owned())
                .to_vec(),
        )?;
        fs::create_dir(root.join("exports"))?;
        fs::write(root.join("exports/keep.txt"), b"keep existing")?;
        return Ok(());
    }
    let backend = jobs::Backend::new(root.join("state"))?;
    let before = backend.snapshot()?;
    if stage == "start" {
        let mut s = model::RenderSettings::default();
        s.export.format = "png".into();
        backend.start_batch(
            root.join("exports").to_string_lossy().into_owned(),
            s,
            before.photos.iter().map(|p| p.id.clone()).collect(),
            Arc::new(|_| {}),
        )?;
    } else if stage == "resume" {
        assert_eq!(before.batch.as_ref().unwrap().status, "interrupted");
        backend.resume_batch(false, Arc::new(|_| {}))?;
    } else {
        return Err("Unknown stage".into());
    }
    let start = Instant::now();
    loop {
        let batch = backend.snapshot()?.batch.unwrap();
        if batch.status != "running" {
            assert!(batch.items.iter().all(|i| i.status == "completed"));
            break;
        }
        if start.elapsed() > Duration::from_secs(60) {
            return Err("Timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if stage == "resume" {
        let entries: Vec<_> = fs::read_dir(root.join("exports"))?
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let stale: Vec<_> = entries
            .iter()
            .filter(|n| n.starts_with(".liubai-") || n.starts_with(".margin-"))
            .collect();
        assert!(
            stale.is_empty(),
            "Incomplete temporary exports survived restart: {stale:?}"
        );
        assert_eq!(entries.len(), 3);
        assert_eq!(fs::read(root.join("exports/keep.txt"))?, b"keep existing");
        println!(
            "{}",
            serde_json::json!({"recoveredAfterRealSigkill":true,"completed":2,"temporaryFiles":0,"preexistingOutputPreserved":true})
        );
    }
    Ok(())
}
