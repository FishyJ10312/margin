//! Ties an AI-styled border to its photograph so the two read as one print
//! rather than a picture pasted onto unrelated paper.
//!
//! Two light, luminance-preserving passes over the border only:
//! - ambient tint: the whole border leans a little towards the photo's
//!   overall colour cast, the way a mat picks up the light of the room;
//! - edge bleed: near the print, the border takes on the hue of the photo's
//!   closest edge, fading out over about a tenth of the short edge.
//!
//! The photo itself is never touched. Every value is a pure function of
//! global canvas coordinates and a small blurred summary of the photo, so a
//! preview window renders exactly like the same region of the full image.
use crate::{artwork::ArtScene, layout::Layout};
use image::{RgbImage, RgbaImage};

const GRID: usize = 16;
const AMBIENT: f32 = 0.2;
const BLEED: f32 = 0.45;
const REACH: f64 = 0.14;

/// A tiny, blurred colour map of the photograph in linear-ish RGB.
pub struct PhotoContext {
    grid: Vec<[f32; 3]>,
    mean: [f32; 3],
    /// Fine luminance texture of the photo, reused as grain on the border so
    /// a clean digital mat does not sit against a grainy print.
    grain: f32,
}

/// Only looks designed with the photo in mind are harmonised: the painterly
/// art styles and AI-painted materials. Classic mats and film stay neutral.
pub fn applies(scene: &ArtScene) -> bool {
    scene.frame_image.as_ref().is_some_and(|f| f.material)
        || scene
            .design
            .as_ref()
            .is_some_and(|d| crate::art_styles::is_art(&d.id))
}

impl PhotoContext {
    pub fn new(source: &RgbaImage) -> Self {
        let small = image::imageops::thumbnail(source, GRID as u32, GRID as u32);
        let (w, h) = small.dimensions();
        let mut raw = vec![[0f32; 3]; GRID * GRID];
        let mut mean = [0f32; 3];
        for gy in 0..GRID {
            for gx in 0..GRID {
                let p = small.get_pixel(
                    (gx as u32 * w / GRID as u32).min(w - 1),
                    (gy as u32 * h / GRID as u32).min(h - 1),
                );
                for c in 0..3 {
                    raw[gy * GRID + gx][c] = p[c] as f32;
                    mean[c] += p[c] as f32 / (GRID * GRID) as f32;
                }
            }
        }
        // Blur so the bleed carries regional colour, never photo detail.
        let mut grid = raw.clone();
        for gy in 0..GRID {
            for gx in 0..GRID {
                let mut sum = [0f32; 3];
                let mut n = 0.;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (x, y) = (gx as i32 + dx, gy as i32 + dy);
                        if x < 0 || y < 0 || x >= GRID as i32 || y >= GRID as i32 {
                            continue;
                        }
                        let v = raw[y as usize * GRID + x as usize];
                        for c in 0..3 {
                            sum[c] += v[c];
                        }
                        n += 1.;
                    }
                }
                grid[gy * GRID + gx] = sum.map(|s| s / n);
            }
        }
        Self { grid, mean, grain: grain(source) }
    }
    fn sample(&self, u: f64, v: f64) -> [f32; 3] {
        let fx = (u * GRID as f64 - 0.5).clamp(0., (GRID - 1) as f64);
        let fy = (v * GRID as f64 - 0.5).clamp(0., (GRID - 1) as f64);
        let (ix, iy) = (fx as usize, fy as usize);
        let (jx, jy) = ((ix + 1).min(GRID - 1), (iy + 1).min(GRID - 1));
        let (tx, ty) = ((fx - ix as f64) as f32, (fy - iy as f64) as f32);
        let g = |x: usize, y: usize| self.grid[y * GRID + x];
        let mut out = [0f32; 3];
        for c in 0..3 {
            let top = g(ix, iy)[c] * (1. - tx) + g(jx, iy)[c] * tx;
            let bottom = g(ix, jy)[c] * (1. - tx) + g(jx, jy)[c] * tx;
            out[c] = top * (1. - ty) + bottom * ty;
        }
        out
    }
}

