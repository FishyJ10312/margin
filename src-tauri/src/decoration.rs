//! Original procedural motifs. A bounded mask is drawn only in the padding.
use crate::{layout::Layout, model::FrameDecoration};
use image::RgbImage;
pub const KINDS: [&str; 7] = [
    "none", "paper", "linen", "dots", "lines", "waves", "corners",
];
pub fn validate(d: &FrameDecoration) -> Result<(), String> {
    if !KINDS.contains(&d.kind.as_str()) {
        return Err("请选择支持的边框花纹".into());
    }
    crate::imaging::parse_color(&d.color)?;
    if !d.opacity.is_finite()
        || !(0.0..=0.3).contains(&d.opacity)
        || !d.scale.is_finite()
        || !(0.5..=3.0).contains(&d.scale)
    {
        return Err("花纹强度应在 0–30%，大小应在 0.5–3 倍之间".into());
    }
    Ok(())
}
fn hash(x: u32, y: u32) -> f64 {
    let mut n = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    f64::from(n ^ (n >> 16)) / f64::from(u32::MAX)
}
fn mask(kind: &str, x: f64, y: f64) -> f64 {
    match kind {
        "paper" => {
            let a = hash((x * 255.0) as u32, (y * 255.0) as u32);
            let b = hash((x * 31.0) as u32, (y * 31.0) as u32);
            (a * 0.65 + b * 0.35).powi(2)
        }
        "linen" => {
            let a = (x * std::f64::consts::TAU * 8.0).sin().abs();
            let b = (y * std::f64::consts::TAU * 8.0).sin().abs();
            (a * b).powi(8) * 0.65
        }
        "dots" => (1.0 - ((x - 0.5).hypot(y - 0.5) - 0.035) / 0.018).clamp(0.0, 1.0),
        "lines" => (1.0 - ((x + y).rem_euclid(1.0) - 0.5).abs() / 0.03).max(0.0),
        "waves" => {
            let mid = 0.5 + 0.12 * (x * std::f64::consts::TAU).sin();
            (1.0 - (y - mid).abs() / 0.025).max(0.0)
        }
        _ => 0.0,
    }
}

pub fn draw(
    canvas: &mut RgbImage,
    d: &FrameDecoration,
    l: Layout,
    full: Layout,
) -> Result<(), String> {
    draw_window(canvas, d, l, full, 0, 0)
}
pub fn draw_window(
    canvas: &mut RgbImage,
    d: &FrameDecoration,
    l: Layout,
    full: Layout,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    if d.kind == "none" || d.opacity == 0.0 {
        return Ok(());
    }
    let color = crate::imaging::parse_color(&d.color)?;
    let short = f64::from(full.photo_width.min(full.photo_height));
    let period = (short * 0.025 * d.scale).max(16.0);
    // Only 64 KiB regardless of the source/canvas size.
    let mut tile = vec![0u8; 256 * 256];
    if d.kind != "corners" {
        for y in 0..256 {
            for x in 0..256 {
                tile[y * 256 + x] =
                    (mask(&d.kind, x as f64 / 256.0, y as f64 / 256.0) * 255.0).round() as u8;
            }
        }
    }
    let fx = f64::from(full.width) / f64::from(l.width);
    let fy = f64::from(full.height) / f64::from(l.height);
    let px = l.x.min(l.width);
    let py = l.y.min(l.height);
    let right = (l.x + l.photo_width).min(l.width);
    let bottom = (l.y + l.photo_height).min(l.height);
    let rectangles = [
        (0, 0, l.width, py),
        (0, bottom, l.width, l.height),
        (0, py, px, bottom),
        (right, py, l.width, bottom),
    ];
    let pad = full
        .x
        .min(full.y)
        .min(full.width - full.x - full.photo_width)
        .min(full.height - full.y - full.photo_height);
    let inset = (f64::from(pad) * 0.35).max(1.0);
    let length = short * 0.065 * d.scale;
    let stroke = (short * 0.00065).max(1.0);
    for (x0, y0, x1, y1) in rectangles {
        for y in y0.max(oy)..y1.min(oy + canvas.height()) {
            for x in x0.max(ox)..x1.min(ox + canvas.width()) {
                let xx = (f64::from(x) + 0.5) * fx;
                let yy = (f64::from(y) + 0.5) * fy;
                let amount = if d.kind == "corners" {
                    let dx = xx.min(f64::from(full.width) - xx) - inset;
                    let dy = yy.min(f64::from(full.height) - yy) - inset;
                    if pad == 0 {
                        0.0
                    } else if (dx.abs() < stroke && dy >= -stroke && dy < length)
                        || (dy.abs() < stroke && dx >= -stroke && dx < length)
                    {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    let tx = ((xx / period).rem_euclid(1.0) * 256.0) as usize;
                    let ty = ((yy / period).rem_euclid(1.0) * 256.0) as usize;
                    f64::from(tile[ty.min(255) * 256 + tx.min(255)]) / 255.0
                };
                let alpha = amount * d.opacity;
                if alpha == 0.0 {
                    continue;
                }
                let p = canvas.get_pixel_mut(x - ox, y - oy);
                for c in 0..3 {
                    p[c] = (f64::from(p[c]) * (1.0 - alpha) + f64::from(color[c]) * alpha).round()
                        as u8;
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout, model::RenderSettings};
    use image::Rgb;
    #[test]
    fn every_motif_only_touches_padding_and_is_repeatable() {
        for kind in KINDS.iter().skip(1) {
            let mut s = RenderSettings::default();
            s.border_percent = 20.0;
            let l = layout::calculate(180, 120, &s).unwrap();
            let mut d = FrameDecoration::default();
            d.kind = kind.to_string();
            d.opacity = 0.3;
            let mut a = RgbImage::from_pixel(l.width, l.height, Rgb([255, 255, 255]));
            let mut b = a.clone();
            draw(&mut a, &d, l, l).unwrap();
            draw(&mut b, &d, l, l).unwrap();
            assert_eq!(a, b);
            assert!(a.pixels().any(|p| p[0] < 255), "{kind}");
            for y in l.y..l.y + l.photo_height {
                for x in l.x..l.x + l.photo_width {
                    assert_eq!(*a.get_pixel(x, y), Rgb([255, 255, 255]));
                }
            }
        }
    }
    #[test]
    fn no_padding_preserves_every_pixel() {
        let mut s = RenderSettings::default();
        s.border_percent = 0.0;
        let l = layout::calculate(100, 60, &s).unwrap();
        let mut a = RgbImage::from_pixel(100, 60, Rgb([40, 80, 120]));
        let before = a.clone();
        for kind in KINDS {
            let mut d = FrameDecoration::default();
            d.kind = kind.into();
            draw(&mut a, &d, l, l).unwrap();
        }
        assert_eq!(a, before);
    }
}
