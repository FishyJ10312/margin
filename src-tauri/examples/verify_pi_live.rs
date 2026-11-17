//! Explicit, one-request-at-a-time live acceptance using a synthetic image.
//! analyze creates three designs; refine reuses one without re-running analysis.
use liubai_lib::{
    art_ai::{AiConfig, ArtAi, DesignGeometry, FrameSuggestion},
    imaging, layout,
    model::{PixelRect, Ratio, RenderSettings},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

fn json_file(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn base_settings() -> RenderSettings {
    let mut s = RenderSettings::default();
    s.ratio = Some(Ratio {
        width: 1.,
        height: 1.,
    });
    s.border_percent = 12.;
    s.export.format = "png".into();
    s
}
fn apply(s: &RenderSettings, v: &FrameSuggestion) -> RenderSettings {
    let mut s = s.clone();
    s.frame_color = v.frame_color.clone();
    s.border_percent = v.border_percent;
    s.decoration = v.decoration.clone();
    s.artwork = Some(v.artwork.clone());
    s
}
fn geometry(s: &RenderSettings) -> Result<DesignGeometry, String> {
    let l = layout::calculate(1200, 800, s)?;
    let n = |v: u32, total: u32| (v as f64 / total as f64 * 1000.).round() as u32;
    Ok(DesignGeometry {
        canvas_width: l.width,
        canvas_height: l.height,
        border_percent: s.border_percent,
        photo_bounds: PixelRect {
            x: n(l.x, l.width),
            y: n(l.y, l.height),
            width: n(l.photo_width, l.width),
            height: n(l.photo_height, l.height),
        },
    })
}
fn synthetic_photo(path: &Path) -> Result<(), String> {
    let mut p = image::RgbImage::new(1200, 800);
    for (x, y, c) in p.enumerate_pixels_mut() {
        let t = y as f64 / 470.;
        let mix = |a: f64, b: f64| (a + (b - a) * t.clamp(0., 1.)) as u8;
        let mut color = if y < 470 {
            [mix(93., 244.), mix(142., 193.), mix(166., 143.)]
        } else {
            let d = (y - 470) as f64 / 330.;
            [
                (51. + 24. * d) as u8,
                (111. + 20. * d) as u8,
                (124. + 9. * d) as u8,
            ]
        };
        let dx = x as f64 - 824.;
        let dy = y as f64 - 277.;
        if dx * dx + dy * dy < 64. * 64. {
            color = [250, 222, 163];
        }
        // Two coastal headlands and restrained sun reflections make a clear
        // warm/cool landscape for style analysis, without any personal media.
        let horizon = 473. + 18. * (x as f64 / 140.).sin();
        if x < 550 && y as f64 > horizon - 130. + x as f64 * 0.19 && y < 510 {
            color = [74, 99, 99];
        }
        if x < 375 && y as f64 > 485. - 90. + x as f64 * 0.29 && y < 555 {
            color = [52, 83, 86];
        }
        let shore = 1030. - (y as f64 - 520.) * 1.7;
        if y > 520 && x as f64 > shore {
            color = [190, 158, 116];
        }
        if y > 520 && (x as f64 - shore).abs() < 7. {
            color = [221, 211, 180];
        }
        if y > 490 && y < 700 && y % 23 < 2 && (x as f64 - 824.).abs() < (y - 460) as f64 * 0.18 {
            color = [181, 174, 132];
        }
        *c = image::Rgb(color);
    }
    p.save(path).map_err(|e| e.to_string())
}
fn export_checked(source: &Path, settings: &RenderSettings, path: &Path) -> Result<(), String> {
    imaging::export_image(source, settings, path)?;
    let original = image::open(source).map_err(|e| e.to_string())?.to_rgb8();
    let output = image::open(path).map_err(|e| e.to_string())?.to_rgb8();
    let l = layout::calculate(original.width(), original.height(), settings)?;
    for (x, y, p) in original.enumerate_pixels() {
        if output.get_pixel(l.x + x, l.y + y) != p {
            return Err("Photo pixels changed".into());
        }
    }
    Ok(())
}
fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|v| v == "--live") {
        return Err(
            "Explicit --live required: uses the saved Pi/OpenAI subscription allowance".into(),
        );
    }
    let output = PathBuf::from(args.get(1).ok_or("output directory")?);
    let action = args.get(2).ok_or("analyze or refine")?;
    if !matches!(action.as_str(), "analyze" | "refine") {
        return Err("analyze or refine".into());
    }
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let config_path = PathBuf::from(args.get(3).ok_or("public model configuration path")?);
    let mut config: AiConfig =
        serde_json::from_slice(&fs::read(config_path).map_err(|e| e.to_string())?)
            .map_err(|_| "Invalid model configuration")?;
    if config.provider != "pi" || config.pi_provider != "openai" {
        return Err("Expected the explicitly selected Pi/OpenAI subscription".into());
    }
    let ai = ArtAi::new(output.join("isolated-model-state"));
    let catalog =
        tauri::async_runtime::block_on(ai.pi.catalog(config.pi_path.as_deref(), "openai", false))?;
    if !catalog.connected || !catalog.models.iter().any(|m| m.id == config.model) {
        return Err("Saved subscription/model is not ready".into());
    }
    let source = output.join("synthetic-coast.png");
    let mut settings = base_settings();
    let stage_file = output.join(format!("{action}-result.json"));
    if stage_file.exists() {
        return Err(
            "This stage has already completed; refusing an accidental duplicate model request"
                .into(),
        );
    }
    if action == "analyze" {
        synthetic_photo(&source)?;
    }
    let bytes_before = fs::read(&source).map_err(|e| e.to_string())?;
    let (data, w, h) = imaging::analysis_image(&source)?;
    let prepared;
    let mut chosen = None;
    if action == "analyze" {
        config.style_direction="海岸旅行的简约艺术边框。参考暖杏色落日、蓝绿色海水与岩岸的色调。设计三套有区别的构图：连续潮汐线、交织航线、少量成组小鱼或贝壳。装饰只在留白，主装饰集中一角，其余留白克制；每套8到12个元素，避免机械撒图标。照片保持完整。".into();
        prepared = ai.prepare_with_geometry(data, w, h, geometry(&settings)?)?;
    } else {
        let suggestions: Vec<FrameSuggestion> = serde_json::from_slice(
            &fs::read(output.join("suggestions.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let choice = args
            .iter()
            .position(|v| v == "--choice")
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or("--choice 1, 2 or 3 required")?;
        let current = suggestions
            .get(choice.checked_sub(1).ok_or("Invalid choice")?)
            .ok_or("Invalid choice")?
            .clone();
        settings = apply(&settings, &current);
        let mut small = settings.clone();
        small.export.long_edge = Some(768);
        let before = imaging::preview(&source, &small, 768)?.data_url;
        config.style_direction="做一次看得见的几何精修：保留海岸暖杏与蓝绿主题，将原来左上的主要交织曲线移到左下宽留白，重画为更舒展、大小错落的连续潮线；右上用两条短曲线轻呼应。加两尾大小不同的细线小鱼融入左下主装饰，形成同向游动的关系，不要散落图标。总元素6到10个，主次清楚，保留至少一半空白。实际改动控制点、形状尺寸和位置；所有控制点及图案含笔画完整在安全留白带内。".into();
        chosen = Some(choice);
        prepared = ai.prepare_refinement(data, w, h, geometry(&settings)?, before, current)?;
    }
    println!(
        "Starting one live {action} request via Pi/OpenAI model {}",
        config.model
    );
    let started = Instant::now();
    let result = tauri::async_runtime::block_on(ai.generate(
        &prepared.token,
        config.clone(),
        String::new(),
    ))?;
    let seconds = started.elapsed().as_secs_f64();
    if result.len() != if action == "analyze" { 3 } else { 1 } {
        return Err("Unexpected number of results".into());
    }
    if action == "analyze" {
        json_file(&output.join("suggestions.json"), &result)?;
        for (i, v) in result.iter().enumerate() {
            let s = apply(&settings, v);
            export_checked(
                &source,
                &s,
                &output.join(format!("suggestion-{}.png", i + 1)),
            )?;
            json_file(
                &output.join(format!("suggestion-{}-settings.json", i + 1)),
                &s,
            )?;
        }
    } else {
        let s = apply(&settings, &result[0]);
        export_checked(&source, &s, &output.join("refined.png"))?;
        json_file(&output.join("refined-settings.json"), &s)?;
        json_file(&output.join("refined.json"), &result)?;
    }
    if fs::read(&source).map_err(|e| e.to_string())? != bytes_before {
        return Err("Source file changed".into());
    }
    let report = serde_json::json!({"live":true,"mocked":false,"provider":"pi/openai","model":config.model,"action":action,"requests":1,"selectedSuggestion":chosen,"seconds":seconds,"results":result.len(),"syntheticPhotoOnly":true,"sourceFileUnchanged":true,"exportedPhotoPixelsUnchanged":true,"inputDimensions":[1200,800],"outputDimensions":[1392,1392]});
    json_file(&stage_file, &report)?;
    println!("{report}");
    Ok(())
}
