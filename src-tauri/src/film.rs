//! Procedural film rebates. Work is clipped to border bands, with no full-size
//! intermediate canvas, downloaded scan asset, font cache or photo modification.
use crate::{designs::DesignRecipe, imaging::parse_color, layout::Layout};
use image::RgbImage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct FilmOptions {
    pub labels: bool,
    pub texture: f32,
}
impl Default for FilmOptions {
    fn default() -> Self {
        Self {
            labels: true,
            texture: 0.35,
        }
    }
}
/// Below this border band (as a share of the photo's short edge) perforations
/// and edge print would shrink into specks, so they are left out and only the
/// clean rebate remains, like a tightly cropped scan.
pub const LEGIBLE_BAND: f32 = 0.05;

pub fn is_scan(id: &str) -> bool {
    matches!(
        id,
        "scan-35mm"
            | "scan-amber"
            | "scan-bw"
            | "scan-120"
            | "scan-16mm"
            | "scan-8mm"
            | "scan-contact"
            | "scan-slide"
            | "scan-instant"
            | "scan-half"
            | "scan-super16"
            | "scan-65mm"
            | "scan-carrier"
            | "scan-e6"
            | "scan-c41"
            | "scan-bw-strip"
            | "scan-sheet"
            | "scan-peel"
    )
}
fn hash(x: u32, y: u32) -> u32 {
    let mut v = x.wrapping_mul(0x45d9f3b) ^ y.wrapping_mul(0x27d4eb2d);
    v ^= v >> 16;
    v = v.wrapping_mul(0x85ebca6b);
    v ^ (v >> 13)
}
fn blend(pixel: &mut [u8; 3], color: [u8; 3], alpha: f32) {
    for i in 0..3 {
        pixel[i] = (pixel[i] as f32 + (color[i] as f32 - pixel[i] as f32) * alpha)
            .round()
            .clamp(0., 255.) as u8;
    }
}
struct Painter<'a> {
    image: &'a mut RgbImage,
    l: Layout,
    ox: u32,
    oy: u32,
}

/// Film coordinates run along the strip, then across the opening. Portrait
/// film rotates clockwise around the opening so that its markings remain legible.
struct FilmStrip {
    x: f32,
    y: f32,
    length: f32,
    opening: f32,
    vertical: bool,
}

impl FilmStrip {
    fn new([x, y, width, height]: [f32; 4], vertical: bool) -> Self {
        let (length, opening) = if vertical {
            (height, width)
        } else {
            (width, height)
        };
        Self {
            x,
            y,
            length,
            opening,
            vertical,
        }
    }

    fn to_canvas(&self, [u, v, width, height]: [f32; 4]) -> [f32; 4] {
        if self.vertical {
            [
                self.x + self.opening - v - height,
                self.y + u,
                height,
                width,
            ]
        } else {
            [self.x + u, self.y + v, width, height]
        }
    }
}

