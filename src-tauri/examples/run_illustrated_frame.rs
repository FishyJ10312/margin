//! Explicit, one-shot acceptance run of the app's complete-frame image path.
use liubai_lib::{
    art_ai::{AiConfig, ArtAi, DesignGeometry},
    artwork::ArtScene,
    frame_art, imaging, layout,
    model::{PixelRect, RenderSettings},
};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
#[derive(Deserialize)]
struct Workspace {
    photos: Vec<Photo>,
    settings: RenderSettings,
}
#[derive(Deserialize)]
struct Photo {
    path: String,
}
fn save<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).map_err(|e| e.to_string())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--live") {
        return Err(
            "Explicit --live required; one image generation with the saved image provider".into(),
        );
    }
    let app = PathBuf::from(args.get(2).ok_or("app data")?);
    let out = PathBuf::from(args.get(3).ok_or("output")?);
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("image-started"))
        .map_err(|_| "Refusing duplicate image request")?;
    let state: Workspace =
        serde_json::from_slice(&fs::read(app.join("state.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if state.photos.len() != 1 {
        return Err("Exactly one imported photo required".into());
    }
    let source = Path::new(&state.photos[0].path);
    let mut config: AiConfig = serde_json::from_slice(
        &fs::read(app.join("model-settings.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if config.image.provider != "codex" {
        return Err("Saved image provider must be Codex".into());
    }
    config.design_density = "balanced".into();
    config.frame_design = liubai_lib::art_direction::FrameDesign {
        style: "graphic".into(),
        placement: "corners".into(),
        palette: "custom".into(),
        colors: vec!["#315D69".into(), "#C27B5E".into(), "#D6C3A0".into()],
        texture: false,
        notes: "用清晰的几何色带、线织和少量圆弧设计两处对角，呼应照片蓝绿、沙色和少量橙色；保持摄影作品的展示感。不画风景或人物，不加水彩、做旧、阴影或文字。四边内沿严格贴合指定照片开口。".into(),
    };
    let info = imaging::inspect_image(source)?;
    let mut base = state.settings.clone();
    base.watermark.kind = "none".into();
    base.export.long_edge = None;
    let l = layout::calculate(info.width, info.height, &base)?;
    let n = |v: u32, t: u32| (v as f64 / t as f64 * 1000.).round() as u32;
    let g = DesignGeometry {
        canvas_width: l.width,
        canvas_height: l.height,
        border_percent: base.border_percent,
        photo_bounds: PixelRect {
            x: n(l.x, l.width),
            y: n(l.y, l.height),
            width: n(l.photo_width, l.width),
            height: n(l.photo_height, l.height),
        },
    };
    let (data, w, h) = imaging::analysis_image(source)?;
    let ai = ArtAi::new(out.join("isolated-model"));
    let prepared = ai.prepare_with_geometry(data, w, h, g)?;
    save(&out.join("generation-config.json"), &config)?;
    let start = Instant::now();
    let result =
        tauri::async_runtime::block_on(ai.complete_frame(&prepared.token, config, String::new()));
    save(
        &out.join("image-run.json"),
        &serde_json::json!({"elapsedSeconds":start.elapsed().as_secs_f64(),"ok":result.is_ok(),"error":result.as_ref().err()}),
    )?;
    let generated = result?;
    save(&out.join("generated.json"), &generated)?;
    base.frame_color = "#FAF5E9".into();
    base.decoration.kind = "none".into();
    base.artwork = Some(ArtScene {
        frame_image: Some(frame_art::from_geometry(generated.image_path, &prepared)?),
        ..Default::default()
    });
    save(&out.join("frame-settings.json"), &base)?;
    let mut review = base.clone();
    review.export.format = "png".into();
    review.export.long_edge = Some(1800);
    imaging::export_image(source, &review, &out.join("illustrated-preview.png"))?;
    if args.iter().any(|a| a == "--preview-only") {
        println!(
            "{}",
            serde_json::json!({"ok":true,"imageSeconds":start.elapsed().as_secs_f64(),"previewOnly":true})
        );
        return Ok(());
    }
    let full = Instant::now();
    base.export.format = "jpg".into();
    imaging::export_image(source, &base, &out.join("illustrated-full.jpg"))?;
    println!(
        "{}",
        serde_json::json!({"ok":true,"imageSeconds":start.elapsed().as_secs_f64(),"fullExportSeconds":full.elapsed().as_secs_f64(),"width":l.width,"height":l.height})
    );
    Ok(())
}
