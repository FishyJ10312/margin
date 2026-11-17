use crate::app_models::{PhotoSelection, StoredState};
use crate::model::RenderSettings;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::NamedTempFile;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPhotoSelection {
    catalog_revision: u64,
    selection: PhotoSelection,
}

pub struct Storage {
    pub directory: PathBuf,
    asset_aliases: Mutex<HashMap<String, String>>,
}

impl Storage {
    fn thumbnail_path(&self, id: &str) -> PathBuf {
        let mut hash = DefaultHasher::new();
        id.hash(&mut hash);
        self.directory
            .join("thumbnails")
            .join(format!("{:016x}.txt", hash.finish()))
    }

    pub fn cached_thumbnail(&self, id: &str) -> String {
        let path = self.thumbnail_path(id);
        if fs::metadata(&path).map_or(true, |m| m.len() > 128 * 1024) {
            return String::new();
        }
        fs::read_to_string(path)
            .ok()
            .filter(|s| s.starts_with("data:image/jpeg;base64,"))
            .unwrap_or_default()
    }

    pub fn cache_thumbnail(&self, id: &str, value: &str) -> Result<(), String> {
        if value.is_empty()
            || value.len() > 128 * 1024
            || !value.starts_with("data:image/jpeg;base64,")
        {
            return Ok(());
        }
        let path = self.thumbnail_path(id);
        if path.is_file() {
            return Ok(());
        }
        let directory = path.parent().unwrap();
        fs::create_dir_all(directory).map_err(|e| format!("无法保存照片缩略图：{e}"))?;
        let mut temporary =
            NamedTempFile::new_in(directory).map_err(|e| format!("无法保存照片缩略图：{e}"))?;
        temporary
            .write_all(value.as_bytes())
            .map_err(|e| format!("无法保存照片缩略图：{e}"))?;
        match temporary.persist_noclobber(path) {
            Ok(_) => Ok(()),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(format!("无法保存照片缩略图：{}", e.error)),
        }
    }

    pub fn forget_thumbnails(&self, ids: &std::collections::HashSet<String>) {
        for id in ids {
            let _ = fs::remove_file(self.thumbnail_path(id));
        }
    }