impl Painter<'_> {
    fn pixels(&mut self, rect: [f32; 4], mut draw: impl FnMut(&mut [u8; 3], u32, u32)) {
        let [x, y, w, h] = rect;
        let x0 = (x.floor().max(0.) as u32).max(self.ox);
        let y0 = (y.floor().max(0.) as u32).max(self.oy);
        let x1 = ((x + w).ceil().max(0.) as u32)
            .min(self.l.width)
            .min(self.ox.saturating_add(self.image.width()));
        let y1 = ((y + h).ceil().max(0.) as u32)
            .min(self.l.height)
            .min(self.oy.saturating_add(self.image.height()));
        for gy in y0..y1 {
            let spans = if gy >= self.l.y && gy < self.l.y + self.l.photo_height {
                [
                    (x0, x1.min(self.l.x)),
                    (x0.max(self.l.x + self.l.photo_width), x1),
                ]
            } else {
                [(x0, x1), (x1, x1)]
            };
            for (a, b) in spans {
                for gx in a..b {
                    draw(
                        &mut self.image.get_pixel_mut(gx - self.ox, gy - self.oy).0,
                        gx,
                        gy,
                    );
                }
            }
        }
    }
    fn rect(&mut self, rect: [f32; 4], radius: f32, color: [u8; 3], alpha: f32) {
        let [x, y, w, h] = rect;
        if w <= 0. || h <= 0. {
            return;
        }
        let r = radius.min(w / 2.).min(h / 2.).max(0.);
        self.pixels(rect, |pixel, gx, gy| {
            let qx = (gx as f32 + 0.5 - x - w / 2.).abs() - (w / 2. - r);
            let qy = (gy as f32 + 0.5 - y - h / 2.).abs() - (h / 2. - r);
            let distance = if qx > 0. && qy > 0. {
                (qx * qx + qy * qy).sqrt() - r
            } else {
                qx.max(qy) - r
            };
            let coverage = (0.5 - distance).clamp(0., 1.);
            if coverage > 0. {
                blend(pixel, color, alpha * coverage);
            }
        });
    }
    /// Filled triangle with 4x4 supersampled edges.
    fn triangle(&mut self, pts: [[f32; 2]; 3], color: [u8; 3], alpha: f32) {
        let xs = pts.map(|q| q[0]);
        let ys = pts.map(|q| q[1]);
        let x0 = xs.iter().cloned().fold(f32::MAX, f32::min);
        let y0 = ys.iter().cloned().fold(f32::MAX, f32::min);
        let x1 = xs.iter().cloned().fold(f32::MIN, f32::max);
        let y1 = ys.iter().cloned().fold(f32::MIN, f32::max);
        let edge = |a: [f32; 2], b: [f32; 2], q: [f32; 2]| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
        let area = edge(pts[0], pts[1], pts[2]);
        if area.abs() < 1e-3 {
            return;
        }
        self.pixels([x0, y0, x1 - x0, y1 - y0], |pixel, gx, gy| {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let q = [gx as f32 + (sx as f32 + 0.5) / 4., gy as f32 + (sy as f32 + 0.5) / 4.];
                    let (a, b, c) = (edge(pts[1], pts[2], q), edge(pts[2], pts[0], q), edge(pts[0], pts[1], q));
                    if (a >= 0. && b >= 0. && c >= 0.) || (a <= 0. && b <= 0. && c <= 0.) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                blend(pixel, color, alpha * hits as f32 / 16.);
            }
        });
    }
    fn rim(&mut self, rect: [f32; 4], size: f32, color: [u8; 3], alpha: f32) {
        let [x, y, w, h] = rect;
        for r in [
            [x, y, w, size],
            [x, y + h - size, w, size],
            [x, y, size, h],
            [x + w - size, y, size, h],
        ] {
            self.rect(r, 0., color, alpha);
        }
    }
    fn text(
        &mut self,
        strip: &FilmStrip,
        value: &str,
        [u, v]: [f32; 2],
        cell: f32,
        color: [u8; 3],
        alpha: f32,
    ) {
        for (i, c) in value.chars().enumerate() {
            for (row, bits) in glyph(c).into_iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.rect(
                            strip.to_canvas([
                                u + (i * 6 + col) as f32 * cell,
                                v + row as f32 * cell,
                                cell * 0.84,
                                cell * 0.84,
                            ]),
                            0.,
                            color,
                            alpha,
                        );
                    }
                }
            }
        }
    }
}
// Original 5x7 microtype. Its few dozen glyphs require no bundled font resource.
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '>' => [16, 24, 28, 30, 28, 24, 16],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        _ => [0; 7],
    }
}

