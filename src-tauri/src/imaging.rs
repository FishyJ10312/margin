use std::{
    fs,
    io::{BufWriter, Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use cosmic_text::{
    fontdb, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::{self, FilterType},
    DynamicImage, ImageDecoder, ImageEncoder, ImageReader, RgbImage, Rgba, RgbaImage,
};
use lcms2::{CIExyY, CIExyYTRIPLE, Intent, PixelFormat, Profile, ToneCurve, Transform};
use libheif_rs::{
    ColorPrimaries, ColorProfileNCLX, ColorSpace, HeifContext, LibHeif, RgbChroma,
    TransferCharacteristics,
};

use crate::{
    source_io,
    layout::{self, Layout},
    model::{
        ColorSample, FontOption, ImageInfo, PixelRect, PreviewResult, RenderSettings,
        WatermarkSettings,
    },
};

struct LoadedImage {
    pixels: RgbaImage,
    camera: crate::camera_info::CameraInfo,
    /// Camera fields to carry into exported JPGs when the user asks for it.
    exif_block: Option<Vec<u8>>,
    warning: Option<String>,
    proxy: OnceLock<RgbaImage>,
}
struct CachedImage {
    path: PathBuf,
    len: u64,
    modified: Option<u128>,
    image: Arc<LoadedImage>,
}
static CACHE: OnceLock<Mutex<Option<CachedImage>>> = OnceLock::new();
static RENDER_LOCK: Mutex<()> = Mutex::new(());
static FONT_DB: OnceLock<fontdb::Database> = OnceLock::new();
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
// Admission budget covers decoded pixels, resampling scratch and the output canvas.
// Only one native image operation runs at a time; this is not an OS RSS guarantee.
const WORKING_BUDGET: u64 = 1024 * 1024 * 1024;
const WATERMARK_RESERVE: u64 = 128 * 1024 * 1024;

#[cfg(test)]
#[path = "imaging_boundary_tests.rs"]
mod boundary_tests;

fn check_decode_budget(width: u32, height: u32, file_bytes: u64, heif: bool) -> Result<(), String> {
    let bytes = u64::from(width) * u64::from(height) * if heif { 20 } else { 16 } + file_bytes;
    if bytes + source_io::abandoned_buffer_bytes() > WORKING_BUDGET {
        return Err("照片解码预计占用过多内存，请先缩小照片后导入".into());
    }
    Ok(())
}

fn check_render_budget(source: (u32, u32), l: Layout, watermark: bool) -> Result<(), String> {
    let source_bytes = u64::from(source.0) * u64::from(source.1) * 4;
    let canvas = u64::from(l.width) * u64::from(l.height) * 3;
    let resizing = source != (l.photo_width, l.photo_height);
    let scratch = if resizing {
        u64::from(source.0) * u64::from(l.photo_height) * 16
            + source_bytes
            + u64::from(l.photo_width) * u64::from(l.photo_height) * 4
    } else {
        0
    };
    if source_bytes + canvas + scratch + source_io::abandoned_buffer_bytes() + if watermark { WATERMARK_RESERVE } else { 0 }
        > WORKING_BUDGET
    {
        return Err("预计处理内存过高，请在导出设置中限制长边，或减小边框与水印".into());
    }
    Ok(())
}

fn rgba_buffer(width: u32, height: u32) -> Result<RgbaImage, String> {
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("图片尺寸溢出")?;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| "内存不足，请缩小输出尺寸后重试")?;
    data.resize(len, 0);
    RgbaImage::from_raw(width, height, data).ok_or("无法建立图片缓冲区".into())
}

/// Camera details for one photo, read from its file without decoding pixels.
pub fn camera_info(path: &Path) -> Result<crate::camera_info::CameraInfo, String> {
    let input = source_io::read(path)?;
    Ok(crate::camera_info::read(&input.bytes))
}

pub fn clear_image_cache() {
    if let Some(cache) = CACHE.get() {
        if let Ok(mut cached) = cache.lock() {
            *cached = None;
        }
    }
}

fn failure(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}：{error}")
}

pub fn validate_settings(settings: &RenderSettings) -> Result<(), String> {
    if !settings.border_percent.is_finite() || !(0.0..=30.0).contains(&settings.border_percent) {
        return Err("边框比例应在 0%–30% 之间".into());
    }
    if let Some(ratio) = &settings.ratio {
        if !ratio.width.is_finite()
            || !ratio.height.is_finite()
            || ratio.width <= 0.0
            || ratio.height <= 0.0
        {
            return Err("最终比例的宽、高必须大于 0".into());
        }
    }
    parse_color(&settings.frame_color)?;
    crate::decoration::validate(&settings.decoration)?;
    crate::backdrop::validate(&settings.background)?;
    if !matches!(settings.export.format.as_str(), "jpg" | "png") {
        return Err("请选择 JPG 或 PNG 导出格式".into());
    }
    if !(1..=100).contains(&settings.export.jpeg_quality) {
        return Err("JPG 质量应在 1–100 之间".into());
    }
    if settings.export.name_pattern.trim().is_empty() {
        return Err("请填写导出文件名".into());
    }
    if settings.export.name_pattern.chars().count() > 120 {
        return Err("导出文件名最多 120 个字符".into());
    }
    if settings
        .export
        .long_edge
        .is_some_and(|n| n == 0 || n > layout::MAX_EDGE)
    {
        return Err("导出长边应在 1–32768 像素之间".into());
    }
    if let Some(art) = &settings.artwork {
        crate::artwork::validate(art)?;
    }
    let w = &settings.watermark;
    if !matches!(w.kind.as_str(), "none" | "text" | "image" | "camera") {
        return Err("水印类型无效".into());
    }
    if w.kind == "none" {
        return Ok(());
    }
    if !w.opacity.is_finite() || !(0.0..=1.0).contains(&w.opacity) {
        return Err("水印透明度应在 0%–100% 之间".into());
    }
    if !w.size_percent.is_finite() || !(0.1..=100.0).contains(&w.size_percent) {
        return Err("水印大小应在 0.1%–100% 之间".into());
    }
    if !w.offset_x.is_finite()
        || !w.offset_y.is_finite()
        || w.offset_x.abs() > 32768.0
        || w.offset_y.abs() > 32768.0
    {
        return Err("水印偏移应在 -32768 到 32768 像素之间".into());
    }
    if !w.margin_percent.is_finite() || !(0.0..=40.0).contains(&w.margin_percent) {
        return Err("水印边距应在 0%–40% 之间".into());
    }
    if !matches!(w.region.as_str(), "photo" | "border") {
        return Err("水印放置区域无效".into());
    }
    if !matches!(
        w.position.as_str(),
        "top-left"
            | "top-center"
            | "top-right"
            | "center-left"
            | "center"
            | "center-right"
            | "bottom-left"
            | "bottom-center"
            | "bottom-right"
    ) {
        return Err("水印位置无效".into());
    }
    if w.kind == "text" || w.kind == "camera" {
        let text = if w.kind == "camera" { &w.camera_template } else { &w.text };
        if text.trim().is_empty() {
            return Err(if w.kind == "camera" { "请填写参数水印的内容" } else { "请输入水印文字" }.into());
        }
        if text.chars().count() > 200 {
            return Err("水印文字最多 200 个字符".into());
        }
        parse_color(&w.color)?;
        if let Some(path) = &w.font_path {
            if !Path::new(path).is_file() {
                return Err("自定义字体已移动或不存在，请重新选择".into());
            }
        }
    }
    if w.kind == "image" {
        let path = w
            .image_path
            .as_ref()
            .filter(|p| !p.is_empty())
            .ok_or("请选择透明 PNG 水印文件")?;
        if !Path::new(path).is_file() {
            return Err("PNG 水印已移动或不存在，请重新选择".into());
        }
    }
    Ok(())
}

/// A camera-info watermark is ordinary text once this photo's EXIF fills its
/// template. Photos without usable EXIF render without the watermark.
const NO_CAMERA_INFO: &str = "这张照片没有拍摄参数，参数水印不会显示";
fn for_photo<'a>(settings: &'a RenderSettings, source: &LoadedImage) -> std::borrow::Cow<'a, RenderSettings> {
    if settings.watermark.kind != "camera" {
        return std::borrow::Cow::Borrowed(settings);
    }
    let mut resolved = settings.clone();
    let text: String = crate::camera_info::resolve(&settings.watermark.camera_template, &source.camera)
        .chars()
        .take(200)
        .collect();
    resolved.watermark.kind = if text.trim().is_empty() { "none" } else { "text" }.into();
    resolved.watermark.align_to_photo = true;
    resolved.watermark.text = text;
    std::borrow::Cow::Owned(resolved)
}

pub fn parse_color(value: &str) -> Result<[u8; 3], String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.is_ascii() {
        return Err("颜色应为六位 HEX，例如 #ffffff".into());
    }
    let mut rgb = [0; 3];
    for (i, channel) in rgb.iter_mut().enumerate() {
        *channel = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| "颜色应为六位 HEX，例如 #ffffff")?;
    }
    Ok(rgb)
}

fn file_check(path: &Path) -> Result<source_io::Metadata, String> {
    let meta = source_io::metadata(path)?;
    if !meta.is_file {
        return Err("请选择图片文件，不能直接处理目录".into());
    }
    if meta.len > MAX_FILE_BYTES {
        return Err("单张照片文件超过 128 MB，请先缩小文件".into());
    }
    Ok(meta)
}

fn heif_path(path: &Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "heic" | "heif" | "hif"))
}

