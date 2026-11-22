use crate::model::RenderSettings;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

pub const REMOVAL_HISTORY_LIMIT: usize = 20;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoSelection {
    pub active_id: String,
    pub ids: Vec<String>,
    pub anchor_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedPhotos {
    pub id: String,
    pub order: Vec<String>,
    pub photos: Vec<Photo>,
    #[serde(default)]
    pub selection: PhotoSelection,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogHistory {
    pub undo: VecDeque<RemovedPhotos>,
    pub redo: VecDeque<RemovedPhotos>,
    #[serde(skip)]
    pub recovered: bool,
}

fn deserialize_history<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<CatalogHistory, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(
        serde_json::from_value(value).unwrap_or_else(|_| CatalogHistory {
            recovered: true,
            ..Default::default()
        }),
    )
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalHistoryEntry {
    pub id: String,
    pub count: usize,
    pub selection: PhotoSelection,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogHistorySnapshot {
    pub undo: Vec<RemovalHistoryEntry>,
    pub redo: Vec<RemovalHistoryEntry>,
}

impl CatalogHistory {
    pub fn snapshot(&self) -> CatalogHistorySnapshot {
        let entries = |records: &VecDeque<RemovedPhotos>| {
            records
                .iter()
                .map(|r| RemovalHistoryEntry {
                    id: r.id.clone(),
                    count: r.photos.len(),
                    selection: r.selection.clone(),
                })
                .collect()
        };
        CatalogHistorySnapshot {
            undo: entries(&self.undo),
            redo: entries(&self.redo),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: String,
    pub path: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub thumbnail: String,
    /// This photo's own frame and watermark, overriding the shared settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<RenderSettings>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub settings: RenderSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTemplate {
    pub id: String,
    pub name: String,
    pub settings: RenderSettings,
    pub thumbnail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobItem {
    pub id: String,
    pub photo_id: String,
    pub name: String,
    pub source_path: String,
    pub status: String,
    /// The photo's own settings, frozen when the export started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<RenderSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSnapshot {
    pub id: String,
    #[serde(default)]
    pub revision: u64,
    pub status: String,
    pub output_directory: String,
    pub settings: RenderSettings,
    pub items: Vec<JobItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredState {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub settings: RenderSettings,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub frame_templates: Vec<FrameTemplate>,
    #[serde(default)]
    pub photos: Vec<Photo>,
    #[serde(default)]
    pub batch: Option<BatchSnapshot>,
    #[serde(default, deserialize_with = "deserialize_history")]
    pub catalog_history: CatalogHistory,
    #[serde(default)]
    pub catalog_revision: u64,
    #[serde(default)]
    pub photo_selection: Option<PhotoSelection>,
}

fn schema_version() -> u32 {
    1
}

impl Default for StoredState {
    fn default() -> Self {
        Self {
            schema_version: schema_version(),
            settings: RenderSettings::default(),
            presets: Vec::new(),
            frame_templates: Vec::new(),
            photos: Vec::new(),
            batch: None,
            catalog_history: CatalogHistory::default(),
            catalog_revision: 0,
            photo_selection: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub settings: RenderSettings,
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub frame_templates: Vec<FrameTemplate>,
    pub photos: Vec<Photo>,
    pub batch: Option<BatchSnapshot>,
    pub warnings: Vec<String>,
    pub catalog_history: CatalogHistorySnapshot,
    pub catalog_revision: u64,
    pub photo_selection: PhotoSelection,
}

/// The normalized, atomically committed settings, without photos/thumbnails.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCommit {
    pub updates: HashMap<String, Option<RenderSettings>>,
    pub shared: Option<RenderSettings>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub photos: Vec<Photo>,
    pub errors: Vec<String>,
    pub catalog_revision: u64,
    pub photo_selection: PhotoSelection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoRemovalReceipt {
    pub id: String,
    pub count: usize,
    pub photo_ids: Vec<String>,
    pub catalog_revision: u64,
    pub photo_selection: PhotoSelection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoredPhotos {
    pub photos: Vec<Photo>,
    pub id_map: std::collections::HashMap<String, String>,
    pub catalog_revision: u64,
    pub photo_selection: PhotoSelection,
}