    pub fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|e| format!("无法建立本机设置目录：{e}"))?;
        Ok(Self {
            directory,
            asset_aliases: Mutex::new(HashMap::new()),
        })
    }

    pub fn load(&self) -> Result<(StoredState, Vec<String>), String> {
        let path = self.directory.join("state.json");
        if !path.exists() {
            return Ok((StoredState::default(), Vec::new()));
        }
        let (mut state, warnings) = match read_state(&path) {
            Ok(state) => (state, Vec::new()),
            Err(error) => {
                let backup = self.directory.join("state.backup.json");
                if let Ok(state) = read_state(&backup) {
                    (
                        state,
                        vec![format!("上次的设置文件无法读取，已恢复备份：{error}")],
                    )
                } else {
                    let preserved = self
                        .directory
                        .join(format!("state-damaged-{}.json", uuid::Uuid::new_v4()));
                    fs::copy(&path, &preserved)
                        .map_err(|e| format!("无法保留损坏的设置文件：{e}"))?;
                    (
                        StoredState::default(),
                        vec!["上次的设置文件损坏，已保留副本并恢复默认设置。".into()],
                    )
                }
            }
        };
        // Selection-only edits use a small sidecar rather than rewriting up to
        // twenty history records on each click. The revision ties it to one catalog.
        let selection_path = self.directory.join("catalog-selection.json");
        if fs::metadata(&selection_path).is_ok_and(|m| m.len() <= 2 * 1024 * 1024) {
            if let Ok(saved) = File::open(&selection_path).map_err(|e| e.to_string()).and_then(|file| serde_json::from_reader::<_, SavedPhotoSelection>(file).map_err(|e| e.to_string())) {
                if saved.catalog_revision == state.catalog_revision { state.photo_selection = Some(saved.selection); }
            }
        }
        Ok((state, warnings))
    }

    pub fn save_photo_selection(&self, catalog_revision: u64, selection: &PhotoSelection) -> Result<(), String> {
        let data = serde_json::to_vec(&SavedPhotoSelection { catalog_revision, selection: selection.clone() }).map_err(|e| format!("无法保存照片选择：{e}"))?;
        let mut temporary = NamedTempFile::new_in(&self.directory).map_err(|e| format!("无法保存照片选择：{e}"))?;
        temporary.write_all(&data).and_then(|_| temporary.as_file().sync_all()).map_err(|e| format!("无法保存照片选择：{e}"))?;
        temporary.persist(self.directory.join("catalog-selection.json")).map_err(|e| format!("无法保存照片选择：{}", e.error))?;
        Ok(())
    }

    pub fn save(&self, state: &StoredState) -> Result<(), String> {
        let path = self.directory.join("state.json");
        let backup = self.directory.join("state.backup.json");
        if path.exists() && read_state(&path).is_ok() {
            fs::copy(&path, backup).map_err(|e| format!("无法备份本机设置：{e}"))?;
        }
        let mut compact = state.clone();
        // Cache small thumbnails separately once. Startup never opens original
        // photos, which may block on cloud storage, disks or OS access prompts.
        for photo in &mut compact.photos {
            let _ = self.cache_thumbnail(&photo.id, &photo.thumbnail);
            photo.thumbnail = String::new();
        }
        for record in compact.catalog_history.undo.iter_mut().chain(&mut compact.catalog_history.redo) {
            for photo in &mut record.photos {
                let _ = self.cache_thumbnail(&photo.id, &photo.thumbnail);
                photo.thumbnail = String::new();
            }
        }
        let data = serde_json::to_vec(&compact).map_err(|e| format!("无法保存本机设置：{e}"))?;
        let mut temporary =
            NamedTempFile::new_in(&self.directory).map_err(|e| format!("无法写入本机设置：{e}"))?;
        temporary
            .write_all(&data)
            .map_err(|e| format!("无法写入本机设置：{e}"))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| format!("无法写入本机设置：{e}"))?;
        temporary
            .persist(&path)
            .map_err(|e| format!("无法提交本机设置：{}", e.error))?;
        Ok(())
    }

    pub fn own_settings_assets(&self, settings: &mut RenderSettings) -> Result<(), String> {
        if let Some(scene) = &mut settings.artwork {
            if let Some(frame) = &mut scene.frame_image {
                frame.image_path = self.copy_asset(Path::new(&frame.image_path), "png")?;
            }
            for sticker in &mut scene.stickers {
                if !sticker.image_path.starts_with("builtin:") {
                    sticker.image_path = self.copy_asset(Path::new(&sticker.image_path), "png")?;
                }
            }
        }
        if let Some(path) = settings
            .watermark
            .image_path
            .as_deref()
            .filter(|_| settings.watermark.kind == "image")
        {
            if !path.is_empty() {
                settings.watermark.image_path = Some(self.copy_asset(Path::new(path), "png")?);
            }
        }
        if let Some(path) = settings
            .watermark
            .font_path
            .as_deref()
            .filter(|_| settings.watermark.kind == "text")
        {
            if !path.is_empty() {
                let suffix = Path::new(path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("otf")
                    .to_ascii_lowercase();
                if !matches!(suffix.as_str(), "otf" | "ttf" | "ttc" | "otc") {
                    return Err("字体请选择 TTF、OTF 或 TTC 文件。".into());
                }
                settings.watermark.font_path = Some(self.copy_asset(Path::new(path), &suffix)?);
            }
        }
        Ok(())
    }

    pub fn resolve_missing_assets(&self, settings: &mut RenderSettings) -> Result<(), String> {
        let aliases = self
            .asset_aliases
            .lock()
            .map_err(|_| "无法读取水印素材缓存。")?;
        for path in [
            &mut settings.watermark.image_path,
            &mut settings.watermark.font_path,
        ] {
            if let Some(original) = path.as_ref().filter(|path| !Path::new(path).is_file()) {
                if let Some(owned) = aliases
                    .get(original)
                    .filter(|owned| Path::new(owned).is_file())
                {
                    *path = Some(owned.clone());
                }
            }
        }
        Ok(())
    }

    fn copy_asset(&self, source: &Path, suffix: &str) -> Result<String, String> {
        let directory = self.directory.join("assets");
        fs::create_dir_all(&directory).map_err(|e| format!("无法保存水印素材：{e}"))?;
        let source_key = source.to_string_lossy().into_owned();
        let canonical = match fs::canonicalize(source) {
            Ok(canonical) => canonical,
            Err(error) => {
                if let Some(owned) = self
                    .asset_aliases
                    .lock()
                    .map_err(|_| "无法读取水印素材缓存。")?
                    .get(&source_key)
                    .filter(|owned| Path::new(owned).is_file())
                {
                    return Ok(owned.clone());
                }
                return Err(format!("无法读取水印或字体素材：{error}"));
            }
        };
        let canonical_assets =
            fs::canonicalize(&directory).map_err(|e| format!("无法读取素材目录：{e}"))?;
        if canonical.starts_with(canonical_assets) {
            return Ok(canonical.to_string_lossy().into_owned());
        }
        let metadata = fs::metadata(&canonical).map_err(|e| format!("无法读取素材：{e}"))?;
        if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
            return Err("水印或字体素材必须是小于 64 MB 的文件。".into());
        }
        let data = fs::read(&canonical).map_err(|e| format!("无法读取素材：{e}"))?;
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let mut target = directory.join(format!("{:016x}.{suffix}", hasher.finish()));
        if target.exists() && fs::read(&target).ok().as_deref() != Some(data.as_slice()) {
            target = directory.join(format!("{}.{}", uuid::Uuid::new_v4(), suffix));
        }
        if !target.exists() {
            let mut temporary =
                NamedTempFile::new_in(&directory).map_err(|e| format!("无法保存素材：{e}"))?;
            temporary
                .write_all(&data)
                .map_err(|e| format!("无法保存素材：{e}"))?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|e| format!("无法保存素材：{e}"))?;
            match temporary.persist_noclobber(&target) {
                Ok(_) => (),
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(format!("无法保存素材：{}", e.error)),
            }
        }
        let owned = target.to_string_lossy().into_owned();
        self.asset_aliases
            .lock()
            .map_err(|_| "无法保存水印素材缓存。")?
            .insert(source_key, owned.clone());
        Ok(owned)
    }
}

