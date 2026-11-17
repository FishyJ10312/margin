//! Explicit live use of the user's single imported photo, with isolated results.
//! This never writes app settings, templates, queues or the original photo.
use liubai_lib::{
    art_ai::{AiConfig, ArtAi, DesignGeometry, FrameSuggestion},
    imaging, layout,
    model::{PixelRect, RenderSettings},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Deserialize)]
struct Workspace {
    photos: Vec<ImportedPhoto>,
    settings: RenderSettings,
}
#[derive(Serialize, Deserialize)]
struct ImportedPhoto {
    id: String,
    name: String,
    path: String,
    width: u32,
    height: u32,
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    photo: ImportedPhoto,
    settings: RenderSettings,
    config: AiConfig,
    source_sha256: String,
}
fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, String> {
    serde_json::from_slice(&fs::read(p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn save(p: &Path, v: &impl Serialize) -> Result<(), String> {
    fs::write(p, serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn digest(p: &Path) -> Result<String, String> {
    Ok(ring::digest::digest(
        &ring::digest::SHA256,
        &fs::read(p).map_err(|e| e.to_string())?,
    )
    .as_ref()
    .iter()
    .map(|v| format!("{v:02x}"))
    .collect())
}
fn apply(base: &RenderSettings, s: &FrameSuggestion) -> RenderSettings {
    let mut out = base.clone();
    out.frame_color = s.frame_color.clone();
    out.border_percent = s.border_percent;
    out.decoration = s.decoration.clone();
    out.artwork = Some(s.artwork.clone());
    out
}
fn geometry(w: u32, h: u32, settings: &RenderSettings) -> Result<DesignGeometry, String> {
    let mut safe = settings.clone();
    safe.watermark.kind = "none".into();
    safe.export.long_edge = None;
    let l = layout::calculate(w, h, &safe)?;
    let n = |v: u32, total: u32| (v as f64 / total as f64 * 1000.).round() as u32;
    Ok(DesignGeometry {
        canvas_width: l.width,
        canvas_height: l.height,
        border_percent: settings.border_percent,
        photo_bounds: PixelRect {
            x: n(l.x, l.width),
            y: n(l.y, l.height),
            width: n(l.photo_width, l.width),
            height: n(l.photo_height, l.height),
        },
    })
}
fn demo(base: &RenderSettings) -> RenderSettings {
    let mut s = base.clone();
    s.export.format = "png".into();
    s.export.long_edge = Some(2400);
    s
}
fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|s| s == "--live") {
        return Err("Explicit --live required: sends the imported photo thumbnail to the saved OpenAI subscription".into());
    }
    let out = PathBuf::from(args.get(1).ok_or("output directory")?);
    let action = args.get(2).ok_or("analyze/refine")?;
    if !matches!(action.as_str(), "analyze" | "refine") {
        return Err("analyze/refine".into());
    }
    fs::create_dir_all(out.join("isolated-state")).map_err(|e| e.to_string())?;
    let snapshot_path = out.join("isolated-state/snapshot.json");
    if out.join(format!("{action}-started.json")).exists() {
        return Err(
            "This stage was already started; refusing an accidental duplicate live request".into(),
        );
    }
    let mut snapshot: Snapshot = if action == "analyze" {
        let app = PathBuf::from(args.get(3).ok_or("app data directory")?);
        let state: Workspace = read(&app.join("state.json"))?;
        if state.photos.len() != 1 {
            return Err(
                "This runner needs exactly one imported photo to avoid choosing the wrong image"
                    .into(),
            );
        }
        let photo = state.photos.into_iter().next().unwrap();
        let config: AiConfig = read(&app.join("model-settings.json"))?;
        if config.provider != "pi" || config.pi_provider != "openai" {
            return Err("The saved analysis connection must be Pi/OpenAI".into());
        }
        let s = Snapshot {
            source_sha256: digest(Path::new(&photo.path))?,
            photo,
            settings: state.settings,
            config,
        };
        save(&snapshot_path, &s)?;
        s
    } else {
        read(&snapshot_path)?
    };
    let source = PathBuf::from(&snapshot.photo.path);
    if digest(&source)? != snapshot.source_sha256 {
        return Err("Source changed since analysis; reopen the task".into());
    }
    let info = imaging::inspect_image(&source)?;
    let ai = ArtAi::new(out.join("isolated-state/model"));
    let catalog = tauri::async_runtime::block_on(ai.pi.catalog(
        snapshot.config.pi_path.as_deref(),
        "openai",
        false,
    ))?;
    if !catalog.connected || !catalog.models.iter().any(|m| m.id == snapshot.config.model) {
        return Err("The saved OpenAI subscription/model is not ready".into());
    }
    let (data, w, h) = imaging::analysis_image(&source)?;
    // Local review thumbnail uses precisely the metadata-free input the model sees.
    use base64::Engine;
    if action == "analyze" {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.split_once(',').ok_or("thumbnail")?.1)
            .map_err(|e| e.to_string())?;
        fs::write(out.join("photo-review.jpg"), bytes).map_err(|e| e.to_string())?;
    }
    let mut base = snapshot.settings.clone();
    let selected;
    let prepared = if action == "analyze" {
        snapshot.config.style_direction="基于这张真实海滩纪实照片设计三套不同的摄影装帧边框，整体安静、简约、有设计感。参考海水与天空的蓝、沙滩米色，少量呼应毛巾的暖橙。完整保留照片的倾斜视角，不旋转、不裁切或改画面。三套分别探索：细腻连续潮线、克制的几何/条纹呼应、轻盈的旅行手账式线迹。每套4到8个元素即可，2到3色、最多两档细线；主装饰集中一处，用一处很轻的呼应，大量留白，不机械撒图标，不加星星或卡通凑数。右下角预留署名净空，不在画布右下五分之一区域安排装饰。维持当前比例与边框宽度。".into();
        snapshot.config.design_density = "simple".into();
        selected = None;
        let g = geometry(info.width, info.height, &base)?;
        let mut neutral = demo(&base);
        neutral.artwork = None;
        imaging::export_image(&source, &neutral, &out.join("baseline.png"))?;
        ai.prepare_with_geometry(data, w, h, g)?
    } else {
        let choice = args
            .iter()
            .position(|v| v == "--choice")
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or("--choice 1/2/3 required")?;
        let suggestions: Vec<FrameSuggestion> = read(&out.join("suggestions.json"))?;
        let current = suggestions
            .get(choice.checked_sub(1).ok_or("choice")?)
            .ok_or("choice")?
            .clone();
        base = apply(&base, &current);
        let i = args
            .iter()
            .position(|v| v == "--direction-file")
            .ok_or("--direction-file required")?;
        snapshot.config.style_direction =
            fs::read_to_string(args.get(i + 1).ok_or("direction file")?)
                .map_err(|e| e.to_string())?;
        let mut safe = base.clone();
        safe.watermark.kind = "none".into();
        safe.export.long_edge = Some(768);
        let before = imaging::preview(&source, &safe, 768)?.data_url;
        selected = Some(choice);
        ai.prepare_refinement(
            data,
            w,
            h,
            geometry(info.width, info.height, &base)?,
            before,
            current,
        )?
    };
    let start = Instant::now();
    save(
        &out.join(format!("{action}-started.json")),
        &serde_json::json!({"action":action,"model":snapshot.config.model,"provider":"pi/openai","photo":snapshot.photo.name,"thumbnailDimensions":[w,h],"referenceImages":if action=="analyze"{1}else{2}}),
    )?;
    println!(
        "Starting one real {action} request: {} / {}",
        snapshot.photo.name, snapshot.config.model
    );
    let result = tauri::async_runtime::block_on(ai.generate(
        &prepared.token,
        snapshot.config.clone(),
        String::new(),
    ));
    let seconds = start.elapsed().as_secs_f64();
    let result = match result {
        Ok(v) => v,
        Err(e) => {
            save(
                &out.join(format!("{action}-failed.json")),
                &serde_json::json!({"seconds":seconds,"error":e}),
            )?;
            return Err(e);
        }
    };
    if result.len() != if action == "analyze" { 3 } else { 1 } {
        return Err("Unexpected result count".into());
    }
    let mut outputs = Vec::new();
    if action == "analyze" {
        save(&out.join("suggestions.json"), &result)?;
        for (i, s) in result.iter().enumerate() {
            let settings = apply(&base, s);
            let path = out.join(format!("suggestion-{}.png", i + 1));
            let dims = imaging::export_image(&source, &demo(&settings), &path)?;
            save(
                &out.join(format!("suggestion-{}-settings.json", i + 1)),
                &settings,
            )?;
            outputs.push(serde_json::json!({"file":path.file_name().unwrap().to_string_lossy(),"dimensions":dims}));
        }
    } else {
        let settings = apply(&base, &result[0]);
        save(&out.join("refined.json"), &result)?;
        save(&out.join("refined-settings.json"), &settings)?;
        let dims = imaging::export_image(&source, &demo(&settings), &out.join("refined.png"))?;
        outputs.push(serde_json::json!({"file":"refined.png","dimensions":dims}));
        let mut full = settings.clone();
        full.export.format = "jpg".into();
        full.export.jpeg_quality = 95;
        full.export.long_edge = None;
        let dims = imaging::export_image(&source, &full, &out.join("photo-refined-full.jpg"))?;
        outputs.push(serde_json::json!({"file":"photo-refined-full.jpg","dimensions":dims}));
    }
    if digest(&source)? != snapshot.source_sha256 {
        return Err("Original file changed".into());
    }
    let l = layout::calculate(info.width, info.height, &demo(&base))?;
    let report = serde_json::json!({"live":true,"mocked":false,"provider":"pi/openai","model":snapshot.config.model,"action":action,"logicalRequests":1,"seconds":seconds,"selectedSuggestion":selected,"photo":snapshot.photo.name,"sourceDimensions":[info.width,info.height],"thumbnailDimensions":[w,h],"ratio":base.ratio,"borderPercent":base.border_percent,"demoPhotoBounds":{"x":l.x,"y":l.y,"width":l.photo_width,"height":l.photo_height},"results":result.len(),"outputs":outputs,"sourceFileUnchanged":true,"originalSha256":snapshot.source_sha256,"userWorkspaceWritten":false});
    save(&out.join(format!("{action}-result.json")), &report)?;
    println!("{report}");
    Ok(())
}
