use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RenderSettings {
    pub ratio: Option<Ratio>,
    pub border_percent: f64,
    pub frame_color: String,
    pub decoration: FrameDecoration,
    /// Plain frame colour or a blurred copy of the photo behind the frame.
    pub background: crate::backdrop::FrameBackground,
    pub artwork: Option<crate::artwork::ArtScene>,
    pub watermark: WatermarkSettings,
    pub export: ExportSettings,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            ratio: None,
            border_percent: 5.0,
            frame_color: "#ffffff".into(),
            decoration: FrameDecoration::default(),
            background: Default::default(),
            artwork: None,
            watermark: WatermarkSettings::default(),
            export: ExportSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ratio {
    pub width: f64,
    pub height: f64,
}

impl Default for Ratio {
    fn default() -> Self {
        Self {
            width: 1.0,
            height: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WatermarkSettings {
    pub kind: String,
    pub text: String,
    pub image_path: Option<String>,
    pub font_path: Option<String>,
    pub font_family: Option<String>,
    pub offset_x: f64,
    pub offset_y: f64,
    pub color: String,
    pub opacity: f64,
    pub size_percent: f64,
    pub region: String,
    pub position: String,
    pub margin_percent: f64,
    /// Template for the "camera" kind, filled per photo from EXIF.
    pub camera_template: String,
    /// Set while rendering a camera caption: border captions line up with the
    /// photo's edges rather than the canvas edge. Never stored.
    #[serde(skip)]
    pub align_to_photo: bool,
}

pub const DEFAULT_CAMERA_TEMPLATE: &str = "{camera} · {focal} · {aperture} · {shutter} · {iso}";

impl Default for WatermarkSettings {
    fn default() -> Self {
        Self {
            kind: "none".into(),
            text: String::new(),
            image_path: None,
            font_path: None,
            font_family: None,
            offset_x: 0.0,
            offset_y: 0.0,
            color: "#ffffff".into(),
            opacity: 0.85,
            size_percent: 3.0,
            region: "photo".into(),
            position: "bottom-right".into(),
            margin_percent: 2.0,
            camera_template: DEFAULT_CAMERA_TEMPLATE.into(),
            align_to_photo: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportSettings {
    pub format: String,
    pub jpeg_quality: u8,
    pub long_edge: Option<u32>,
    /// Output file name without extension. `{name}` is the original file name,
    /// `{index}` the photo's position in the export (001, 002, …).
    pub name_pattern: String,
    /// Copy camera, lens and exposure details into exported JPGs. Location,
    /// orientation and other metadata are never copied.
    pub keep_camera_info: bool,
}

pub const DEFAULT_NAME_PATTERN: &str = "{name}-framed";

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: "jpg".into(),
            jpeg_quality: 95,
            long_edge: None,
            name_pattern: DEFAULT_NAME_PATTERN.into(),
            keep_camera_info: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub data_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_thumbnail: Option<String>,
    pub width: u32,
    pub height: u32,
    pub warning: Option<String>,
    pub photo_bounds: PixelRect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontOption {
    pub family: String,
    pub path: String,
    pub chinese: bool,
    pub bundled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorSample {
    pub x: u32,
    pub y: u32,
    pub hex: String,
    pub patch_data_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_gain_zero_offsets_without_losing_the_signature() {
        let s: RenderSettings = serde_json::from_str(
            r#"{"watermark":{"kind":"text","text":"旧署名","fontPath":null}}"#,
        )
        .unwrap();
        assert_eq!(s.watermark.offset_x, 0.0);
        assert_eq!(s.watermark.offset_y, 0.0);
        assert_eq!(s.watermark.text, "旧署名");
        assert!(s.watermark.font_family.is_none());
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FrameDecoration {
    pub kind: String,
    pub color: String,
    pub opacity: f64,
    pub scale: f64,
}
impl Default for FrameDecoration {
    fn default() -> Self {
        Self {
            kind: "none".into(),
            color: "#75816F".into(),
            opacity: 0.12,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleImage {
    pub data_url: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsetLimits {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkDetail {
    pub data_url: String,
    pub view_width: u32,
    pub view_height: u32,
    pub watermark_bounds: PixelRect,
    pub position: PixelRect,
    pub width: u32,
    pub height: u32,
    pub limits: OffsetLimits,
}
