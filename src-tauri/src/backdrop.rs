//! Optional blurred-photo background for the frame.
//!
//! The backdrop is built once per render from a small (long edge ≤ 192 px)
//! "cover" crop of the photo, blurred and tinted toward the frame colour, then
//! sampled bilinearly across the canvas. Because it only depends on the photo,
//! the canvas aspect ratio and these settings (not on the output resolution),
//! the preview, watermark close-up and full-size export agree. The photo itself
//! is still drawn whole on top: nothing is cropped or stretched.

use image::{imageops, imageops::FilterType, Rgb, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};

const BACKDROP_EDGE: f64 = 192.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FrameBackground {
    /// "color" (the frame colour, the default) or "blur" (a blurred photo).
    pub kind: String,
    /// Blur strength, 0–1.
    pub blur: f64,
    /// How far the blurred photo is mixed toward the frame colour, 0–0.8.
    pub dim: f64,
}

impl Default for FrameBackground {
    fn default() -> Self {
        Self { kind: "color".into(), blur: 0.6, dim: 0.15 }
    }
}

pub fn validate(background: &FrameBackground) -> Result<(), String> {
    if !matches!(background.kind.as_str(), "color" | "blur") {
        return Err("边框底色只能是纯色或照片模糊".into());
    }
    if !background.blur.is_finite() || !(0.0..=1.0).contains(&background.blur) {
        return Err("模糊程度应在 0–100% 之间".into());
    }
    if !background.dim.is_finite() || !(0.0..=0.8).contains(&background.dim) {
        return Err("底色叠加应在 0–80% 之间".into());
    }
    Ok(())
}

/// Small blurred backdrop for a canvas of `canvas_w` × `canvas_h`, or `None`
/// for a plain colour frame.
pub fn prepare(
    source: &RgbaImage,
    background: &FrameBackground,
    tint: [u8; 3],
    canvas_w: u32,
    canvas_h: u32,
) -> Option<RgbImage> {
    if background.kind != "blur" || source.width() == 0 || source.height() == 0 || canvas_w == 0 || canvas_h == 0 {
        return None;
    }
    let factor = BACKDROP_EDGE / f64::from(canvas_w.max(canvas_h));
    let sw = (f64::from(canvas_w) * factor).round().max(1.0) as u32;
    let sh = (f64::from(canvas_h) * factor).round().max(1.0) as u32;
    // A cheap first reduction bounds the work for 100 MP sources.
    let reduced = if source.width().max(source.height()) > 768 {
        let f = 768.0 / f64::from(source.width().max(source.height()));
        imageops::thumbnail(
            source,
            ((f64::from(source.width()) * f).round() as u32).max(1),
            ((f64::from(source.height()) * f).round() as u32).max(1),
        )
    } else {
        source.clone()
    };
    // Cover the small canvas, centred, like CSS object-fit: cover.
    let cover = (f64::from(sw) / f64::from(reduced.width())).max(f64::from(sh) / f64::from(reduced.height()));
    let rw = ((f64::from(reduced.width()) * cover).ceil() as u32).max(sw);
    let rh = ((f64::from(reduced.height()) * cover).ceil() as u32).max(sh);
    let scaled = imageops::resize(&reduced, rw, rh, FilterType::Triangle);
    let cropped = imageops::crop_imm(&scaled, (rw - sw) / 2, (rh - sh) / 2, sw, sh).to_image();
    let sigma = (0.6 + background.blur * f64::from(sw.max(sh)) * 0.07) as f32;
    let blurred = imageops::blur(&cropped, sigma);
    let dim = background.dim;
    let mut out = RgbImage::new(sw, sh);
    for (o, p) in out.pixels_mut().zip(blurred.pixels()) {
        let alpha = f64::from(p[3]) / 255.0;
        for c in 0..3 {
            let over = f64::from(p[c]) * alpha + f64::from(tint[c]) * (1.0 - alpha);
            o[c] = (over * (1.0 - dim) + f64::from(tint[c]) * dim).round().clamp(0.0, 255.0) as u8;
        }
    }
    Some(out)
}