pub fn draw_window(
    image: &mut RgbImage,
    recipe: &DesignRecipe,
    l: Layout,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    if !is_scan(&recipe.id) {
        return Ok(());
    }
    crate::designs::validate(recipe)?;
    let g =
        (l.x.min(l.y)
            .min(l.width - l.x - l.photo_width)
            .min(l.height - l.y - l.photo_height)) as f32;
    if g < 2. {
        return Ok(());
    }
    let options = recipe.film.clone().unwrap_or_default();
    let colors = recipe
        .colors
        .iter()
        .map(|c| parse_color(c))
        .collect::<Result<Vec<_>, _>>()?;
    let (ink, paper, dark) = (colors[0], colors[1], colors[2]);
    let (x, y, w, h) = (
        l.x as f32,
        l.y as f32,
        l.photo_width as f32,
        l.photo_height as f32,
    );
    let band = ((g * 0.94).min(w.min(h) * 0.18) * recipe.scale).min(g);
    let alpha = (recipe.intensity * 0.95).min(1.);
    let legible = band >= w.min(h) * LEGIBLE_BAND;
    let mut p = Painter { image, l, ox, oy };
    let id = recipe.id.as_str();
    if matches!(id, "scan-e6" | "scan-c41" | "scan-bw-strip" | "scan-sheet" | "scan-peel") {
        let palette = Palette { ink, light: paper, base: dark };
        let photo = [x, y, w, h];
        match id {
            "scan-sheet" => sheet(&mut p, photo, band, palette, alpha, legible),
            "scan-peel" => peel(&mut p, photo, band, palette, alpha),
            _ => strip_scan(&mut p, id, photo, g, recipe.scale, palette, alpha, &options),
        }
        let amount = options.texture * if id == "scan-peel" { 4. } else { 10. };
        grain(&mut p, amount);
        return Ok(());
    }
    let board = matches!(
        id,
        "scan-contact" | "scan-slide" | "scan-instant" | "scan-carrier"
    );
    if board {
        let side = band * if id == "scan-slide" { 0.85 } else { 0.65 };
        let top = band * if id == "scan-instant" { 0.4 } else { 0.65 };
        let bottom = band * if id == "scan-instant" { 0.98 } else { 0.65 };
        let card = [x - side, y - top, w + 2. * side, h + top + bottom];
        p.rect(
            [card[0] - 1., card[1] + 1., card[2] + 2., card[3] + 1.],
            band * 0.025,
            dark,
            0.12 * alpha,
        );
        p.rect(
            card,
            band * 0.02,
            if matches!(id, "scan-contact" | "scan-carrier") {
                dark
            } else {
                paper
            },
            alpha,
        );
    }
    let edge = (w.min(h) * 0.0025 * recipe.scale).clamp(0.8, 10.).min(band);
    p.rim(
        [x - edge, y - edge, w + 2. * edge, h + 2. * edge],
        edge,
        if board { dark } else { paper },
        0.72 * alpha,
    );
    // A fine uneven rebate remains strictly outside the rectangular photograph.
    if !matches!(id, "scan-instant" | "scan-slide") {
        let count = 64;
        for i in 0..count {
            let t = i as f32 / count as f32;
            let rough = (hash(i, 17) % 100) as f32 / 100. * options.texture;
            let e = (edge * (1.2 + rough * if id == "scan-carrier" { 4. } else { 1.8 })).min(band);
            p.rect(
                [x + t * w, y - e, w / count as f32 + 0.5, e],
                0.,
                ink,
                0.32 * alpha,
            );
            p.rect(
                [x + t * w, y + h, w / count as f32 + 0.5, e],
                0.,
                ink,
                0.3 * alpha,
            );
            p.rect(
                [x - e, y + t * h, e, h / count as f32 + 0.5],
                0.,
                ink,
                0.26 * alpha,
            );
            p.rect(
                [x + w, y + t * h, e, h / count as f32 + 0.5],
                0.,
                ink,
                0.28 * alpha,
            );
        }
    }
    // Half-frame exposures sit across the 35 mm strip, so their perforations
    // follow the short edge. Printed mounts keep their labels horizontal.
    let vertical = if id == "scan-half" {
        w > h
    } else {
        !board && h > w
    };
    let strip = FilmStrip::new([x, y, w, h], vertical);
    let (length, opening) = (strip.length, strip.opening);
    let perforations = match id {
        "scan-35mm" | "scan-amber" => Some((8, 2, 0.37, 1.45)),
        "scan-half" => Some((4, 2, 0.43, 1.48)),
        "scan-16mm" => Some((1, 2, 0.51, 1.35)),
        "scan-super16" => Some((1, 1, 0.48, 1.34)),
        "scan-8mm" => Some((1, 1, 0.58, 1.05)),
        "scan-65mm" => Some((5, 2, 0.36, 1.43)),
        _ => None,
    };
    if let Some((count, sides, depth, aspect)) = perforations.filter(|_| legible) {
        let pitch = length / count as f32;
        let hole_height = band * depth;
        let hole_width = (hole_height * aspect).min(pitch * 0.65);
        let bevel = (band * 0.019).clamp(0.45, 3.);
        for rail_center in [-band * 0.43, opening + band * 0.43]
            .into_iter()
            .take(sides)
        {
            let rail_edge = (edge * 0.32).clamp(0.4, 2.);
            p.rect(
                strip.to_canvas([0., rail_center - band * 0.29, length, rail_edge]),
                0.,
                dark,
                0.42 * alpha,
            );
            p.rect(
                strip.to_canvas([0., rail_center + band * 0.29, length, rail_edge]),
                0.,
                ink,
                0.18 * alpha,
            );
            for n in 0..count {
                let u = pitch * (n as f32 + 0.5) - hole_width / 2.;
                let v = rail_center - hole_height / 2.;
                p.rect(
                    strip.to_canvas([
                        u - bevel,
                        v - bevel,
                        hole_width + 2. * bevel,
                        hole_height + 2. * bevel,
                    ]),
                    hole_height * 0.24,
                    dark,
                    0.9 * alpha,
                );
                p.rect(
                    strip.to_canvas([u, v - bevel * 0.45, hole_width, hole_height + bevel]),
                    hole_height * 0.2,
                    ink,
                    0.38 * alpha,
                );
                p.rect(
                    strip.to_canvas([u + bevel * 0.3, v, hole_width - bevel * 0.6, hole_height]),
                    hole_height * 0.18,
                    paper,
                    alpha,
                );
            }
        }
    }
    if id == "scan-amber" {
        for i in 0..4 {
            let e = edge * (2. + i as f32);
            p.rim(
                [x - e, y - e, w + 2. * e, h + 2. * e],
                edge,
                ink,
                alpha * 0.09,
            );
        }
    }
    if legible && options.labels && matches!(id, "scan-120" | "scan-65mm") {
        let mut u = length * 0.60;
        let v = opening + band * if id == "scan-120" { 0.37 } else { 0.78 };
        for width in [0.010, 0.026, 0.011, 0.019, 0.029, 0.013, 0.010, 0.022] {
            p.rect(
                strip.to_canvas([u, v, band * width, band * 0.095]),
                0.,
                ink,
                0.74 * alpha,
            );
            u += band * (width + 0.018);
        }
    }
    if id == "scan-contact" {
        let red = ink;
        p.rim(
            [
                x - band * 0.79,
                y - band * 0.78,
                w + band * 1.58,
                h + band * 1.56,
            ],
            edge * 0.8,
            red,
            alpha * 0.65,
        );
        p.rect(
            [
                x - band * 0.88,
                y + h + band * 0.5,
                band * 0.15,
                band * 0.04,
            ],
            0.,
            red,
            alpha,
        );
    }
    if id == "scan-slide" {
        for (cx, cy) in [
            (x - band * 0.57, y - band * 0.4),
            (x + w + band * 0.57, y + h + band * 0.4),
        ] {
            p.rect(
                [
                    cx - band * 0.025,
                    cy - band * 0.025,
                    band * 0.05,
                    band * 0.05,
                ],
                band * 0.025,
                ink,
                alpha * 0.3,
            );
        }
    }
    // Global coordinates and fixed seed ensure full-image / ROI composition parity.
    if options.texture > 0. {
        let amount = options.texture
            * if id == "scan-bw" {
                14.0
            } else if board {
                3.2
            } else {
                12.0
            };
        grain(&mut p, amount);
    }
    if legible && options.labels {
        let label = match id {
            "scan-35mm" => "35MM COLOR 400",
            "scan-amber" => "COLOR NEG 200",
            "scan-bw" => "B/W NEG 400",
            "scan-120" => "120 / FRAME 08",
            "scan-16mm" => "16MM 01",
            "scan-8mm" => "SUPER 8",
            "scan-contact" => "CONTACT 24",
            "scan-slide" => "35MM / MOUNT 01",
            "scan-half" => "18X24 / HALF FRAME",
            "scan-super16" => "SUPER 16 / 01",
            "scan-65mm" => "65MM / 5 PERF",
            _ => "",
        };
        if !label.is_empty() {
            let number = if id == "scan-120" { "08" } else { "24A" };
            let cell = (band * 0.125 / 7.)
                .max(0.45)
                .min(length * 0.48 / (label.len() * 6) as f32);
            let v = opening + band * if board { 0.34 } else { 0.76 };
            p.text(&strip, label, [length * 0.055, v], cell, ink, alpha * 0.85);
            p.text(
                &strip,
                number,
                [length * 0.945 - (number.len() * 6) as f32 * cell, v],
                cell,
                ink,
                alpha,
            );
            if matches!(id, "scan-35mm" | "scan-amber" | "scan-half") {
                p.text(&strip, "24", [length * 0.78, -band * 0.9], cell, ink, alpha);
            }
        }
    }
    Ok(())
}