fn png_color_profile(bytes: &[u8]) -> Result<Option<Profile>, String> {
    // image's decoder exposes iCCP but not cICP. Read bounded metadata through
    // the same pinned PNG codec before allowing a silent HDR/wide-gamut relabel.
    // PNG 3 section 11.3.2 gives cICP precedence over iCCP and sRGB.
    let decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: 8 * 1024 * 1024,
        },
    );
    let reader = decoder
        .read_info()
        .map_err(|e| failure("PNG 颜色信息无效或过大", e))?;
    let Some(cicp) = reader.info().coding_independent_code_points else {
        let info = reader.info();
        if info.icc_profile.is_some() {
            return Ok(None);
        }
        if info.srgb.is_some() {
            return Ok(Some(Profile::new_srgb()));
        }
        if info.gama_chunk.is_none() && info.chrm_chunk.is_none() {
            return Ok(None);
        }
        let curve = if let Some(gamma) = info.gama_chunk {
            let value = f64::from(gamma.into_scaled()) / 100_000.0;
            if !(0.0001..=10.0).contains(&value) {
                return Err("这张 PNG 的颜色曲线无效，请先转换为 SDR sRGB".into());
            }
            ToneCurve::new(1.0 / value)
        } else {
            srgb_curve()
        };
        let chroma = info.chrm_chunk.unwrap_or_else(|| {
            png::SourceChromaticities::new(
                (0.3127, 0.3290),
                (0.64, 0.33),
                (0.30, 0.60),
                (0.15, 0.06),
            )
        });
        let xy = |v: (png::ScaledFloat, png::ScaledFloat)| CIExyY {
            x: f64::from(v.0.into_scaled()) / 100_000.0,
            y: f64::from(v.1.into_scaled()) / 100_000.0,
            Y: 1.0,
        };
        return Profile::new_rgb(
            &xy(chroma.white),
            &CIExyYTRIPLE {
                Red: xy(chroma.red),
                Green: xy(chroma.green),
                Blue: xy(chroma.blue),
            },
            &[&curve, &curve, &curve],
        )
        .map(Some)
        .map_err(|e| failure("PNG 颜色转换失败", e));
    };
    if matches!(cicp.transfer_function, 16 | 18) {
        return Err("这张 PNG 使用 HDR PQ/HLG，暂不能可靠转换；请先导出为 SDR JPG 或 PNG".into());
    }
    if cicp.transfer_function != 13
        || cicp.matrix_coefficients != 0
        || !cicp.is_video_full_range_image
        || !matches!(cicp.color_primaries, 1 | 12)
    {
        return Err("这张 PNG 的 cICP 颜色空间暂不支持，请先转换为 SDR sRGB".into());
    }
    if cicp.color_primaries == 1 {
        return Ok(Some(Profile::new_srgb()));
    }
    let curve = srgb_curve();
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    Profile::new_rgb(
        &xy(0.3127, 0.3290),
        &CIExyYTRIPLE {
            Red: xy(0.68, 0.32),
            Green: xy(0.265, 0.69),
            Blue: xy(0.15, 0.06),
        },
        &[&curve, &curve, &curve],
    )
    .map(Some)
    .map_err(|e| failure("PNG Display P3 转换失败", e))
}

fn srgb_curve() -> ToneCurve {
    let values: Vec<u16> = (0..4096)
        .map(|n| {
            let x = f64::from(n) / 4095.0;
            let linear = if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            };
            (linear * 65535.0).round() as u16
        })
        .collect();
    ToneCurve::new_tabulated(&values)
}

pub fn inspect_image(path: &Path) -> Result<ImageInfo, String> {
    let input = source_io::read(path)?;
    inspect_bytes(path, &input.bytes)
}

fn inspect_bytes(path: &Path, bytes: &[u8]) -> Result<ImageInfo, String> {
    if heif_path(path) {
        let _lib = LibHeif::new();
        let ctx = HeifContext::read_from_bytes(&bytes)
            .map_err(|e| failure("HEIC 文件无效或已损坏", e))?;
        let handle = ctx
            .primary_image_handle()
            .map_err(|e| failure("HEIC 没有可读取的主图", e))?;
        let info = ImageInfo {
            width: handle.width(),
            height: handle.height(),
        };
        layout::check_source(info.width, info.height)?;
        check_decode_budget(
            info.width,
            info.height,
            bytes.len() as u64,
            true,
        )?;
        return Ok(info);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| failure("无法识别图片格式", e))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
    ) {
        return Err("支持 JPG、PNG、HEIC 和 HEIF 照片".into());
    }
    if reader.format() == Some(image::ImageFormat::Png) {
        png_color_profile(bytes)?;
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| failure("图片无效或已损坏", e))?;
    let (mut width, mut height) = decoder.dimensions();
    layout::check_source(width, height)?;
    check_decode_budget(
        width,
        height,
        bytes.len() as u64,
        heif_path(path),
    )?;
    let orientation = decoder
        .orientation()
        .map_err(|e| failure("无法读取照片方向", e))?;
    use image::metadata::Orientation;
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        std::mem::swap(&mut width, &mut height);
    }
    Ok(ImageInfo { width, height })
}

fn loaded(path: &Path) -> Result<Arc<LoadedImage>, String> {
    let meta = file_check(path)?;
    let canonical = &meta.canonical;
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "图片缓存不可用，请重试")?;
    let modified = meta.modified;
    if let Some(entry) = cache.as_ref() {
        if entry.path == *canonical && entry.len == meta.len && entry.modified == modified {
            return Ok(entry.image.clone());
        }
    }
    // Release the previous full-resolution image before decoding the next file.
    *cache = None;
    drop(cache);
    let input = source_io::read(path)?;
    let decoded = if heif_path(path) {
        decode_heif(&input.bytes)?
    } else {
        decode_standard_bytes(path, &input.bytes)?
    };
    let mut decoded = decoded;
    decoded.camera = crate::camera_info::read(&input.bytes);
    decoded.exif_block = crate::camera_info::portable_exif(&input.bytes);
    let image = Arc::new(decoded);
    let mut cache = CACHE
        .get()
        .ok_or("图片缓存未初始化")?
        .lock()
        .map_err(|_| "图片缓存不可用")?;
    *cache = Some(CachedImage {
        path: input.metadata.canonical,
        len: input.metadata.len,
        modified: input.metadata.modified,
        image: image.clone(),
    });
    Ok(image)
}

#[cfg(test)]
fn decode_standard(path: &Path) -> Result<LoadedImage, String> {
    let input = source_io::read(path)?;
    decode_standard_bytes(path, &input.bytes)
}

fn decode_standard_bytes(path: &Path, bytes: &[u8]) -> Result<LoadedImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| failure("无法识别图片格式", e))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
    ) {
        return Err("支持 JPG、PNG、HEIC 和 HEIF 照片".into());
    }
    let png_profile = if reader.format() == Some(image::ImageFormat::Png) {
        png_color_profile(bytes)?
    } else {
        None
    };
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| failure("图片无效或已损坏", e))?;
    let (width, height) = decoder.dimensions();
    layout::check_source(width, height)?;
    check_decode_budget(
        width,
        height,
        bytes.len() as u64,
        heif_path(path),
    )?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512 * 1024 * 1024);
    decoder
        .set_limits(limits)
        .map_err(|e| failure("照片占用内存过大", e))?;
    let icc = if png_profile.is_some() {
        None
    } else {
        decoder
            .icc_profile()
            .map_err(|e| failure("无法读取颜色配置", e))?
    };
    let orientation = decoder
        .orientation()
        .map_err(|e| failure("无法读取照片方向", e))?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| failure("图片解码失败", e))?;
    image.apply_orientation(orientation);
    let mut pixels = image.into_rgba8();
    if let Some(profile) = png_profile {
        convert_profile(&mut pixels, &profile)?;
    } else if let Some(icc) = icc.filter(|p| !p.is_empty()) {
        let profile = Profile::new_icc(&icc).map_err(|e| failure("照片颜色配置无效", e))?;
        convert_profile(&mut pixels, &profile)?;
    }
    Ok(LoadedImage {
        camera: Default::default(),
        exif_block: None,
        pixels,
        proxy: OnceLock::new(),
        warning: None,
    })
}

fn decode_heif(bytes: &[u8]) -> Result<LoadedImage, String> {
    let lib = LibHeif::new();
    let ctx =
        HeifContext::read_from_bytes(&bytes).map_err(|e| failure("HEIC 文件无效或已损坏", e))?;
    let handle = ctx
        .primary_image_handle()
        .map_err(|e| failure("HEIC 没有可读取的主图", e))?;
    layout::check_source(handle.width(), handle.height())?;
    check_decode_budget(handle.width(), handle.height(), bytes.len() as u64, true)?;
    let nclx = handle.color_profile_nclx();
    if nclx.as_ref().is_some_and(|p| {
        matches!(
            p.transfer_characteristics(),
            TransferCharacteristics::ITU_R_BT_2100_0_PQ
                | TransferCharacteristics::ITU_R_BT_2100_0_HLG
        )
    }) {
        return Err(
            "这张 HEIC 的主图使用 HDR PQ/HLG，首版无法可靠转换；请先导出为 SDR JPG 后添加边框"
                .into(),
        );
    }
    // libheif applies irot/imir and clean-aperture transformations by default.
    // Reapplying the EXIF orientation here would rotate iPhone photos twice.
    let decoded = lib
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), None)
        .map_err(|e| failure("HEIC 解码失败", e))?;
    let actual_nclx = decoded.color_profile_nclx();
    let color = nclx.as_ref().or(actual_nclx.as_ref());
    if color.is_some_and(|p| {
        matches!(
            p.transfer_characteristics(),
            TransferCharacteristics::ITU_R_BT_2100_0_PQ
                | TransferCharacteristics::ITU_R_BT_2100_0_HLG
        )
    }) {
        return Err("这张 HEIC 的主图使用 HDR PQ/HLG，请先转换为 SDR JPG".into());
    }
    let plane = decoded
        .planes()
        .interleaved
        .ok_or("HEIC 没有可读取的像素")?;
    let width = plane.width;
    let height = plane.height;
    layout::check_source(width, height)?;
    check_decode_budget(
        width,
        height,
        bytes.len() as u64,
        true,
    )?;
    let row_len = width as usize * 4;
    if plane.stride < row_len {
        return Err("HEIC 像素数据不完整".into());
    }
    let mut pixels = rgba_buffer(width, height)?;
    for (y, row) in pixels.as_mut().chunks_exact_mut(row_len).enumerate() {
        let start = y * plane.stride;
        let source = plane
            .data
            .get(start..start + row_len)
            .ok_or("HEIC 像素数据不完整")?;
        row.copy_from_slice(source);
    }
    if let Some(raw) = handle
        .color_profile_raw()
        .or_else(|| decoded.color_profile_raw())
    {
        let profile = Profile::new_icc(&raw.data).map_err(|e| failure("HEIC 颜色配置无效", e))?;
        convert_profile(&mut pixels, &profile)?;
    } else if let Some(color) = color {
        if handle.luma_bits_per_pixel() > 8
            && (matches!(
                color.color_primaries(),
                ColorPrimaries::Unknown | ColorPrimaries::Unspecified
            ) || matches!(
                color.transfer_characteristics(),
                TransferCharacteristics::Unknown | TransferCharacteristics::Unspecified
            ))
        {
            return Err("这张高位深 HEIC 缺少颜色信息，无法保证 SDR 颜色，请先转为 JPG".into());
        }
        let profile = nclx_profile(color)?;
        convert_profile(&mut pixels, &profile)?;
    } else if handle.luma_bits_per_pixel() > 8 {
        return Err("这张高位深 HEIC 缺少颜色信息，无法保证 SDR 颜色，请先转为 JPG".into());
    }
    Ok(LoadedImage {
        camera: Default::default(),
        exif_block: None,
        pixels,
        proxy: OnceLock::new(),
        warning: Some("HEIC 已读取静态主图，导出为 SDR sRGB".into()),
    })
}

