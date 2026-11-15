//! By default this exports local samples only. --live explicitly exercises Codex
//! with a synthetic test image, never a photo from the user's library.
use liubai_lib::{
    art_ai::{AiConfig, ArtAi},
    imaging,
    model::RenderSettings,
};
use std::{fs, path::PathBuf};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let output = PathBuf::from(args.get(1).ok_or("output directory")?);
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let source = output.join("synthetic-landscape.png");
    let mut picture = image::RgbImage::new(900, 600);
    for (x, y, p) in picture.enumerate_pixels_mut() {
        *p = if y < 380 {
            image::Rgb([
                70 + (x / 30) as u8,
                140 + (y / 8) as u8,
                185 + (y / 12) as u8,
            ])
        } else {
            image::Rgb([205 + (x / 40) as u8, 180 + (y / 30) as u8, 145])
        };
        if (x as i32 - 610).pow(2) + (y as i32 - 145).pow(2) < 45 * 45 {
            *p = image::Rgb([239, 206, 149]);
        }
    }
    picture.save(&source).map_err(|e| e.to_string())?;
    for kind in ["paper", "linen", "dots", "lines", "waves", "corners"] {
        let mut s = RenderSettings::default();
        s.border_percent = 10.0;
        s.frame_color = "#F2EDDF".into();
        s.decoration.kind = kind.into();
        s.decoration.opacity = 0.18;
        s.export.format = "png".into();
        imaging::export_image(&source, &s, &output.join(format!("sample-{kind}.png")))?;
    }
    if args.iter().any(|a| a == "--live") {
        let state = ArtAi::new(output.join("model-state"));
        let (data, w, h) = imaging::analysis_image(&source)?;
        let mut target = RenderSettings::default();
        target.border_percent = 10.;
        target.ratio = Some(liubai_lib::model::Ratio {
            width: 1.,
            height: 1.,
        });
        let l = liubai_lib::layout::calculate(900, 600, &target)?;
        let n = |v: u32, total: u32| (v as f64 / total as f64 * 1000.).round() as u32;
        let p = state.prepare_with_geometry(
            data,
            w,
            h,
            liubai_lib::art_ai::DesignGeometry {
                canvas_width: l.width,
                canvas_height: l.height,
                border_percent: 10.,
                photo_bounds: liubai_lib::model::PixelRect {
                    x: n(l.x, l.width),
                    y: n(l.y, l.height),
                    width: n(l.photo_width, l.width),
                    height: n(l.photo_height, l.height),
                },
            },
        )?;
        let start = std::time::Instant::now();
        let suggestions = tauri::async_runtime::block_on(state.generate(
            &p.token,
            AiConfig {
                style_direction: "海洋旅行手账，蓝橙两色，连续线条像潮汐，角落小鱼形成组群；简约但构图完整，不要机械撒星星".into(),
                ..AiConfig::default()
            },
            String::new(),
        ))?;
        for (i, suggestion) in suggestions.iter().enumerate() {
            let mut s = target.clone();
            s.frame_color = suggestion.frame_color.clone();
            s.border_percent = suggestion.border_percent;
            s.decoration = suggestion.decoration.clone();
            s.artwork = Some(suggestion.artwork.clone());
            s.export.format = "png".into();
            imaging::export_image(
                &source,
                &s,
                &output.join(format!("codex-suggestion-{}.png", i + 1)),
            )?;
        }
        fs::write(
            output.join("codex-suggestions.json"),
            serde_json::to_vec_pretty(&suggestions).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::json!({"liveCodex":true,"syntheticPhotoOnly":true,"suggestions":suggestions.len(),"seconds":start.elapsed().as_secs_f64()})
        );
    } else {
        println!("6 local motifs exported; no model called");
    }
    Ok(())
}