/// Grain with a fixed seed in canvas coordinates, so full-image and window
/// renders match. Photo pixels are never touched.
fn grain(p: &mut Painter, amount: f32) {
    if amount <= 0. {
        return;
    }
    let (w, h) = (p.l.width as f32, p.l.height as f32);
    p.pixels([0., 0., w, h], |pixel, gx, gy| {
        let n = (hash(gx, gy) & 255) as f32 / 127.5 - 1.;
        let mottle = (hash(gx / 2, gy / 3) & 255) as f32 / 127.5 - 1.;
        for value in pixel {
            *value = (*value as f32 + (n * 0.75 + mottle * 0.25) * amount)
                .round()
                .clamp(0., 255.) as u8;
        }
    });
}

#[derive(Clone, Copy)]
struct Palette {
    /// Edge print / chemistry colour.
    ink: [u8; 3],
    /// Backlight, light through perforations, or paper.
    light: [u8; 3],
    /// Film base.
    base: [u8; 3],
}

/// Smooth 1-D value noise in 0..1 for irregular edges.
fn wobble(seed: u32, t: f32) -> f32 {
    let i = t.floor();
    let f = t - i;
    let a = (hash(i as i32 as u32, seed) & 1023) as f32 / 1023.;
    let b = (hash((i as i32 + 1) as u32, seed) & 1023) as f32 / 1023.;
    let s = f * f * (3. - 2. * f);
    a + (b - a) * s
}

