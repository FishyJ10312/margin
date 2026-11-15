//! Small, bounded RGBA assets composited directly into the RGB border/ROI.
use crate::{artwork::ArtScene, layout::Layout};
use image::{ImageReader, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sticker {
    pub image_path: String,
    pub anchor: String,
    pub scale: f32,
    pub opacity: f32,
    #[serde(default)]
    pub flip_x: bool,
}
pub fn validate(s: &[Sticker]) -> Result<(), String> {
    if s.len() > 4 {
        return Err("手绘素材最多四张".into());
    }
    for s in s {
        if !matches!(
            s.anchor.as_str(),
            "top-left"
                | "top-center"
                | "top-right"
                | "bottom-left"
                | "bottom-center"
                | "bottom-right"
        ) || !s.scale.is_finite()
            || !(0.2..=1.).contains(&s.scale)
            || !s.opacity.is_finite()
            || !(0.0..=1.).contains(&s.opacity)
            || s.image_path.len() > 4096
        {
            return Err("手绘素材位置或大小无效".into());
        }
    }
    Ok(())
}
pub fn validate_png(bytes: &[u8], require_alpha: bool) -> Result<RgbaImage, String> {
    decode_png(bytes, require_alpha, true)
}
pub(crate) fn decode_png(
    bytes: &[u8],
    require_alpha: bool,
    trim: bool,
) -> Result<RgbaImage, String> {
    if bytes.len() > 20 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("手绘素材须为不超过 20 MiB 的 PNG".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let im = reader
        .decode()
        .map_err(|_| "无法解码手绘 PNG（最长边不超过 4096）")?;
    let mut rgba = im.to_rgba8();
    if !rgba.pixels().any(|p| p[3] > 8) {
        return Err("手绘素材完全透明".into());
    }
    if require_alpha && (!im.color().has_alpha() || !rgba.pixels().any(|p| p[3] < 8)) {
        return Err("模型未返回真正透明的 PNG，请重试并要求透明背景".into());
    }
    // Trim empty alpha margins for predictable placement; original asset remains untouched.
    let mut x0 = rgba.width();
    let mut y0 = rgba.height();
    let mut x1 = 0;
    let mut y1 = 0;
    for (x, y, p) in rgba.enumerate_pixels() {
        if p[3] > 4 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if trim {
        rgba = image::imageops::crop_imm(&rgba, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
    }
    if rgba.width().max(rgba.height()) > 2048 {
        rgba = image::DynamicImage::ImageRgba8(rgba)
            .resize(2048, 2048, image::imageops::FilterType::Triangle)
            .to_rgba8()
    }
    Ok(rgba)
}
struct Entry {
    key: String,
    trim: bool,
    modified: Option<SystemTime>,
    size: u64,
    image: Arc<RgbaImage>,
    opening: Option<crate::model::PixelRect>,
}
fn load(path: &str) -> Result<Arc<RgbaImage>, String> {
    load_variant(path, true).map(|v| v.0)
}
#[cfg(test)]
pub(crate) fn load_frame(path: &str) -> Result<Arc<RgbaImage>, String> {
    load_variant(path, false).map(|v| v.0)
}
pub(crate) fn registered_frame(
    path: &str,
) -> Result<(Arc<RgbaImage>, Option<crate::model::PixelRect>), String> {
    load_variant(path, false)
}
fn load_variant(
    path: &str,
    trim: bool,
) -> Result<(Arc<RgbaImage>, Option<crate::model::PixelRect>), String> {
    static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let (modified, size) = if path == "builtin:eucalyptus" {
        (None, 0)
    } else {
        let m = std::fs::metadata(path).map_err(|_| "找不到已保存的手绘素材")?;
        if !m.is_file() || m.len() > 20 * 1024 * 1024 {
            return Err("手绘素材过大或无效".into());
        }
        (m.modified().ok(), m.len())
    };
    let mut c = cache.lock().map_err(|_| "手绘素材缓存不可用")?;
    if let Some(index) = c
        .iter()
        .position(|v| v.key == path && v.trim == trim && v.modified == modified && v.size == size)
    {
        let entry = c.remove(index);
        let im = entry.image.clone();
        let opening = entry.opening.clone();
        c.push(entry);
        return Ok((im, opening));
    }
    let bytes = if path == "builtin:eucalyptus" {
        include_bytes!("../resources/art/eucalyptus-watercolor.png").to_vec()
    } else {
        std::fs::read(Path::new(path)).map_err(|_| "无法读取手绘素材")?
    };
    let im = Arc::new(decode_png(&bytes, false, trim)?);
    let opening = if trim {
        None
    } else {
        crate::frame_opening::detect(&im)
    };
    if c.len() >= 2 {
        c.remove(0);
    }
    c.push(Entry {
        key: path.into(),
        trim,
        modified,
        size,
        image: im.clone(),
        opening: opening.clone(),
    });
    Ok((im, opening))
}
pub fn draw_window(
    canvas: &mut RgbImage,
    scene: &ArtScene,
    l: Layout,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    let mut stickers = scene.stickers.clone();
    if scene.design.as_ref().is_some_and(|r| r.id == "watercolor") {
        stickers.insert(
            0,
            Sticker {
                image_path: "builtin:eucalyptus".into(),
                anchor: "bottom-left".into(),
                scale: 0.94 * scene.design.as_ref().unwrap().scale,
                opacity: scene.design.as_ref().unwrap().intensity.min(1.),
                flip_x: false,
            },
        );
    }
    for s in stickers {
        let bottom = s.anchor.starts_with("bottom");
        let bh = if bottom {
            l.height - l.y - l.photo_height
        } else {
            l.y
        };
        if bh < 2 || s.opacity == 0. {
            continue;
        }
        let im = load(&s.image_path)?;
        let pad = (bh as f32 * 0.07).max(1.);
        let corner_cluster = s.image_path == "builtin:eucalyptus";
        let maxh = if corner_cluster {
            (bh.min(l.x) as f32 * 2.15).min(l.width.min(l.height) as f32 * 0.30) * s.scale
        } else {
            (bh as f32 - pad * 2.) * s.scale
        };
        let maxw = l.width as f32 * 0.38;
        let k = (maxh / im.height() as f32).min(maxw / im.width() as f32);
        if k <= 0. {
            continue;
        }
        let dw = (im.width() as f32 * k).round().max(1.) as u32;
        let dh = (im.height() as f32 * k).round().max(1.) as u32;
        let x = if s.anchor.ends_with("left") {
            pad as u32
        } else if s.anchor.ends_with("right") {
            l.width.saturating_sub(dw + pad as u32)
        } else {
            l.width.saturating_sub(dw) / 2
        };
        let y = if bottom && corner_cluster {
            l.height.saturating_sub(dh + pad as u32)
        } else if bottom {
            l.height - bh + (bh - dh.min(bh)) / 2
        } else {
            (bh - dh.min(bh)) / 2
        };
        let xend = (x + dw).min(ox + canvas.width());
        let yend = (y + dh).min(oy + canvas.height());
        for dy in y.max(oy)..yend {
            for dx in x.max(ox)..xend {
                if dx >= l.x && dx < l.x + l.photo_width && dy >= l.y && dy < l.y + l.photo_height {
                    continue;
                }
                let mut fx = ((dx - x) as f32 + 0.5) / k - 0.5;
                if s.flip_x {
                    fx = im.width() as f32 - 1. - fx
                }
                let fy = ((dy - y) as f32 + 0.5) / k - 0.5;
                let fx = fx.clamp(0., im.width() as f32 - 1.);
                let fy = fy.clamp(0., im.height() as f32 - 1.);
                let ix = fx as u32;
                let iy = fy as u32;
                let tx = fx - ix as f32;
                let ty = fy - iy as f32;
                let mut a = 0.;
                let mut rgb = [0.; 3];
                for (xx, yy, weight) in [
                    (ix, iy, (1. - tx) * (1. - ty)),
                    ((ix + 1).min(im.width() - 1), iy, tx * (1. - ty)),
                    (ix, (iy + 1).min(im.height() - 1), (1. - tx) * ty),
                    (
                        (ix + 1).min(im.width() - 1),
                        (iy + 1).min(im.height() - 1),
                        tx * ty,
                    ),
                ] {
                    let p = im.get_pixel(xx, yy);
                    let alpha = p[3] as f32 / 255. * s.opacity * weight;
                    a += alpha;
                    for c in 0..3 {
                        rgb[c] += p[c] as f32 * alpha;
                    }
                }
                let p = canvas.get_pixel_mut(dx - ox, dy - oy);
                for c in 0..3 {
                    p[c] = (rgb[c] + p[c] as f32 * (1. - a)).round().clamp(0., 255.) as u8;
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_opaque_or_oversized_generated_assets() {
        let mut b = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(RgbImage::new(10, 10))
            .write_to(&mut b, image::ImageFormat::Png)
            .unwrap();
        assert!(validate_png(b.get_ref(), true).is_err());
        assert!(validate_png(b"not png", true).is_err());
        let im = validate_png(
            include_bytes!("../resources/art/eucalyptus-watercolor.png"),
            true,
        )
        .unwrap();
        assert!(im.width() <= 2048);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn frame_template_owns_png_and_survives_original_asset_removal() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("temporary.png");
        std::fs::write(
            &original,
            include_bytes!("../resources/art/eucalyptus-watercolor.png"),
        )
        .unwrap();
        let storage = crate::storage::Storage::new(dir.path().join("app")).unwrap();
        let mut settings = crate::model::RenderSettings::default();
        settings.border_percent = 20.;
        settings.artwork = Some(ArtScene {
            stickers: vec![Sticker {
                image_path: original.to_string_lossy().into_owned(),
                anchor: "bottom-left".into(),
                scale: 0.9,
                opacity: 1.,
                flip_x: false,
            }],
            ..Default::default()
        });
        storage.own_settings_assets(&mut settings).unwrap();
        let owned = &settings.artwork.as_ref().unwrap().stickers[0].image_path;
        assert!(Path::new(owned).starts_with(dir.path().join("app/assets")));
        std::fs::remove_file(original).unwrap();
        assert!(crate::imaging::frame_thumbnail(&settings).is_ok());
    }
}
