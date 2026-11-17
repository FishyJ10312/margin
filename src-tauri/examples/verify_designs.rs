//! Local-only gallery acceptance; --live-image/--live-refine are explicit quota-consuming checks.
use liubai_lib::{
    art_ai::{AiConfig, ArtAi, DesignGeometry, FrameSuggestion},
    designs, imaging, layout,
    model::{PixelRect, RenderSettings},
};
use std::{fs, path::PathBuf};
fn main() -> Result<(), String> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args: Vec<_> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).ok_or("output path")?);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (name, w, h) in [
        ("landscape", 900, 600),
        ("portrait", 600, 900),
        ("square", 700, 700),
    ] {
        let source = dir.join(format!("source-{name}.png"));
        let mut im = image::RgbImage::new(w, h);
        for (x, y, p) in im.enumerate_pixels_mut() {
            let t = y as f32 / h as f32;
            *p = if t < 0.58 {
                image::Rgb([95 + (x * 45 / w) as u8, 158 + (y * 50 / h) as u8, 176])
            } else {
                image::Rgb([175 + (y * 35 / h) as u8, 166 + (x * 30 / w) as u8, 130])
            };
            if (x as f32 - w as f32 * 0.7).powi(2) + (y as f32 - h as f32 * 0.22).powi(2)
                < (w.min(h) as f32 * 0.075).powi(2)
            {
                *p = image::Rgb([244, 221, 166]);
            }
        }
        im.save(&source).map_err(|e| e.to_string())?;
        for item in designs::library()? {
            let mut s = item.settings;
            s.export.format = "png".into();
            imaging::export_image(&source, &s, &dir.join(format!("{}-{name}.png", item.id)))?;
        }
    }
    let library = designs::library()?;
    fs::write(
        dir.join("library.json"),
        serde_json::to_vec_pretty(&library).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let service = ArtAi::new(dir.join("ai-state"));
    let source = dir.join("source-landscape.png");
    let (data, w, h) = imaging::analysis_image(&source)?;
    let settings = designs::settings("botanical").unwrap();
    let l = layout::calculate(900, 600, &settings)?;
    let n = |v: u32, t: u32| (v as f64 / t as f64 * 1000.).round() as u32;
    let geometry = DesignGeometry {
        canvas_width: l.width,
        canvas_height: l.height,
        border_percent: settings.border_percent,
        photo_bounds: PixelRect {
            x: n(l.x, l.width),
            y: n(l.y, l.height),
            width: n(l.photo_width, l.width),
            height: n(l.photo_height, l.height),
        },
    };
    if args.iter().any(|a| a == "--live-refine") {
        let before = imaging::preview(&source, &settings, 768)?.data_url;
        let current = FrameSuggestion {
            title: "枝叶笺".into(),
            rationale: String::new(),
            frame_color: settings.frame_color.clone(),
            border_percent: settings.border_percent,
            decoration: settings.decoration.clone(),
            artwork: settings.artwork.clone().unwrap(),
        };
        let p =
            service.prepare_refinement(data.clone(), w, h, geometry.clone(), before, current)?;
        let start = std::time::Instant::now();
        let result = tauri::async_runtime::block_on(service.generate(
            &p.token,
            AiConfig {
                style_direction:
                    "保持植物主题，减轻装饰强度，用更淡的暖绿色呼应照片，让画面更克制。".into(),
                ..Default::default()
            },
            String::new(),
        ))?;
        let r = &result[0];
        let mut s = settings.clone();
        s.frame_color = r.frame_color.clone();
        s.artwork = Some(r.artwork.clone());
        s.export.format = "png".into();
        imaging::export_image(&source, &s, &dir.join("live-refined.png"))?;
        fs::write(
            dir.join("live-refined.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "refine: {}",
            serde_json::json!({"seconds":start.elapsed().as_secs_f64(),"results":result.len(),"recipe":s.artwork.as_ref().unwrap().design.as_ref().unwrap().id})
        );
    }
    if args.iter().any(|a| a == "--live-image") {
        let p = service.prepare_with_geometry(data, w, h, geometry)?;
        let start = std::time::Instant::now();
        let result=tauri::async_runtime::block_on(service.handdrawn(&p.token,AiConfig{style_direction:"一组原创细腻水彩海洋手绘：两条小鱼沿同一条浅蓝水流游动，少量珊瑚与海草相连，横向构图，低饱和青蓝与暖珊瑚色，真实纸笔颗粒与不规则水彩边缘，透明背景，无文字。".into(),..Default::default()},false))?;
        let mut s = RenderSettings::default();
        s.border_percent = 18.;
        s.frame_color = "#F5F0E4".into();
        s.export.format = "png".into();
        s.artwork = Some(liubai_lib::artwork::ArtScene {
            stickers: vec![liubai_lib::stickers::Sticker {
                image_path: result.image_path.clone(),
                anchor: "bottom-left".into(),
                scale: 0.92,
                opacity: 1.,
                flip_x: false,
            }],
            ..Default::default()
        });
        for name in ["landscape", "portrait"] {
            imaging::export_image(
                &dir.join(format!("source-{name}.png")),
                &s,
                &dir.join(format!("live-handdrawn-{name}.png")),
            )?;
        }
        fs::write(
            dir.join("live-handdrawn-settings.json"),
            serde_json::to_vec_pretty(&s).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "image: {}",
            serde_json::json!({"seconds":start.elapsed().as_secs_f64(),"width":result.width,"height":result.height,"summary":result.summary})
        );
    }
    println!("{} designs × 3 orientations exported locally", library.len());
    Ok(())
}