/// A whole 35 mm strip on a light box: base running off both canvas ends,
/// neighbouring frames, perforations at the real 4.75 mm pitch (so their
/// number follows the photo's proportions), edge print and DX code.
#[allow(clippy::too_many_arguments)]
fn strip_scan(p: &mut Painter, id: &str, [x, y, w, h]: [f32; 4], g: f32, scale: f32, c: Palette, alpha: f32, options: &FilmOptions) {
    let vertical = h > w;
    let strip = FilmStrip::new([x, y, w, h], vertical);
    let (length, opening) = (strip.length, strip.opening);
    // 35 mm film: 24 mm picture, about 5.5 mm of rebate on each side.
    let side = (g * 0.78).min(opening * 0.23) * scale;
    let (before, after) = if vertical {
        (y, p.l.height as f32 - y - h)
    } else {
        (x, p.l.width as f32 - x - w)
    };
    let (u0, u1) = (-before - 2., length + after + 2.);
    p.rect(strip.to_canvas([u0, -side, u1 - u0, opening + 2. * side]), 0., c.base, alpha);
    if side < opening * LEGIBLE_BAND {
        return;
    }
    // Neighbouring frames, 2 mm away, read as denser exposed rectangles.
    let gap = opening * 0.085;
    let frame = match id {
        "scan-e6" => [44, 40, 36],
        "scan-c41" => [118, 60, 30],
        _ => [72, 72, 70],
    };
    for start in [-gap - length, length + gap] {
        p.rect(strip.to_canvas([start, 0., length, opening]), opening * 0.012, frame, alpha * 0.9);
    }
    // KS perforations: 2.79 mm along, 1.98 mm across, 4.75 mm pitch.
    let pitch = opening * 0.198;
    let along = pitch * 0.588;
    let across = side * 0.36;
    // Inverted B/W film turns the holes black; on a light box they glow.
    let hole = c.light;
    let first = (u0 / pitch).floor() as i32 - 1;
    let last = (u1 / pitch).ceil() as i32 + 1;
    for v in [-side * 0.64, opening + side * 0.28] {
        for k in first..=last {
            let u = k as f32 * pitch + pitch * 0.21;
            if id != "scan-bw-strip" {
                // Light spreads a little around each backlit hole.
                let halo = across * 0.16;
                p.rect(strip.to_canvas([u - halo, v - halo, along + 2. * halo, across + 2. * halo]), across * 0.3, hole, alpha * 0.22);
            }
            p.rect(strip.to_canvas([u, v, along, across]), across * 0.2, hole, alpha);
        }
    }
    if !options.labels {
        return;
    }
    let cell = side * 0.031;
    let char_w = 6. * cell;
    let (label, number) = match id {
        "scan-e6" => ("REVERSAL 100  E-6", 17),
        "scan-c41" => ("COLOR 400  C-41", 23),
        _ => ("B/W 400", 31),
    };
    let ink_alpha = alpha * if id == "scan-c41" { 0.78 } else { 0.7 };
    // Outer top rebate: film type, repeated along the strip.
    let top = -side * 0.93;
    let repeat = (label.len() as f32 + 18.) * char_w;
    let mut u = (u0 / repeat).floor() * repeat + length * 0.08;
    while u < u1 {
        p.text(&strip, label, [u, top], cell, c.ink, ink_alpha);
        u += repeat;
    }
    // Outer bottom rebate: frame numbers with arrows, DX code between them.
    let bottom = opening + side * 0.68;
    for (k, start) in [(-1, -gap - length), (0, 0.), (1, length + gap)] {
        let n = number + k;
        let whole = format!(">{n}");
        let half = format!("{n}A");
        p.text(&strip, &whole, [start + length * 0.05, bottom], cell, c.ink, ink_alpha);
        p.text(&strip, &half, [start + length * 0.52, bottom], cell, c.ink, ink_alpha);
        if id != "scan-e6" {
            let mut bar = start + length * 0.05 + 5. * char_w;
            let end = start + length * 0.5;
            let mut i = 0u32;
            while bar < end {
                let wide = hash(i, n as u32) & 1 == 1;
                let bw = cell * if wide { 1.6 } else { 0.8 };
                p.rect(strip.to_canvas([bar, bottom - cell * 0.5, bw, cell * 5.5]), 0., c.ink, ink_alpha * 0.85);
                bar += bw + cell * 0.9;
                i += 1;
            }
        }
    }
}

