//! Shared validation and ownership for Codex and direct Image API artwork.
use base64::Engine;
use serde::Serialize;
use std::{fs, io::Write, path::PathBuf, time::SystemTime};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedArt {
    pub image_path: String,
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
    pub summary: String,
    /// For complete frames: the border percentage at which this artwork's own
    /// band thickness fits the photograph, so applying it never squeezes motifs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_percent: Option<f64>,
}

pub(crate) async fn save_png(
    bytes: Vec<u8>,
    directory: PathBuf,
    provenance: serde_json::Value,
) -> Result<GeneratedArt, String> {
    save(bytes, directory, provenance, false).await
}
pub(crate) async fn save_frame_png(
    bytes: Vec<u8>,
    directory: PathBuf,
    provenance: serde_json::Value,
) -> Result<GeneratedArt, String> {
    save(bytes, directory, provenance, true).await
}
async fn save(
    bytes: Vec<u8>,
    directory: PathBuf,
    mut provenance: serde_json::Value,
    frame: bool,
) -> Result<GeneratedArt, String> {
    tokio::task::spawn_blocking(move || {
        // Complete frames retain their exact canvas geometry. An opaque center is
        // safe too: the compositor excludes the photograph rectangle unconditionally.
        let im = crate::stickers::decode_png(&bytes, !frame, !frame)?;
        let (width, height) = im.dimensions();
        let mut thumbnail = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(im)
            .thumbnail(480, 320)
            .write_to(&mut thumbnail, image::ImageFormat::Png)
            .map_err(|_| "无法生成素材预览")?;
        let assets = directory.join("assets");
        fs::create_dir_all(&assets).map_err(|_| "无法保存手绘素材")?;
        let id = uuid::Uuid::new_v4().to_string();
        let path = assets.join(format!("{id}.png"));
        let mut temp = tempfile::NamedTempFile::new_in(&assets).map_err(|_| "无法保存手绘素材")?;
        temp.write_all(&bytes).map_err(|_| "无法保存手绘素材")?;
        temp.as_file().sync_all().map_err(|_| "无法保存手绘素材")?;
        temp.persist_noclobber(&path)
            .map_err(|_| "无法提交手绘素材")?;
        provenance["width"] = width.into();
        provenance["kind"] = if frame { "complete-frame" } else { "sticker" }.into();
        provenance["height"] = height.into();
        provenance["createdUnix"] = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .into();
        let summary = provenance["summary"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        fs::write(
            assets.join(format!("{id}.provenance.json")),
            provenance.to_string(),
        )
        .map_err(|_| "无法保存素材来源记录")?;
        Ok(GeneratedArt {
            image_path: path.to_string_lossy().into_owned(),
            thumbnail: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(thumbnail.into_inner())
            ),
            width,
            height,
            summary,
            border_percent: None,
        })
    })
    .await
    .map_err(|_| "素材处理意外中断")?
}