fn nclx_profile(p: &ColorProfileNCLX) -> Result<Profile, String> {
    use TransferCharacteristics::*;
    let transfer = p.transfer_characteristics();
    if matches!(
        transfer,
        Unknown
            | Logarithmic100
            | Logarithmic100Sqrt10
            | SMPTE_ST_428_1
            | ITU_R_BT_2100_0_PQ
            | ITU_R_BT_2100_0_HLG
    ) {
        return Err("这张 HEIC 的颜色转换暂不支持，请先导出为 SDR JPG".into());
    }
    let primaries = p.color_primaries();
    if matches!(
        primaries,
        ColorPrimaries::Unknown | ColorPrimaries::Unspecified
    ) {
        if matches!(transfer, Unspecified | IEC_61966_2_1 | ITU_R_BT_709_5) {
            return Ok(Profile::new_srgb());
        }
        return Err("HEIC 缺少可靠的色域信息，请先导出为 SDR JPG".into());
    }
    let values: Vec<u16> = (0..4096)
        .map(|n| {
            let x = f64::from(n) / 4095.0;
            let linear = match transfer {
                IEC_61966_2_1 | Unspecified => {
                    if x <= 0.04045 {
                        x / 12.92
                    } else {
                        ((x + 0.055) / 1.055).powf(2.4)
                    }
                }
                Linear => x,
                ITU_R_BT_470_6_System_M => x.powf(2.2),
                ITU_R_BT_470_6_System_B_G => x.powf(2.8),
                _ => {
                    if x < 0.081 {
                        x / 4.5
                    } else {
                        ((x + 0.099) / 1.099).powf(1.0 / 0.45)
                    }
                }
            };
            (linear.clamp(0.0, 1.0) * 65535.0).round() as u16
        })
        .collect();
    let curve = ToneCurve::new_tabulated(&values);
    let xy = |x: f32, y: f32| CIExyY {
        x: f64::from(x),
        y: f64::from(y),
        Y: 1.0,
    };
    let white = xy(p.color_primary_white_x(), p.color_primary_white_y());
    let rgb = CIExyYTRIPLE {
        Red: xy(p.color_primary_red_x(), p.color_primary_red_y()),
        Green: xy(p.color_primary_green_x(), p.color_primary_green_y()),
        Blue: xy(p.color_primary_blue_x(), p.color_primary_blue_y()),
    };
    Profile::new_rgb(&white, &rgb, &[&curve, &curve, &curve])
        .map_err(|e| failure("HEIC 色域转换失败", e))
}

fn convert_profile(pixels: &mut RgbaImage, input: &Profile) -> Result<(), String> {
    let output = Profile::new_srgb();
    // Convert one row at a time, retaining alpha explicitly and bounding scratch memory.
    if input.color_space() == lcms2::ColorSpaceSignature::GrayData {
        let transform: Transform<u8, [u8; 3]> = Transform::new(
            input,
            PixelFormat::GRAY_8,
            &output,
            PixelFormat::RGB_8,
            Intent::RelativeColorimetric,
        )
        .map_err(|e| failure("灰度颜色转换失败", e))?;
        for row in pixels.rows_mut() {
            let mut samples: Vec<_> = row.collect();
            let gray: Vec<u8> = samples.iter().map(|p| p[0]).collect();
            let mut rgb = vec![[0; 3]; gray.len()];
            transform.transform_pixels(&gray, &mut rgb);
            for (pixel, color) in samples.iter_mut().zip(rgb) {
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
            }
        }
    } else {
        let transform: Transform<[u8; 3], [u8; 3]> = Transform::new(
            input,
            PixelFormat::RGB_8,
            &output,
            PixelFormat::RGB_8,
            Intent::RelativeColorimetric,
        )
        .map_err(|e| failure("颜色转换失败（这张照片可能不是 RGB）", e))?;
        for row in pixels.rows_mut() {
            let mut samples: Vec<_> = row.collect();
            let mut rgb: Vec<[u8; 3]> = samples.iter().map(|p| [p[0], p[1], p[2]]).collect();
            transform.transform_in_place(&mut rgb);
            for (pixel, color) in samples.iter_mut().zip(rgb) {
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
            }
        }
    }
    Ok(())
}

fn thumbnail_fit(source: &RgbaImage, max_edge: u32) -> RgbaImage {
    let scale =
        (f64::from(max_edge.max(1)) / f64::from(source.width().max(source.height()))).min(1.0);
    let width = (f64::from(source.width()) * scale).round().max(1.0) as u32;
    let height = (f64::from(source.height()) * scale).round().max(1.0) as u32;
    imageops::thumbnail(source, width, height)
}

pub fn thumbnail(path: &Path, max_edge: u32) -> Result<String, String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let small = thumbnail_fit(&source.pixels, max_edge.clamp(1, 512));
    jpeg_data_url(&small, 80)
}

pub fn preview(
    path: &Path,
    settings: &RenderSettings,
    max_edge: u32,
) -> Result<PreviewResult, String> {
    validate_settings(settings)?;
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let requested = settings;
    let settings = &*for_photo(requested, &source);
    let full = layout::calculate(source.pixels.width(), source.pixels.height(), settings)?;
    let small = layout::scaled(full, max_edge.clamp(128, 2048));
    check_render_budget(
        source.pixels.dimensions(),
        small,
        settings.watermark.kind != "none",
    )?;
    let proxy = source.proxy.get_or_init(|| {
        // Area downsampling avoids a full floating-point Lanczos intermediate for
        // high-resolution opaque photos. Full export keeps the quality resampler.
        if source.pixels.pixels().all(|p| p[3] == 255) {
            return thumbnail_fit(&source.pixels, 2048);
        }
        let factor =
            (2048.0 / f64::from(source.pixels.width().max(source.pixels.height()))).min(1.0);
        resize_alpha(
            &source.pixels,
            (f64::from(source.pixels.width()) * factor).round().max(1.0) as u32,
            (f64::from(source.pixels.height()) * factor)
                .round()
                .max(1.0) as u32,
        )
    });
    // Validate watermarks against the full-resolution layout before scaling the preview.
    let mark = watermark_bitmap(&settings.watermark, full)?;
    let placement = mark
        .as_ref()
        .map(|m| watermark_position(full, &settings.watermark, m.width(), m.height()))
        .transpose()?;
    let result = compose(proxy, settings, small, mark.as_ref(), placement, full)?;
    Ok(PreviewResult {
        data_url: rgb_data_url(&result, 90)?,
        source_thumbnail: None,
        width: full.width,
        height: full.height,
        warning: source.warning.clone().or_else(|| {
            (requested.watermark.kind == "camera" && settings.watermark.kind == "none")
                .then(|| NO_CAMERA_INFO.to_string())
        }),
        photo_bounds: PixelRect {
            x: full.x,
            y: full.y,
            width: full.photo_width,
            height: full.photo_height,
        },
    })
}

pub fn export_image(
    source: &Path,
    settings: &RenderSettings,
    destination: &Path,
) -> Result<(u32, u32), String> {
    validate_settings(settings)?;
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(source)?;
    let settings = &*for_photo(settings, &source);
    let full = layout::calculate(source.pixels.width(), source.pixels.height(), settings)?;
    check_render_budget(
        source.pixels.dimensions(),
        full,
        settings.watermark.kind != "none",
    )?;
    let mark = watermark_bitmap(&settings.watermark, full)?;
    let placement = mark
        .as_ref()
        .map(|m| watermark_position(full, &settings.watermark, m.width(), m.height()))
        .transpose()?;
    let result = compose(
        &source.pixels,
        settings,
        full,
        mark.as_ref(),
        placement,
        full,
    )?;
    let file = fs::File::create(destination).map_err(|e| failure("无法写入导出文件", e))?;
    let mut writer = BufWriter::new(file);
    match source
        .exif_block
        .as_deref()
        .filter(|_| settings.export.keep_camera_info && settings.export.format == "jpg")
    {
        Some(block) => {
            let mut jpeg = Vec::new();
            encode_rgb_image(&result, settings, &mut jpeg)?;
            let jpeg = crate::camera_info::insert_into_jpeg(&jpeg, block).unwrap_or(jpeg);
            writer
                .write_all(&jpeg)
                .map_err(|e| failure("导出文件未写入完整，请检查磁盘空间", e))?;
        }
        None => encode_rgb_image(&result, settings, &mut writer)?,
    }
    writer
        .flush()
        .map_err(|e| failure("导出文件未写入完整，请检查磁盘空间", e))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|e| failure("无法保存导出文件，请检查磁盘空间", e))?;
    Ok((full.width, full.height))
}