/// A sheet of 4x5 film on a light table: wide black rebate with rounded
/// corners and the maker's notch code cut into the top edge.
fn sheet(p: &mut Painter, [x, y, w, h]: [f32; 4], band: f32, c: Palette, alpha: f32, legible: bool) {
    let r = band * 0.62;
    let film = [x - r, y - r, w + 2. * r, h + 2. * r];
    p.rect(film, r * 0.16, c.base, alpha);
    // The clear film edge catches a little light.
    p.rim([film[0] + r * 0.05, film[1] + r * 0.05, film[2] - r * 0.1, film[3] - r * 0.1], (r * 0.035).max(0.6), c.ink, alpha * 0.55);
    // Uneven density where the holder's lip shaded the edge of the picture.
    let steps = 90;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let e = r * 0.08 * (0.4 + wobble(3, t * 9.));
        p.rect([x + t * w, y - e, w / steps as f32 + 0.5, e], 0., c.ink, alpha * 0.35);
        p.rect([x + t * w, y + h, w / steps as f32 + 0.5, e * 0.8], 0., c.ink, alpha * 0.3);
        let e = r * 0.08 * (0.4 + wobble(7, t * 9.));
        p.rect([x - e, y + t * h, e, h / steps as f32 + 0.5], 0., c.ink, alpha * 0.3);
        p.rect([x + w, y + t * h, e * 0.9, h / steps as f32 + 0.5], 0., c.ink, alpha * 0.32);
    }
    if !legible {
        return;
    }
    // Notch code at the top right: a V, a U and a square, cut from the edge.
    let size = r * 0.5;
    let mut nx = film[0] + film[2] - r * 1.25;
    let top = film[1];
    p.triangle([[nx - size / 2., top - 1.], [nx + size / 2., top - 1.], [nx, top + size]], c.light, alpha);
    nx -= size * 1.7;
    p.rect([nx - size * 0.32, top - size, size * 0.64, size * 1.75], size * 0.32, c.light, alpha);
    nx -= size * 1.6;
    p.rect([nx - size * 0.3, top - 1., size * 0.6, size * 0.55 + 1.], 0., c.light, alpha);
}

