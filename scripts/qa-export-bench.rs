//! Copied into isolated queue benchmark crates by qa-export-performance.py.
#![allow(dead_code)]
use liubai_lib::{art_ai, imaging, layout, model, source_io};
mod app_models;
mod jobs;
mod storage;
use jobs::{Backend, ProgressSink};
use std::{
    fs,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<_> = std::env::args().collect();
    let inputs = PathBuf::from(args.get(1).ok_or("input directory")?);
    let output = PathBuf::from(args.get(2).ok_or("fresh output directory")?);
    if output.exists() {
        return Err("Use a new benchmark directory".into());
    }
    fs::create_dir_all(&output)?;
    let backend = Backend::new(output.join("state"))?;
    let mut paths = fs::read_dir(inputs)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let imported =
        backend.import_photos(paths.iter().map(|p| p.to_string_lossy().into()).collect())?;
    assert!(imported.errors.is_empty());
    assert_eq!(imported.photos.len(), paths.len());
    let ids = imported.photos.iter().map(|p| p.id.clone()).collect();
    let settings = model::RenderSettings::default();
    let (sender, receiver) = mpsc::channel();
    let sink: ProgressSink = Arc::new(move |batch| {
        if batch.status != "running" {
            let _ = sender.send(batch);
        }
    });
    storage::qa_reset_metrics();
    let started = Instant::now();
    backend.start_batch(
        output.join("exports").to_string_lossy().into(),
        settings,
        ids,
        sink,
    )?;
    let batch = receiver.recv_timeout(Duration::from_secs(90))?;
    let export_ms = started.elapsed().as_secs_f64() * 1000.0;
    let (journal_saves, journal_nanos) = storage::qa_metrics();
    assert_eq!(batch.status, "completed");
    for item in &batch.items {
        assert_eq!(item.status, "completed", "{:?}", item.error);
        let path = PathBuf::from(item.output_path.as_ref().unwrap());
        let info = imaging::inspect_image(&path)?;
        assert_eq!(Some(info.width), item.width);
        assert_eq!(Some(info.height), item.height);
    }
    assert!(fs::read_dir(output.join("exports"))?
        .all(|e| { e.is_ok_and(|e| !e.file_name().to_string_lossy().starts_with('.')) }));
    let report = serde_json::json!({"exportMs": export_ms, "photos": batch.items.len(),
        "journalSaves": journal_saves, "journalMs": journal_nanos as f64 / 1_000_000.0,
        "allOutputsValidated": true, "timingScope": "start_batch to terminal progress event; imports and output validation excluded"});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
