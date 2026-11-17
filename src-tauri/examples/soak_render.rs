//! Paced active core soak; fixed QA outputs bound disk use. No models or user state.
use liubai_lib::{
    imaging,
    model::{Ratio, RenderSettings},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn rss() -> u64 {
    std::process::Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "rss="])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
fn digest(p: &Path) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, &fs::read(p).unwrap())
        .as_ref()
        .to_vec()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let a: Vec<_> = std::env::args().collect();
    let input = PathBuf::from(a.get(1).ok_or("fixture directory")?);
    let output = PathBuf::from(a.get(2).ok_or("new output directory")?);
    let seconds: u64 = a.get(3).ok_or("duration in seconds")?.parse()?;
    if output.exists() || !(1..=7200).contains(&seconds) {
        return Err("Use a new QA directory and a bounded duration".into());
    }
    fs::create_dir_all(&output)?;
    let paths = [input.join("one.png"), input.join("two.png")];
    let hashes: Vec<_> = paths.iter().map(|p| digest(p)).collect();
    let mut settings = RenderSettings::default();
    settings.watermark.kind = "text".into();
    settings.watermark.text = "Margin 稳定性 · QA".into();
    settings.watermark.color = "#4A3830".into();
    let start = Instant::now();
    let mut n = 0u64;
    let mut rows = Vec::new();
    let mut warm = None;
    let mut max_late = 0;
    loop {
        let iteration = Instant::now();
        let path = &paths[(n / 8) as usize % 2];
        settings.border_percent = 5.0 + (n % 15) as f64;
        settings.ratio = if n % 2 == 0 {
            None
        } else {
            Some(Ratio {
                width: 4.0,
                height: 5.0,
            })
        };
        settings.watermark.size_percent = 2.0 + (n % 3) as f64;
        settings.watermark.font_family =
            Some(["Noto Sans SC", "ZCOOL XiaoWei", "Ma Shan Zheng"][(n % 3) as usize].into());
        let at = Instant::now();
        imaging::preview(path, &settings, 1400)?;
        let preview_ms = at.elapsed().as_secs_f64() * 1000.0;
        imaging::watermark_detail(path, &settings)?;
        let info = imaging::inspect_image(path)?;
        let tile = imaging::sampler_image(
            path,
            Some((n as u32 * 17) % info.width),
            Some((n as u32 * 29) % info.height),
        )?;
        assert!(tile.width <= 544 && tile.height <= 544);
        if n % 16 == 0 {
            settings.export.format = if n % 32 == 0 { "jpg" } else { "png" }.into();
            imaging::export_image(
                path,
                &settings,
                &output.join(format!("latest.{}", settings.export.format)),
            )?;
        }
        let memory = rss();
        if n == 128 {
            warm = Some(memory);
        }
        if n >= 128 {
            max_late = max_late.max(memory);
        }
        assert!(
            memory < 800 * 1024,
            "QA backend exceeded its 800 MiB stop threshold"
        );
        if n % 16 == 0 {
            let files: Vec<_> = fs::read_dir(&output)?
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "jpg" || e == "png"))
                .collect();
            let bytes: u64 = files.iter().map(|p| p.metadata().unwrap().len()).sum();
            assert!(files.len() <= 2 && bytes < 64 * 1024 * 1024);
            rows.push(serde_json::json!({"iterations":n+1,"elapsedSeconds":start.elapsed().as_secs_f64(),"rssKiB":memory,"previewMs":preview_ms,"retainedOutputs":files.len(),"outputBytes":bytes}));
            fs::write(
                output.join("samples.json"),
                serde_json::to_vec_pretty(&rows)?,
            )?;
            if n % 160 == 0 {
                println!("{}", rows.last().unwrap());
            }
        }
        n += 1;
        if start.elapsed() >= Duration::from_secs(seconds) {
            break;
        }
        // Low duty cycle while the user's other work continues.
        if iteration.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_secs(1) - iteration.elapsed());
        }
    }
    for (p, h) in paths.iter().zip(hashes) {
        assert_eq!(digest(p), h);
    }
    if let Some(warm) = warm {
        assert!(
            max_late < warm + 96 * 1024,
            "Late-cycle retained RSS grew by more than 96 MiB"
        );
    }
    imaging::clear_image_cache();
    let report = serde_json::json!({"durationSeconds":start.elapsed().as_secs_f64(),"iterations":n,"previews":n,"watermarkDetails":n,"samplerTiles":n,"lateWarmRssKiB":warm,"lateMaxRssKiB":max_late,"sourceCopiesUnchanged":true,"limits":"Active release-core soak, separate from native WebKit UI observations; not OS memory exhaustion or a whole-app peak guarantee"});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
