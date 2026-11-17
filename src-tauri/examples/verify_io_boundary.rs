//! Fault injection restricted to this QA child process. Never fills a volume.
#[cfg(unix)]
use liubai_lib::{art_ai, imaging, layout, model, source_io};
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../src/app_models.rs"]
mod app_models;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../src/jobs.rs"]
mod jobs;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../src/storage.rs"]
mod storage;

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };
    let root = PathBuf::from(std::env::args().nth(1).ok_or("new QA output directory")?);
    if root.exists() {
        return Err("Output exists; preserve prior evidence".into());
    }
    fs::create_dir_all(&root)?;
    let noisy = root.join("noisy.png");
    let tiny = root.join("tiny.png");
    let mut seed = 12345u32;
    let image = image::RgbImage::from_fn(600, 500, |_, _| {
        let mut c = [0; 3];
        for v in &mut c {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = (seed >> 24) as u8;
        }
        image::Rgb(c)
    });
    image.save(&noisy)?;
    image::RgbImage::from_pixel(8, 8, image::Rgb([40, 80, 120])).save(&tiny)?;
    let originals = [fs::read(&noisy)?, fs::read(&tiny)?];
    let backend = jobs::Backend::new(root.join("state"))?;
    let photos = backend
        .import_photos(vec![
            noisy.to_string_lossy().into(),
            tiny.to_string_lossy().into(),
        ])?
        .photos;
    fn finish(b: &jobs::Backend) -> app_models::BatchSnapshot {
        let at = Instant::now();
        loop {
            let s = b.snapshot().unwrap().batch.unwrap();
            if s.status != "running" {
                return s;
            }
            assert!(at.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let mut previous = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut previous) } != 0 {
        return Err("getrlimit failed".into());
    }
    let old_signal = unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN) };
    let quota = libc::rlimit {
        rlim_cur: 8192,
        rlim_max: previous.rlim_max,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &quota) } != 0 {
        return Err("setrlimit failed".into());
    }
    let output = root.join("exports");
    backend.start_batch(
        output.to_string_lossy().into(),
        model::RenderSettings::default(),
        photos.iter().map(|p| p.id.clone()).collect(),
        Arc::new(|_| {}),
    )?;
    let failed = finish(&backend);
    // Restore the quota before retry; changes apply only to this process.
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &previous) }, 0);
    unsafe { libc::signal(libc::SIGXFSZ, old_signal) };
    assert_eq!(
        failed.items.iter().filter(|i| i.status == "failed").count(),
        1
    );
    assert_eq!(
        failed
            .items
            .iter()
            .filter(|i| i.status == "completed")
            .count(),
        1
    );
    assert!(!output.join("noisy-framed.jpg").exists());
    assert_eq!(fs::read_dir(&output)?.count(), 1);
    let until = Instant::now();
    loop {
        match backend.resume_batch(true, Arc::new(|_| {})) {
            Ok(_) => break,
            Err(e) if e == "已有导出任务正在进行。" && until.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e.into()),
        }
    }
    let recovered = finish(&backend);
    assert!(recovered.items.iter().all(|i| i.status == "completed"));
    assert_eq!(fs::read_dir(&output)?.count(), 2);
    for p in [&noisy, &tiny].into_iter().zip(originals) {
        assert_eq!(fs::read(p.0)?, p.1);
    }
    let report = serde_json::json!({"mechanism":"RLIMIT_FSIZE=8192 bytes in this child only; EFBIG, not actual ENOSPC or a full disk","failedIsolated":1,"followingPhotoCompleted":1,"retriedAfterQuotaRestored":1,"noPartialFinalOrTemporaryFiles":true,"sourcesUnchanged":true,"quotaRestored":true,"firstError":failed.items.iter().find(|i|i.status=="failed").and_then(|i|i.error.as_ref())});
    fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
#[cfg(not(unix))]
fn main() {
    eprintln!("This process-quota runner is Unix-only; it is not Windows validation.");
    std::process::exit(2);
}