fn read_state(path: &Path) -> Result<StoredState, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let state: StoredState = serde_json::from_reader(file).map_err(|e| e.to_string())?;
    if state.schema_version != 1 {
        return Err("设置文件版本不受支持".into());
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_round_trip_and_backup_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(directory.path().to_path_buf()).unwrap();
        let mut state = StoredState::default();
        storage.save(&state).unwrap();
        state.settings.border_percent = 9.0;
        storage.save(&state).unwrap();
        let (saved, warnings) = storage.load().unwrap();
        assert_eq!(saved.settings.border_percent, 9.0);
        assert!(warnings.is_empty());
        fs::write(directory.path().join("state.json"), b"{broken").unwrap();
        let (recovered, warnings) = storage.load().unwrap();
        assert_eq!(recovered.settings.border_percent, 5.0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn imported_asset_survives_source_removal() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(directory.path().join("state")).unwrap();
        let source = directory.path().join("我的水印.png");
        fs::write(&source, b"asset bytes").unwrap();
        let first = storage.copy_asset(&source, "png").unwrap();
        let second = storage.copy_asset(&source, "png").unwrap();
        assert_eq!(first, second);
        fs::remove_file(&source).unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"asset bytes");
        assert_eq!(storage.copy_asset(&source, "png").unwrap(), first);
    }
}
