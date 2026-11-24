//! Curated compositions. Every coordinate is derived from the actual border bands.
use crate::{
    artwork::{ArtElement, ArtScene},
    layout::Layout,
    model::RenderSettings,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesignRecipe {
    pub id: String,
    pub colors: Vec<String>,
    pub intensity: f32,
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub film: Option<crate::film::FilmOptions>,
}
fn default_scale() -> f32 {
    1.
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinFrame {
    pub id: String,
    pub name: String,
    pub english_name: String,
    pub category: String,
    pub description: String,
    pub settings: RenderSettings,
    pub thumbnail: String,
}
const SPECS: [(&str, &str, &str, &str, &str, &str, [&str; 3]); 34] = [
    (
        "gallery",
        "摄影画廊",
        "Gallery Mat",
        "photo",
        "柔白卡纸与一圈贴合照片的细线，保留安静的展示空间。",
        "#F8F7F3",
        ["#9B9D96", "#D6D6CE", "#626960"],
    ),
    (
        "double-mat",
        "双层卡纸",
        "Double Mat",
        "photo",
        "内层卡纸与外层留白形成两级层次，四边贴合照片开口。",
        "#F4F1E9",
        ["#CCD2C7", "#7E887A", "#E6E2D8"],
    ),
    (
        "keyline",
        "细线双框",
        "Double Keyline",
        "photo",
        "两圈有节奏的精细线框，颜色、线条强度和大小均可调整。",
        "#FCFBF8",
        ["#36494D", "#B19A75", "#D6D6CC"],
    ),
    (
        "film-edge",
        "胶片边界",
        "Film Edge",
        "film",
        "炭黑底、细相纸边与克制的片孔标记，保持清晰的照片边界。",
        "#242928",
        ["#EEE5D1", "#B59B69", "#5F706C"],
    ),
    (
        "film-negative",
        "黑金底片",
        "Amber Negative",
        "film",
        "深黑底片边、琥珀色细线与四角定位标记，干净利落。",
        "#1C201F",
        ["#D2AA68", "#EEE4CF", "#77694E"],
    ),
    (
        "film-paper",
        "暖白相纸",
        "Warm Photo Paper",
        "film",
        "暖白相纸、双层细边与底部短线，像一张整理好的暗房印相。",
        "#F5F0E5",
        ["#6D7067", "#B5A68B", "#D8CEBB"],
    ),
    (
        "scan-35mm",
        "35mm 全幅扫描",
        "35mm Full Scan",
        "film",
        "圆角齿孔、片边微字与细微扫描颗粒，模拟完整底片。",
        "#1C2325",
        ["#D1B57D", "#040505", "#72513A"],
    ),
    (
        "scan-amber",
        "彩负暖边",
        "Warm Color Negative",
        "film",
        "暖棕片基、琥珀色边缘与完整齿孔，像彩负扫描的留边。",
        "#3A271D",
        ["#E5A55E", "#060504", "#986C45"],
    ),
    (
        "scan-bw",
        "黑白底片扫描",
        "Silver Negative",
        "film",
        "无齿孔的黑色片边、银灰片号与轻微不规则内缘。",
        "#111413",
        ["#D0D0C8", "#8D918B", "#050706"],
    ),
    (
        "scan-120",
        "120 中画幅",
        "120 Medium Format",
        "film",
        "宽厚黑色片边、120 格式标记与简洁定位条，不添加齿孔。",
        "#191C19",
        ["#C7B782", "#7D8173", "#0B0F0D"],
    ),
    (
        "scan-16mm",
        "16mm 电影胶片",
        "16mm Cine Scan",
        "film",
        "双侧单孔、电影片号与黑色扫描边，保留完整照片。",
        "#292D29",
        ["#DACB9D", "#050706", "#72624E"],
    ),
    (
        "scan-8mm",
        "Super 8 单孔",
        "Super 8 Scan",
        "film",
        "单侧大齿孔与暖色片边微字，模拟 Super 8 的外观。",
        "#312716",
        ["#D6AE73", "#050403", "#83613D"],
    ),
    (
        "scan-contact",
        "暗房接触印样",
        "Darkroom Contact",
        "film",
        "暖白纸上的黑色底片条、红色选片线与接触印样编号。",
        "#EAE6DC",
        ["#A65C4D", "#DFD9CC", "#242621"],
    ),
    (
        "scan-slide",
        "纸质幻灯片框",
        "Cardboard Slide",
        "film",
        "米白装片卡、黑色开口细边与微小装片标记。",
        "#DAD8CF",
        ["#777466", "#F2EEE1", "#3B3C35"],
    ),
    (
        "scan-instant",
        "拍立得相纸",
        "Instant Photo Paper",
        "film",
        "轻微纸张颗粒、较宽的下沿与干净相纸边，完整保留照片。",
        "#D8D6CE",
        ["#8F8B7E", "#F8F5E9", "#5F6057"],
    ),
    (
        "scan-half",
        "35mm 半格扫描",
        "35mm Half-frame",
        "film",
        "半格相机的 18×24 开口，短边各有四枚齿孔，竖片上方和下方保留片边。",
        "#29231F",
        ["#C8A57C", "#090707", "#774B34"],
    ),
    (
        "scan-super16",
        "Super 16 单侧扫描",
        "Super 16 Rebate",
        "film",
        "单侧走片孔、细绿灰片基与琥珀片号，像保留了画门的电影扫描。",
        "#272C25",
        ["#D2B687", "#0C1110", "#55523A"],
    ),
    (
        "scan-65mm",
        "65mm 五齿孔扫描",
        "65mm Five-perf",
        "film",
        "两侧各五枚齿孔，冷灰片基、银色细标和细密的扫描编号。",
        "#30383B",
        ["#D1C5AA", "#101719", "#6B7979"],
    ),
    (
        "scan-carrier",
        "放大机手挫黑边",
        "Hand-filed Darkroom Edge",
        "film",
        "暖白相纸托住无齿孔的深色画门，细微不规则黑边模拟手挫底片夹。",
        "#E2DBCA",
        ["#897E69", "#BAB09A", "#171A17"],
    ),
    (
        "color-band",
        "现代色带",
        "Modern Color Bands",
        "graphic",
        "两组长短色带建立版面秩序，搭配很轻的对角呼应。",
        "#F9F6EE",
        ["#315D69", "#BF7155", "#D6C3A0"],
    ),
    (
        "corner-blocks",
        "几何角构",
        "Geometric Corners",
        "graphic",
        "圆与直角色块组成对角构图，清晰平面色，不添加绘画纹理。",
        "#F5F2E9",
        ["#435F67", "#BF775E", "#D5BD88"],
    ),
    (
        "pinstripe",
        "条纹纸边",
        "Pinstripe Margin",
        "graphic",
        "细密而规整的边缘条纹，用疏密差异呼应照片。",
        "#F7F4EB",
        ["#596E68", "#B59771", "#C9CDBE"],
    ),
    (
        "paper-tabs",
        "拼贴纸角",
        "Paper Tabs",
        "graphic",
        "两块平面纸角和短线标签，保留手账感与干净轮廓。",
        "#FAF6EE",
        ["#8B9C89", "#D1AD87", "#6B7C81"],
    ),
    (
        "tidal",
        "潮汐留白",
        "Tidal Lines",
        "lines",
        "沿海岸起伏的连续细线，暖色落点与大块留白。",
        "#F6F2E9",
        ["#426C70", "#B97A5A", "#8DA6A0"],
    ),
    (
        "contour",
        "山间等高线",
        "Contour Study",
        "lines",
        "两组不对称等高线，在对角之间建立呼应。",
        "#F4EDDF",
        ["#94744D", "#BA9D70", "#635E49"],
    ),
    (
        "botanical",
        "枝叶笺",
        "Botanical Note",
        "botanical",
        "叶片沿同一枝条生长，细线枝叶与远端的小枝呼应。",
        "#F4F3E9",
        ["#64795D", "#9CA989", "#BC9C69"],
    ),
    (
        "ribbon",
        "丝带交响",
        "Woven Ribbon",
        "lines",
        "两条丝带穿插而过，用断线表现交织层次。",
        "#F9EFE7",
        ["#A55E54", "#6E8497", "#CEAB81"],
    ),
    (
        "lunar",
        "月相书签",
        "Lunar Ledger",
        "geometry",
        "月相序列与细框线，保留安静的视觉中心。",
        "#EBEDE7",
        ["#56636A", "#A99B78", "#7B8C8C"],
    ),
    (
        "camellia",
        "山茶纸笺",
        "Camellia Letter",
        "botanical",
        "花瓣、花蕊与枝叶组成一幅连续的角落插画。",
        "#FAEEE8",
        ["#B97973", "#72816B", "#C0A37B"],
    ),
    (
        "deco",
        "拱廊拾光",
        "Deco Arches",
        "geometry",
        "细框与成组扇形拱线，像书籍扉页的装饰秩序。",
        "#EDE8DC",
        ["#394F58", "#AD8B53", "#8D9A91"],
    ),
    (
        "orbit",
        "星轨",
        "Quiet Orbit",
        "geometry",
        "椭圆轨迹与少量星点组成有方向的星图。",
        "#EEF1ED",
        ["#576A80", "#B49E79", "#8F9A9E"],
    ),
    (
        "cat",
        "午后猫咪",
        "Afternoon Cat",
        "illustration",
        "伏在细线上的猫咪、弯曲尾巴与一只小蝴蝶。",
        "#F7F0DF",
        ["#726050", "#BE8D64", "#9AA68D"],
    ),
    (
        "journal",
        "折页旅记",
        "Folded Journal",
        "illustration",
        "半透明纸角、邮笺式细线与路线，像轻盈旅行手账。",
        "#F4EEE1",
        ["#8A947D", "#C39A75", "#6D8390"],
    ),
    (
        "watercolor",
        "水彩桉叶",
        "Eucalyptus Watercolor",
        "botanical",
        "真实透明水彩素材，与克制的书页线条搭配。",
        "#F7F3E8",
        ["#7C8C6E", "#B79E6B", "#A8B496"],
    ),
];
pub fn validate(r: &DesignRecipe) -> Result<(), String> {
    if !SPECS.iter().any(|s| s.0 == r.id)
        || r.colors.len() != 3
        || !r.intensity.is_finite()
        || !(0.2..=1.5).contains(&r.intensity)
        || !r.scale.is_finite()
        || !(0.2..=1.).contains(&r.scale)
    {
        return Err("设计模板参数无效".into());
    }
    if let Some(film) = &r.film {
        if !crate::film::is_scan(&r.id)
            || !film.texture.is_finite()
            || !(0.0..=1.0).contains(&film.texture)
        {
            return Err("胶片质感参数无效".into());
        }
    }
    for c in &r.colors {
        crate::imaging::parse_color(c)?;
    }
    Ok(())
}
pub fn settings(id: &str) -> Option<RenderSettings> {
    let s = SPECS.iter().find(|s| s.0 == id)?;
    let mut v = RenderSettings::default();
    v.border_percent = match id {
        "scan-65mm" | "scan-carrier" => 15.,
        "scan-half" | "scan-super16" => 13.,
        id if crate::film::is_scan(id) => 12.,
        _ => 18.,
    };
    v.frame_color = s.5.into();
    v.artwork = Some(ArtScene {
        design: Some(DesignRecipe {
            id: s.0.into(),
            colors: s.6.map(String::from).to_vec(),
            intensity: 1.,
            scale: 1.,
            film: crate::film::is_scan(id).then(crate::film::FilmOptions::default),
        }),
        ..Default::default()
    });
    Some(v)
}
pub fn library() -> Result<Vec<BuiltinFrame>, String> {
    static CACHE: std::sync::OnceLock<Vec<BuiltinFrame>> = std::sync::OnceLock::new();
    if let Some(c) = CACHE.get() {
        return Ok(c.clone());
    }
    let mut out = Vec::new();
    for s in SPECS {
        let settings = settings(s.0).unwrap();
        let thumbnail = crate::imaging::frame_thumbnail(&settings)?;
        out.push(BuiltinFrame {
            id: s.0.into(),
            name: s.1.into(),
            english_name: s.2.into(),
            category: s.3.into(),
            description: s.4.into(),
            settings,
            thumbnail,
        });
    }
    let _ = CACHE.set(out.clone());
    Ok(out)
}
struct Composer {
    l: Layout,
    colors: Vec<String>,
    opacity: f32,
    out: Vec<ArtElement>,
}
impl Composer {
    fn line(&mut self, pts: &[[f32; 2]], c: usize, width: f32, curve: bool, filled: bool) {
        if pts.len() < 2 {
            return;
        }
        let n = pts
            .iter()
            .map(|p| {
                [
                    p[0] * 1000. / self.l.width as f32,
                    p[1] * 1000. / self.l.height as f32,
                ]
            })
            .collect();
        self.out.push(ArtElement {
            kind: if curve { "curve" } else { "polyline" }.into(),
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
            rotation: 0.,
            color: self.colors[c % 3].clone(),
            opacity: self.opacity,
            stroke_width: width * 1.9,
            filled,
            points: n,
        });
    }
    fn rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        c: usize,
        filled: bool,
        stroke: f32,
    ) {
        if width <= 0. || height <= 0. {
            return;
        }
        self.line(
            &[
                [x, y],
                [x + width, y],
                [x + width, y + height],
                [x, y + height],
                [x, y],
            ],
            c,
            stroke,
            false,
            filled,
        );
    }
    fn curve(&mut self, pts: &[[f32; 2]], c: usize, width: f32) {
        self.line(pts, c, width, true, false)
    }
    fn shape(&mut self, kind: &str, x: f32, y: f32, size: f32, c: usize, width: f32, filled: bool) {
        self.out.push(ArtElement {
            kind: kind.into(),
            x: x * 1000. / self.l.width as f32,
            y: y * 1000. / self.l.height as f32,
            width: size * 1000. / self.l.width as f32,
            height: size * 1000. / self.l.height as f32,
            rotation: 0.,
            color: self.colors[c % 3].clone(),
            opacity: self.opacity,
            stroke_width: width * 1.9,
            filled,
            points: vec![],
        });
    }
    fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, c: usize, w: f32) {
        let k = 0.5522848;
        self.curve(
            &[
                [x - rx, y],
                [x - rx, y - k * ry],
                [x - k * rx, y - ry],
                [x, y - ry],
                [x + k * rx, y - ry],
                [x + rx, y - k * ry],
                [x + rx, y],
                [x + rx, y + k * ry],
                [x + k * rx, y + ry],
                [x, y + ry],
                [x - k * rx, y + ry],
                [x - rx, y + k * ry],
                [x - rx, y],
            ],
            c,
            w,
        );
    }
    fn branch(&mut self, x: f32, y: f32, s: f32, reverse: bool) {
        let sign = if reverse { -1. } else { 1. };
        self.curve(
            &[
                [x, y],
                [x + s * 0.3 * sign, y - s * 0.06],
                [x + s * 0.5 * sign, y - s * 0.55],
                [x + s * 0.9 * sign, y - s * 0.7],
            ],
            0,
            0.9,
        );
        for i in 1..6 {
            let t = i as f32 / 6.;
            let bx = x + s * t * 0.85 * sign;
            let by = y - s * t * t * 0.7;
            let side = if i % 2 == 0 { -1. } else { 1. };
            let ex = bx + s * 0.18 * sign;
            let ey = by + s * 0.22 * side;
            self.line(
                &[
                    [bx, by],
                    [bx + s * 0.06 * sign, by + s * 0.18 * side],
                    [ex, ey],
                    [ex, ey],
                    [bx + s * 0.24 * sign, by + s * 0.01],
                    [bx, by],
                    [bx, by],
                ],
                (i % 2) as usize,
                0.7,
                true,
                false,
            );
        }
    }
}
/// Compiles only a small set of paths; no bitmap or full-canvas allocation.
pub fn elements(r: &DesignRecipe, l: Layout) -> Vec<ArtElement> {
    let w = l.width as f32;
    let h = l.height as f32;
    let short = w.min(h);
    let top = l.y as f32;
    let bottom = (l.height - l.y - l.photo_height) as f32;
    let band = top.min(bottom) * r.scale;
    if band < 2. || l.x == 0 {
        return Vec::new();
    }
    let y = h - bottom * 0.48;
    let yt = top * 0.48;
    let pad = (short * 0.025).min(band * 0.27);
    let size = band * 0.75;
    let mut a = Composer {
        l,
        colors: r.colors.clone(),
        opacity: (r.intensity * 0.82).min(1.),
        out: Vec::new(),
    };
    match r.id.as_str() {
        "gallery" | "double-mat" | "keyline" | "film-edge" | "film-negative" | "film-paper" => {
            let (px, py, pw, ph) = (
                l.x as f32,
                l.y as f32,
                l.photo_width as f32,
                l.photo_height as f32,
            );
            let gutter = (px.min(w - px - pw).min(py).min(h - py - ph) * 0.24 * r.scale).max(1.);
            if r.id == "double-mat" {
                a.rect(
                    px - gutter,
                    py - gutter,
                    pw + 2. * gutter,
                    gutter,
                    0,
                    true,
                    0.3,
                );
                a.rect(px - gutter, py + ph, pw + 2. * gutter, gutter, 0, true, 0.3);
                a.rect(px - gutter, py, gutter, ph, 0, true, 0.3);
                a.rect(px + pw, py, gutter, ph, 0, true, 0.3);
                a.rect(
                    px - gutter * 1.1,
                    py - gutter * 1.1,
                    pw + gutter * 2.2,
                    ph + gutter * 2.2,
                    1,
                    false,
                    0.35,
                );
            } else {
                let inset = if r.id == "film-edge" {
                    gutter * 0.18
                } else {
                    gutter * 0.35
                };
                a.rect(
                    px - inset,
                    py - inset,
                    pw + inset * 2.,
                    ph + inset * 2.,
                    0,
                    false,
                    if r.id == "gallery" { 0.35 } else { 0.6 },
                );
                if r.id == "keyline" || r.id == "film-paper" {
                    a.rect(
                        px - gutter,
                        py - gutter,
                        pw + gutter * 2.,
                        ph + gutter * 2.,
                        1,
                        false,
                        0.4,
                    );
                }
                if r.id == "film-edge" {
                    // Perforated rails hug the photo like a real strip and turn with
                    // portrait photos; space added by a fixed ratio stays plain.
                    let vertical = ph > pw;
                    let (start, length, room) = if vertical {
                        (py, ph, px.min(w - px - pw))
                    } else {
                        (px, pw, py.min(h - py - ph))
                    };
                    let rail = room.min(pw.min(ph) * 0.14) * r.scale;
                    if rail >= pw.min(ph) * crate::film::LEGIBLE_BAND {
                        let count = ((length / (rail * 0.9)).round() as usize).clamp(6, 24);
                        let pitch = length / count as f32;
                        let along = (pitch * 0.36).min(rail * 0.4);
                        let across = (rail * 0.16).max(1.);
                        let offset = rail * 0.55 + across / 2.;
                        for i in 0..count {
                            let t = start + pitch * (i as f32 + 0.5) - along / 2.;
                            if vertical {
                                a.rect(px - offset, t, across, along, 0, true, 0.3);
                                a.rect(px + pw + offset - across, t, across, along, 0, true, 0.3);
                            } else {
                                a.rect(t, py - offset, along, across, 0, true, 0.3);
                                a.rect(t, py + ph + offset - across, along, across, 0, true, 0.3);
                            }
                        }
                    }
                }
                if r.id == "film-negative" {
                    let mark = gutter * 0.55;
                    for (cx, cy) in [
                        (px - gutter, py - gutter),
                        (px + pw + gutter, py - gutter),
                        (px - gutter, py + ph + gutter),
                        (px + pw + gutter, py + ph + gutter),
                    ] {
                        a.line(&[[cx - mark, cy], [cx + mark, cy]], 0, 0.5, false, false);
                        a.line(&[[cx, cy - mark], [cx, cy + mark]], 0, 0.5, false, false);
                    }
                }
                if r.id == "film-paper" {
                    a.line(&[[w * 0.39, y], [w * 0.61, y]], 1, 0.55, false, false);
                    a.line(
                        &[[w * 0.46, y + band * 0.07], [w * 0.54, y + band * 0.07]],
                        0,
                        0.3,
                        false,
                        false,
                    );
                }
            }
        }
        "color-band" => {
            let lineh = (band * 0.085).max(1.);
            a.rect(pad, y - lineh, w * 0.28, lineh, 0, true, 0.3);
            a.rect(pad + w * 0.29, y - lineh, w * 0.065, lineh, 1, true, 0.3);
            a.rect(pad, y + lineh * 1.4, w * 0.17, lineh * 0.3, 2, true, 0.3);
            a.rect(w * 0.72, yt, w * 0.22, lineh * 0.45, 0, true, 0.3);
        }
        "corner-blocks" => {
            let s = band * 0.72;
            a.shape("circle", pad + s * 0.5, y, s, 0, 0.4, true);
            a.rect(
                pad + s * 0.65,
                y - s * 0.33,
                s * 0.38,
                s * 0.65,
                1,
                true,
                0.3,
            );
            a.rect(w - pad - s, yt - s * 0.32, s * 0.8, s * 0.3, 0, true, 0.3);
            a.rect(
                w - pad - s * 0.6,
                yt + s * 0.1,
                s * 0.6,
                s * 0.3,
                2,
                true,
                0.3,
            );
        }
        "pinstripe" => {
            let left = l.x as f32;
            let right = (l.x + l.photo_width) as f32;
            for i in 0..8 {
                let fx = left * (0.20 + i as f32 * 0.062);
                a.line(
                    &[[fx, h * 0.14], [fx, h * 0.86]],
                    if i % 3 == 0 { 1 } else { 0 },
                    0.3,
                    false,
                    false,
                );
            }
            for i in 0..3 {
                let fx = right + (w - right) * (0.40 + i as f32 * 0.12);
                a.line(&[[fx, h * 0.22], [fx, h * 0.78]], 0, 0.35, false, false);
            }
            a.line(&[[w * 0.21, y], [w * 0.43, y]], 1, 0.6, false, false);
        }
        "paper-tabs" => {
            let s = band * 0.75;
            a.line(
                &[
                    [pad, y - s * 0.4],
                    [pad + s, y - s * 0.2],
                    [pad + s * 0.8, y + s * 0.4],
                    [pad, y + s * 0.2],
                    [pad, y - s * 0.4],
                ],
                0,
                0.3,
                false,
                true,
            );
            a.line(
                &[
                    [w - pad - s, yt - s * 0.15],
                    [w - pad, yt - s * 0.4],
                    [w - pad, yt + s * 0.2],
                    [w - pad - s * 0.8, yt + s * 0.4],
                    [w - pad - s, yt - s * 0.15],
                ],
                1,
                0.3,
                false,
                true,
            );
            a.line(
                &[[pad + s * 1.2, y], [pad + s * 2., y]],
                2,
                0.7,
                false,
                false,
            );
        }
        "tidal" => {
            for i in 0..4 {
                let d = i as f32 * band * 0.12;
                a.curve(
                    &[
                        [pad, y + d - band * 0.23],
                        [w * 0.2, y + d - band * 0.65],
                        [w * 0.33, y + d + band * 0.32],
                        [w * 0.58, y + d - band * 0.1],
                        [w * 0.68, y + d - band * 0.3],
                        [w * 0.76, y + d - band * 0.1],
                        [w * 0.83, y + d - band * 0.4],
                    ],
                    if i == 3 { 1 } else { 0 },
                    if i == 0 { 1.4 } else { 0.65 },
                );
            }
            a.curve(
                &[
                    [w * 0.66, yt],
                    [w * 0.78, yt - band * 0.2],
                    [w * 0.85, yt + band * 0.24],
                    [w - pad, yt],
                ],
                2,
                0.9,
            );
        }
        "contour" => {
            for i in 0..7 {
                let d = i as f32 * band * 0.1;
                a.curve(
                    &[
                        [pad, yt + d * 0.42],
                        [w * 0.16, yt + d * 0.42 - band * 0.28],
                        [w * 0.21, yt - d * 0.08],
                        [w * 0.34, yt + d * 0.3],
                    ],
                    i % 2,
                    0.65,
                );
                a.curve(
                    &[
                        [w * 0.58, y - d * 0.35],
                        [w * 0.76, y + band * 0.3 - d * 0.45],
                        [w * 0.8, y - band * 0.25 + d * 0.1],
                        [w - pad, y - d * 0.3],
                    ],
                    i % 2,
                    0.65,
                );
            }
        }
        "botanical" => {
            a.branch(pad * 2., h - pad * 1.8, band * 1.05, false);
            a.branch(w - pad * 2., top * 0.85, band * 0.73, true);
            a.line(&[[w * 0.38, y], [w * 0.72, y]], 2, 0.65, false, false);
        }
        "ribbon" => {
            for (c, shift) in [(0, 0.), (1, band * 0.25)] {
                let yy = y - band * 0.14 + shift;
                a.curve(
                    &[
                        [pad, yy],
                        [w * 0.18, yy - band * 0.55],
                        [w * 0.29, yy + band * 0.36],
                        [w * 0.43, yy],
                    ],
                    c,
                    2.,
                );
                a.curve(
                    &[
                        [w * 0.46, yy],
                        [w * 0.62, yy - band * 0.45],
                        [w * 0.7, yy + band * 0.38],
                        [w * 0.87, yy - band * 0.12],
                    ],
                    c,
                    2.,
                );
                a.curve(
                    &[
                        [w * 0.65, yt + shift * 0.5],
                        [w * 0.74, yt - band * 0.2],
                        [w * 0.85, yt + band * 0.2],
                        [w - pad, yt],
                    ],
                    c,
                    0.9,
                );
            }
        }
        "lunar" => {
            a.line(&[[pad, yt], [w * 0.64, yt]], 1, 0.65, false, false);
            let s = size * 0.3;
            for i in 0..5 {
                let x = w * 0.65 + i as f32 * s * 1.75;
                a.ellipse(x, y, s * 0.48, s * 0.48, 0, 0.75);
                if i != 2 {
                    a.curve(
                        &[
                            [x, y - s * 0.48],
                            [x + (i as f32 - 2.) * s * 0.3, y - s * 0.28],
                            [x + (i as f32 - 2.) * s * 0.3, y + s * 0.28],
                            [x, y + s * 0.48],
                        ],
                        1,
                        0.8,
                    );
                }
            }
        }
        "camellia" => {
            let cx = pad + size * 0.55;
            let cy = y;
            let radius = size * 0.32;
            for i in 0..7 {
                let angle = i as f32 * std::f32::consts::TAU / 7.;
                let x = cx + angle.cos() * radius * 0.58;
                let yy = cy + angle.sin() * radius * 0.58;
                a.ellipse(x, yy, radius * 0.6, radius * 0.6, 0, 0.7);
            }
            a.shape("circle", cx, cy, radius * 0.4, 2, 0.8, true);
            a.branch(cx + radius * 0.7, cy + radius * 0.8, size * 0.74, false);
            a.curve(
                &[
                    [w * 0.66, yt],
                    [w * 0.79, yt + band * 0.12],
                    [w * 0.85, yt - band * 0.12],
                    [w - pad, yt],
                ],
                1,
                0.65,
            );
        }
        "deco" => {
            let x = pad * 1.1;
            let yy = pad * 1.1;
            for i in 0..5 {
                let s = band * (0.23 + i as f32 * 0.12);
                a.curve(
                    &[
                        [x, yy + s],
                        [x, yy + s * 0.46],
                        [x + s * 0.46, yy],
                        [x + s, yy],
                    ],
                    i % 2,
                    0.7,
                );
                a.curve(
                    &[
                        [w - x, h - yy - s],
                        [w - x, h - yy - s * 0.46],
                        [w - x - s * 0.46, h - yy],
                        [w - x - s, h - yy],
                    ],
                    i % 2,
                    0.7,
                );
            }
            a.line(
                &[[pad, top * 0.82], [pad, h - pad], [w * 0.5, h - pad]],
                0,
                0.65,
                false,
                false,
            );
            a.line(
                &[[w - pad, h - top * 0.82], [w - pad, pad], [w * 0.5, pad]],
                0,
                0.65,
                false,
                false,
            );
        }
        "orbit" => {
            let x = w * 0.75;
            for i in 0..3 {
                a.ellipse(
                    x,
                    y,
                    band * (0.72 - i as f32 * 0.12),
                    band * (0.12 + i as f32 * 0.09),
                    i,
                    0.65,
                );
            }
            a.shape(
                "sparkle",
                x + band * 0.64,
                y - band * 0.18,
                band * 0.19,
                1,
                0.8,
                false,
            );
            a.shape("sparkle", w * 0.16, yt, band * 0.26, 0, 0.7, false);
            a.curve(
                &[
                    [pad, yt],
                    [w * 0.23, yt - band * 0.16],
                    [w * 0.26, yt + band * 0.16],
                    [w * 0.39, yt],
                ],
                2,
                0.65,
            );
        }
        "cat" => {
            let x = w * 0.78;
            let s = size * 0.82;
            a.shape("cat", x, y, s, 0, 1.15, false);
            a.line(
                &[[w * 0.58, y + s * 0.45], [w - pad, y + s * 0.45]],
                1,
                0.7,
                false,
                false,
            );
            a.curve(
                &[
                    [x + s * 0.4, y + s * 0.33],
                    [x + s * 0.98, y + s * 0.4],
                    [x + s * 1.1, y - s * 0.38],
                    [x + s * 0.64, y - s * 0.2],
                ],
                0,
                1.1,
            );
            a.ellipse(w * 0.15, yt, band * 0.10, band * 0.16, 1, 0.7);
            a.ellipse(w * 0.15 + band * 0.2, yt, band * 0.10, band * 0.16, 2, 0.7);
        }
        "journal" => {
            a.opacity *= 0.32;
            let s = band * 0.65;
            a.line(
                &[[pad, pad], [pad + s * 1.3, pad], [pad, pad + s]],
                0,
                0.7,
                false,
                true,
            );
            a.line(
                &[
                    [w - pad, h - pad],
                    [w - pad - s * 1.4, h - pad],
                    [w - pad, h - pad - s],
                ],
                1,
                0.7,
                false,
                true,
            );
            a.opacity = (r.intensity * 0.8).min(1.);
            for i in 0..3 {
                let yy = y - band * 0.12 + i as f32 * band * 0.14;
                a.line(
                    &[[w * 0.58, yy], [w * 0.84 - i as f32 * w * 0.035, yy]],
                    2,
                    0.6,
                    false,
                    false,
                );
            }
            a.curve(
                &[
                    [w * 0.14, yt],
                    [w * 0.26, yt - band * 0.25],
                    [w * 0.3, yt + band * 0.27],
                    [w * 0.42, yt],
                ],
                0,
                0.8,
            );
        }
        "watercolor" => {
            a.line(&[[w * 0.43, y], [w * 0.82, y]], 1, 0.6, false, false);
            a.line(&[[w * 0.73, yt], [w - pad, yt]], 0, 0.6, false, false);
        }
        _ => {}
    }
    a.out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn film_edge_perforations_hug_the_photo_and_turn_for_portraits() {
        let holes = |w: u32, h: u32, ratio: Option<crate::model::Ratio>, border: f64| {
            let mut s = settings("film-edge").unwrap();
            s.ratio = ratio;
            s.border_percent = border;
            let l = crate::layout::calculate(w, h, &s).unwrap();
            let recipe = s.artwork.unwrap().design.unwrap();
            // Points are stored per mille of the canvas; turn them back into pixel boxes.
            let found: Vec<[f32; 4]> = elements(&recipe, l)
                .into_iter()
                .filter(|e| e.filled)
                .map(|e| {
                    let xs = e.points.iter().map(|p| p[0] * l.width as f32 / 1000.);
                    let ys = e.points.iter().map(|p| p[1] * l.height as f32 / 1000.);
                    let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max));
                    let (y0, y1) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
                    [x0, y0, x1 - x0, y1 - y0]
                })
                .collect();
            (l, found)
        };
        // A square canvas around a landscape photo: holes stay next to the photo.
        let (l, found) = holes(900, 600, Some(crate::model::Ratio { width: 1., height: 1. }), 18.);
        assert!(found.len() >= 12);
        for [_, y, width, height] in &found {
            let centre = y + height / 2.;
            let gap = (l.y as f32 - centre).max(centre - (l.y + l.photo_height) as f32);
            assert!(gap > 0. && gap < 600. * 0.14, "hole {gap}px from the photo");
            assert!(width > height, "landscape strip holes run horizontally");
        }
        // Portrait photos put the rails at the sides.
        let (l, found) = holes(600, 900, None, 18.);
        assert!(!found.is_empty());
        assert!(found.iter().all(|[x, _, width, _]| x + width <= l.x as f32 + 0.5 || *x >= (l.x + l.photo_width) as f32 - 0.5));
        assert!(found.iter().all(|[_, _, width, height]| height > width));
        // Too thin to read: no specks.
        assert!(holes(900, 600, None, 4.).1.is_empty());
    }
    #[test]
    fn all_compositions_reflow_and_keep_photos_intact_with_roi_parity() {
        for id in SPECS.iter().map(|s| s.0) {
            for (w, h) in [(480, 320), (320, 480), (400, 400)] {
                let s = settings(id).unwrap();
                let l = crate::layout::calculate(w, h, &s).unwrap();
                let base = image::Rgb([223, 225, 218]);
                let mut full = image::RgbImage::from_pixel(l.width, l.height, base);
                let art = s.artwork.as_ref().unwrap();
                crate::artwork::draw(&mut full, art, l).unwrap();
                assert!(full.pixels().any(|p| p != &base), "{id} must visibly draw");
                for y in l.y..l.y + l.photo_height {
                    for x in l.x..l.x + l.photo_width {
                        assert_eq!(*full.get_pixel(x, y), base, "{id} changed photo");
                    }
                }
                let ox = l.width / 3;
                let oy = l.height - l.y;
                let mut roi = image::RgbImage::from_pixel(l.width / 2, l.height - oy, base);
                crate::artwork::draw_window(&mut roi, art, l, ox, oy).unwrap();
                for (x, y, p) in roi.enumerate_pixels() {
                    assert_eq!(p, full.get_pixel(x + ox, y + oy), "{id} ROI diverged");
                }
                let zero = Layout {
                    width: w,
                    height: h,
                    x: 0,
                    y: 0,
                    photo_width: w,
                    photo_height: h,
                    scale: 1.,
                };
                let mut canvas = image::RgbImage::from_pixel(w, h, base);
                crate::artwork::draw(&mut canvas, art, zero).unwrap();
                assert!(canvas.pixels().all(|p| p == &base));
            }
        }
    }
    #[test]
    fn native_design_motifs_keep_equal_pixel_axes_in_portrait_and_landscape() {
        for (w, h) in [(480, 320), (320, 480)] {
            let s = settings("lunar").unwrap();
            let l = crate::layout::calculate(w, h, &s).unwrap();
            let e = elements(s.artwork.unwrap().design.as_ref().unwrap(), l);
            assert!(!e.is_empty());
            let mut a = Composer {
                l,
                colors: vec!["#000000".into(); 3],
                opacity: 1.,
                out: vec![],
            };
            a.shape("circle", 30., 30., 20., 0, 1., false);
            let circle = &a.out[0];
            assert!((circle.width * l.width as f32 - circle.height * l.height as f32).abs() < 0.01);
        }
    }
}
