//! Local-only acceptance of curated styles and existing raster registration.
use liubai_lib::{designs, imaging, layout, model::RenderSettings};
use serde::Deserialize;
use std::{fs, path::PathBuf, time::Instant};
#[derive(Deserialize)]
struct Workspace {
    photos: Vec<Photo>,
    settings: RenderSettings,
}
#[derive(Deserialize)]
struct Photo {
    path: String,
}
fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--inspect-frame") {
        let check = liubai_lib::frame_art::validate_generated(args.get(2).ok_or("asset")?);
        println!(
            "{}",
            serde_json::json!({"accepted":check.is_ok(),"error":check.err()})
        );
        return Ok(());
    }
    let app = PathBuf::from(args.get(1).ok_or("app data")?);
    let out = PathBuf::from(args.get(2).ok_or("output")?);
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let state: Workspace =
        serde_json::from_slice(&fs::read(app.join("state.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let source = PathBuf::from(&state.photos.first().ok_or("photo required")?.path);
    let before = ring::digest::digest(
        &ring::digest::SHA256,
        &fs::read(&source).map_err(|e| e.to_string())?,
    );
    let mut settings = state.settings.clone();
    settings.watermark.kind = "none".into();
    settings.export.format = "png".into();
    settings.export.long_edge = Some(1600);
    let info = imaging::inspect_image(&source)?;
    let l = layout::calculate(info.width, info.height, &settings)?;
    imaging::export_image(&source, &settings, &out.join("existing-frame-aligned.png"))?;
    let mut plain = settings.clone();
    plain.artwork = None;
    plain.decoration.kind = "none".into();
    imaging::export_image(&source, &plain, &out.join("baseline.png"))?;
    let crop = |name: &str| -> Result<image::RgbImage, String> {
        let im = image::open(out.join(name))
            .map_err(|e| e.to_string())?
            .to_rgb8();
        Ok(image::imageops::crop_imm(&im, l.x, l.y, l.photo_width, l.photo_height).to_image())
    };
    let baseline = crop("baseline.png")?;
    if crop("existing-frame-aligned.png")? != baseline {
        return Err("Existing frame changed photo pixels".into());
    }
    let library = designs::library()?;
    fs::write(
        out.join("library.json"),
        serde_json::to_vec(&library).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let mut timings = Vec::new();
    let film_only = args.iter().any(|a| a == "--film-only");
    for item in library
        .iter()
        .filter(|item| !film_only || item.category == "film")
        .take(if film_only { usize::MAX } else { 8 })
    {
        let mut s = item.settings.clone();
        s.ratio = settings.ratio.clone();
        s.border_percent = settings.border_percent;
        s.export = settings.export.clone();
        let filename = format!("{}.png", item.id);
        let t = Instant::now();
        imaging::export_image(&source, &s, &out.join(&filename))?;
        if crop(&filename)? != baseline {
            return Err(format!("{} changed photo pixels", item.id));
        }
        timings.push(serde_json::json!({"id":item.id,"ms":t.elapsed().as_millis()}));
    }
    let after = ring::digest::digest(
        &ring::digest::SHA256,
        &fs::read(&source).map_err(|e| e.to_string())?,
    );
    if before.as_ref() != after.as_ref() {
        return Err("Source changed".into());
    }
    let report = serde_json::json!({"styles":library.len(),"newStylesVerified":timings.len(),"photoPixelsUnchanged":true,"originalUnchanged":true,"previewDimensions":[l.width,l.height],"renders":timings});
    fs::write(
        out.join("local-verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
