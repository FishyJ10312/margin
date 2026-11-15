use crate::model::RenderSettings;
use serde::{Deserialize, Serialize};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
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
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub photos: Vec<Photo>,
    pub errors: Vec<String>,
}
