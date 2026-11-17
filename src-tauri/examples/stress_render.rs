use liubai_lib::{
    imaging,
    model::{Ratio, RenderSettings},
};
use std::{fs, path::PathBuf, time::Instant};
fn rss_kib() -> u64 {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let a: Vec<String> = std::env::args().collect();
    let source = PathBuf::from(a.get(1).ok_or("source")?);
    let output = PathBuf::from(a.get(2).ok_or("output")?);
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let before = fs::read(&source).map_err(|e| e.to_string())?;
    let mut s = RenderSettings::default();
    if let Some(i) = a.iter().position(|v| v == "--art") {
        let text =
            fs::read_to_string(a.get(i + 1).ok_or("art json")?).map_err(|e| e.to_string())?;
        let designs: Vec<liubai_lib::art_ai::FrameSuggestion> =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
        s.artwork = Some(designs[0].artwork.clone());
    }
    if let Some(i) = a.iter().position(|v| v == "--settings") {
        s = serde_json::from_slice(
            &fs::read(a.get(i + 1).ok_or("settings json")?).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    s.ratio = Some(Ratio {
        width: 1.0,
        height: 1.0,
    });
    s.border_percent = 15.0;
    let fonts = imaging::list_fonts();
    let info = imaging::inspect_image(&source)?;
    let cold = Instant::now();
    imaging::preview(&source, &s, 1400)?;
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let warm_rss = rss_kib();
    let mut times = Vec::new();
    for i in 0..60 {
        if a.iter().any(|a| a == "--patterns") {
            s.decoration.kind =
                ["paper", "linen", "dots", "lines", "waves", "corners"][i as usize % 6].into();
        }
        s.border_percent = 5.0 + f64::from(i % 20);
        s.frame_color = format!("#{:02X}8899", 80 + i);
        let t = Instant::now();
        imaging::preview(&source, &s, 1400)?;
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        let c = imaging::sample_color(&source, info.width / 2 + i, info.height / 2)?;
        if c.patch_data_url.len() > 10000 {
            return Err("unbounded patch".into());
        }
    }
    if !a.iter().any(|v| v == "--no-sampling") {
        let proxy = imaging::sampler_image(&source, None, None)?;
        if proxy.width.max(proxy.height) > 2048 {
            return Err("unbounded sampling proxy".into());
        }
        for i in 0..120 {
            let t = imaging::sampler_image(
                &source,
                Some((i * 137) % info.width),
                Some((i * 229) % info.height),
            )?;
            if t.width > 544 || t.height > 544 {
                return Err("unbounded tile".into());
            }
        }
    }
    let after_previews_rss = rss_kib();
    s.border_percent = 15.0;
    s.watermark.kind = "text".into();
    s.watermark.text = "留白 FishyJ · 摄影".into();
    s.watermark.position = "center".into();
    s.watermark.offset_x = -100.0;
    s.watermark.offset_y = 80.0;
    if let Some(f) = fonts.iter().find(|f| f.family == "Times New Roman") {
        s.watermark.font_family = Some(f.family.clone());
        s.watermark.font_path = Some(f.path.clone());
    }
    if !a.iter().any(|v| v == "--no-detail") {
        for i in 0..30 {
            s.watermark.offset_x = i as f64;
            imaging::watermark_detail(&source, &s)?;
        }
    }
    if a.iter().any(|v| v == "--no-mark") {
        s.watermark.kind = "none".into();
    }
    let mut outputs = Vec::new();
    let mut export_rss = Vec::new();
    for i in 0..24 {
        if a.iter().any(|a| a == "--patterns") {
            s.decoration.kind =
                ["paper", "linen", "dots", "lines", "waves", "corners"][i as usize % 6].into();
        }
        s.export.format = if i % 2 == 0 { "jpg" } else { "png" }.into();
        s.export.long_edge = if i % 4 < 2 { None } else { Some(3840) };
        let p = output.join(format!("stress-{i}.{}", s.export.format));
        let dims = imaging::export_image(&source, &s, &p)?;
        outputs.push(dims);
        export_rss.push(rss_kib());
    }
    let after_exports_rss = rss_kib();
    if export_rss[23] > export_rss[15] + 64 * 1024 || after_exports_rss > 1024 * 1024 {
        return Err(format!("export RSS grew across cycles: {:?}", export_rss));
    }
    if fs::read(&source).map_err(|e| e.to_string())? != before {
        return Err("source mutated".into());
    }
    times.sort_by(f64::total_cmp);
    // Cache is bounded by one source + one <=2048px proxy; repeated settings must not grow per request.
    if after_previews_rss > warm_rss + 64 * 1024 {
        return Err(format!(
            "preview RSS growth exceeded 64 MiB: {warm_rss} -> {after_previews_rss}"
        ));
    }
    imaging::clear_image_cache();
    println!(
        "{}",
        serde_json::json!({"sourceDimensions":[info.width,info.height],"systemFontFamilies":fonts.len(),"previews":60,"colorSamples":60,"originalTiles":120,"watermarkDetails":30,"artworkElements":s.artwork.as_ref().map(|v|v.elements.len()).unwrap_or(0),"coldPreviewMs":cold_ms,"warmPreviewMedianMs":times[30],"warmPreviewP95Ms":times[57],"warmRssKiB":warm_rss,"afterPreviewsRssKiB":after_previews_rss,"afterExportsRssKiB":after_exports_rss,"exportRssKiB":export_rss,"outputs":outputs,"sourceUnchanged":true})
    );
    Ok(())
}