fn encode_rgb_image(
    result: &RgbImage,
    settings: &RenderSettings,
    writer: &mut impl Write,
) -> Result<(), String> {
    let icc = Profile::new_srgb()
        .icc()
        .map_err(|e| failure("无法生成 sRGB 配置", e))?;
    if settings.export.format == "png" {
        let mut encoder = PngEncoder::new(&mut *writer);
        encoder
            .set_icc_profile(icc)
            .map_err(|e| failure("无法写入 PNG 颜色配置", e))?;
        encoder
            .write_image(
                result.as_raw(),
                result.width(),
                result.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| failure("PNG 导出失败", e))?;
    } else {
        let rgb = result;
        let mut encoder = JpegEncoder::new_with_quality(&mut *writer, settings.export.jpeg_quality);
        encoder
            .set_icc_profile(icc)
            .map_err(|e| failure("无法写入 JPG 颜色配置", e))?;
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| failure("JPG 导出失败", e))?;
    }
    Ok(())
}

fn blend_rgb(canvas: &mut RgbImage, source: &RgbaImage, x: u32, y: u32) {
    for sy in 0..source.height().min(canvas.height().saturating_sub(y)) {
        for sx in 0..source.width().min(canvas.width().saturating_sub(x)) {
            let p = source.get_pixel(sx, sy);
            let target = canvas.get_pixel_mut(x + sx, y + sy);
            let alpha = u32::from(p[3]);
            for c in 0..3 {
                target[c] = ((u32::from(p[c]) * alpha + u32::from(target[c]) * (255 - alpha) + 127)
                    / 255) as u8;
            }
        }
    }
}

fn compose(
    source: &RgbaImage,
    settings: &RenderSettings,
    layout: Layout,
    mark: Option<&RgbaImage>,
    placement: Option<(u32, u32)>,
    full: Layout,
) -> Result<RgbImage, String> {
    let c = parse_color(&settings.frame_color)?;
    let len = layout.width as usize * layout.height as usize * 3;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| "内存不足，请限制导出长边后重试")?;
    data.resize(len, 0);
    for pixel in data.chunks_exact_mut(3) {
        pixel.copy_from_slice(&c);
    }
    let mut canvas = RgbImage::from_raw(layout.width, layout.height, data).ok_or("画布尺寸无效")?;
    if let Some(backdrop) = crate::backdrop::prepare(source, &settings.background, c, layout.width, layout.height) {
        crate::backdrop::paint(&mut canvas, &backdrop, 0, 0, layout.width, layout.height);
    }
    if source.dimensions() == (layout.photo_width, layout.photo_height) {
        blend_rgb(&mut canvas, source, layout.x, layout.y);
    } else {
        let resized = resize_alpha(source, layout.photo_width, layout.photo_height);
        blend_rgb(&mut canvas, &resized, layout.x, layout.y);
    }
    crate::decoration::draw(&mut canvas, &settings.decoration, layout, full)?;
    if let Some(art) = &settings.artwork {
        crate::artwork::draw(&mut canvas, art, layout)?;
    }
    if let (Some(mark), Some((x, y))) = (mark, placement) {
        let factor = f64::from(layout.width) / f64::from(full.width);
        let width = (f64::from(mark.width()) * factor).round().max(1.0) as u32;
        let height = (f64::from(mark.height()) * factor).round().max(1.0) as u32;
        let x = (f64::from(x) * factor).round() as u32;
        let y = (f64::from(y) * factor).round() as u32;
        if (width, height) == mark.dimensions() {
            blend_rgb(&mut canvas, mark, x, y);
        } else {
            let small = resize_alpha(mark, width, height);
            blend_rgb(&mut canvas, &small, x, y);
        }
    }
    Ok(canvas)
}

fn resize_alpha(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if source.pixels().all(|pixel| pixel[3] == 255) {
        return imageops::resize(source, width.max(1), height.max(1), FilterType::Lanczos3);
    }
    // Premultiplication keeps transparent PNG edges from developing dark fringes.
    let mut premult = source.clone();
    for p in premult.pixels_mut() {
        let a = u16::from(p[3]);
        for c in 0..3 {
            p[c] = ((u16::from(p[c]) * a + 127) / 255) as u8;
        }
    }
    let mut resized = imageops::resize(&premult, width.max(1), height.max(1), FilterType::Lanczos3);
    for p in resized.pixels_mut() {
        let a = u16::from(p[3]);
        if a > 0 {
            for c in 0..3 {
                p[c] = ((u16::from(p[c]) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    resized
}

fn watermark_bitmap(w: &WatermarkSettings, layout: Layout) -> Result<Option<RgbaImage>, String> {
    if w.kind == "none" {
        return Ok(None);
    }
    let short = f64::from(layout.photo_width.min(layout.photo_height));
    let mut mark = if w.kind == "image" {
        let path = Path::new(w.image_path.as_ref().ok_or("请选择 PNG 水印")?);
        let input = source_io::read(path)?;
        let format = ImageReader::new(Cursor::new(&input.bytes))
            .with_guessed_format()
            .map_err(|e| failure("无法读取 PNG 水印", e))?;
        if format.format() != Some(image::ImageFormat::Png) {
            return Err("图片水印需要透明 PNG 文件".into());
        }
        let dimensions = inspect_bytes(path, &input.bytes)?;
        if u64::from(dimensions.width) * u64::from(dimensions.height) > 16_000_000 {
            return Err("PNG 水印过大，请使用不超过 1600 万像素的 Logo".into());
        }
        let decoded = decode_standard_bytes(path, &input.bytes)?.pixels;
        let width = (short * w.size_percent / 100.0).round().max(1.0) as u32;
        let height = (f64::from(decoded.height()) * f64::from(width) / f64::from(decoded.width()))
            .round()
            .max(1.0) as u32;
        if u64::from(width) * u64::from(height) > 16_000_000 {
            return Err("PNG 水印尺寸过大，请减小水印比例".into());
        }
        resize_alpha(&decoded, width, height)
    } else {
        text_bitmap(w, (short * w.size_percent / 100.0).max(1.0) as f32)?
    };
    for pixel in mark.pixels_mut() {
        pixel[3] = (f64::from(pixel[3]) * w.opacity).round() as u8;
    }
    Ok(Some(mark))
}

fn text_bitmap(w: &WatermarkSettings, font_size: f32) -> Result<RgbaImage, String> {
    static CACHE: OnceLock<Mutex<Vec<(String, RgbaImage)>>> = OnceLock::new();
    let stamp = w
        .font_path
        .as_ref()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| (m.len(), m.modified().ok()));
    let key = format!(
        "{:?}|{:?}|{:?}|{}|{}|{:?}",
        w.font_path, w.font_family, w.text, w.color, font_size, stamp
    );
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Some((_, m)) = cache
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|(k, _)| *k == key)
    {
        return Ok(m.clone());
    }
    let mark = text_bitmap_uncached(w, font_size)?;
    // Limit retained rasters to 16 MiB in total, even with long signatures.
    if mark.as_raw().len() <= 8 * 1024 * 1024 {
        let mut c = cache.lock().unwrap_or_else(|p| p.into_inner());
        while c.len() >= 3
            || c.iter().map(|(_, m)| m.as_raw().len()).sum::<usize>() + mark.as_raw().len()
                > 16 * 1024 * 1024
        {
            c.remove(0);
        }
        c.push((key, mark.clone()));
    }
    Ok(mark)
}
fn text_bitmap_uncached(w: &WatermarkSettings, font_size: f32) -> Result<RgbaImage, String> {
    if font_size > 4096.0 {
        return Err("水印字号过大，请减小水印比例".into());
    }
    let mut db = bundled_font_db().clone();
    let default_family = db
        .faces()
        .next()
        .and_then(|f| f.families.first())
        .map(|(n, _)| n.clone())
        .ok_or("内置字体无法加载")?;
    let mut custom_family = None;
    if let Some(path) = &w.font_path {
        if fs::metadata(path)
            .map_err(|e| failure("无法读取自定义字体", e))?
            .len()
            > 64 * 1024 * 1024
        {
            return Err("字体文件过大，请选择小于 64 MB 的 TTF 或 OTF".into());
        }
        let before: Vec<_> = db.faces().map(|f| f.id).collect();
        db.load_font_file(path)
            .map_err(|e| failure("无法读取字体", e))?;
        let ids: Vec<_> = db
            .faces()
            .filter(|f| !before.contains(&f.id))
            .map(|f| f.id)
            .collect();
        custom_family = ids
            .iter()
            .find(|id| {
                db.face(**id).is_some_and(|f| {
                    w.font_family
                        .as_ref()
                        .is_some_and(|name| f.families.iter().any(|(n, _)| n == name))
                })
            })
            .or_else(|| ids.first())
            .and_then(|id| db.face(*id))
            .and_then(|f| f.families.first())
            .map(|(name, _)| name.clone());
        if custom_family.is_none() {
            return Err("这个字体文件无效，请选择 TTF 或 OTF".into());
        }
    }
    let builtin_family = w.font_family.as_deref().filter(|name| {
        db.faces()
            .any(|f| f.families.iter().any(|(n, _)| n == name))
    });
    let family = custom_family
        .as_deref()
        .or(builtin_family)
        .unwrap_or(&default_family);
    let mut fonts = FontSystem::new_with_locale_and_db("zh-CN".into(), db);
    let mut buffer = Buffer::new(&mut fonts, Metrics::new(font_size, font_size * 1.35));
    let rgb = parse_color(&w.color)?;
    let color = Color::rgb(rgb[0], rgb[1], rgb[2]);
    let mut cache = SwashCache::new();
    let mut borrowed = buffer.borrow_with(&mut fonts);
    borrowed.set_size(None, None);
    borrowed.set_wrap(Wrap::None);
    borrowed.set_text(
        &w.text,
        &Attrs::new().family(Family::Name(family)),
        Shaping::Advanced,
        None,
    );
    borrowed.shape_until_scroll(true);
    let max_width = borrowed
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0.0_f32, f32::max);
    let total_height = borrowed.layout_runs().count() as f32 * font_size * 1.35;
    if max_width > layout::MAX_EDGE as f32 || max_width * total_height > 16_000_000.0 {
        return Err("水印文字过大或过长，请减小字号或缩短文字".into());
    }
    let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    borrowed.draw(&mut cache, color, |x, y, width, height, c| {
        if c.a() > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + width as i32);
            bottom = bottom.max(y + height as i32);
        }
    });
    if left == i32::MAX {
        return Err("水印文字没有可显示的字形，请更换文字或字体".into());
    }
    let width = (right - left + 2) as u32;
    let height = (bottom - top + 2) as u32;
    if u64::from(width) * u64::from(height) > 16_000_000 || width > layout::MAX_EDGE {
        return Err("水印文字过大或过长，请减小字号或缩短文字".into());
    }
    let mut image = rgba_buffer(width, height)?;
    borrowed.draw(&mut cache, color, |x, y, w, h, c| {
        for py in 0..h {
            for px in 0..w {
                let dx = x - left + 1 + px as i32;
                let dy = y - top + 1 + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < width && (dy as u32) < height {
                    let pixel = image.get_pixel_mut(dx as u32, dy as u32);
                    image::Pixel::blend(pixel, &Rgba([c.r(), c.g(), c.b(), c.a()]));
                }
            }
        }
    });
    Ok(image)
}

