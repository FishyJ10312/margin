use base64::Engine;
use liubai_lib::{
    imaging, layout,
    model::{Ratio, RenderSettings},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

fn check(original: &Path, output: &Path, settings: &RenderSettings) -> Result<(), String> {
    let info = imaging::inspect_image(original)?;
    let expected = layout::calculate(info.width, info.height, settings)?;
    let dimensions = imaging::export_image(original, settings, output)?;
    let actual = image::image_dimensions(output).map_err(|e| e.to_string())?;
    if dimensions != actual || actual != (expected.width, expected.height) {
        return Err(format!(
            "Wrong dimensions for {}: {:?}",
            original.display(),
            actual
        ));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<String> = std::env::args().collect();
    let fixture = PathBuf::from(args.get(1).ok_or("Expected fixture directory")?);
    let output = PathBuf::from(args.get(2).ok_or("Expected output directory")?);
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut files = Vec::new();
    for directory in [fixture.join("照片"), fixture.join("另一文件夹")] {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| matches!(e, "jpg" | "png"))
            {
                files.push(path);
            }
        }
    }
    files.push(fixture.join("apple-synthetic.heic"));
    files.sort();
    let mut settings = RenderSettings::default();
    settings.ratio = Some(Ratio {
        width: 4.0,
        height: 5.0,
    });
    settings.frame_color = "#f4efdf".into();
    settings.watermark.kind = "text".into();
    settings.watermark.text = "留白 · FishyJ".into();
    settings.watermark.color = "#344c3b".into();
    settings.watermark.region = "border".into();
    settings.watermark.margin_percent = 0.5;
    settings.export.long_edge = Some(1600);
    let mut completed = 0;
    let mut rejected = Vec::new();
    for (index, path) in files.iter().enumerate() {
        let before = fs::read(path).map_err(|e| e.to_string())?;
        let destination = output.join(format!("batch-{index:02}.jpg"));
        match check(path, &destination, &settings) {
            Ok(()) => {
                completed += 1;
            }
            Err(error) if path.file_name().and_then(|p| p.to_str()) == Some("损坏.jpg") => {
                rejected.push(error)
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
        if fs::read(path).map_err(|e| e.to_string())? != before {
            return Err("Original changed".into());
        }
    }
    if completed < 53 || rejected.len() != 1 {
        return Err(format!(
            "Incomplete fixture verification: {completed} valid, {} rejected",
            rejected.len()
        ));
    }
    let source = fixture.join("照片/照片_01.jpg");
    settings.ratio = Some(Ratio {
        width: 1.0,
        height: 1.0,
    });
    settings.export.long_edge = None;
    settings.watermark.kind = "none".into();
    check(
        &source,
        &output.join("full-resolution-square.jpg"),
        &settings,
    )?;
    let square = image::image_dimensions(output.join("full-resolution-square.jpg"))
        .map_err(|e| e.to_string())?;
    if square != (4300, 4300) {
        return Err(format!("Square reference mismatch: {square:?}"));
    }
    settings.export.format = "png".into();
    settings.export.long_edge = Some(1200);
    settings.watermark.kind = "image".into();
    settings.watermark.image_path = Some(fixture.join("水印.png").to_string_lossy().into_owned());
    settings.watermark.region = "photo".into();
    settings.watermark.size_percent = 20.0;
    check(&source, &output.join("transparent-logo.png"), &settings)?;
    let preview = imaging::preview(&source, &settings, 1000)?;
    if (preview.width, preview.height) != (1200, 1200) {
        return Err("Preview export size mismatch".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(
            preview
                .data_url
                .split_once(',')
                .ok_or("Invalid preview data URL")?
                .1,
        )
        .map_err(|e| e.to_string())?;
    fs::write(output.join("preview.jpg"), bytes).map_err(|e| e.to_string())?;
    imaging::clear_image_cache();
    println!(
        "{}",
        serde_json::json!({"validPhotosExported":completed,"damagedPhotosRejected":rejected.len(),"fullResolutionSquare":square,"previewDimensions":[preview.width,preview.height],"sourceFilesUnchanged":true,"seconds":started.elapsed().as_secs_f64(),"output":output})
    );
    Ok(())
}
