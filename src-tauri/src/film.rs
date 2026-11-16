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
    fn film_rect(
        &mut self,
        rect: [f32; 4],
        radius: f32,
        color: [u8; 3],
        alpha: f32,
        rotated: bool,
    ) {
        let [x, y, w, h] = rect;
        self.rect(
            if rotated { [y, x, h, w] } else { rect },
            radius,
            color,
            alpha,
        );
    }
    fn text(
        &mut self,
        value: &str,
        x: f32,
        y: f32,
        cell: f32,
        color: [u8; 3],
        alpha: f32,
        rotated: bool,
    ) {
        for (i, c) in value.chars().enumerate() {
            for (row, bits) in glyph(c).into_iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.film_rect(
                            [
                                x + (i * 6 + col) as f32 * cell,
                                y + row as f32 * cell,
                                cell * 0.84,
                                cell * 0.84,
                            ],
                            0.,
                            color,
                            alpha,
                            rotated,
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
    let band = (g * 0.94).min(w.min(h) * 0.18) * recipe.scale;
    let alpha = (recipe.intensity * 0.95).min(1.);
    let mut p = Painter { image, l, ox, oy };
    let id = recipe.id.as_str();
    let board = matches!(id, "scan-contact" | "scan-slide" | "scan-instant");
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
            if id == "scan-contact" { dark } else { paper },
            alpha,
        );
    }
    let edge = (w.min(h) * 0.0025 * recipe.scale).clamp(0.8, 10.);
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
            let e = edge * (1.2 + rough * 1.8);
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
    let rotated = h > w;
    let (u, v, long, short) = if rotated { (y, x, h, w) } else { (x, y, w, h) };
    let (count, one_sided) = match id {
        "scan-35mm" | "scan-amber" => (8, false),
        "scan-16mm" => (1, false),
        "scan-8mm" => (1, true),
        _ => (0, false),
    };
    if count > 0 {
        let pitch = long / count as f32;
        let hole_h = band * if count == 1 { 0.55 } else { 0.38 };
        let hole_w = (hole_h * if id == "scan-8mm" { 1.05 } else { 1.4 }).min(pitch * 0.65);
        for i in 0..count {
            let hu = u + pitch * (i as f32 + 0.5) - hole_w / 2.;
            for bottom in [false, true] {
                if one_sided && bottom {
                    continue;
                }
                let hv = if bottom {
                    v + short + band * 0.43 - hole_h / 2.
                } else {
                    v - band * 0.43 - hole_h / 2.
                };
                p.film_rect(
                    [hu - 1., hv - 1., hole_w + 2., hole_h + 2.],
                    hole_h * 0.17,
                    dark,
                    alpha,
                    rotated,
                );
                p.film_rect(
                    [hu, hv, hole_w, hole_h],
                    hole_h * 0.14,
                    paper,
                    alpha,
                    rotated,
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
    if id == "scan-120" {
        // A cut 120 rebate has no sprocket perforations.
        for i in 0..3 {
            p.film_rect(
                [
                    u + long * 0.76 + i as f32 * band * 0.045,
                    v + short + band * 0.36,
                    band * 0.018,
                    band * 0.22,
                ],
                0.,
                ink,
                alpha,
                rotated,
            );
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
        p.pixels(
            [0., 0., l.width as f32, l.height as f32],
            |pixel, gx, gy| {
                let n = (hash(gx, gy) & 255) as f32 / 127.5 - 1.;
                for value in pixel {
                    *value = (*value as f32 + n * amount).round().clamp(0., 255.) as u8;
                }
            },
        );
    }
    if options.labels {
        let cell = (band * 0.115 / 7.).max(0.55);
        let label = match id {
            "scan-35mm" => "35MM COLOR 400",
            "scan-amber" => "COLOR NEG 200",
            "scan-bw" => "B/W NEG 400",
            "scan-120" => "120 / FRAME 08",
            "scan-16mm" => "16MM 01",
            "scan-8mm" => "SUPER 8",
            "scan-contact" => "CONTACT 24",
            "scan-slide" => "35MM / MOUNT 01",
            _ => "",
        };
        if !label.is_empty() {
            let ty = v + short + band * if board { 0.34 } else { 0.75 };
            p.text(
                label,
                u + long * 0.055,
                ty,
                cell,
                ink,
                alpha * 0.85,
                rotated,
            );
            p.text(
                if id == "scan-120" { "08" } else { "24A" },
                u + long * 0.84,
                ty,
                cell,
                ink,
                alpha,
                rotated,
            );
            if count == 8 {
                p.text(
                    "24",
                    u + long * 0.78,
                    v - band * 0.9,
                    cell,
                    ink,
                    alpha,
                    rotated,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