fn watermark_position(
    layout: Layout,
    w: &WatermarkSettings,
    width: u32,
    height: u32,
) -> Result<(u32, u32), String> {
    let short = f64::from(layout.photo_width.min(layout.photo_height));
    let margin = (short * w.margin_percent / 100.0).round() as u32;
    let (rx, ry, rw, rh) = if w.region == "border" {
        (
            0,
            layout.y + layout.photo_height,
            layout.width,
            layout.height - layout.y - layout.photo_height,
        )
    } else {
        (layout.x, layout.y, layout.photo_width, layout.photo_height)
    };
    if width.saturating_add(margin.saturating_mul(2)) > rw
        || height.saturating_add(margin.saturating_mul(2)) > rh
    {
        return Err(if w.region == "border" {
            "下边框放不下当前水印，请增加边框比例、减小水印或边距"
        } else {
            "水印超出照片范围，请减小水印、缩短文字或减少边距"
        }
        .into());
    }
    let photo_aligned = w.region == "border" && w.align_to_photo && width <= layout.photo_width;
    let x = if photo_aligned {
        if w.position.ends_with("left") {
            layout.x
        } else if w.position.ends_with("right") {
            layout.x + layout.photo_width - width
        } else {
            layout.x + (layout.photo_width - width) / 2
        }
    } else if w.position.ends_with("left") {
        rx + margin
    } else if w.position.ends_with("right") {
        rx + rw - margin - width
    } else {
        rx + (rw - width) / 2
    };
    let y = if w.region == "border" {
        ry + (rh - height) / 2
    } else if w.position.starts_with("top") {
        ry + margin
    } else if w.position.starts_with("bottom") {
        ry + rh - margin - height
    } else {
        ry + (rh - height) / 2
    };
    let x = i64::from(x) + (w.offset_x * layout.scale).round() as i64;
    let y = i64::from(y) + (w.offset_y * layout.scale).round() as i64;
    if x < i64::from(rx)
        || y < i64::from(ry)
        || x + i64::from(width) > i64::from(rx + rw)
        || y + i64::from(height) > i64::from(ry + rh)
    {
        return Err("水印偏移超出所选区域，请调整水平或垂直偏移".into());
    }
    Ok((x as u32, y as u32))
}

fn jpeg_data_url(image: &RgbaImage, quality: u8) -> Result<String, String> {
    let rgb: RgbImage = DynamicImage::ImageRgba8(image.clone()).into_rgb8();
    rgb_data_url(&rgb, quality)
}

