//! Painterly, material-led frame styles rendered procedurally on the border.
//!
//! Every value is a pure function of the pixel's position on the final canvas,
//! measured in units of the photograph's short edge, so a preview window, a
//! zoomed region and the full export agree and the look is resolution
//! independent. Work stays outside the photograph rectangle and needs no
//! full-size intermediate canvas.
use crate::{designs::DesignRecipe, imaging::parse_color, layout::Layout};
use image::RgbImage;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub const IDS: [&str; 9] = [
    "art-museum",
    "art-editorial",
    "art-field",
    "art-deckle",
    "art-ink",
    "art-cyanotype",
    "art-wash",
    "art-collage",
    "art-botanical",
];

pub fn is_art(id: &str) -> bool {
    IDS.contains(&id)
}

/// Border percentage each style needs to read as intended.
pub fn default_border(id: &str) -> f64 {
    match id {
        "art-museum" | "art-editorial" => 16.,
        "art-field" => 9.,
        "art-deckle" => 14.,
        "art-ink" | "art-cyanotype" => 15.,
        _ => 16.,
    }
}

// ---------- noise ----------
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_216.
}
fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ux, uy) = (fx * fx * (3. - 2. * fx), fy * fy * (3. - 2. * fy));
    let (ix, iy) = (ix as i32, iy as i32);
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0., 0.5, 1., 0.);
    for o in 0..octaves {
        sum += noise(x * freq, y * freq, seed.wrapping_add(o * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn mix(dst: &mut image::Rgb<u8>, c: [f32; 3], a: f32) {
    let a = a.clamp(0., 1.);
    for i in 0..3 {
        dst[i] = (dst[i] as f32 * (1. - a) + c[i] * a)
            .round()
            .clamp(0., 255.) as u8;
    }
}
fn multiply(dst: &mut image::Rgb<u8>, c: [f32; 3], a: f32) {
    let a = a.clamp(0., 1.);
    for i in 0..3 {
        let m = dst[i] as f32 * c[i] / 255.;
        dst[i] = (dst[i] as f32 * (1. - a) + m * a).round().clamp(0., 255.) as u8;
    }
}
fn shade(dst: &mut image::Rgb<u8>, amount: f32) {
    // Positive lightens toward white, negative darkens toward black.
    for i in 0..3 {
        let v = dst[i] as f32;
        let v = if amount >= 0. {
            v + (255. - v) * amount
        } else {
            v * (1. + amount)
        };
        dst[i] = v.round().clamp(0., 255.) as u8;
    }
}
fn rgb(c: [u8; 3]) -> [f32; 3] {
    [c[0] as f32, c[1] as f32, c[2] as f32]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Geometry of one pixel relative to the photograph.
struct Px {
    /// Canvas position in short-edge units.
    u: f32,
    v: f32,
    /// Distance outside the photograph, short-edge units (0 at its edge).
    d: f32,
    /// Position along the nearest photo side (short-edge units) and which side:
    /// 0 top, 1 right, 2 bottom, 3 left.
    along: f32,
    side: u8,
}
struct Geo {
    s: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    w: f32,
    h: f32,
    /// Thinnest border band, short-edge units.
    band: f32,
}
impl Geo {
    fn new(l: Layout) -> Self {
        let s = l.photo_width.min(l.photo_height).max(1) as f32;
        let (x0, y0) = (l.x as f32, l.y as f32);
        let (x1, y1) = (x0 + l.photo_width as f32, y0 + l.photo_height as f32);
        let band = x0
            .min(y0)
            .min(l.width as f32 - x1)
            .min(l.height as f32 - y1)
            / s;
        Self {
            s,
            x0,
            y0,
            x1,
            y1,
            w: l.width as f32,
            h: l.height as f32,
            band,
        }
    }
    fn at(&self, gx: u32, gy: u32) -> Px {
        let (x, y) = (gx as f32 + 0.5, gy as f32 + 0.5);
        let dx = (self.x0 - x).max(x - self.x1).max(0.);
        let dy = (self.y0 - y).max(y - self.y1).max(0.);
        let d = (dx * dx + dy * dy).sqrt() / self.s;
        // Nearest side by the larger overshoot (ties go to top/bottom).
        let side = if dy >= dx {
            if y < self.y0 {
                0
            } else {
                2
            }
        } else if x > self.x1 {
            1
        } else {
            3
        };
        let along = if side % 2 == 0 { x } else { y } / self.s;
        Px {
            u: x / self.s,
            v: y / self.s,
            d,
            along,
            side,
        }
    }
    /// Signed distance (short-edge units) to the photo rectangle grown by `pad`.
    fn sdf(&self, gx: f32, gy: f32, pad: f32) -> f32 {
        let p = pad * self.s;
        let (cx, cy) = ((self.x0 + self.x1) / 2., (self.y0 + self.y1) / 2.);
        let (hx, hy) = ((self.x1 - self.x0) / 2. + p, (self.y1 - self.y0) / 2. + p);
        let qx = (gx - cx).abs() - hx;
        let qy = (gy - cy).abs() - hy;
        let outside = (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt();
        (outside + qx.max(qy).min(0.)) / self.s
    }
}

pub fn draw_window(
    canvas: &mut RgbImage,
    recipe: &DesignRecipe,
    l: Layout,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    if !is_art(&recipe.id) {
        return Ok(());
    }
    crate::designs::validate(recipe)?;
    let g = Geo::new(l);
    if g.band * g.s < 2. {
        return Ok(());
    }
    let colors = recipe
        .colors
        .iter()
        .map(|c| parse_color(c).map(rgb))
        .collect::<Result<Vec<_>, _>>()?;
    let k = recipe.intensity;
    let scale = recipe.scale;
    if recipe.id == "art-botanical" {
        grain(canvas, &g, ox, oy, 0.022 * k);
        return botanical(canvas, &g, &colors, k, scale, ox, oy);
    }
    let (cw, ch) = canvas.dimensions();
    for y in 0..ch {
        for x in 0..cw {
            let (gx, gy) = (x + ox, y + oy);
            if gx as f32 >= g.x0
                && gx as f32 + 1. <= g.x1
                && gy as f32 >= g.y0
                && gy as f32 + 1. <= g.y1
            {
                continue;
            }
            let p = g.at(gx, gy);
            let dst = canvas.get_pixel_mut(x, y);
            match recipe.id.as_str() {
                "art-museum" => museum(dst, &g, &p, &colors, k, scale),
                "art-editorial" => editorial(dst, &g, &p, gx, gy, &colors, k, scale),
                "art-field" => field(dst, &g, &p, &colors, k, scale),
                "art-deckle" => deckle(dst, &g, &p, gx, gy, &colors, k, scale),
                "art-ink" => ink(dst, &g, &p, &colors, k, scale),
                "art-cyanotype" => cyanotype(dst, &g, &p, &colors, k, scale),
                "art-wash" => wash(dst, &g, &p, &colors, k, scale),
                "art-collage" => collage(dst, &g, &p, gx, gy, &colors, k, scale),
                _ => {}
            }
        }
    }
    Ok(())
}

fn paper_grain(dst: &mut image::Rgb<u8>, p: &Px, amount: f32) {
    if amount <= 0. {
        return;
    }
    let n = fbm(p.u * 220., p.v * 220., 2, 17) - 0.5;
    let fibre = noise(p.u * 30., p.v * 420., 29) - 0.5;
    shade(dst, (n * 1.4 + fibre * 0.6) * amount);
}
fn grain(canvas: &mut RgbImage, g: &Geo, ox: u32, oy: u32, amount: f32) {
    let (cw, ch) = canvas.dimensions();
    for y in 0..ch {
        for x in 0..cw {
            let (gx, gy) = (x + ox, y + oy);
            if gx as f32 >= g.x0
                && gx as f32 + 1. <= g.x1
                && gy as f32 >= g.y0
                && gy as f32 + 1. <= g.y1
            {
                continue;
            }
            let p = g.at(gx, gy);
            paper_grain(canvas.get_pixel_mut(x, y), &p, amount);
        }
    }
}

/// A gallery mat with a bevelled window: light catches the top and left bevel,
/// the bottom and right fall into soft shade; a hairline marks the cut.
fn museum(dst: &mut image::Rgb<u8>, g: &Geo, p: &Px, _c: &[[f32; 3]], k: f32, scale: f32) {
    paper_grain(dst, p, 0.018 * k);
    let bevel = (0.014 * scale).min(g.band * 0.3);
    let px = 1. / g.s;
    if p.d < bevel + px {
        let inside = 1. - smooth(bevel - px * 0.5, bevel + px * 0.5, p.d);
        let light = match p.side {
            0 => 0.7,
            3 => 0.55,
            1 => -0.08,
            _ => -0.14,
        };
        // The cut core is bright paper; it shades with the bevel's orientation.
        let mut bevel_col = *dst;
        shade(
            &mut bevel_col,
            if light > 0. { light * 0.6 } else { light * k },
        );
        mix(dst, rgb(bevel_col.0), inside);
        // Hairline where the bevel meets the mat face.
        let line = 1. - smooth(0., px * 0.9, (p.d - bevel).abs());
        shade(dst, -0.16 * line * k);
    }
    // A barely-there shadow cast by the mat onto its own bevel near the photo.
    if p.d < px * 1.5 {
        shade(dst, -0.08 * k);
    }
}

/// A wide off-white page with one hairline rule below the photograph, ready for
/// a caption in the lower margin.
fn editorial(
    dst: &mut image::Rgb<u8>,
    g: &Geo,
    p: &Px,
    gx: u32,
    gy: u32,
    c: &[[f32; 3]],
    k: f32,
    scale: f32,
) {
    paper_grain(dst, p, 0.014 * k);
    let below = (gy as f32 - g.y1) / g.s;
    let gap = (0.045 * scale).min(g.band * 0.45);
    let px = 1. / g.s;
    let weight = (0.0011 * g.s).max(1.) / g.s;
    if (gx as f32) >= g.x0 && (gx as f32) < g.x1 && (below - gap).abs() < weight + px {
        let a = 1. - smooth(weight * 0.5, weight * 0.5 + px, (below - gap).abs());
        mix(dst, c[0], a * 0.85 * k.min(1.));
    }
}

/// A deep colour field taken from the photo, a quiet keyline and a gentle
/// tonal falloff toward the outer edge.
fn field(dst: &mut image::Rgb<u8>, g: &Geo, p: &Px, c: &[[f32; 3]], k: f32, scale: f32) {
    let outer = ((p.u * g.s)
        .min(g.w - p.u * g.s)
        .min(p.v * g.s)
        .min(g.h - p.v * g.s))
        / g.s;
    shade(dst, -0.07 * k * (1. - smooth(0., g.band * 0.9, outer)));
    let mottle = fbm(p.u * 3., p.v * 3., 4, 5) - 0.5;
    shade(dst, mottle * 0.06 * k);
    paper_grain(dst, p, 0.02 * k);
    let gap = (0.028 * scale).min(g.band * 0.4);
    let px = 1. / g.s;
    let weight = (0.0012 * g.s).max(1.) / g.s;
    let line = 1. - smooth(weight * 0.5, weight * 0.5 + px, (p.d - gap).abs());
    if line > 0. {
        mix(dst, c[0], line * 0.75 * k.min(1.));
    }
}

/// A print on deckle-edged paper laid on a dark ground with a soft shadow.
fn deckle(
    dst: &mut image::Rgb<u8>,
    g: &Geo,
    p: &Px,
    gx: u32,
    gy: u32,
    c: &[[f32; 3]],
    k: f32,
    scale: f32,
) {
    let pad = (0.05 * scale).min(g.band * 0.55);
    let (fx, fy) = (gx as f32 + 0.5, gy as f32 + 0.5);
    let rough = (fbm(p.u * 22., p.v * 22., 4, 41) - 0.5) * 0.022
        + (noise(p.u * 140., p.v * 140., 43) - 0.5) * 0.006;
    let sheet = g.sdf(fx, fy, pad) + rough * k.min(1.2);
    // Shadow: the same sheet, offset down-right and blurred by distance.
    let off = 0.006;
    let shadow_d = g.sdf(fx - off * g.s, fy - off * 1.6 * g.s, pad) + rough;
    let shadow = (-shadow_d.max(0.) / 0.018).exp() * 0.5 * k.min(1.);
    let px = 1. / g.s;
    if sheet > px {
        shade(dst, -shadow);
        // Faint felt texture on the ground.
        shade(dst, (fbm(p.u * 60., p.v * 60., 2, 47) - 0.5) * 0.05);
        return;
    }
    let cover = 1. - smooth(-px, px, sheet);
    let mut paper = image::Rgb([c[1][0] as u8, c[1][1] as u8, c[1][2] as u8]);
    paper_grain(&mut paper, p, 0.035 * k);
    // Torn fibres are thinner and brighter right at the edge.
    let rim = smooth(-0.006, 0., sheet);
    shade(&mut paper, 0.25 * rim);
    let mut ground = *dst;
    shade(&mut ground, -shadow);
    mix(dst, rgb(ground.0), 1.);
    mix(dst, rgb(paper.0), cover * (1. - 0.25 * rim));
}

/// Dry-brush ink: four straight strokes laid along the photo edges that
/// overshoot and cross at the corners, with bristle streaks that break up as
/// the brush runs dry.
fn ink(dst: &mut image::Rgb<u8>, g: &Geo, p: &Px, c: &[[f32; 3]], k: f32, scale: f32) {
    paper_grain(dst, p, 0.02 * k);
    let width = (0.1 * scale).min(g.band * 0.85);
    let (x0, y0, x1, y1) = (g.x0 / g.s, g.y0 / g.s, g.x1 / g.s, g.y1 / g.s);
    let over = width * 1.2;
    let mut best = 0f32;
    for side in 0..4u32 {
        // across: distance outward from the photo edge; along: position on it.
        let (across, along, lo, hi) = match side {
            0 => (y0 - p.v, p.u, x0, x1),
            1 => (p.u - x1, p.v, y0, y1),
            2 => (p.v - y1, p.u, x0, x1),
            _ => (x0 - p.u, p.v, y0, y1),
        };
        // Each stroke wobbles a little off the edge along its length.
        let seed = 61 + side * 7;
        let drift = (fbm(along * 1.3, 0.2, 2, seed + 9) - 0.5) * width * 0.35;
        let across = across + drift;
        if across < -width * 0.15 || across > width * 1.3 {
            continue;
        }
        let start = lo - over * (0.4 + hash(side as i32, 1, seed));
        let end = hi + over * (0.4 + hash(side as i32, 2, seed));
        if along < start || along > end {
            continue;
        }
        let a = (across.max(0.) / width).min(1.3);
        // Ends of the stroke lift off: the brush lands heavy and leaves dry.
        let t = (along - start) / (end - start);
        let lift = smooth(0.82, 1., t) * 0.6 + (1. - smooth(0., 0.06, t)) * 0.35;
        let bristle = fbm(along * 1.4, across * 320. / scale.max(0.2), 4, seed);
        let load = fbm(along * 1.1, 0.5 + side as f32, 3, seed + 3);
        let splay = (fbm(along * 6., across * 18., 3, seed + 5) - 0.5) * 0.6;
        let threshold = 0.2 + a.powf(0.8) * 0.75 + splay - (load - 0.5) * 0.9 + lift;
        let coverage = smooth(threshold - 0.05, threshold + 0.05, bristle);
        let tone = 0.62 + 0.38 * smooth(0.3, 0.7, bristle + (1. - a) * 0.3);
        best = best.max(coverage * tone);
    }
    if best > 0. {
        multiply(dst, lerp3([255.; 3], c[0], 1.), best * k.min(1.));
    }
}

/// Cyanotype: a broad wash of Prussian-blue emulsion brushed onto the sheet,
/// its outer edge streaked by the brush, its body faintly mottled.
fn cyanotype(dst: &mut image::Rgb<u8>, g: &Geo, p: &Px, c: &[[f32; 3]], k: f32, scale: f32) {
    paper_grain(dst, p, 0.022 * k);
    let reach = (0.11 * scale).min(g.band * 0.85);
    if p.d > reach * 1.4 {
        return;
    }
    let seed = 73 + p.side as u32 * 11;
    let bristle = fbm(p.along * 1.6, p.d * 160., 3, seed);
    let swell = (fbm(p.along * 1.3, 0.3, 3, seed + 1) - 0.5) * 0.45;
    let edge = reach * (0.82 + swell) + (bristle - 0.5) * reach * 0.5;
    let px = 1. / g.s;
    let coverage = 1. - smooth(edge - px * 1.5, edge + px * 1.5, p.d);
    if coverage <= 0. {
        return;
    }
    let mottle = fbm(p.u * 7., p.v * 7., 4, 79);
    let deep = c[0];
    let light = c[1];
    let body = lerp3(
        deep,
        light,
        (mottle - 0.35).clamp(0., 0.5) * 0.9 + smooth(0.6, 1., p.d / edge.max(px)) * 0.25,
    );
    mix(dst, body, coverage * k.min(1.));
}

/// Loose watercolour blooms at the corners with pigment pooling at their
/// edges and granulation, leaving a clean margin around the photograph.
fn wash(dst: &mut image::Rgb<u8>, g: &Geo, p: &Px, c: &[[f32; 3]], k: f32, scale: f32) {
    paper_grain(dst, p, 0.03 * k);
    let (w, h) = (g.w / g.s, g.h / g.s);
    let r = (0.55 * scale).max(0.12);
    let blooms = [
        (0.0, 0.0, r * 1.15, 0, 3u32),
        (w, h, r * 1.2, 1, 7),
        (w, 0.0, r * 0.65, 2, 11),
        (0.0, h, r * 0.6, 1, 13),
        (w * 0.5, h, r * 0.45, 2, 17),
    ];
    let clear = 1. - smooth(0.015, 0.05, p.d);
    for (bx, by, br, ci, seed) in blooms {
        let dist = ((p.u - bx).powi(2) + (p.v - by).powi(2)).sqrt() / br;
        if dist > 1.6 {
            continue;
        }
        let wobble = (fbm(p.u * 1.8, p.v * 1.8, 5, seed) - 0.5) * 0.8;
        let f = 1. - dist + wobble;
        if f <= 0. {
            continue;
        }
        let pool = (-f / 0.045).exp();
        let gran = 0.8 + 0.4 * fbm(p.u * 90., p.v * 90., 2, seed + 1);
        let body = 0.3 + 0.25 * smooth(0.05, 0.6, f);
        let a = (body + 0.45 * pool) * gran * smooth(0., 0.012, f) * (1. - clear) * k;
        multiply(dst, c[ci], a.min(0.9));
    }
}

/// Torn paper pieces layered at two corners: coloured face, white torn core,
/// slight shadow underneath.
fn collage(
    dst: &mut image::Rgb<u8>,
    g: &Geo,
    p: &Px,
    gx: u32,
    gy: u32,
    c: &[[f32; 3]],
    k: f32,
    scale: f32,
) {
    paper_grain(dst, p, 0.025 * k);
    let b = g.band * 1.25 * scale.max(0.4);
    let (w, h) = (g.w / g.s, g.h / g.s);
    let (x0, y0, x1, y1) = (g.x0 / g.s, g.y0 / g.s, g.x1 / g.s, g.y1 / g.s);
    // centre x, y, half width, half height, rotation (rad), colour, seed
    let pieces = [
        (
            x0 * 0.5 + b * 0.2,
            y0 * 0.5 + b * 0.55,
            b * 1.05,
            b * 0.42,
            -0.14f32,
            0usize,
            5u32,
        ),
        (
            x0 * 0.5 + b * 0.95,
            y0 * 0.42,
            b * 0.75,
            b * 0.3,
            0.09,
            1,
            9,
        ),
        (
            w - (w - x1) * 0.5 - b * 0.35,
            h - (h - y1) * 0.5,
            b * 1.1,
            b * 0.38,
            0.11,
            2,
            13,
        ),
        (
            w - (w - x1) * 0.45,
            h - (h - y1) * 0.5 - b * 0.6,
            b * 0.36,
            b * 0.62,
            -0.05,
            0,
            21,
        ),
    ];
    let _ = (gx, gy);
    let px = 1. / g.s;
    for (cx, cy, hw, hh, rot, ci, seed) in pieces {
        let (s, co) = rot.sin_cos();
        let local = |du: f32, dv: f32| {
            let (dx, dy) = (p.u - cx - du, p.v - cy - dv);
            let (lx, ly) = (dx * co + dy * s, -dx * s + dy * co);
            let tear = (fbm(lx * 26. + seed as f32, ly * 26., 4, seed) - 0.5) * b * 0.18;
            let qx = lx.abs() - hw;
            let qy = ly.abs() - hh;
            (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt() + qx.max(qy).min(0.) + tear
        };
        let sh = local(0.004, 0.007);
        if sh < 0.02 {
            shade(dst, -0.18 * (1. - smooth(-0.004, 0.02, sh)) * k.min(1.));
        }
        let sd = local(0., 0.);
        if sd > px {
            continue;
        }
        let cover = 1. - smooth(-px, px, sd);
        let core = smooth(-b * 0.06, -b * 0.015, sd);
        let mut face = image::Rgb([c[ci][0] as u8, c[ci][1] as u8, c[ci][2] as u8]);
        paper_grain(&mut face, p, 0.05);
        let torn = lerp3(rgb(face.0), [248., 245., 238.], core * 0.9);
        mix(dst, torn, cover);
    }
}

/// Fine botanical line drawings that grow along the band from two corners.
fn botanical(
    canvas: &mut RgbImage,
    g: &Geo,
    c: &[[f32; 3]],
    k: f32,
    scale: f32,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    let (cw, ch) = canvas.dimensions();
    // Paths are rasterized in tiles on a fixed global 256 px grid so that a
    // preview window and the full export produce identical anti-aliasing.
    let mut ops: Vec<(tiny_skia::Path, Paint<'static>, Option<Stroke>)> = Vec::new();
    let band_top = g.y0;
    let band_left = g.x0;
    let band_bottom = g.h - g.y1;
    let band_right = g.w - g.x1;
    let stroke_w = (g.s * 0.0026).max(1.);
    let mut paint: Paint<'static> = Paint::default();
    paint.anti_alias = true;
    // A stem runs along the band's centre line from a corner, leaves splay to
    // both sides within the band.
    let mut sprig =
        |sx: f32, sy: f32, dx: f32, dy: f32, len: f32, room: f32, colour: [f32; 3], seed: u32| {
            let room = room * 0.58;
            let mut stem = PathBuilder::new();
            let (nx, ny) = (-dy, dx);
            let bow = room * 0.35;
            let pt = |t: f32| {
                let off = (t * std::f32::consts::PI).sin() * bow;
                (sx + dx * len * t + nx * off, sy + dy * len * t + ny * off)
            };
            stem.move_to(sx, sy);
            for i in 1..=24 {
                let (x, y) = pt(i as f32 / 24.);
                stem.line_to(x, y);
            }
            let mut leaves = PathBuilder::new();
            let count = ((len / (room * 0.42)).clamp(5., 26.)) as usize;
            for i in 1..count {
                let t = i as f32 / count as f32;
                let (x, y) = pt(t);
                let side = if i % 2 == 0 { 1. } else { -1. };
                let jitter = hash(i as i32, 0, seed) - 0.5;
                let ll = room * (0.95 - 0.5 * t) * (0.8 + jitter * 0.45);
                let lw = ll * 0.32;
                let ang = (dy.atan2(dx)) + side * (0.75 + jitter * 0.3);
                let (ca, sa) = (ang.cos(), ang.sin());
                let tip = (x + ca * ll, y + sa * ll);
                let mid = (x + ca * ll * 0.5, y + sa * ll * 0.5);
                let (px_, py_) = (-sa * lw, ca * lw);
                leaves.move_to(x, y);
                leaves.quad_to(mid.0 + px_, mid.1 + py_, tip.0, tip.1);
                leaves.quad_to(mid.0 - px_, mid.1 - py_, x, y);
                // Central vein, stopping short of the tip.
                leaves.move_to(x, y);
                leaves.line_to(x + ca * ll * 0.78, y + sa * ll * 0.78);
                // Every third node carries a small berry twig on the other side.
                if i % 3 == 0 {
                    let b = ang - side * 1.4;
                    let (tx, ty) = (x + b.cos() * room * 0.45, y + b.sin() * room * 0.45);
                    leaves.move_to(x, y);
                    leaves.quad_to(
                        x + b.cos() * room * 0.2 + ca * room * 0.1,
                        y + b.sin() * room * 0.2 + sa * room * 0.1,
                        tx,
                        ty,
                    );
                    leaves.push_circle(tx, ty, room * 0.055);
                }
            }
            // A small seed head at the tip.
            let (ex, ey) = pt(1.);
            for j in 0..3 {
                let a = (dy.atan2(dx)) + (j as f32 - 1.) * 0.5;
                let r = room * 0.12;
                let (bx, by) = (ex + a.cos() * room * 0.35, ey + a.sin() * room * 0.35);
                leaves.move_to(ex, ey);
                leaves.line_to(bx, by);
                leaves.push_circle(bx, by, r * 0.5);
            }
            paint.set_color_rgba8(
                colour[0] as u8,
                colour[1] as u8,
                colour[2] as u8,
                (230. * k.min(1.)) as u8,
            );
            let stroke = Stroke {
                width: stroke_w,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            if let Some(path) = stem.finish() {
                ops.push((
                    path,
                    paint.clone(),
                    Some(Stroke {
                        width: stroke_w * 1.25,
                        ..stroke.clone()
                    }),
                ));
            }
            if let Some(path) = leaves.finish() {
                let mut tint = paint.clone();
                tint.set_color_rgba8(
                    colour[0] as u8,
                    colour[1] as u8,
                    colour[2] as u8,
                    (46. * k.min(1.)) as u8,
                );
                ops.push((path.clone(), tint, None));
                ops.push((path, paint.clone(), Some(stroke.clone())));
            }
        };
    let top_room = band_top.min(band_left) * scale;
    let bot_room = band_bottom.min(band_right) * scale;
    let long = (g.x1 - g.x0).max(g.y1 - g.y0);
    // Top-left: one sprig along the top band, one shorter down the left band.
    sprig(
        band_left * 0.5,
        band_top * 0.55,
        1.,
        0.03,
        long * 0.42 * scale,
        top_room,
        c[0],
        3,
    );
    sprig(
        band_left * 0.55,
        band_top * 0.5,
        0.03,
        1.,
        long * 0.24 * scale,
        top_room,
        c[1],
        5,
    );
    // Bottom-right: a single answering sprig.
    sprig(
        g.w - band_right * 0.5,
        g.h - band_bottom * 0.55,
        -1.,
        -0.03,
        long * 0.34 * scale,
        bot_room,
        c[0],
        7,
    );
    const TILE: u32 = 256;
    let mut pix = Pixmap::new(TILE, TILE).ok_or("无法绘制线描")?;
    for ty in ((oy / TILE * TILE)..oy + ch).step_by(TILE as usize) {
        for tx in ((ox / TILE * TILE)..ox + cw).step_by(TILE as usize) {
            pix.fill(tiny_skia::Color::TRANSPARENT);
            let t = Transform::from_translate(-(tx as f32), -(ty as f32));
            for (path, paint, stroke) in &ops {
                match stroke {
                    Some(st) => pix.stroke_path(path, paint, st, t, None),
                    None => pix.fill_path(path, paint, FillRule::Winding, t, None),
                }
            }
            for gy in ty.max(oy)..(ty + TILE).min(oy + ch) {
                for gx in tx.max(ox)..(tx + TILE).min(ox + cw) {
                    if gx as f32 >= g.x0
                        && gx as f32 + 1. <= g.x1
                        && gy as f32 >= g.y0
                        && gy as f32 + 1. <= g.y1
                    {
                        continue;
                    }
                    let (x, y) = (gx - ox, gy - oy);
                    let s = pix.pixel(gx - tx, gy - ty).unwrap();
                    let a = s.alpha() as f32 / 255.;
                    if a > 0. {
                        let col = s.demultiply();
                        mix(
                            canvas.get_pixel_mut(x, y),
                            [col.red() as f32, col.green() as f32, col.blue() as f32],
                            a,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe(id: &str) -> DesignRecipe {
        let s = crate::designs::settings(id).unwrap();
        s.artwork.unwrap().design.unwrap()
    }
    #[test]
    fn styles_leave_the_photo_untouched_and_match_in_any_window() {
        let l = Layout {
            width: 600,
            height: 460,
            x: 70,
            y: 60,
            photo_width: 460,
            photo_height: 340,
            scale: 1.,
        };
        for id in IDS {
            let r = recipe(id);
            let base = RgbImage::from_pixel(600, 460, image::Rgb([240, 236, 228]));
            let mut full = base.clone();
            draw_window(&mut full, &r, l, 0, 0).unwrap();
            assert_ne!(full, base, "{id} drew nothing");
            for y in 60..400 {
                for x in 70..530 {
                    assert_eq!(
                        full.get_pixel(x, y),
                        base.get_pixel(x, y),
                        "{id} touched the photo"
                    );
                }
            }
            let (ox, oy) = (20, 380);
            let mut roi = image::imageops::crop_imm(&base, ox, oy, 200, 80).to_image();
            draw_window(&mut roi, &r, l, ox, oy).unwrap();
            assert_eq!(
                roi,
                image::imageops::crop_imm(&full, ox, oy, 200, 80).to_image(),
                "{id} window parity"
            );
        }
    }
}
