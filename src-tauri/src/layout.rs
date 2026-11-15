use crate::model::RenderSettings;

pub const MAX_SOURCE_PIXELS: u64 = 100_000_000;
pub const MAX_CANVAS_PIXELS: u64 = 120_000_000;
pub const MAX_EDGE: u32 = 32_768;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub photo_width: u32,
    pub photo_height: u32,
    pub x: u32,
    pub y: u32,
    pub scale: f64,
}

pub fn check_source(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS
    {
        return Err("照片尺寸过大或无效（支持最长边 32768 像素、最多 1 亿像素）".into());
    }
    Ok(())
}

/// The border value is a minimum on each side. A fixed aspect ratio only adds space.
pub fn calculate(width: u32, height: u32, settings: &RenderSettings) -> Result<Layout, String> {
    check_source(width, height)?;
    let border = (f64::from(width.min(height)) * settings.border_percent / 100.0).round();
    let mut canvas_w = f64::from(width) + 2.0 * border;
    let mut canvas_h = f64::from(height) + 2.0 * border;
    if let Some(ratio) = &settings.ratio {
        let r = ratio.width / ratio.height;
        if canvas_w / canvas_h < r {
            canvas_w = (canvas_h * r).ceil();
        } else {
            canvas_h = (canvas_w / r).ceil();
        }
    }
    if !canvas_w.is_finite()
        || !canvas_h.is_finite()
        || canvas_w > f64::from(u32::MAX)
        || canvas_h > f64::from(u32::MAX)
    {
        return Err("最终比例导致画布过大，请调整比例或边框".into());
    }
    let scale = settings
        .export
        .long_edge
        .map(|edge| (f64::from(edge) / canvas_w.max(canvas_h)).min(1.0))
        .unwrap_or(1.0);
    let out_w = (canvas_w * scale).round().max(1.0) as u32;
    let out_h = (canvas_h * scale).round().max(1.0) as u32;
    if out_w > MAX_EDGE
        || out_h > MAX_EDGE
        || u64::from(out_w) * u64::from(out_h) > MAX_CANVAS_PIXELS
    {
        return Err("最终画布过大，请在高级导出中限制长边，或减少留白".into());
    }
    let photo_w = (f64::from(width) * scale).round().max(1.0) as u32;
    let photo_h = (f64::from(height) * scale).round().max(1.0) as u32;
    Ok(Layout {
        width: out_w,
        height: out_h,
        photo_width: photo_w.min(out_w),
        photo_height: photo_h.min(out_h),
        x: (out_w - photo_w.min(out_w)) / 2,
        y: (out_h - photo_h.min(out_h)) / 2,
        scale,
    })
}

pub fn scaled(layout: Layout, max_edge: u32) -> Layout {
    let scale = (f64::from(max_edge.max(1)) / f64::from(layout.width.max(layout.height))).min(1.0);
    Layout {
        width: (f64::from(layout.width) * scale).round().max(1.0) as u32,
        height: (f64::from(layout.height) * scale).round().max(1.0) as u32,
        photo_width: (f64::from(layout.photo_width) * scale).round().max(1.0) as u32,
        photo_height: (f64::from(layout.photo_height) * scale).round().max(1.0) as u32,
        x: (f64::from(layout.x) * scale).round() as u32,
        y: (f64::from(layout.y) * scale).round() as u32,
        scale: layout.scale * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Ratio;
    #[test]
    fn reference_dimensions_and_centering() {
        let mut s = RenderSettings::default();
        assert_eq!(
            (
                calculate(4000, 3000, &s).unwrap().width,
                calculate(4000, 3000, &s).unwrap().height
            ),
            (4300, 3300)
        );
        s.ratio = Some(Ratio {
            width: 1.0,
            height: 1.0,
        });
        let l = calculate(4000, 3000, &s).unwrap();
        assert_eq!((l.width, l.height, l.x, l.y), (4300, 4300, 150, 650));
        s.ratio = Some(Ratio {
            width: 4.0,
            height: 5.0,
        });
        let l = calculate(4000, 3000, &s).unwrap();
        assert_eq!((l.width, l.height, l.x, l.y), (4300, 5375, 150, 1187));
    }
    #[test]
    fn zero_border_fixed_ratio_keeps_photo_and_long_edge_only_shrinks() {
        let mut s = RenderSettings::default();
        s.border_percent = 0.0;
        s.ratio = Some(Ratio {
            width: 1.0,
            height: 1.0,
        });
        let l = calculate(3000, 4000, &s).unwrap();
        assert_eq!(
            (l.width, l.height, l.photo_width, l.photo_height),
            (4000, 4000, 3000, 4000)
        );
        s.export.long_edge = Some(2000);
        let l = calculate(3000, 4000, &s).unwrap();
        assert_eq!(
            (l.width, l.height, l.photo_width, l.photo_height),
            (2000, 2000, 1500, 2000)
        );
        s.export.long_edge = Some(8000);
        assert_eq!(calculate(3000, 4000, &s).unwrap().height, 4000);
    }
    #[test]
    fn extreme_ratio_and_sources_fail_before_allocation() {
        let mut s = RenderSettings::default();
        s.ratio = Some(Ratio {
            width: 1000000.0,
            height: 1.0,
        });
        assert!(calculate(4000, 3000, &s).is_err());
        assert!(calculate(0, 3000, &s).is_err());
        assert!(calculate(40000, 3000, &s).is_err());
    }
}
