//! Bounded vector scenes. Rasterization uses reusable 256 px tiles, never a full RGBA canvas.
use crate::{imaging::parse_color, layout::Layout};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Stroke, Transform};
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtScene {
    #[serde(default)]
    pub elements: Vec<ArtElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::designs::DesignRecipe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ArtReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stickers: Vec<crate::stickers::Sticker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_image: Option<crate::frame_art::FrameImage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtReference {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub photo_width: u32,
    pub photo_height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtElement {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub color: String,
    pub opacity: f32,
    pub stroke_width: f32,
    pub filled: bool,
    pub points: Vec<[f32; 2]>,
}
pub fn validate(s: &ArtScene) -> Result<(), String> {
    if let Some(frame) = &s.frame_image {
        crate::frame_art::validate(frame)?;
    }
    if let Some(d) = &s.design {
        crate::designs::validate(d)?;
    }
    crate::stickers::validate(&s.stickers)?;
    if let Some(r) = &s.reference {
        if r.width == 0
            || r.height == 0
            || r.width > 32768
            || r.height > 32768
            || r.x > r.width
            || r.y > r.height
            || r.photo_width > r.width - r.x
            || r.photo_height > r.height - r.y
        {
            return Err("艺术边框参考尺寸无效".into());
        }
    }
    if s.elements.len() > 48 {
        return Err("艺术边框最多 48 个元素".into());
    }
    for e in &s.elements {
        if !matches!(
            e.kind.as_str(),
            "curve"
                | "polyline"
                | "star"
                | "sparkle"
                | "heart"
                | "cloud"
                | "leaf"
                | "fish"
                | "cat"
                | "flower"
                | "circle"
        ) {
            return Err("不支持的艺术元素".into());
        }
        parse_color(&e.color)?;
        for (v, lo, hi) in [
            (e.x, 0., 1000.),
            (e.y, 0., 1000.),
            (e.width, 1., 350.),
            (e.height, 1., 350.),
            (e.rotation, -360., 360.),
            (e.opacity, 0., 1.),
            (e.stroke_width, 0.3, 12.),
        ] {
            if !v.is_finite() || v < lo || v > hi {
                return Err("艺术边框参数超出范围".into());
            }
        }
        if e.points.len() > 16
            || e.points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || *v < 0. || *v > 1000.)
        {
            return Err("艺术线条坐标无效".into());
        }
        if (e.kind == "curve" && (e.points.len() < 4 || (e.points.len() - 1) % 3 != 0))
            || (e.kind == "polyline" && e.points.len() < 2)
        {
            return Err("艺术线条控制点不完整".into());
        }
    }
    Ok(())
}
fn path(e: &ArtElement, l: Layout) -> Option<tiny_skia::Path> {
    let mut b = PathBuilder::new();
    if e.kind == "curve" || e.kind == "polyline" {
        let p = &e.points;
        b.move_to(
            p[0][0] * l.width as f32 / 1000.,
            p[0][1] * l.height as f32 / 1000.,
        );
        if e.kind == "curve" {
            for c in p[1..].chunks_exact(3) {
                b.cubic_to(
                    c[0][0] * l.width as f32 / 1000.,
                    c[0][1] * l.height as f32 / 1000.,
                    c[1][0] * l.width as f32 / 1000.,
                    c[1][1] * l.height as f32 / 1000.,
                    c[2][0] * l.width as f32 / 1000.,
                    c[2][1] * l.height as f32 / 1000.,
                );
            }
        } else {
            for p in &p[1..] {
                b.line_to(
                    p[0] * l.width as f32 / 1000.,
                    p[1] * l.height as f32 / 1000.,
                );
            }
        }
        if e.filled {
            b.close();
        }
    } else {
        static SHAPES: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        let shape = SHAPES.get_or_init(|| {
            serde_json::from_str(include_str!("../../src/lib/art-shapes.json")).unwrap()
        });
        let commands = shape.get(&e.kind)?.as_array()?;
        let angle = e.rotation.to_radians();
        let (sin, cos) = angle.sin_cos();
        let point = |x: f32, y: f32| {
            let dx = (x - 50.) * e.width / 100.;
            let dy = (y - 50.) * e.height / 100.;
            (
                (e.x + dx * cos - dy * sin) * l.width as f32 / 1000.,
                (e.y + dx * sin + dy * cos) * l.height as f32 / 1000.,
            )
        };
        for c in commands {
            let c = c.as_array()?;
            let op = c[0].as_str()?;
            let vals: Vec<f32> = c[1..].iter().map(|v| v.as_f64().unwrap() as f32).collect();
            let pts: Vec<_> = vals.chunks_exact(2).map(|v| point(v[0], v[1])).collect();
            match op {
                "M" => b.move_to(pts[0].0, pts[0].1),
                "L" => b.line_to(pts[0].0, pts[0].1),
                "Q" => b.quad_to(pts[0].0, pts[0].1, pts[1].0, pts[1].1),
                "C" => b.cubic_to(pts[0].0, pts[0].1, pts[1].0, pts[1].1, pts[2].0, pts[2].1),
                "Z" => b.close(),
                _ => return None,
            }
        }
    }
    b.finish()
}
/// Reject misplaced revised geometry before rasterization. Each element lives
/// in one band; checking its control-point bounds is deliberately conservative.
/// This uses paths only, with no full-canvas mask or image allocation.
pub(crate) fn validate_placement(elements: &[ArtElement], l: Layout) -> Result<(), String> {
    const ERROR: &str = "精修装饰超出留白，请缩小图案或调整布局";
    for e in elements.iter().filter(|e| e.opacity > 0.) {
        let p = path(e, l).ok_or(ERROR)?;
        let b = p.bounds();
        let pad = e.stroke_width * l.width.min(l.height) as f32 / 2000. + 1.;
        let (left, top, right, bottom) = (
            b.left() - pad,
            b.top() - pad,
            b.right() + pad,
            b.bottom() + pad,
        );
        if left < 0.
            || top < 0.
            || right > l.width as f32
            || bottom > l.height as f32
            || !(bottom <= l.y as f32
                || top >= (l.y + l.photo_height) as f32
                || right <= l.x as f32
                || left >= (l.x + l.photo_width) as f32)
        {
            return Err(ERROR.into());
        }
    }
    Ok(())
}
fn adaptive_path(
    e: &ArtElement,
    l: Layout,
    reference: Option<&ArtReference>,
) -> Option<tiny_skia::Path> {
    let Some(r) = reference else {
        return path(e, l);
    };
    let old = Layout {
        width: r.width,
        height: r.height,
        x: r.x,
        y: r.y,
        photo_width: r.photo_width,
        photo_height: r.photo_height,
        scale: 1.,
    };
    let p = path(e, old)?;
    let b = p.bounds();
    let cx = (b.left() + b.right()) / 2.;
    let cy = (b.top() + b.bottom()) / 2.;
    let horizontal = cy < r.y as f32 || cy > (r.y + r.photo_height) as f32;
    let (sx, sy, dx, dy) = if horizontal {
        let bottom = cy > r.y as f32;
        let ob = if bottom {
            r.height - r.y - r.photo_height
        } else {
            r.y
        } as f32;
        let nb = if bottom {
            l.height - l.y - l.photo_height
        } else {
            l.y
        } as f32;
        if ob < 1. || nb < 1. {
            return None;
        }
        let sx = l.width as f32 / r.width as f32;
        let sy = nb / ob;
        let oy = if bottom { r.height as f32 - ob } else { 0. };
        let ny = if bottom { l.height as f32 - nb } else { 0. };
        (sx, sy, 0., ny - oy * sy)
    } else {
        let right = cx > (r.x + r.photo_width) as f32;
        let ob = if right {
            r.width - r.x - r.photo_width
        } else {
            r.x
        } as f32;
        let nb = if right {
            l.width - l.x - l.photo_width
        } else {
            l.x
        } as f32;
        if ob < 1. || nb < 1. {
            return None;
        }
        let sx = nb / ob;
        let sy = l.height as f32 / r.height as f32;
        let ox = if right { r.width as f32 - ob } else { 0. };
        let nx = if right { l.width as f32 - nb } else { 0. };
        (sx, sy, nx - ox * sx, 0.)
    };
    let (ax, ay) = if matches!(e.kind.as_str(), "curve" | "polyline") {
        (sx, sy)
    } else {
        let k = sx.min(sy);
        (k, k)
    };
    p.transform(Transform::from_row(
        ax,
        0.,
        0.,
        ay,
        dx + cx * (sx - ax),
        dy + cy * (sy - ay),
    ))
}
pub fn draw(canvas: &mut RgbImage, scene: &ArtScene, l: Layout) -> Result<(), String> {
    draw_window(canvas, scene, l, 0, 0)
}
pub fn draw_window(
    canvas: &mut RgbImage,
    scene: &ArtScene,
    l: Layout,
    ox: u32,
    oy: u32,
) -> Result<(), String> {
    validate(scene)?;
    if scene.elements.is_empty()
        && scene.design.is_none()
        && scene.stickers.is_empty()
        && scene.frame_image.is_none()
    {
        return Ok(());
    }
    if let Some(frame) = &scene.frame_image {
        crate::frame_art::draw_window(canvas, frame, l, ox, oy)?;
    }
    if let Some(d) = &scene.design {
        crate::film::draw_window(canvas, d, l, ox, oy)?;
        crate::art_styles::draw_window(canvas, d, l, ox, oy)?;
    }
    let mut elements = scene.elements.clone();
    if let Some(d) = &scene.design {
        elements.extend(crate::designs::elements(d, l));
    }
    let paths: Vec<_> = elements
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let p = if i < scene.elements.len() {
                adaptive_path(e, l, scene.reference.as_ref())
            } else {
                path(e, l)
            };
            p.map(|p| (e, p))
        })
        .collect();
    if paths.is_empty() {
        // Procedural film and bitmap frames need no transparent vector tiles.
        return crate::stickers::draw_window(canvas, scene, l, ox, oy);
    }
    for ty in ((oy / 256 * 256)..oy + canvas.height()).step_by(256) {
        for tx in ((ox / 256 * 256)..ox + canvas.width()).step_by(256) {
            let w = 256.min(l.width - tx);
            let h = 256.min(l.height - ty);
            if tx >= l.x
                && ty >= l.y
                && tx + w <= l.x + l.photo_width
                && ty + h <= l.y + l.photo_height
            {
                continue;
            }
            let mut tile = tiny_skia::Pixmap::new(w, h).ok_or("无法分配艺术边框小块")?;
            for (e, path) in &paths {
                let stroke = e.stroke_width * l.width.min(l.height) as f32 / 1000.;
                let b = path.bounds();
                if b.right() + stroke < tx as f32
                    || b.bottom() + stroke < ty as f32
                    || b.left() - stroke > (tx + w) as f32
                    || b.top() - stroke > (ty + h) as f32
                {
                    continue;
                }
                let c = parse_color(&e.color)?;
                let mut paint = Paint::default();
                paint.set_color_rgba8(c[0], c[1], c[2], (e.opacity * 255.).round() as u8);
                paint.anti_alias = true;
                let t = Transform::from_translate(-(tx as f32), -(ty as f32));
                if e.filled {
                    tile.fill_path(path, &paint, FillRule::EvenOdd, t, None);
                } else {
                    tile.stroke_path(
                        path,
                        &paint,
                        &Stroke {
                            width: stroke,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                            ..Stroke::default()
                        },
                        t,
                        None,
                    );
                }
            }
            for y in 0..h {
                for x in 0..w {
                    let dx = tx + x;
                    let dy = ty + y;
                    if dx < ox || dy < oy || dx >= ox + canvas.width() || dy >= oy + canvas.height()
                    {
                        continue;
                    }
                    if dx >= l.x
                        && dx < l.x + l.photo_width
                        && dy >= l.y
                        && dy < l.y + l.photo_height
                    {
                        continue;
                    }
                    let p = tile.pixels()[(y * w + x) as usize];
                    let a = u32::from(p.alpha());
                    if a == 0 {
                        continue;
                    }
                    let rgb = [p.red(), p.green(), p.blue()];
                    let dest = canvas.get_pixel_mut(dx - ox, dy - oy);
                    for c in 0..3 {
                        dest[c] = (u32::from(rgb[c])
                            + ((u32::from(dest[c]) * (255 - a) + 127) / 255))
                            .min(255) as u8;
                    }
                }
            }
        }
    }
    crate::stickers::draw_window(canvas, scene, l, ox, oy)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> ArtScene {
        ArtScene {
            elements: vec![
                ArtElement {
                    kind: "curve".into(),
                    x: 50.,
                    y: 50.,
                    width: 100.,
                    height: 100.,
                    rotation: 0.,
                    color: "#114488".into(),
                    opacity: 0.8,
                    stroke_width: 5.,
                    filled: false,
                    points: vec![[0., 0.], [200., 800.], [800., 200.], [1000., 1000.]],
                },
                ArtElement {
                    kind: "cat".into(),
                    x: 80.,
                    y: 900.,
                    width: 100.,
                    height: 100.,
                    rotation: 0.,
                    color: "#D07030".into(),
                    opacity: 1.,
                    stroke_width: 3.,
                    filled: false,
                    points: vec![],
                },
            ],
            ..Default::default()
        }
    }
    #[test]
    fn artwork_changes_border_but_never_source_pixels() {
        let mut settings = crate::model::RenderSettings::default();
        settings.border_percent = 20.;
        let l = crate::layout::calculate(400, 300, &settings).unwrap();
        let mut c = RgbImage::from_pixel(l.width, l.height, image::Rgb([255, 255, 255]));
        draw(&mut c, &scene(), l).unwrap();
        assert!(c.pixels().any(|p| p[0] < 240));
        for y in l.y..l.y + l.photo_height {
            for x in l.x..l.x + l.photo_width {
                assert_eq!(c.get_pixel(x, y).0, [255, 255, 255]);
            }
        }
    }
    #[test]
    fn rejects_unbounded_and_executable_elements() {
        let mut s = scene();
        s.elements[0].kind = "<svg>".into();
        assert!(validate(&s).is_err());
        s = scene();
        s.elements[0].points[0][0] = f32::NAN;
        assert!(validate(&s).is_err());
        s = scene();
        s.elements = vec![s.elements[0].clone(); 49];
        assert!(validate(&s).is_err());
    }
}
