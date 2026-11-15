//! Local-only verification of an existing frame; never calls any model.
use liubai_lib::{
    imaging, layout,
    model::{Ratio, RenderSettings},
};
use std::{fs, path::PathBuf, time::Instant};
fn rss() -> u64 {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
fn hash(p: &std::path::Path) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, &fs::read(p).unwrap())
        .as_ref()
        .to_vec()
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let source = PathBuf::from(args.get(1).ok_or("source")?);
    let settings = PathBuf::from(args.get(2).ok_or("settings")?);
    let out = PathBuf::from(args.get(3).ok_or("output")?);
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let original = hash(&source);
    let mut s: RenderSettings =
        serde_json::from_slice(&fs::read(settings).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let info = imaging::inspect_image(&source)?;
    s.watermark.kind = "none".into();
    s.export.long_edge = Some(1800);
    s.export.format = "png".into();
    let t = Instant::now();
    imaging::export_image(&source, &s, &out.join("illustrated-preview.png"))?;
    let review_ms = t.elapsed().as_secs_f64() * 1000.;
    let mut plain = s.clone();
    plain.artwork = None;
    imaging::export_image(&source, &plain, &out.join("baseline.png"))?;
    let l = layout::calculate(info.width, info.height, &s)?;
    let a = image::open(out.join("illustrated-preview.png"))
        .map_err(|e| e.to_string())?
        .to_rgb8();
    let b = image::open(out.join("baseline.png"))
        .map_err(|e| e.to_string())?
        .to_rgb8();
    let photo_equal = image::imageops::crop_imm(&a, l.x, l.y, l.photo_width, l.photo_height)
        .to_image()
        == image::imageops::crop_imm(&b, l.x, l.y, l.photo_width, l.photo_height).to_image();
    if !photo_equal {
        return Err("Original photo pixels changed".into());
    }
    drop(a);
    drop(b);
    let mut times = Vec::new();
    let mut retained = Vec::new();
    s.export.long_edge = None;
    for i in 0..20 {
        let mut variant = s.clone();
        variant.border_percent = 12. + f64::from(i % 9);
        let t = Instant::now();
        imaging::preview(&source, &variant, 1400)?;
        times.push(t.elapsed().as_secs_f64() * 1000.);
        retained.push(rss());
    }
    // Change final canvas proportions without generating again.
    for (name, ratio) in [
        (
            "portrait",
            Ratio {
                width: 4.,
                height: 5.,
            },
        ),
        (
            "landscape",
            Ratio {
                width: 3.,
                height: 2.,
            },
        ),
    ] {
        let mut adapted = s.clone();
        adapted.ratio = Some(ratio);
        adapted.export.long_edge = Some(1600);
        adapted.export.format = "png".into();
        imaging::export_image(&source, &adapted, &out.join(format!("adapted-{name}.png")))?;
    }
    let mut export_ms = Vec::new();
    s.export.format = "jpg".into();
    for i in 0..4 {
        let t = Instant::now();
        imaging::export_image(
            &source,
            &s,
            &out.join(format!("illustrated-full-{}.jpg", i + 1)),
        )?;
        export_ms.push(t.elapsed().as_secs_f64() * 1000.);
        retained.push(rss());
    }
    if hash(&source) != original {
        return Err("Source file changed".into());
    }
    times.sort_by(f64::total_cmp);
    let report = serde_json::json!({"photoPixelsEqualToUnframedRender":photo_equal,"sourceFileUnchanged":true,"reviewExportMs":review_ms,"warmPreviewMedianMs":times[times.len()/2],"warmPreviewMaxMs":times.last(),"fullExportMs":export_ms,"retainedRssKiB":retained,"modelCalls":0});
    fs::write(
        out.join("local-verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