/// Paints the backdrop into `canvas`, which shows the window starting at
/// (`ox`, `oy`) of a full canvas measuring `full_w` × `full_h` pixels.
pub fn paint(canvas: &mut RgbImage, backdrop: &RgbImage, ox: u32, oy: u32, full_w: u32, full_h: u32) {
    let bw = backdrop.width();
    let bh = backdrop.height();
    let sx = f64::from(bw) / f64::from(full_w.max(1));
    let sy = f64::from(bh) / f64::from(full_h.max(1));
    let maxx = f64::from(bw - 1);
    let maxy = f64::from(bh - 1);
    let width = canvas.width();
    for y in 0..canvas.height() {
        let fy = ((f64::from(y + oy) + 0.5) * sy - 0.5).clamp(0.0, maxy);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(bh - 1);
        let ty = fy - f64::from(y0);
        for x in 0..width {
            let fx = ((f64::from(x + ox) + 0.5) * sx - 0.5).clamp(0.0, maxx);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(bw - 1);
            let tx = fx - f64::from(x0);
            let a = backdrop.get_pixel(x0, y0);
            let b = backdrop.get_pixel(x1, y0);
            let c = backdrop.get_pixel(x0, y1);
            let d = backdrop.get_pixel(x1, y1);
            let mut px = [0u8; 3];
            for i in 0..3 {
                let top = f64::from(a[i]) * (1.0 - tx) + f64::from(b[i]) * tx;
                let bottom = f64::from(c[i]) * (1.0 - tx) + f64::from(d[i]) * tx;
                px[i] = (top * (1.0 - ty) + bottom * ty).round() as u8;
            }
            canvas.put_pixel(x, y, Rgb(px));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn split() -> RgbaImage {
        RgbaImage::from_fn(400, 300, |x, _| if x < 200 { Rgba([200, 40, 40, 255]) } else { Rgba([40, 40, 200, 255]) })
    }

    #[test]
    fn colour_mode_has_no_backdrop_and_values_are_validated() {
        let colour = FrameBackground::default();
        assert!(prepare(&split(), &colour, [255; 3], 500, 400).is_none());
        assert!(validate(&colour).is_ok());
        assert!(validate(&FrameBackground { kind: "stripes".into(), ..colour.clone() }).is_err());
        assert!(validate(&FrameBackground { blur: 1.5, ..colour.clone() }).is_err());
        assert!(validate(&FrameBackground { dim: f64::NAN, ..colour }).is_err());
    }

    #[test]
    fn blur_follows_the_photo_and_tints_toward_the_frame_colour() {
        let blur = FrameBackground { kind: "blur".into(), blur: 0.5, dim: 0.0 };
        let small = prepare(&split(), &blur, [255; 3], 500, 400).unwrap();
        assert_eq!(small.dimensions(), (192, 154));
        let left = small.get_pixel(5, 77);
        let right = small.get_pixel(186, 77);
        assert!(left[0] > left[2] && right[2] > right[0], "left stays red, right stays blue");
        let dimmed = prepare(&split(), &FrameBackground { dim: 0.8, ..blur }, [255; 3], 500, 400).unwrap();
        assert!(dimmed.get_pixel(5, 77)[1] > left[1] + 100, "dim mixes toward the white frame colour");
    }

    #[test]
    fn window_painting_matches_the_full_canvas() {
        let blur = FrameBackground { kind: "blur".into(), blur: 0.8, dim: 0.2 };
        let small = prepare(&split(), &blur, [240, 230, 220], 640, 480).unwrap();
        let mut full = RgbImage::new(640, 480);
        paint(&mut full, &small, 0, 0, 640, 480);
        let mut window = RgbImage::new(100, 60);
        paint(&mut window, &small, 300, 410, 640, 480);
        for y in 0..60 {
            for x in 0..100 {
                assert_eq!(window.get_pixel(x, y), full.get_pixel(x + 300, y + 410));
            }
        }
    }
}