fn rgb_data_url(rgb: &RgbImage, quality: u8) -> Result<String, String> {
    let mut bytes = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| failure("生成预览失败", e))?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Ratio;
    #[test]
    fn analysis_and_filmstrip_thumbnails_preserve_source_aspect_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portrait.png");
        RgbaImage::from_pixel(600, 1200, Rgba([120, 180, 220, 255]))
            .save(&path)
            .unwrap();
        let (data, w, h) = analysis_image(&path).unwrap();
        assert_eq!((w, h), (384, 768));
        let bytes = STANDARD.decode(data.split_once(',').unwrap().1).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (384, 768));
        let data = thumbnail(&path, 180).unwrap();
        let bytes = STANDARD.decode(data.split_once(',').unwrap().1).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (90, 180));
    }
    #[test]
    fn pixel_sampler_uses_oriented_source_and_a_lossless_center_patch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("取色.png");
        let mut pixels = RgbaImage::from_pixel(30, 20, Rgba([12, 34, 56, 255]));
        pixels.put_pixel(12, 8, Rgba([201, 19, 77, 255]));
        pixels.save(&path).unwrap();
        let c = sample_color(&path, 12, 8).unwrap();
        assert_eq!(c.hex, "#C9134D");
        let bytes = STANDARD
            .decode(c.patch_data_url.split_once(',').unwrap().1)
            .unwrap();
        let patch = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(*patch.get_pixel(10, 10), *pixels.get_pixel(12, 8));
        let edge = sample_color(&path, 100, 100).unwrap();
        assert_eq!((edge.x, edge.y), (29, 19));
    }
    #[test]
    fn exact_offsets_scale_with_export_and_reject_overflow_without_clipping() {
        let mut s = RenderSettings::default();
        s.watermark.position = "center".into();
        s.watermark.offset_x = 100.0;
        s.watermark.offset_y = -50.0;
        let l = layout::calculate(4000, 3000, &s).unwrap();
        assert_eq!(
            watermark_position(l, &s.watermark, 200, 80).unwrap(),
            (2150, 1560)
        );
        s.export.long_edge = Some(2150);
        let l = layout::calculate(4000, 3000, &s).unwrap();
        assert_eq!(
            watermark_position(l, &s.watermark, 100, 40).unwrap(),
            (1075, 780)
        );
        s.watermark.offset_x = 30000.0;
        assert!(watermark_position(l, &s.watermark, 100, 40).is_err());
    }
    #[test]
    fn admission_rejects_expensive_allocations_but_accepts_a_48mp_original() {
        assert!(check_decode_budget(12000, 8000, 10_000_000, false).is_err());
        assert!(check_decode_budget(8000, 6000, 20_000_000, false).is_ok());
        let mut s = RenderSettings::default();
        s.border_percent = 20.0;
        s.ratio = Some(Ratio {
            width: 1.0,
            height: 1.0,
        });
        let l = layout::calculate(8000, 6000, &s).unwrap();
        assert!(check_render_budget((8000, 6000), l, true).is_ok());
        let huge = Layout {
            width: 32768,
            height: 32768,
            photo_width: 10000,
            photo_height: 10000,
            x: 0,
            y: 0,
            scale: 1.0,
        };
        assert!(check_render_budget((10000, 10000), huge, true).is_err());
    }
    #[test]
    fn system_fonts_are_real_and_selected_family_changes_the_render() {
        let list = list_fonts();
        // CI may have no system fonts; the bundled fallback is separately covered.
        if let Some(font) = list.iter().find(|f| {
            f.family == "Courier New" || f.family == "Menlo" || f.family == "DejaVu Sans Mono"
        }) {
            let mut w = WatermarkSettings::default();
            w.kind = "text".into();
            w.text = "留白 Signature 123".into();
            let default = text_bitmap(&w, 40.0).unwrap();
            w.font_family = Some(font.family.clone());
            w.font_path = Some(font.path.clone());
            let selected = text_bitmap(&w, 40.0).unwrap();
            assert_ne!(default, selected);
            assert!(selected.pixels().any(|p| p[3] > 0));
        }
    }
    #[test]
    fn validation_rejects_nan_and_incomplete_watermarks() {
        let mut s = RenderSettings::default();
        s.border_percent = f64::NAN;
        assert!(validate_settings(&s).is_err());
        s.border_percent = 5.0;
        s.ratio = Some(Ratio {
            width: 0.0,
            height: 1.0,
        });
        assert!(validate_settings(&s).is_err());
        s.ratio = None;
        s.watermark.kind = "text".into();
        assert!(validate_settings(&s).is_err());
        s.watermark.text = "中文署名".into();
        assert!(validate_settings(&s).is_ok());
    }
    #[test]
    fn png_alpha_resizing_keeps_white_logo_edges_white() {
        let mut logo = RgbaImage::new(4, 4);
        for y in 1..3 {
            for x in 1..3 {
                logo.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let small = resize_alpha(&logo, 2, 2);
        assert!(small
            .pixels()
            .filter(|p| p[3] > 32)
            .all(|p| p[0] > 240 && p[1] > 240 && p[2] > 240));
    }
    #[test]
    fn border_watermark_never_silently_overlaps_photo() {
        let mut s = RenderSettings::default();
        s.border_percent = 0.0;
        let l = layout::calculate(1000, 1000, &s).unwrap();
        let mut w = WatermarkSettings::default();
        w.region = "border".into();
        assert!(watermark_position(l, &w, 100, 20).is_err());
    }
    #[test]
    fn real_export_embeds_srgb_and_works_with_a_temporary_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("中文原片.png");
        RgbaImage::from_pixel(400, 300, Rgba([220, 90, 40, 255]))
            .save(&source)
            .unwrap();
        let s = RenderSettings::default();
        let output = dir.path().join("result.tmp");
        assert_eq!(export_image(&source, &s, &output).unwrap(), (430, 330));
        let mut decoder = ImageReader::open(&output)
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        assert_eq!(decoder.dimensions(), (430, 330));
        assert!(decoder.icc_profile().unwrap().is_some());
        let preview = preview(&source, &s, 300).unwrap();
        assert_eq!((preview.width, preview.height), (430, 330));
    }
    #[test]
    fn bundled_chinese_font_and_custom_font_render_real_pixels() {
        let mut w = WatermarkSettings::default();
        w.kind = "text".into();
        w.text = "留白 FishyJ".into();
        let mark = text_bitmap(&w, 32.0).unwrap();
        assert!(mark.width() > 80);
        assert!(mark.pixels().any(|p| p[3] > 0));
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("自定义.otf");
        fs::write(&custom, include_bytes!("../resources/default-font.otf")).unwrap();
        w.font_path = Some(custom.to_string_lossy().into());
        let custom_mark = text_bitmap(&w, 32.0).unwrap();
        assert_eq!(mark, custom_mark);
    }
    #[test]
    fn heic_fixtures_decode_and_export_a_real_static_srgb_image() {
        for name in ["example.heic", "fixtures/apple-synthetic.heic"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests")
                .join(name);
            assert!(
                source.is_file(),
                "missing HEIC test fixture {}",
                source.display()
            );
            let info = inspect_image(&source).unwrap();
            assert!(info.width > 0 && info.height > 0);
            let mut s = RenderSettings::default();
            s.export.long_edge = Some(640);
            let result = preview(&source, &s, 320).unwrap();
            assert!(result.data_url.starts_with("data:image/jpeg;base64,"));
            let dir = tempfile::tempdir().unwrap();
            let output = dir.path().join("转换.jpg");
            let (w, h) = export_image(&source, &s, &output).unwrap();
            assert!(w.max(h) <= 640);
            let decoded = image::open(output).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (w, h));
        }
    }
    #[test]
    fn icc_profile_is_converted_instead_of_merely_relabelled() {
        let curve = ToneCurve::new(2.2);
        let point = |x, y| CIExyY { x, y, Y: 1.0 };
        let p3 = Profile::new_rgb(
            &point(0.3127, 0.3290),
            &CIExyYTRIPLE {
                Red: point(0.68, 0.32),
                Green: point(0.265, 0.69),
                Blue: point(0.15, 0.06),
            },
            &[&curve, &curve, &curve],
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide-gamut.png");
        let mut encoder = PngEncoder::new(fs::File::create(&source).unwrap());
        encoder.set_icc_profile(p3.icc().unwrap()).unwrap();
        encoder
            .write_image(&[180, 100, 50, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let decoded = decode_standard(&source).unwrap();
        let pixel = decoded.pixels.get_pixel(0, 0);
        assert!(
            pixel[0].abs_diff(180) > 5 || pixel[1].abs_diff(100) > 5 || pixel[2].abs_diff(50) > 5
        );
        assert_eq!(pixel[3], 255);
    }
    #[test]
    fn jpeg_orientation_is_applied_once_before_layout() {
        let image = RgbaImage::from_pixel(40, 30, Rgba([180, 90, 50, 255]));
        let data = jpeg_data_url(&image, 95).unwrap();
        let jpeg = STANDARD.decode(data.split(',').nth(1).unwrap()).unwrap();
        let mut exif =
            b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
        let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1];
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.append(&mut exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("portrait.jpg");
        fs::write(&source, oriented).unwrap();
        let info = inspect_image(&source).unwrap();
        assert_eq!((info.width, info.height), (30, 40));
        let decoded = decode_standard(&source).unwrap();
        assert_eq!(decoded.pixels.dimensions(), (30, 40));
    }

    fn jpeg_with_camera_exif(path: &Path, exif_fields: bool) {
        use exif::{experimental::Writer, Field, In, Rational, Tag, Value};
        let image = RgbaImage::from_pixel(600, 400, Rgba([70, 120, 170, 255]));
        let data = jpeg_data_url(&image, 92).unwrap();
        let jpeg = STANDARD.decode(data.split(',').nth(1).unwrap()).unwrap();
        if !exif_fields {
            fs::write(path, jpeg).unwrap();
            return;
        }
        let field = |tag, value| Field { tag, ifd_num: In::PRIMARY, value };
        let fields = [
            field(Tag::Make, Value::Ascii(vec![b"SONY".to_vec()])),
            field(Tag::Model, Value::Ascii(vec![b"ILCE-7M4".to_vec()])),
            field(Tag::FNumber, Value::Rational(vec![Rational { num: 18, denom: 10 }])),
            field(Tag::ExposureTime, Value::Rational(vec![Rational { num: 1, denom: 250 }])),
            field(Tag::FocalLength, Value::Rational(vec![Rational { num: 35, denom: 1 }])),
            field(Tag::PhotographicSensitivity, Value::Short(vec![100])),
            field(Tag::DateTimeOriginal, Value::Ascii(vec![b"2026:11:17 09:30:00".to_vec()])),
        ];
        let mut writer = Writer::new();
        for f in &fields {
            writer.push_field(f);
        }
        let mut tiff = Cursor::new(Vec::new());
        writer.write(&mut tiff, true).unwrap();
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(tiff.get_ref());
        let mut out = vec![0xff, 0xd8, 0xff, 0xe1];
        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        fs::write(path, out).unwrap();
    }

    #[test]
    fn camera_watermark_fills_from_each_photos_exif() {
        let dir = tempfile::tempdir().unwrap();
        let with = dir.path().join("sony.jpg");
        let plain = dir.path().join("no-exif.jpg");
        jpeg_with_camera_exif(&with, true);
        jpeg_with_camera_exif(&plain, false);
        let info = camera_info(&with).unwrap();
        assert_eq!(info.camera.as_deref(), Some("SONY ILCE-7M4"));
        assert_eq!(info.date.as_deref(), Some("2026.11.17"));

        let mut s = RenderSettings::default();
        s.border_percent = 12.0;
        s.frame_color = "#FFFFFF".into();
        s.watermark.kind = "camera".into();
        s.watermark.region = "border".into();
        s.watermark.position = "bottom-left".into();
        s.watermark.color = "#000000".into();
        s.watermark.opacity = 1.0;
        s.watermark.size_percent = 4.0;
        s.watermark.margin_percent = 1.0;
        validate_settings(&s).unwrap();

        let source = loaded(&with).unwrap();
        assert_eq!(
            for_photo(&s, &source).watermark.text,
            "SONY ILCE-7M4 · 35mm · f/1.8 · 1/250s · ISO 100"
        );
        assert_eq!(preview(&with, &s, 512).unwrap().warning, None);
        assert!(watermark_detail(&with, &s).is_ok());
        assert_eq!(preview(&plain, &s, 512).unwrap().warning.as_deref(), Some(NO_CAMERA_INFO));
        assert_eq!(watermark_detail(&plain, &s).unwrap_err(), NO_CAMERA_INFO);

        // Only the photo with EXIF gets dark glyphs in its white bottom border.
        let dark_in_border = |path: &Path| {
            let out = dir.path().join(format!("out-{}.png", path.file_stem().unwrap().to_string_lossy()));
            let mut png = s.clone();
            png.export.format = "png".into();
            export_image(path, &png, &out).unwrap();
            let img = image::open(&out).unwrap().to_rgb8();
            let border_top = img.height() - (400.0 * 0.12) as u32 + 2;
            (border_top..img.height())
                .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
                .filter(|&(x, y)| img.get_pixel(x, y)[0] < 128)
                .count()
        };
        assert!(dark_in_border(&with) > 50);
        // The caption lines up with the photo edge, not the canvas edge.
        let full = layout::calculate(600, 400, &s).unwrap();
        let resolved = for_photo(&s, &source);
        let mark = watermark_bitmap(&resolved.watermark, full).unwrap().unwrap();
        let (x, _) = watermark_position(full, &resolved.watermark, mark.width(), mark.height()).unwrap();
        assert_eq!(x, full.x);
        assert_eq!(dark_in_border(&plain), 0);

        // Camera details travel into exported JPGs only when asked for, and
        // never orientation or location.
        let jpg_meta = |keep: bool| {
            let out = dir.path().join(format!("meta-{keep}.jpg"));
            let mut jpg = s.clone();
            jpg.export.format = "jpg".into();
            jpg.export.keep_camera_info = keep;
            export_image(&with, &jpg, &out).unwrap();
            let bytes = fs::read(&out).unwrap();
            assert!(image::load_from_memory(&bytes).is_ok());
            crate::camera_info::read(&bytes)
        };
        assert!(jpg_meta(false).is_empty());
        let kept = jpg_meta(true);
        assert_eq!(kept.aperture.as_deref(), Some("f/1.8"));
        assert_eq!(kept.camera.as_deref(), Some("SONY ILCE-7M4"));
        let block = crate::camera_info::portable_exif(&fs::read(&with).unwrap()).unwrap();
        let parsed = exif::Reader::new().read_raw(block[6..].to_vec()).unwrap();
        assert!(parsed.get_field(exif::Tag::Orientation, exif::In::PRIMARY).is_none());
        assert!(parsed.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).is_none());

        s.watermark.camera_template = "  ".into();
        assert!(validate_settings(&s).is_err());
    }
}

fn bundled_font_db() -> &'static fontdb::Database {
    FONT_DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_font_data(include_bytes!("../resources/default-font.otf").to_vec());
        db.load_font_data(include_bytes!("../resources/ZCOOLXiaoWei-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../resources/MaShanZheng-Regular.ttf").to_vec());
        db
    })
}
fn face_has_chinese(db: &fontdb::Database, id: fontdb::ID) -> bool {
    db.with_face_data(id, |data, index| {
        ttf_parser::Face::parse(data, index).is_ok_and(|f| {
            ['中', '文', '摄', '影']
                .iter()
                .all(|c| f.glyph_index(*c).is_some())
        })
    })
    .unwrap_or(false)
}
pub fn list_fonts() -> Vec<FontOption> {
    static OPTIONS: OnceLock<Vec<FontOption>> = OnceLock::new();
    OPTIONS
        .get_or_init(|| {
            let mut result: Vec<_> = bundled_font_db()
                .faces()
                .filter_map(|f| {
                    f.families.first().map(|(family, _)| FontOption {
                        family: family.clone(),
                        path: String::new(),
                        chinese: face_has_chinese(bundled_font_db(), f.id),
                        bundled: true,
                    })
                })
                .collect();
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let mut choices = std::collections::BTreeMap::new();
            for f in db.faces() {
                let path = match &f.source {
                    fontdb::Source::File(p) | fontdb::Source::SharedFile(p, _) => p,
                    _ => continue,
                };
                if !path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                    matches!(
                        s.to_ascii_lowercase().as_str(),
                        "otf" | "ttf" | "ttc" | "otc"
                    )
                }) || fs::metadata(path).map_or(true, |m| m.len() > 64 * 1024 * 1024)
                {
                    continue;
                }
                if let Some((family, _)) = f.families.first() {
                    if family.starts_with('.') || result.iter().any(|r| r.family == *family) {
                        continue;
                    }
                    let score = f.weight.0.abs_diff(400)
                        + if f.style == fontdb::Style::Normal {
                            0
                        } else {
                            1000
                        };
                    let entry =
                        choices
                            .entry(family.clone())
                            .or_insert((u16::MAX, path.clone(), false));
                    if score < entry.0 {
                        *entry = (score, path.clone(), face_has_chinese(&db, f.id));
                    }
                }
            }
            result.extend(
                choices
                    .into_iter()
                    .map(|(family, (_, path, chinese))| FontOption {
                        family,
                        path: path.to_string_lossy().into_owned(),
                        chinese,
                        bundled: false,
                    }),
            );
            result
        })
        .clone()
}
pub fn frame_thumbnail(settings: &RenderSettings) -> Result<String, String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut s = settings.clone();
    s.watermark.kind = "none".into();
    s.export.long_edge = Some(360);
    validate_settings(&s)?;
    // A plain white sample keeps the frame itself in focus. Reuse one tiny
    // local canvas; browsing never decodes a stock photo or accesses a network.
    static SAMPLE: std::sync::OnceLock<RgbaImage> = std::sync::OnceLock::new();
    let source = SAMPLE.get_or_init(|| RgbaImage::from_pixel(480, 320, Rgba([255, 255, 255, 255])));
    let l = layout::calculate(source.width(), source.height(), &s)?;
    let image = compose(source, &s, l, None, None, l)?;
    rgb_data_url(&image, 85)
}