/// Peel-apart instant print: developer reagent spreads unevenly past the
/// picture, thickest at the pod end, with comb marks at the far end.
fn peel(p: &mut Painter, [x, y, w, h]: [f32; 4], band: f32, c: Palette, alpha: f32) {
    let steps = 180;
    let base = band * 0.24;
    let (pod, far) = (band * 0.62, band * 0.3);
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let along_w = w / steps as f32 + 0.6;
        let along_h = h / steps as f32 + 0.6;
        let top = far * (0.55 + 0.6 * wobble(11, t * 7.) + 0.25 * wobble(12, t * 31.));
        let bottom = pod * (0.5 + 0.7 * wobble(13, t * 5.) + 0.2 * wobble(14, t * 23.));
        let left = base * (0.6 + 0.8 * wobble(15, t * 6.) + 0.3 * wobble(16, t * 27.));
        let right = base * (0.6 + 0.8 * wobble(17, t * 6.) + 0.3 * wobble(18, t * 27.));
        for (rect, halo) in [
            ([x + t * w, y - top, along_w, top], [x + t * w, y - top * 1.12, along_w, top * 0.12]),
            ([x + t * w, y + h, along_w, bottom], [x + t * w, y + h + bottom, along_w, bottom * 0.1]),
            ([x - left, y + t * h, left, along_h], [x - left * 1.14, y + t * h, left * 0.14, along_h]),
            ([x + w, y + t * h, right, along_h], [x + w + right, y + t * h, right * 0.14, along_h]),
        ] {
            p.rect(halo, 0., c.ink, alpha * 0.35);
            p.rect(rect, 0., c.base, alpha * 0.96);
        }
    }
    // Corners stay filled so the edge reads as one continuous spread.
    for (cx, cy, cw, ch) in [
        (x - base * 1.1, y - far * 0.9, base * 1.1, far * 0.9),
        (x + w, y - far * 0.9, base * 1.1, far * 0.9),
        (x - base * 1.1, y + h, base * 1.1, pod * 0.8),
        (x + w, y + h, base * 1.1, pod * 0.8),
    ] {
        p.rect([cx, cy, cw, ch], base * 0.4, c.base, alpha * 0.96);
    }
    // Comb marks dragged from the far end.
    for k in 0..9 {
        let t = 0.08 + k as f32 * 0.105 + wobble(21, k as f32 * 1.7) * 0.03;
        let length = far * (0.5 + wobble(22, k as f32 * 2.3) * 0.9);
        let width = (band * 0.012).max(0.8);
        p.rect([x + t * w, y - far * 0.5 - length, width, length], width / 2., c.base, alpha * 0.55);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_perforations_keep_the_real_pitch_for_any_photo_shape() {
        let holes = |w: u32, h: u32| {
            let s = crate::designs::settings("scan-e6").unwrap();
            let l = crate::layout::calculate(w, h, &s).unwrap();
            let mut recipe = s.artwork.unwrap().design.unwrap();
            recipe.film = Some(FilmOptions { labels: false, texture: 0. });
            let mut frame = RgbImage::from_pixel(l.width, l.height, image::Rgb([241, 239, 233]));
            draw_window(&mut frame, &recipe, l, 0, 0).unwrap();
            let g = l.y as f32;
            let side = (g * 0.78).min(l.photo_height as f32 * 0.23);
            let row = (l.y as f32 - side * 0.46).round() as u32;
            let bright = |x: u32| frame.get_pixel(x, row)[0] > 200;
            (l.x + 1..l.x + l.photo_width).filter(|&x| bright(x) && !bright(x - 1)).count()
        };
        // 4.75 mm pitch against a 24 mm picture height.
        assert_eq!(holes(900, 600), 8, "a 3:2 frame spans about eight perforations");
        assert_eq!(holes(600, 600), 5, "a square frame spans about five");
    }
    #[test]
    fn thin_borders_drop_perforations_and_edge_print_instead_of_drawing_specks() {
        let rail_row_varies = |border: f64| {
            let mut s = crate::designs::settings("scan-35mm").unwrap();
            s.border_percent = border;
            let l = crate::layout::calculate(900, 600, &s).unwrap();
            let mut recipe = s.artwork.unwrap().design.unwrap();
            recipe.film = Some(FilmOptions { labels: true, texture: 0. });
            let mut frame = RgbImage::from_pixel(l.width, l.height, image::Rgb([28, 35, 37]));
            draw_window(&mut frame, &recipe, l, 0, 0).unwrap();
            let g = l.x.min(l.y) as f32;
            let band = (g * 0.94).min(600. * 0.18);
            let row = l.y - (band * 0.43).round() as u32;
            let first = *frame.get_pixel(l.x, row);
            (l.x..l.x + l.photo_width).any(|x| *frame.get_pixel(x, row) != first)
        };
        assert!(rail_row_varies(12.), "default border shows perforations");
        assert!(!rail_row_varies(4.), "a 4% border leaves a clean rebate");
    }
    #[test]
    fn portrait_film_edge_markings_are_rotated_without_mirroring() {
        let markings = |w, h| {
            let s = crate::designs::settings("scan-35mm").unwrap();
            let l = crate::layout::calculate(w, h, &s).unwrap();
            let mut recipe = s.artwork.unwrap().design.unwrap();
            let paint = |recipe: &DesignRecipe| {
                let mut frame = RgbImage::from_pixel(l.width, l.height, image::Rgb([28, 35, 37]));
                draw_window(&mut frame, recipe, l, 0, 0).unwrap();
                frame
            };
            recipe.film = Some(FilmOptions {
                labels: false,
                texture: 0.,
            });
            let plain = paint(&recipe);
            recipe.film.as_mut().unwrap().labels = true;
            let labeled = paint(&recipe);
            image::GrayImage::from_fn(l.width, l.height, |x, y| {
                let a = plain.get_pixel(x, y)[0];
                let b = labeled.get_pixel(x, y)[0];
                image::Luma([u8::from(a.abs_diff(b) > 40)])
            })
        };
        let expected = image::imageops::rotate90(&markings(1200, 800));
        let portrait = markings(800, 1200);
        let union = expected
            .pixels()
            .zip(portrait.pixels())
            .filter(|(a, b)| a[0] != 0 || b[0] != 0)
            .count();
        let matching = expected
            .pixels()
            .zip(portrait.pixels())
            .filter(|(a, b)| a[0] != 0 && b[0] != 0)
            .count();
        assert!(union > 100);
        assert!(
            matching * 100 >= union * 95,
            "Only {matching} of {union} edge-mark pixels match a real rotation"
        );
    }

    #[test]
    fn new_scans_have_format_specific_rails_even_on_a_square_canvas() {
        for (id, expected) in [
            ("scan-half", [4, 4, 0, 0]),
            ("scan-super16", [0, 0, 0, 1]),
            ("scan-65mm", [0, 0, 5, 5]),
        ] {
            let mut settings = crate::designs::settings(id).unwrap();
            settings.ratio = Some(crate::model::Ratio::default());
            let l = crate::layout::calculate(480, 720, &settings).unwrap();
            assert_eq!(l.width, l.height);
            let border = parse_color(&settings.frame_color).unwrap();
            let mut frame = RgbImage::from_pixel(l.width, l.height, image::Rgb(border));
            let art = settings.artwork.as_mut().unwrap();
            let recipe = art.design.as_mut().unwrap();
            recipe.film = Some(FilmOptions {
                labels: false,
                texture: 0.,
            });
            let hole = parse_color(&recipe.colors[1]).unwrap();
            draw_window(&mut frame, recipe, l, 0, 0).unwrap();

            let in_hole = |x, y| {
                let p: &image::Rgb<u8> = frame.get_pixel(x, y);
                p.0.iter().zip(hole).all(|(a, b)| a.abs_diff(b) <= 8)
            };
            let count_runs = |points: &[bool]| {
                points
                    .iter()
                    .enumerate()
                    .filter(|(i, on)| **on && (*i == 0 || !points[*i - 1]))
                    .count()
            };
            let rail_distance = (l.y as f32 * 0.94 * 0.43).round() as u32;
            let along_x = |y| {
                (l.x..l.x + l.photo_width)
                    .map(|x| in_hole(x, y))
                    .collect::<Vec<_>>()
            };
            let along_y = |x| {
                (l.y..l.y + l.photo_height)
                    .map(|y| in_hole(x, y))
                    .collect::<Vec<_>>()
            };
            let counts = [
                count_runs(&along_x(l.y - rail_distance)),
                count_runs(&along_x(l.y + l.photo_height + rail_distance)),
                count_runs(&along_y(l.x - rail_distance)),
                count_runs(&along_y(l.x + l.photo_width + rail_distance)),
            ];
            assert_eq!(counts, expected, "{id}");
            assert_eq!(
                image::imageops::crop_imm(&frame, l.x, l.y, l.photo_width, l.photo_height)
                    .to_image(),
                RgbImage::from_pixel(l.photo_width, l.photo_height, image::Rgb(border)),
                "{id} must keep every pixel in the opening",
            );
        }
    }

    #[test]
    fn legacy_templates_load_and_invalid_film_options_are_rejected() {
        let legacy: DesignRecipe=serde_json::from_str(r##"{"id":"film-edge","colors":["#EEEEEE","#CCCCCC","#111111"],"intensity":1,"scale":1}"##).unwrap();
        crate::designs::validate(&legacy).unwrap();
        assert!(legacy.film.is_none());
        let mut s = crate::designs::settings("scan-35mm").unwrap();
        let r = s.artwork.as_mut().unwrap().design.as_mut().unwrap();
        r.film.as_mut().unwrap().texture = f32::NAN;
        assert!(crate::designs::validate(r).is_err());
        r.film.as_mut().unwrap().texture = 2.;
        assert!(crate::designs::validate(r).is_err());
        r.film.as_mut().unwrap().texture = 0.2;
        r.id = "film-edge".into();
        assert!(crate::designs::validate(r).is_err());
    }
    #[test]
    fn texture_and_labels_persist_change_only_border_and_match_corner_closeup() {
        for (w, h) in [(360, 240), (240, 360)] {
            let mut s = crate::designs::settings("scan-35mm").unwrap();
            let l = crate::layout::calculate(w, h, &s).unwrap();
            let original = RgbImage::from_fn(l.width, l.height, |x, y| {
                image::Rgb([(x % 251) as u8, (y % 251) as u8, 171])
            });
            let mut full = original.clone();
            crate::artwork::draw(&mut full, s.artwork.as_ref().unwrap(), l).unwrap();
            let (ox, oy) = (l.x.saturating_sub(7), l.y.saturating_sub(9));
            let mut corner = image::imageops::crop_imm(&original, ox, oy, 45, 47).to_image();
            crate::artwork::draw_window(&mut corner, s.artwork.as_ref().unwrap(), l, ox, oy)
                .unwrap();
            assert_eq!(
                corner,
                image::imageops::crop_imm(&full, ox, oy, 45, 47).to_image()
            );
            let film = s
                .artwork
                .as_mut()
                .unwrap()
                .design
                .as_mut()
                .unwrap()
                .film
                .as_mut()
                .unwrap();
            film.labels = false;
            film.texture = 0.;
            let restored: crate::model::RenderSettings =
                serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            let opt = restored
                .artwork
                .as_ref()
                .unwrap()
                .design
                .as_ref()
                .unwrap()
                .film
                .as_ref()
                .unwrap();
            assert!(!opt.labels && opt.texture == 0.);
            let mut clean = original.clone();
            crate::artwork::draw(&mut clean, restored.artwork.as_ref().unwrap(), l).unwrap();
            assert_ne!(
                clean, full,
                "The user's film detail switches must affect the output"
            );
            for y in l.y..l.y + l.photo_height {
                for x in l.x..l.x + l.photo_width {
                    assert_eq!(full.get_pixel(x, y), original.get_pixel(x, y));
                    assert_eq!(clean.get_pixel(x, y), original.get_pixel(x, y));
                }
            }
        }
    }
}