fn grain(source: &RgbaImage) -> f32 {
    let t = image::imageops::thumbnail(source, 256, 256);
    let (w, h) = t.dimensions();
    if w < 3 || h < 3 {
        return 0.;
    }
    let l = |x: u32, y: u32| {
        let p = t.get_pixel(x, y);
        luma([p[0] as f32, p[1] as f32, p[2] as f32])
    };
    let (mut sum, mut n) = (0f32, 0f32);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let lap = l(x, y) - (l(x - 1, y) + l(x + 1, y) + l(x, y - 1) + l(x, y + 1)) / 4.;
            // Ignore real edges; keep the fine texture between them.
            sum += lap.abs().min(12.);
            n += 1.;
        }
    }
    (sum / n * 0.8).clamp(1., 6.)
}
fn hash(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ 0x2545_f491;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535. - 0.5
}
fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// `colour`'s hue and saturation at `target`'s brightness.
fn recolour(target: [f32; 3], colour: [f32; 3]) -> [f32; 3] {
    let (lt, lc) = (luma(target), luma(colour).max(1.));
    let k = lt / lc;
    let mut out = colour.map(|v| v * k);
    // Keep within range by easing towards grey rather than clipping hue.
    let peak = out.iter().cloned().fold(0., f32::max);
    if peak > 255. {
        let t = (peak - 255.) / (peak - lt).max(1.);
        out = out.map(|v| v + (lt - v) * t.min(1.));
    }
    out
}

pub fn apply(canvas: &mut RgbImage, ctx: &PhotoContext, l: Layout, ox: u32, oy: u32) {
    if l.x == 0 && l.y == 0 && l.photo_width == l.width && l.photo_height == l.height {
        return;
    }
    let short = l.photo_width.min(l.photo_height).max(1) as f64;
    let reach = short * REACH;
    let (px0, py0) = (l.x as f64, l.y as f64);
    let (pw, ph) = (l.photo_width.max(1) as f64, l.photo_height.max(1) as f64);
    for cy in 0..canvas.height() {
        let y = cy + oy;
        for cx in 0..canvas.width() {
            let x = cx + ox;
            if x >= l.x && x < l.x + l.photo_width && y >= l.y && y < l.y + l.photo_height {
                continue;
            }
            let (gx, gy) = (x as f64 + 0.5, y as f64 + 0.5);
            let nx = gx.clamp(px0, px0 + pw);
            let ny = gy.clamp(py0, py0 + ph);
            let d = ((gx - nx).powi(2) + (gy - ny).powi(2)).sqrt();
            let edge = ctx.sample((nx - px0) / pw, (ny - py0) / ph);
            let p = canvas.get_pixel_mut(cx, cy);
            let base = [p[0] as f32, p[1] as f32, p[2] as f32];
            let ambient = recolour(base, ctx.mean);
            let bleed = recolour(base, edge);
            let w = BLEED * (-(d / reach) as f32).exp();
            let g = (hash(x, y) + hash(x.wrapping_add(7919), y.wrapping_add(104_729))) * ctx.grain;
            for c in 0..3 {
                let v = base[c] + (ambient[c] - base[c]) * AMBIENT;
                let v = v + (bleed[c] - v) * w + g;
                p[c] = v.round().clamp(0., 255.) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, Rgba};
    #[test]
    fn border_takes_the_photos_light_without_touching_it_and_matches_roi() {
        // A warm left half and a cool right half.
        let photo = RgbaImage::from_fn(80, 60, |x, _| {
            if x < 40 { Rgba([220, 120, 60, 255]) } else { Rgba([60, 110, 200, 255]) }
        });
        let ctx = PhotoContext::new(&photo);
        let l = Layout { width: 300, height: 220, x: 40, y: 40, photo_width: 220, photo_height: 140, scale: 1. };
        let paper = Rgb([235, 232, 226]);
        let marker = Rgb([1, 2, 3]);
        let mut full = RgbImage::from_fn(l.width, l.height, |x, y| {
            if x >= l.x && x < l.x + l.photo_width && y >= l.y && y < l.y + l.photo_height { marker } else { paper }
        });
        let before = full.clone();
        apply(&mut full, &ctx, l, 0, 0);
        assert_eq!(full.get_pixel(150, 100), &marker);
        let left = full.get_pixel(36, 110);
        let right = full.get_pixel(264, 110);
        assert!(left[0] > left[2] && right[2] > right[0], "{left:?} {right:?}");
        // Brightness is kept: the mat stays a mat.
        let lum = |p: &Rgb<u8>| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
        assert!((lum(left) - lum(before.get_pixel(36, 110))).abs() < 6.);
        let mut roi = image::imageops::crop_imm(&before, 20, 150, 90, 60).to_image();
        apply(&mut roi, &ctx, l, 20, 150);
        for (x, y, p) in roi.enumerate_pixels() {
            assert_eq!(p, full.get_pixel(x + 20, y + 150));
        }
    }
}