pub fn sample_color(path: &Path, x: u32, y: u32) -> Result<ColorSample, String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let x = x.min(source.pixels.width() - 1);
    let y = y.min(source.pixels.height() - 1);
    // A tiny lossless patch of oriented sRGB source pixels, never the composed preview.
    let mut patch = rgba_buffer(21, 21)?;
    for py in 0..21 {
        for px in 0..21 {
            let sx = (i64::from(x) + i64::from(px) - 10)
                .clamp(0, i64::from(source.pixels.width() - 1)) as u32;
            let sy = (i64::from(y) + i64::from(py) - 10)
                .clamp(0, i64::from(source.pixels.height() - 1)) as u32;
            patch.put_pixel(px, py, *source.pixels.get_pixel(sx, sy));
        }
    }
    let p = source.pixels.get_pixel(x, y);
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(patch.as_raw(), 21, 21, image::ExtendedColorType::Rgba8)
        .map_err(|e| failure("生成放大镜失败", e))?;
    Ok(ColorSample {
        x,
        y,
        hex: format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]),
        patch_data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
    })
}

pub fn analysis_image(path: &Path) -> Result<(String, u32, u32), String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let small = thumbnail_fit(&source.pixels, 768);
    let dimensions = small.dimensions();
    // A white background makes transparent PNG subjects visible to the model.
    let mut rgb = RgbImage::from_pixel(dimensions.0, dimensions.1, image::Rgb([255, 255, 255]));
    blend_rgb(&mut rgb, &small, 0, 0);
    Ok((rgb_data_url(&rgb, 80)?, dimensions.0, dimensions.1))
}

fn png_url(image: &RgbaImage) -> Result<String, String> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| failure("生成局部预览失败", e))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}
pub fn sampler_image(
    path: &Path,
    x: Option<u32>,
    y: Option<u32>,
) -> Result<crate::model::SampleImage, String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let (image, x, y) = if let (Some(x), Some(y)) = (x, y) {
        let x = (x.min(source.pixels.width() - 1) / 512 * 512).saturating_sub(16);
        let y = (y.min(source.pixels.height() - 1) / 512 * 512).saturating_sub(16);
        let w = 544.min(source.pixels.width() - x);
        let h = 544.min(source.pixels.height() - y);
        (
            imageops::crop_imm(&source.pixels, x, y, w, h).to_image(),
            x,
            y,
        )
    } else {
        (thumbnail_fit(&source.pixels, 2048), 0, 0)
    };
    Ok(crate::model::SampleImage {
        data_url: png_url(&image)?,
        x,
        y,
        width: image.width(),
        height: image.height(),
    })
}
fn mark_geometry(
    width: u32,
    height: u32,
    settings: &RenderSettings,
) -> Result<(Layout, RgbaImage, (u32, u32), crate::model::OffsetLimits), String> {
    validate_settings(settings)?;
    let full = layout::calculate(width, height, settings)?;
    let mark = watermark_bitmap(&settings.watermark, full)?.ok_or("请先开启水印")?;
    let mut neutral = settings.watermark.clone();
    neutral.offset_x = 0.;
    neutral.offset_y = 0.;
    let (x, y) = watermark_position(full, &neutral, mark.width(), mark.height())?;
    let (rx, ry, rw, rh) = if neutral.region == "border" {
        (
            0,
            full.y + full.photo_height,
            full.width,
            full.height - full.y - full.photo_height,
        )
    } else {
        (full.x, full.y, full.photo_width, full.photo_height)
    };
    let limits = crate::model::OffsetLimits {
        min_x: (f64::from(rx) - f64::from(x)) / full.scale,
        max_x: (f64::from(rx + rw - mark.width()) - f64::from(x)) / full.scale,
        min_y: (f64::from(ry) - f64::from(y)) / full.scale,
        max_y: (f64::from(ry + rh - mark.height()) - f64::from(y)) / full.scale,
    };
    let position = watermark_position(full, &settings.watermark, mark.width(), mark.height())?;
    Ok((full, mark, position, limits))
}
/// Paint the actual watermark with its photo/frame surroundings without allocating
/// a full-size export canvas. Both border painters and glyph rasterizer are shared.
pub fn watermark_detail(
    path: &Path,
    settings: &RenderSettings,
) -> Result<crate::model::WatermarkDetail, String> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let source = loaded(path)?;
    let resolved = for_photo(settings, &source);
    if settings.watermark.kind == "camera" && resolved.watermark.kind == "none" {
        return Err(NO_CAMERA_INFO.into());
    }
    let settings = &*resolved;
    let (full, mark, (mx, my), limits) =
        mark_geometry(source.pixels.width(), source.pixels.height(), settings)?;
    let pad = (mark.height() / 2).clamp(24, 180);
    let x0 = mx.saturating_sub(pad);
    let y0 = my.saturating_sub(pad);
    let x1 = (mx + mark.width() + pad).min(full.width);
    let y1 = (my + mark.height() + pad).min(full.height);
    let factor = (1600.0 / f64::from(x1 - x0))
        .min(900.0 / f64::from(y1 - y0))
        .min(1.0);
    let scale = |v: u32| (f64::from(v) * factor).round() as u32;
    let l = Layout {
        width: scale(full.width).max(1),
        height: scale(full.height).max(1),
        photo_width: scale(full.photo_width).max(1),
        photo_height: scale(full.photo_height).max(1),
        x: scale(full.x),
        y: scale(full.y),
        scale: full.scale * factor,
    };
    let ox = scale(x0);
    let oy = scale(y0);
    let w = (scale(x1) - ox).max(1);
    let h = (scale(y1) - oy).max(1);
    let c = parse_color(&settings.frame_color)?;
    let mut canvas = RgbImage::from_pixel(w, h, image::Rgb(c));
    if let Some(backdrop) = crate::backdrop::prepare(&source.pixels, &settings.background, c, l.width, l.height) {
        crate::backdrop::paint(&mut canvas, &backdrop, ox, oy, l.width, l.height);
    }
    for y in 0..h {
        for x in 0..w {
            let gx = x + ox;
            let gy = y + oy;
            if gx < l.x || gy < l.y || gx >= l.x + l.photo_width || gy >= l.y + l.photo_height {
                continue;
            }
            let sx = ((f64::from(gx - l.x) + 0.5) * f64::from(source.pixels.width())
                / f64::from(l.photo_width)
                - 0.5)
                .clamp(0., f64::from(source.pixels.width() - 1));
            let sy = ((f64::from(gy - l.y) + 0.5) * f64::from(source.pixels.height())
                / f64::from(l.photo_height)
                - 0.5)
                .clamp(0., f64::from(source.pixels.height() - 1));
            let bx = sx.floor() as u32;
            let by = sy.floor() as u32;
            let fx = sx.fract();
            let fy = sy.fract();
            let mut rgba = [0.; 4];
            for (px, py, weight) in [
                (bx, by, (1. - fx) * (1. - fy)),
                ((bx + 1).min(source.pixels.width() - 1), by, fx * (1. - fy)),
                (bx, (by + 1).min(source.pixels.height() - 1), (1. - fx) * fy),
                (
                    (bx + 1).min(source.pixels.width() - 1),
                    (by + 1).min(source.pixels.height() - 1),
                    fx * fy,
                ),
            ] {
                let p = source.pixels.get_pixel(px, py);
                let alpha = f64::from(p[3]) / 255.;
                rgba[3] += alpha * weight;
                for i in 0..3 {
                    rgba[i] += f64::from(p[i]) * alpha * weight;
                }
            }
            let target = canvas.get_pixel_mut(x, y);
            for i in 0..3 {
                target[i] = (rgba[i] + f64::from(target[i]) * (1. - rgba[3])).round() as u8;
            }
        }
    }
    crate::decoration::draw_window(&mut canvas, &settings.decoration, l, full, ox, oy)?;
    if let Some(art) = &settings.artwork {
        crate::artwork::draw_window(&mut canvas, art, l, ox, oy)?;
    }
    let mark = if factor < 1.0 {
        resize_alpha(
            &mark,
            scale(mark.width()).max(1),
            scale(mark.height()).max(1),
        )
    } else {
        mark
    };
    let rx = scale(mx).saturating_sub(ox);
    let ry = scale(my).saturating_sub(oy);
    blend_rgb(&mut canvas, &mark, rx, ry);
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(canvas.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| failure("生成局部预览失败", e))?;
    Ok(crate::model::WatermarkDetail {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        view_width: w,
        view_height: h,
        width: ((mark.width() as f64) / factor).round() as u32,
        height: ((mark.height() as f64) / factor).round() as u32,
        watermark_bounds: PixelRect {
            x: rx,
            y: ry,
            width: mark.width(),
            height: mark.height(),
        },
        position: PixelRect {
            x: mx,
            y: my,
            width: ((mark.width() as f64) / factor).round() as u32,
            height: ((mark.height() as f64) / factor).round() as u32,
        },
        limits,
    })
}

#[cfg(test)]
mod detail_tests {
    use super::*;
    fn png(url: &str) -> RgbaImage {
        image::load_from_memory(&STANDARD.decode(url.split_once(',').unwrap().1).unwrap())
            .unwrap()
            .to_rgba8()
    }
    #[test]
    fn detail_matches_actual_photo_frame_and_watermark_pixels_and_moves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("照片.png");
        RgbaImage::from_pixel(900, 600, Rgba([70, 120, 190, 255]))
            .save(&path)
            .unwrap();
        let mut s = RenderSettings::default();
        s.border_percent = 15.;
        s.watermark.kind = "text".into();
        s.watermark.text = "留白 FishyJ".into();
        s.watermark.color = "#101010".into();
        s.watermark.size_percent = 0.5;
        s.watermark.opacity = 0.4;
        for region in ["photo", "border"] {
            s.watermark.region = region.into();
            s.watermark.margin_percent = 1.;
            let d = watermark_detail(&path, &s).unwrap();
            let (l, m, pos, _) = mark_geometry(900, 600, &s).unwrap();
            let src = loaded(&path).unwrap();
            let full = compose(&src.pixels, &s, l, Some(&m), Some(pos), l).unwrap();
            let crop = png(&d.data_url);
            let ox = pos.0 - d.watermark_bounds.x;
            let oy = pos.1 - d.watermark_bounds.y;
            for y in 0..crop.height() {
                for x in 0..crop.width() {
                    assert_eq!(
                        &crop.get_pixel(x, y).0[..3],
                        &full.get_pixel(x + ox, y + oy).0
                    );
                }
            }
            assert!(crop.width() > m.width());
            assert!(crop.height() > m.height());
            s.watermark.offset_y = -4.;
            let next = watermark_detail(&path, &s).unwrap();
            assert_eq!(next.position.y + 4, d.position.y);
            s.watermark.offset_y = 0.;
        }
    }
    #[test]
    fn offset_slider_limits_stay_inside_the_selected_region_and_scale_in_source_pixels() {
        let mut s = RenderSettings::default();
        s.border_percent = 15.;
        s.watermark.kind = "text".into();
        s.watermark.text = "署名".into();
        s.watermark.region = "border".into();
        s.export.long_edge = Some(2000);
        let (l, mark, _, limits) = mark_geometry(4000, 3000, &s).unwrap();
        for x in [limits.min_x.ceil(), limits.max_x.floor()] {
            for y in [limits.min_y.ceil(), limits.max_y.floor()] {
                s.watermark.offset_x = x;
                s.watermark.offset_y = y;
                assert!(watermark_position(l, &s.watermark, mark.width(), mark.height()).is_ok());
            }
        }
        s.watermark.offset_y = limits.max_y + 4.;
        assert!(watermark_position(l, &s.watermark, mark.width(), mark.height()).is_err());
    }
    #[test]
    fn source_tiles_are_bounded_lossless_and_do_not_replace_the_source_with_a_preview() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("取色.png");
        let mut src = RgbaImage::from_pixel(2200, 1300, Rgba([12, 34, 56, 255]));
        src.put_pixel(1400, 800, Rgba([201, 19, 77, 255]));
        src.save(&p).unwrap();
        let tile = sampler_image(&p, Some(1400), Some(800)).unwrap();
        assert!(tile.width <= 544 && tile.height <= 544);
        let decoded = png(&tile.data_url);
        assert_eq!(
            decoded.get_pixel(1400 - tile.x, 800 - tile.y),
            src.get_pixel(1400, 800)
        );
        let proxy = sampler_image(&p, None, None).unwrap();
        assert_eq!(proxy.width, 2048);
        assert!(proxy.height < 2048);
    }
}

#[cfg(test)]
mod chinese_tests {
    use super::*;
    #[test]
    fn three_bundled_chinese_faces_have_own_glyphs_and_different_shapes() {
        let options = list_fonts();
        let bundled: Vec<_> = options.iter().filter(|f| f.bundled).collect();
        assert_eq!(bundled.len(), 3);
        assert!(bundled.iter().all(|f| f.chinese && f.path.is_empty()));
        let mut w = WatermarkSettings::default();
        w.kind = "text".into();
        w.text = "中文摄影 · 留白".into();
        let mut rendered = Vec::new();
        for f in bundled {
            w.font_family = Some(f.family.clone());
            rendered.push(text_bitmap(&w, 48.).unwrap());
        }
        assert_ne!(rendered[0], rendered[1]);
        assert_ne!(rendered[1], rendered[2]);
    }
}

/// Landscape / portrait / square checks for every built-in template.
/// `template_board` (ignored) writes a visual acceptance board per template:
/// `MARGIN_BOARD_DIR=/some/dir cargo test --lib template_board -- --ignored`.
#[cfg(test)]
mod template_qa {
    use super::*;
    use image::Rgb;
    use crate::model::Ratio;

    /// A busy, asymmetric test photo so misplaced art over the opening shows.
    fn photo(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let edge = x < 3 || y < 3 || x >= w - 3 || y >= h - 3;
            if edge {
                return Rgba([230, 40, 40, 255]);
            }
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let grid = (x / 40 + y / 40) % 2 == 0;
            let g = if grid { 18 } else { 0 };
            Rgba([(60. + 120. * fx) as u8 + g, (90. + 80. * fy) as u8 + g, (150. - 60. * fx) as u8 + g, 255])
        })
    }

    /// Photo sizes are 900/600/720 px divided by `div`.
    /// The board can use a real picture (MARGIN_BOARD_PHOTO), cover-cropped
    /// to each case; tests always use the synthetic photo.
    fn sample_photo(w: u32, h: u32) -> RgbaImage {
        match std::env::var("MARGIN_BOARD_PHOTO") {
            Ok(path) if !path.is_empty() => {
                let img = image::open(path).unwrap().to_rgba8();
                let f = (w as f32 / img.width() as f32).max(h as f32 / img.height() as f32);
                let (rw, rh) = ((img.width() as f32 * f).ceil() as u32, (img.height() as f32 * f).ceil() as u32);
                let scaled = imageops::resize(&img, rw.max(w), rh.max(h), imageops::FilterType::Triangle);
                imageops::crop_imm(&scaled, (scaled.width() - w) / 2, (scaled.height() - h) / 2, w, h).to_image()
            }
            _ => photo(w, h),
        }
    }

    fn cases(div: u32) -> Vec<(&'static str, u32, u32, Option<Ratio>)> {
        let (a, b, c) = (900 / div, 600 / div, 720 / div);
        vec![
            ("landscape", a, b, None),
            ("portrait", b, a, None),
            ("square", c, c, None),
            ("landscape-1x1", a, b, Some(Ratio { width: 1., height: 1. })),
            ("portrait-4x5", b, a, Some(Ratio { width: 4., height: 5. })),
            ("landscape-16x9", a, b, Some(Ratio { width: 16., height: 9. })),
        ]
    }

    fn render(id: &str, w: u32, h: u32, ratio: Option<Ratio>, border: Option<f64>) -> (RgbImage, Layout, RgbaImage) {
        let mut s = crate::designs::settings(id).unwrap();
        s.watermark.kind = "none".into();
        s.export.long_edge = None;
        s.ratio = ratio;
        if let Some(b) = border {
            s.border_percent = b;
        }
        validate_settings(&s).unwrap();
        let source = sample_photo(w, h);
        let l = layout::calculate(w, h, &s).unwrap();
        (compose(&source, &s, l, None, None, l).unwrap(), l, source)
    }

    #[test]
    fn every_template_keeps_the_photo_opening_untouched_in_every_orientation() {
        let mut problems = Vec::new();
        for frame in crate::designs::library().unwrap() {
            for (name, w, h, ratio) in cases(3) {
                for border in [None, Some(4.)] {
                    let (out, l, source) = render(&frame.id, w, h, ratio.clone(), border);
                    assert_eq!((l.photo_width, l.photo_height), (w, h), "{} {name}: photo is never resized", frame.id);
                    let mut changed = 0usize;
                    for y in 0..h {
                        for x in 0..w {
                            let s = source.get_pixel(x, y);
                            let o = out.get_pixel(x + l.x, y + l.y);
                            if (0..3).any(|c| s[c] != o[c]) {
                                changed += 1;
                            }
                        }
                    }
                    if changed > 0 {
                        problems.push(format!("{} {name} border={border:?}: {changed} photo pixels covered", frame.id));
                    }
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    #[ignore]
    fn template_board() {
        let dir = std::path::PathBuf::from(std::env::var("MARGIN_BOARD_DIR").expect("set MARGIN_BOARD_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let cell = 300u32;
        // MARGIN_BOARD_IDS=a,b limits the board to some templates.
        let only = std::env::var("MARGIN_BOARD_IDS").unwrap_or_default();
        for frame in crate::designs::library().unwrap() {
            if !only.is_empty() && !only.split(',').any(|id| id == frame.id) {
                continue;
            }
            let cases = cases(1);
            let mut board = RgbImage::from_pixel(cell * cases.len() as u32 + 10 * (cases.len() as u32 + 1), cell * 2 + 30, Rgb([40, 40, 40]));
            for (row, border) in [None, Some(4.)].into_iter().enumerate() {
                for (col, (_, w, h, ratio)) in cases.iter().enumerate() {
                    let (out, _, _) = render(&frame.id, *w, *h, ratio.clone(), border);
                    let f = cell as f32 / out.width().max(out.height()) as f32;
                    let small = imageops::resize(&out, (out.width() as f32 * f) as u32, (out.height() as f32 * f) as u32, imageops::FilterType::Triangle);
                    let ox = 10 + col as u32 * (cell + 10) + (cell - small.width()) / 2;
                    let oy = 10 + row as u32 * (cell + 10) + (cell - small.height()) / 2;
                    imageops::overlay(&mut board, &small, ox.into(), oy.into());
                }
            }
            board.save(dir.join(format!("{}.png", frame.id))).unwrap();
        }
    }
}
