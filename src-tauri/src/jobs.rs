use crate::app_models::{
    AppSnapshot, BatchSnapshot, ImportResult, JobItem, Photo, Preset, StoredState,
};
use crate::imaging;
use crate::model::RenderSettings;
use crate::storage::Storage;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub type ProgressSink = Arc<dyn Fn(BatchSnapshot) + Send + Sync>;

pub struct Backend {
    data: Mutex<StoredState>,
    storage: Storage,
    warnings: Mutex<Vec<String>>,
    image_gate: Mutex<()>,
    cancelled: AtomicBool,
    worker_active: AtomicBool,
    preview_generation: AtomicU64,
}

impl Backend {
    pub fn exported_image(&self, path: &str) -> Result<String, String> {
        let permitted = self.lock()?.batch.as_ref().is_some_and(|batch| {
            batch
                .items
                .iter()
                .any(|item| item.status == "completed" && item.output_path.as_deref() == Some(path))
        });
        if !permitted || !Path::new(path).is_file() {
            return Err("这张导出照片已移动或不属于当前任务。".into());
        }
        Ok(path.to_owned())
    }

    pub fn export_directory(&self) -> Result<String, String> {
        let directory = self
            .lock()?
            .batch
            .as_ref()
            .map(|batch| batch.output_directory.clone())
            .ok_or("还没有导出任务，请先选择照片并导出。")?;
        if !Path::new(&directory).is_dir() {
            return Err("导出文件夹已移动或无法访问。".into());
        }
        Ok(directory)
    }

    pub fn new(directory: PathBuf) -> Result<Arc<Self>, String> {
        let storage = Storage::new(directory)?;
        let (mut data, mut warnings) = storage.load()?;
        if let Some(batch) = &mut data.batch {
            if batch.status == "running" {
                batch.status = "interrupted".into();
                for item in &mut batch.items {
                    if item.status == "running" {
                        // A crash may happen after the atomic output commit but
                        // before the completed journal write. The intended path
                        // and dimensions were persisted before that commit.
                        let committed = item
                            .output_path
                            .as_ref()
                            .and_then(|path| {
                                let info = imaging::inspect_image(Path::new(path)).ok()?;
                                Some(
                                    item.width == Some(info.width)
                                        && item.height == Some(info.height),
                                )
                            })
                            .unwrap_or(false);
                        if committed {
                            item.status = "completed".into();
                        } else {
                            item.status = "pending".into();
                            item.output_path = None;
                            item.width = None;
                            item.height = None;
                        }
                    }
                }
                warnings.push("发现上次未完成的导出，可点击继续导出。".into());
                batch.revision = batch.revision.saturating_add(1);
            }
        }
        storage.save(&data)?;
        Ok(Arc::new(Self {
            data: Mutex::new(data),
            storage,
            warnings: Mutex::new(warnings),
            image_gate: Mutex::new(()),
            cancelled: AtomicBool::new(false),
            worker_active: AtomicBool::new(false),
            preview_generation: AtomicU64::new(0),
        }))
    }

    fn lock(&self) -> Result<MutexGuard<'_, StoredState>, String> {
        self.data
            .lock()
            .map_err(|_| "任务状态暂时无法访问，请重新打开软件。".into())
    }

    fn commit(&self, current: &mut StoredState, candidate: StoredState) -> Result<(), String> {
        self.storage.save(&candidate)?;
        *current = candidate;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<AppSnapshot, String> {
        // Restore only local cached thumbnails. Never open original photo paths
        // during workspace bootstrap: even File::open can wait indefinitely.
        let mut photos = self.lock()?.photos.clone();
        for photo in &mut photos {
            if photo.thumbnail.is_empty() {
                photo.thumbnail = self.storage.cached_thumbnail(&photo.id);
            }
        }
        let mut data = self.lock()?;
        for photo in photos {
            if let Some(stored) = data.photos.iter_mut().find(|p| p.id == photo.id) {
                if stored.thumbnail.is_empty() {
                    stored.thumbnail = photo.thumbnail;
                }
            }
        }
        Ok(AppSnapshot {
            settings: data.settings.clone(),
            presets: data.presets.clone(),
            frame_templates: data.frame_templates.clone(),
            photos: data.photos.clone(),
            batch: data.batch.clone(),
            warnings: self
                .warnings
                .lock()
                .map_err(|_| "无法读取本机提示。")?
                .clone(),
        })
    }

    pub fn import_photos(&self, paths: Vec<String>) -> Result<ImportResult, String> {
        let mut known: HashSet<String> = self
            .lock()?
            .photos
            .iter()
            .map(|p| path_key(Path::new(&p.path)))
            .collect();
        let mut photos = Vec::new();
        let mut errors = Vec::new();
        for raw in paths {
            let path = PathBuf::from(&raw);
            if !path.is_file() {
                if !path.is_dir() {
                    errors.push(format!("找不到文件：{}", display_name(&path)));
                }
                continue;
            }
            if !supported_input(&path) {
                continue;
            }
            let canonical = match fs::canonicalize(&path) {
                Ok(path) => path,
                Err(error) => {
                    errors.push(format!("{}：{error}", display_name(&path)));
                    continue;
                }
            };
            if !known.insert(path_key(&canonical)) {
                continue;
            }
            let result = (|| {
                let _image = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
                let info = imaging::inspect_image(&canonical)?;
                let thumbnail = imaging::thumbnail(&canonical, 180)?;
                Ok::<Photo, String>(Photo {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: canonical.to_string_lossy().into_owned(),
                    name: display_name(&canonical),
                    width: info.width,
                    height: info.height,
                    thumbnail,
                })
            })();
            match result {
                Ok(photo) => photos.push(photo),
                Err(error) => errors.push(format!("{}：{error}", display_name(&path))),
            }
        }
        let mut data = self.lock()?;
        // Another import may have finished while decoding; deduplicate again
        // under the state lock before publishing its additions.
        let existing: HashSet<String> = data
            .photos
            .iter()
            .map(|p| path_key(Path::new(&p.path)))
            .collect();
        photos.retain(|photo| !existing.contains(&path_key(Path::new(&photo.path))));
        let mut candidate = data.clone();
        candidate.photos.extend(photos.clone());
        self.commit(&mut data, candidate)?;
        Ok(ImportResult { photos, errors })
    }

    pub fn remove_photos(&self, ids: Vec<String>) -> Result<(), String> {
        let ids: HashSet<String> = ids.into_iter().collect();
        let mut data = self.lock()?;
        let mut candidate = data.clone();
        candidate.photos.retain(|photo| !ids.contains(&photo.id));
        self.commit(&mut data, candidate)?;
        self.storage.forget_thumbnails(&ids);
        let empty = data.photos.is_empty();
        drop(data);
        if empty {
            imaging::clear_image_cache();
        }
        Ok(())
    }

    pub fn preview(
        &self,
        photo_id: &str,
        settings: &RenderSettings,
    ) -> Result<crate::model::PreviewResult, String> {
        let (path, needs_thumbnail) = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .map(|p| (p.path.clone(), p.thumbnail.is_empty()))
            .ok_or("请先选择一张照片。")?;
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let _image = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        if generation != self.preview_generation.load(Ordering::Acquire) {
            return Err("预览设置已更新。".into());
        }
        let mut settings = settings.clone();
        self.storage.resolve_missing_assets(&mut settings)?;
        let mut result = imaging::preview(Path::new(&path), &settings, 1400)?;
        if needs_thumbnail {
            // A successful preview already decoded this source. Backfill the
            // migration cache lazily, never as a prerequisite for app startup.
            if let Ok(thumbnail) = imaging::thumbnail(Path::new(&path), 180) {
                let _ = self.storage.cache_thumbnail(photo_id, &thumbnail);
                if let Some(photo) = self.lock()?.photos.iter_mut().find(|p| p.id == photo_id) {
                    photo.thumbnail = thumbnail.clone();
                }
                result.source_thumbnail = Some(thumbnail);
            }
        }
        Ok(result)
    }

    pub fn sample_color(
        &self,
        photo_id: &str,
        x: u32,
        y: u32,
    ) -> Result<crate::model::ColorSample, String> {
        let path = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .map(|p| p.path.clone())
            .ok_or("请先选择照片")?;
        let _image = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        imaging::sample_color(Path::new(&path), x, y)
    }

    pub fn sampler_image(
        &self,
        photo_id: &str,
        x: Option<u32>,
        y: Option<u32>,
    ) -> Result<crate::model::SampleImage, String> {
        let path = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .map(|p| p.path.clone())
            .ok_or("请先选择照片")?;
        let _guard = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        imaging::sampler_image(Path::new(&path), x, y)
    }
    pub fn watermark_detail(
        &self,
        photo_id: &str,
        settings: &RenderSettings,
    ) -> Result<crate::model::WatermarkDetail, String> {
        let photo = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .cloned()
            .ok_or("请先选择照片")?;
        let mut settings = settings.clone();
        self.storage.resolve_missing_assets(&mut settings)?;
        let _guard = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        imaging::watermark_detail(Path::new(&photo.path), &settings)
    }

    pub fn design_geometry(
        &self,
        photo_id: &str,
        settings: &RenderSettings,
    ) -> Result<crate::art_ai::DesignGeometry, String> {
        let p = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .cloned()
            .ok_or("请先选择照片")?;
        let mut safe = settings.clone();
        safe.watermark.kind = "none".into();
        imaging::validate_settings(&safe)?;
        let l = crate::layout::calculate(p.width, p.height, &safe)?;
        if l.x == 0 && l.y == 0 {
            return Err("当前没有留白区域，请先增加边框比例或调整最终比例".into());
        }
        let norm =
            |value: u32, total: u32| ((f64::from(value) / f64::from(total)) * 1000.).round() as u32;
        Ok(crate::art_ai::DesignGeometry {
            canvas_width: l.width,
            canvas_height: l.height,
            border_percent: settings.border_percent,
            photo_bounds: crate::model::PixelRect {
                x: norm(l.x, l.width),
                y: norm(l.y, l.height),
                width: norm(l.photo_width, l.width),
                height: norm(l.photo_height, l.height),
            },
        })
    }

    pub fn analysis_image(&self, photo_id: &str) -> Result<(String, u32, u32), String> {
        let path = self
            .lock()?
            .photos
            .iter()
            .find(|p| p.id == photo_id)
            .map(|p| p.path.clone())
            .ok_or("请先选择照片")?;
        let _guard = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        imaging::analysis_image(Path::new(&path))
    }

    pub fn save_settings(&self, mut settings: RenderSettings) -> Result<(), String> {
        self.storage.own_settings_assets(&mut settings)?;
        imaging::validate_settings(&settings)?;
        let mut data = self.lock()?;
        let mut candidate = data.clone();
        candidate.settings = settings;
        self.commit(&mut data, candidate)
    }

    pub fn save_preset(
        &self,
        name: String,
        mut settings: RenderSettings,
    ) -> Result<Preset, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err("预设名称请填写 1–60 个字符。".into());
        }
        self.storage.own_settings_assets(&mut settings)?;
        imaging::validate_settings(&settings)?;
        let mut data = self.lock()?;
        let existing = data.presets.iter().position(|p| p.name == name);
        let preset = Preset {
            id: existing
                .map(|i| data.presets[i].id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: name.into(),
            settings,
        };
        let mut candidate = data.clone();
        if let Some(i) = existing {
            candidate.presets[i] = preset.clone();
        } else {
            candidate.presets.push(preset.clone());
        }
        self.commit(&mut data, candidate)?;
        Ok(preset)
    }

    pub fn save_frame_template(
        &self,
        name: &str,
        settings: &RenderSettings,
    ) -> Result<crate::app_models::FrameTemplate, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err("模板名称应为 1–60 个字符".into());
        }
        let mut style = RenderSettings {
            ratio: settings.ratio.clone(),
            border_percent: settings.border_percent,
            frame_color: settings.frame_color.clone(),
            decoration: settings.decoration.clone(),
            artwork: settings.artwork.clone(),
            ..RenderSettings::default()
        };
        self.storage.own_settings_assets(&mut style)?;
        imaging::validate_settings(&style)?;
        let thumbnail = imaging::frame_thumbnail(&style)?;
        let mut data = self.lock()?;
        if data.frame_templates.len() >= 100 {
            return Err("最多保存 100 个边框模板，请先删除不用的模板".into());
        }
        let mut label = name.to_string();
        let mut n = 2;
        while data.frame_templates.iter().any(|t| t.name == label) {
            label = format!("{name} ({n})");
            n += 1;
        }
        let template = crate::app_models::FrameTemplate {
            id: uuid::Uuid::new_v4().to_string(),
            name: label,
            settings: style,
            thumbnail,
        };
        let mut candidate = data.clone();
        candidate.frame_templates.push(template.clone());
        self.commit(&mut data, candidate)?;
        Ok(template)
    }
    pub fn delete_frame_template(&self, id: &str) -> Result<(), String> {
        let mut data = self.lock()?;
        let mut candidate = data.clone();
        candidate.frame_templates.retain(|t| t.id != id);
        self.commit(&mut data, candidate)
    }

    pub fn delete_preset(&self, id: &str) -> Result<(), String> {
        let mut data = self.lock()?;
        let mut candidate = data.clone();
        candidate.presets.retain(|preset| preset.id != id);
        self.commit(&mut data, candidate)
    }

    pub fn start_batch(
        self: &Arc<Self>,
        output_directory: String,
        mut settings: RenderSettings,
        photo_ids: Vec<String>,
        sink: ProgressSink,
    ) -> Result<BatchSnapshot, String> {
        self.storage.own_settings_assets(&mut settings)?;
        imaging::validate_settings(&settings)?;
        let directory = prepare_output_directory(&output_directory)?;
        self.claim_worker()?;
        let result = (|| {
            let mut data = self.lock()?;
            let ids: HashSet<String> = photo_ids.into_iter().collect();
            let items: Vec<JobItem> = data
                .photos
                .iter()
                .filter(|p| ids.contains(&p.id))
                .map(|p| JobItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    photo_id: p.id.clone(),
                    name: p.name.clone(),
                    source_path: p.path.clone(),
                    status: "pending".into(),
                    output_path: None,
                    error: None,
                    width: None,
                    height: None,
                })
                .collect();
            if items.is_empty() {
                return Err("请先导入并选择要导出的照片。".into());
            }
            let batch = BatchSnapshot {
                id: uuid::Uuid::new_v4().to_string(),
                revision: 0,
                status: "running".into(),
                output_directory: directory.to_string_lossy().into_owned(),
                settings,
                items,
                error: None,
            };
            let mut candidate = data.clone();
            candidate.batch = Some(batch.clone());
            self.commit(&mut data, candidate)?;
            Ok(batch)
        })();
        self.launch_result(result, sink)
    }

    pub fn resume_batch(
        self: &Arc<Self>,
        failed_only: bool,
        sink: ProgressSink,
    ) -> Result<BatchSnapshot, String> {
        self.claim_worker()?;
        let result = (|| {
            let mut data = self.lock()?;
            let mut candidate = data.clone();
            let batch = candidate.batch.as_mut().ok_or("没有可恢复的导出任务。")?;
            prepare_output_directory(&batch.output_directory)?;
            imaging::validate_settings(&batch.settings)?;
            let mut count = 0;
            for item in &mut batch.items {
                let restart = if failed_only {
                    item.status == "failed"
                } else {
                    matches!(item.status.as_str(), "pending" | "cancelled" | "running")
                };
                if restart {
                    item.status = "pending".into();
                    item.error = None;
                    item.output_path = None;
                    item.width = None;
                    item.height = None;
                    count += 1;
                }
            }
            if count == 0 {
                return Err(if failed_only {
                    "没有需要重试的失败照片。"
                } else {
                    "没有未完成的照片。"
                }
                .into());
            }
            batch.status = "running".into();
            batch.error = None;
            batch.revision = batch.revision.saturating_add(1);
            let snapshot = batch.clone();
            self.commit(&mut data, candidate)?;
            Ok(snapshot)
        })();
        self.launch_result(result, sink)
    }

    pub fn cancel_batch(&self) -> Result<(), String> {
        if self.worker_active.load(Ordering::Acquire) {
            self.cancelled.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn claim_worker(&self) -> Result<(), String> {
        self.worker_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "已有导出任务正在进行。")?;
        self.cancelled.store(false, Ordering::Release);
        Ok(())
    }

    fn launch_result(
        self: &Arc<Self>,
        result: Result<BatchSnapshot, String>,
        sink: ProgressSink,
    ) -> Result<BatchSnapshot, String> {
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.worker_active.store(false, Ordering::Release);
                return Err(error);
            }
        };
        let backend = Arc::clone(self);
        let worker_sink = Arc::clone(&sink);
        if let Err(error) = std::thread::Builder::new()
            .name("liubai-export".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    backend.run_worker(&worker_sink)
                }));
                let error = match outcome {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(_) => Some("图片处理意外中断，已保留任务，可重试未完成照片。".into()),
                };
                if let Some(error) = error {
                    backend.interrupt_worker(error, &worker_sink);
                }
                imaging::clear_image_cache();
                backend.worker_active.store(false, Ordering::Release);
            })
        {
            self.worker_active.store(false, Ordering::Release);
            self.interrupt_worker(format!("无法启动导出任务：{error}"), &sink);
            return Err(format!("无法启动导出任务：{error}"));
        }
        // The returned value is the transaction snapshot, even if the worker
        // has already progressed. Live events carry newer snapshots.
        Ok(snapshot)
    }

    fn run_worker(&self, sink: &ProgressSink) -> Result<(), String> {
        loop {
            let (next, snapshot) = {
                let mut data = self.lock()?;
                let batch = data.batch.as_mut().ok_or("导出任务不存在。")?;
                if self.cancelled.load(Ordering::Acquire) {
                    for item in &mut batch.items {
                        if item.status == "pending" {
                            item.status = "cancelled".into();
                        }
                    }
                    batch.status = "cancelled".into();
                    batch.revision = batch.revision.saturating_add(1);
                    let snapshot = batch.clone();
                    self.storage.save(&data)?;
                    (None, snapshot)
                } else if let Some(index) =
                    batch.items.iter().position(|item| item.status == "pending")
                {
                    batch.items[index].status = "running".into();
                    batch.items[index].error = None;
                    batch.revision = batch.revision.saturating_add(1);
                    let item = batch.items[index].clone();
                    let settings = batch.settings.clone();
                    let directory = PathBuf::from(&batch.output_directory);
                    let snapshot = batch.clone();
                    self.storage.save(&data)?;
                    (Some((index, item, settings, directory)), snapshot)
                } else {
                    batch.status = if batch.items.iter().any(|item| item.status == "cancelled") {
                        "cancelled"
                    } else {
                        "completed"
                    }
                    .into();
                    batch.revision = batch.revision.saturating_add(1);
                    let snapshot = batch.clone();
                    self.storage.save(&data)?;
                    (None, snapshot)
                }
            };
            sink(snapshot);
            let Some((index, item, settings, directory)) = next else {
                return Ok(());
            };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.export_one(index, &item, &settings, &directory)
            }));
            let outcome = match outcome {
                Ok(result) => result,
                Err(_) => Err("这张图片处理异常，请检查文件后重试。".into()),
            };
            let mut data = self.lock()?;
            let batch = data.batch.as_mut().ok_or("导出任务不存在。")?;
            match outcome {
                Ok((path, width, height)) => {
                    batch.items[index].status = "completed".into();
                    batch.items[index].output_path = Some(path.to_string_lossy().into_owned());
                    batch.items[index].width = Some(width);
                    batch.items[index].height = Some(height);
                    batch.items[index].error = None;
                }
                Err(error) => {
                    batch.items[index].status = "failed".into();
                    batch.items[index].error = Some(error);
                    batch.items[index].output_path = None;
                }
            }
            batch.revision = batch.revision.saturating_add(1);
            let snapshot = batch.clone();
            self.storage.save(&data)?;
            drop(data);
            sink(snapshot);
        }
    }

    fn export_one(
        &self,
        index: usize,
        item: &JobItem,
        settings: &RenderSettings,
        directory: &Path,
    ) -> Result<(PathBuf, u32, u32), String> {
        let _image = self.image_gate.lock().unwrap_or_else(|p| p.into_inner());
        let mut temporary = tempfile::Builder::new()
            .prefix(".liubai-")
            .suffix(".tmp")
            .tempfile_in(directory)
            .map_err(|e| format!("无法创建导出文件，请检查目录权限与磁盘空间：{e}"))?;
        let (width, height) =
            imaging::export_image(Path::new(&item.source_path), settings, temporary.path())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| format!("无法完整保存照片：{e}"))?;
        let extension = if settings.export.format == "png" {
            "png"
        } else {
            "jpg"
        };
        let stem = safe_stem(Path::new(&item.name));
        for number in 1..100_000u32 {
            let suffix = if number == 1 {
                String::new()
            } else {
                format!("-{number}")
            };
            let target = directory.join(format!("{stem}-framed{suffix}.{extension}"));
            if target.exists() {
                continue;
            }
            {
                let mut data = self.lock()?;
                let batch = data.batch.as_mut().ok_or("导出任务不存在。")?;
                batch.items[index].output_path = Some(target.to_string_lossy().into_owned());
                batch.items[index].width = Some(width);
                batch.items[index].height = Some(height);
                batch.revision = batch.revision.saturating_add(1);
                self.storage.save(&data)?;
            }
            // tempfile uses a platform-specific no-replace commit (rename
            // where available), so a concurrent writer cannot be overwritten.
            match temporary.persist_noclobber(&target) {
                Ok(_) => return Ok((target, width, height)),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    temporary = error.file;
                }
                Err(error) => {
                    return Err(format!(
                        "无法提交照片，请检查磁盘与目录权限：{}",
                        error.error
                    ))
                }
            }
        }
        Err("相同文件名太多，请选择另一个导出目录。".into())
    }

    fn interrupt_worker(&self, error: String, sink: &ProgressSink) {
        if let Ok(mut data) = self.lock() {
            if let Some(batch) = &mut data.batch {
                batch.status = "interrupted".into();
                batch.error = Some(error);
                for item in &mut batch.items {
                    if item.status == "running" {
                        item.status = "pending".into();
                    }
                }
                batch.revision = batch.revision.saturating_add(1);
                let snapshot = batch.clone();
                let _ = self.storage.save(&data);
                drop(data);
                sink(snapshot);
            }
        }
    }
}

fn prepare_output_directory(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("请选择导出目录。".into());
    }
    let directory = PathBuf::from(raw);
    fs::create_dir_all(&directory).map_err(|e| format!("无法使用导出目录：{e}"))?;
    let directory = fs::canonicalize(&directory).map_err(|e| format!("无法使用导出目录：{e}"))?;
    let _probe =
        tempfile::NamedTempFile::new_in(&directory).map_err(|e| format!("导出目录不可写：{e}"))?;
    Ok(directory)
}

fn supported_input(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "heic" | "heif"
            )
        })
        .unwrap_or(false)
}

fn path_key(path: &Path) -> String {
    let key = fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    if cfg!(target_os = "windows") {
        key.to_lowercase()
    } else {
        key
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn safe_stem(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let cleaned: String = stem
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let mut trimmed = cleaned
        .trim_matches(|ch: char| ch == '.' || ch.is_whitespace())
        .to_string();
    while trimmed.len() > 120 {
        trimmed.pop();
    }
    if trimmed.is_empty() {
        trimmed = "photo".into();
    }
    trimmed
}

#[cfg(test)]
mod tests {
    #[test]
    fn successful_preview_backfills_a_missing_legacy_thumbnail_cache() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("old-photo.png");
        image::RgbImage::from_pixel(120, 80, image::Rgb([40, 120, 180]))
            .save(&source)
            .unwrap();
        let state = dir.path().join("state");
        let backend = Backend::new(state.clone()).unwrap();
        let added = backend
            .import_photos(vec![source.to_string_lossy().into_owned()])
            .unwrap();
        let id = &added.photos[0].id;
        drop(backend);
        fs::remove_dir_all(state.join("thumbnails")).unwrap();
        let backend = Backend::new(state.clone()).unwrap();
        assert!(backend.snapshot().unwrap().photos[0].thumbnail.is_empty());
        let result = backend.preview(id, &RenderSettings::default()).unwrap();
        assert!(result
            .source_thumbnail
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert!(!backend.storage.cached_thumbnail(id).is_empty());
        drop(backend);
        fs::remove_file(source).unwrap();
        assert!(!Backend::new(state).unwrap().snapshot().unwrap().photos[0]
            .thumbnail
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn startup_restores_cached_thumbnails_without_opening_blocked_photo_sources() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let source = dir.path().join("照片.png");
        image::RgbImage::from_pixel(120, 80, image::Rgb([40, 120, 180]))
            .save(&source)
            .unwrap();
        let backend = Backend::new(state.clone()).unwrap();
        let added = backend
            .import_photos(vec![source.to_string_lossy().into_owned()])
            .unwrap();
        assert_eq!(added.photos.len(), 1);
        drop(backend);
        fs::remove_file(&source).unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(&source)
            .status()
            .unwrap()
            .success());
        for cached in [true, false] {
            if !cached {
                fs::remove_dir_all(state.join("thumbnails")).unwrap();
            }
            let backend = Backend::new(state.clone()).unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = backend.clone();
            std::thread::spawn(move || {
                let _ = tx.send(worker.snapshot());
            });
            let snapshot = rx
                .recv_timeout(std::time::Duration::from_millis(500))
                .expect("Restoring the workspace must not open an original photo that blocks")
                .unwrap();
            assert_eq!(snapshot.photos.len(), 1);
            assert_eq!(!snapshot.photos[0].thumbnail.is_empty(), cached);
            backend
                .remove_photos(vec![snapshot.photos[0].id.clone()])
                .unwrap();
            if cached {
                // Restore the journal for the cache-missing migration case.
                let mut stored = StoredState::default();
                stored.photos = added.photos.clone();
                backend.storage.save(&stored).unwrap();
            }
        }
    }

    #[test]
    fn frame_template_survives_restart_and_excludes_photo_watermark_and_export_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let backend = Backend::new(state.clone()).unwrap();
        let mut s = RenderSettings::default();
        s.frame_color = "#D8E0C8".into();
        s.border_percent = 12.;
        s.ratio = Some(crate::model::Ratio {
            width: 4.,
            height: 5.,
        });
        s.watermark.kind = "text".into();
        s.watermark.text = "私人的署名".into();
        s.export.long_edge = Some(1200);
        s.artwork = Some(crate::artwork::ArtScene {
            elements: vec![crate::artwork::ArtElement {
                kind: "leaf".into(),
                x: 35.,
                y: 35.,
                width: 30.,
                height: 30.,
                rotation: 20.,
                color: "#637A50".into(),
                opacity: 0.8,
                stroke_width: 2.,
                filled: false,
                points: vec![],
            }],
            ..Default::default()
        });
        let before = backend.snapshot().unwrap();
        let saved = backend.save_frame_template("中文模板", &s).unwrap();
        assert_eq!(saved.settings.watermark.kind, "none");
        assert_eq!(saved.settings.watermark.text, "");
        assert_eq!(saved.settings.export.long_edge, None);
        assert!(saved.thumbnail.starts_with("data:image/jpeg;base64,"));
        let duplicate = backend.save_frame_template("中文模板", &s).unwrap();
        assert_eq!(duplicate.name, "中文模板 (2)");
        drop(backend);
        let reopened = Backend::new(state).unwrap();
        let current = reopened.snapshot().unwrap();
        assert_eq!(current.frame_templates.len(), 2);
        assert_eq!(
            current.frame_templates[0]
                .settings
                .artwork
                .as_ref()
                .unwrap()
                .elements[0]
                .kind,
            "leaf"
        );
        assert_eq!(
            current.settings.watermark.text,
            before.settings.watermark.text
        );
        assert_eq!(current.photos.len(), before.photos.len());
        reopened.delete_frame_template(&saved.id).unwrap();
        assert_eq!(reopened.snapshot().unwrap().frame_templates.len(), 1);
    }

    use super::*;
    use image::{Rgb, RgbImage};
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn wait_for_worker(backend: &Backend) {
        let start = Instant::now();
        while backend.worker_active.load(Ordering::Acquire) {
            assert!(start.elapsed() < Duration::from_secs(60), "batch timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn recovered_operation_gate_keeps_the_next_photo_usable_after_a_task_panic() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recover.png");
        RgbImage::from_pixel(40, 30, Rgb([25, 80, 150]))
            .save(&path)
            .unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _gate = backend.image_gate.lock().unwrap();
            panic!("injected task interruption");
        }));
        let imported = backend
            .import_photos(vec![path.to_string_lossy().into_owned()])
            .unwrap();
        assert!(imported.errors.is_empty());
        let id = &imported.photos[0].id;
        assert!(backend.preview(id, &RenderSettings::default()).is_ok());
        assert!(backend.sample_color(id, 10, 10).is_ok());
    }
    #[test]
    fn fifty_photos_dedupe_fail_isolation_collision_and_no_source_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("中文照片");
        fs::create_dir(&source).unwrap();
        let mut paths = Vec::new();
        let mut originals = Vec::new();
        for index in 0..50 {
            let path = source.join(format!("照片-{index}.png"));
            let (w, h) = if index % 2 == 0 { (40, 30) } else { (30, 40) };
            RgbImage::from_pixel(w, h, Rgb([index as u8, 80, 140]))
                .save(&path)
                .unwrap();
            originals.push((path.clone(), fs::read(&path).unwrap()));
            paths.push(path.to_string_lossy().into_owned());
        }
        let bad = source.join("损坏.jpg");
        fs::write(&bad, b"not an image").unwrap();
        let text = source.join("说明.txt");
        fs::write(&text, b"text").unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        paths.extend([
            paths[0].clone(),
            bad.to_string_lossy().into_owned(),
            text.to_string_lossy().into_owned(),
            source.to_string_lossy().into_owned(),
        ]);
        let imported = backend.import_photos(paths).unwrap();
        assert_eq!(imported.photos.len(), 50);
        assert_eq!(imported.errors.len(), 1);
        // A file can become unreadable after import. It must fail independently.
        fs::write(&originals[7].0, b"newly corrupted").unwrap();
        originals[7].1 = b"newly corrupted".to_vec();
        let output = directory.path().join("导出");
        fs::create_dir(&output).unwrap();
        let existing = output.join("照片-0-framed.jpg");
        fs::write(&existing, b"keep existing").unwrap();
        let events = Arc::new(AtomicUsize::new(0));
        let count = events.clone();
        let sink: ProgressSink = Arc::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        let ids = imported.photos.iter().map(|p| p.id.clone()).collect();
        backend
            .start_batch(
                output.to_string_lossy().into_owned(),
                RenderSettings::default(),
                ids,
                sink,
            )
            .unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert_eq!(batch.status, "completed");
        assert_eq!(
            batch
                .items
                .iter()
                .filter(|i| i.status == "completed")
                .count(),
            49
        );
        assert_eq!(
            batch.items.iter().filter(|i| i.status == "failed").count(),
            1
        );
        assert_eq!(fs::read(existing).unwrap(), b"keep existing");
        assert!(output.join("照片-0-framed-2.jpg").exists());
        for (path, bytes) in &originals {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
        assert!(events.load(Ordering::Relaxed) >= 51);
        RgbImage::from_pixel(40, 30, Rgb([80, 80, 140]))
            .save(&originals[7].0)
            .unwrap();
        backend.resume_batch(true, Arc::new(|_| {})).unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert!(batch.items.iter().all(|i| i.status == "completed"));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 51);
    }

    #[test]
    fn cancellation_and_restart_resume_without_reexporting_completed_items() {
        let directory = tempfile::tempdir().unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        let mut paths = Vec::new();
        for index in 0..8 {
            let path = directory.path().join(format!("{index}.png"));
            RgbImage::from_pixel(20, 20, Rgb([20, 40, 60]))
                .save(&path)
                .unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let photos = backend.import_photos(paths).unwrap().photos;
        let weak = Arc::downgrade(&backend);
        let sink: ProgressSink = Arc::new(move |batch| {
            if batch.items.iter().any(|i| i.status == "completed") {
                if let Some(backend) = weak.upgrade() {
                    backend.cancel_batch().unwrap();
                }
            }
        });
        let output = directory.path().join("out");
        backend
            .start_batch(
                output.to_string_lossy().into_owned(),
                RenderSettings::default(),
                photos.iter().map(|p| p.id.clone()).collect(),
                sink,
            )
            .unwrap();
        wait_for_worker(&backend);
        let snapshot = backend.snapshot().unwrap();
        assert_eq!(snapshot.batch.as_ref().unwrap().status, "cancelled");
        assert_eq!(
            snapshot
                .batch
                .unwrap()
                .items
                .iter()
                .filter(|i| i.status == "completed")
                .count(),
            1
        );
        drop(backend);
        let backend = Backend::new(directory.path().join("state")).unwrap();
        backend.resume_batch(false, Arc::new(|_| {})).unwrap();
        wait_for_worker(&backend);
        assert!(backend
            .snapshot()
            .unwrap()
            .batch
            .unwrap()
            .items
            .iter()
            .all(|i| i.status == "completed"));
        assert_eq!(fs::read_dir(output).unwrap().count(), 8);
    }

    #[test]
    fn recovery_recognizes_output_committed_before_journal_completion() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(directory.path().join("state")).unwrap();
        let output = directory.path().join("committed.png");
        RgbImage::from_pixel(22, 24, Rgb([0, 0, 0]))
            .save(&output)
            .unwrap();
        let mut state = StoredState::default();
        state.batch = Some(BatchSnapshot {
            id: "batch".into(),
            revision: 0,
            status: "running".into(),
            output_directory: directory.path().to_string_lossy().into_owned(),
            settings: RenderSettings::default(),
            items: vec![JobItem {
                id: "item".into(),
                photo_id: "photo".into(),
                name: "source.png".into(),
                source_path: directory
                    .path()
                    .join("source.png")
                    .to_string_lossy()
                    .into_owned(),
                status: "running".into(),
                output_path: Some(output.to_string_lossy().into_owned()),
                error: None,
                width: Some(22),
                height: Some(24),
            }],
            error: None,
        });
        storage.save(&state).unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        assert_eq!(
            backend.snapshot().unwrap().batch.unwrap().items[0].status,
            "completed"
        );
    }

    #[test]
    fn running_batch_keeps_its_assets_and_settings_when_ui_changes() {
        let directory = tempfile::tempdir().unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        let logo = directory.path().join("logo.png");
        image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 255, 0, 255]))
            .save(&logo)
            .unwrap();
        let mut paths = Vec::new();
        for index in 0..4 {
            let path = directory.path().join(format!("image-{index}.png"));
            RgbImage::from_pixel(100, 100, Rgb([200, 50, 50]))
                .save(&path)
                .unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let photos = backend.import_photos(paths).unwrap().photos;
        let mut settings = RenderSettings::default();
        settings.export.format = "png".into();
        settings.watermark.kind = "image".into();
        settings.watermark.image_path = Some(logo.to_string_lossy().into_owned());
        settings.watermark.opacity = 1.0;
        settings.watermark.size_percent = 10.0;
        settings.watermark.margin_percent = 0.0;
        settings.watermark.position = "top-left".into();
        let weak = Arc::downgrade(&backend);
        let once = AtomicBool::new(false);
        let sink: ProgressSink = Arc::new(move |batch| {
            if batch.items.iter().any(|item| item.status == "completed")
                && !once.swap(true, Ordering::AcqRel)
            {
                fs::remove_file(&logo).unwrap();
                let mut changed = RenderSettings::default();
                changed.frame_color = "#000000".into();
                weak.upgrade().unwrap().save_settings(changed).unwrap();
            }
        });
        backend
            .start_batch(
                directory
                    .path()
                    .join("output")
                    .to_string_lossy()
                    .into_owned(),
                settings,
                photos.iter().map(|p| p.id.clone()).collect(),
                sink,
            )
            .unwrap();
        wait_for_worker(&backend);
        let snapshot = backend.snapshot().unwrap();
        assert_eq!(snapshot.settings.frame_color, "#000000");
        for item in snapshot.batch.unwrap().items {
            assert_eq!(item.status, "completed");
            let image = image::open(item.output_path.unwrap()).unwrap().to_rgb8();
            assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255]);
            assert_eq!(image.get_pixel(5, 5).0, [0, 255, 0]);
        }
    }

    #[test]
    fn saved_png_is_usable_through_original_ui_settings_after_source_is_removed() {
        let directory = tempfile::tempdir().unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        let source = directory.path().join("photo.png");
        RgbImage::from_pixel(100, 100, Rgb([200, 30, 30]))
            .save(&source)
            .unwrap();
        let photo = backend
            .import_photos(vec![source.to_string_lossy().into_owned()])
            .unwrap()
            .photos
            .remove(0);
        let logo = directory.path().join("original-logo.png");
        image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 255, 0, 255]))
            .save(&logo)
            .unwrap();
        let mut ui_settings = RenderSettings::default();
        ui_settings.export.format = "png".into();
        ui_settings.watermark.kind = "image".into();
        ui_settings.watermark.image_path = Some(logo.to_string_lossy().into_owned());
        ui_settings.watermark.opacity = 1.0;
        ui_settings.watermark.size_percent = 10.0;
        ui_settings.watermark.margin_percent = 0.0;
        ui_settings.watermark.position = "top-left".into();
        backend.save_settings(ui_settings.clone()).unwrap();
        let saved = backend.snapshot().unwrap().settings;
        assert_ne!(saved.watermark.image_path, ui_settings.watermark.image_path);
        fs::remove_file(logo).unwrap();
        let preview = backend.preview(&photo.id, &ui_settings).unwrap();
        assert_eq!((preview.width, preview.height), (110, 110));
        assert!(preview.data_url.starts_with("data:image/jpeg;base64,"));
        backend.save_settings(ui_settings.clone()).unwrap();
        backend
            .save_preset("我的水印".into(), ui_settings.clone())
            .unwrap();
        backend
            .start_batch(
                directory.path().join("out").to_string_lossy().into_owned(),
                ui_settings,
                vec![photo.id],
                Arc::new(|_| {}),
            )
            .unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert_eq!(batch.items[0].status, "completed");
        let pixels = image::open(batch.items[0].output_path.as_ref().unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(pixels.get_pixel(5, 5).0, [0, 255, 0]);
    }

    #[test]
    fn retrying_failed_items_preserves_cancelled_items_for_later_resume() {
        let directory = tempfile::tempdir().unwrap();
        let backend = Backend::new(directory.path().join("state")).unwrap();
        let mut paths = Vec::new();
        for index in 0..6 {
            let path = directory.path().join(format!("{index}.png"));
            RgbImage::from_pixel(20, 20, Rgb([20, 40, 60]))
                .save(&path)
                .unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let photos = backend.import_photos(paths.clone()).unwrap().photos;
        fs::write(&paths[0], b"damaged after import").unwrap();
        let weak = Arc::downgrade(&backend);
        let sink: ProgressSink = Arc::new(move |batch| {
            if batch.items.iter().any(|item| item.status == "failed") {
                if let Some(backend) = weak.upgrade() {
                    backend.cancel_batch().unwrap();
                }
            }
        });
        let output = directory.path().join("out");
        backend
            .start_batch(
                output.to_string_lossy().into_owned(),
                RenderSettings::default(),
                photos.iter().map(|p| p.id.clone()).collect(),
                sink,
            )
            .unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert_eq!(batch.status, "cancelled");
        assert_eq!(
            batch
                .items
                .iter()
                .filter(|item| item.status == "failed")
                .count(),
            1
        );
        assert_eq!(
            batch
                .items
                .iter()
                .filter(|item| item.status == "cancelled")
                .count(),
            5
        );
        RgbImage::from_pixel(20, 20, Rgb([20, 40, 60]))
            .save(&paths[0])
            .unwrap();
        backend.resume_batch(true, Arc::new(|_| {})).unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert_eq!(batch.status, "cancelled");
        assert_eq!(
            batch
                .items
                .iter()
                .filter(|item| item.status == "completed")
                .count(),
            1
        );
        assert_eq!(
            batch
                .items
                .iter()
                .filter(|item| item.status == "cancelled")
                .count(),
            5
        );
        backend.resume_batch(false, Arc::new(|_| {})).unwrap();
        wait_for_worker(&backend);
        let batch = backend.snapshot().unwrap().batch.unwrap();
        assert_eq!(batch.status, "completed");
        assert!(batch.items.iter().all(|item| item.status == "completed"));
        assert_eq!(fs::read_dir(output).unwrap().count(), 6);
    }

    #[test]
    fn persistence_failure_keeps_previous_queue_settings_photos_and_presets() {
        let directory = tempfile::tempdir().unwrap();
        let state_directory = directory.path().join("state");
        let backend = Backend::new(state_directory.clone()).unwrap();
        let mut paths = Vec::new();
        for index in 0..3 {
            let path = directory.path().join(format!("{index}.png"));
            RgbImage::from_pixel(20, 20, Rgb([20, 40, 60]))
                .save(&path)
                .unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let photos = backend.import_photos(paths[..2].to_vec()).unwrap().photos;
        let preset = backend
            .save_preset("保留预设".into(), RenderSettings::default())
            .unwrap();
        let weak = Arc::downgrade(&backend);
        let sink: ProgressSink = Arc::new(move |batch| {
            if batch.items.iter().any(|item| item.status == "completed") {
                if let Some(backend) = weak.upgrade() {
                    backend.cancel_batch().unwrap();
                }
            }
        });
        let output = directory.path().join("out");
        backend
            .start_batch(
                output.to_string_lossy().into_owned(),
                RenderSettings::default(),
                photos.iter().map(|p| p.id.clone()).collect(),
                sink,
            )
            .unwrap();
        wait_for_worker(&backend);
        let previous = backend.snapshot().unwrap();
        assert_eq!(previous.batch.as_ref().unwrap().status, "cancelled");
        let journal = state_directory.join("state.json");
        let held_journal = state_directory.join("state-held.json");
        fs::rename(&journal, &held_journal).unwrap();
        // A directory at the journal path deterministically prevents the atomic
        // commit on both platforms, without relying on user permission bits.
        fs::create_dir(&journal).unwrap();
        assert!(backend
            .start_batch(
                output.to_string_lossy().into_owned(),
                RenderSettings::default(),
                photos.iter().map(|p| p.id.clone()).collect(),
                Arc::new(|_| {})
            )
            .is_err());
        assert!(!backend.worker_active.load(Ordering::Acquire));
        assert!(backend.resume_batch(false, Arc::new(|_| {})).is_err());
        assert!(!backend.worker_active.load(Ordering::Acquire));
        let mut changed = RenderSettings::default();
        changed.border_percent = 12.0;
        assert!(backend.save_settings(changed.clone()).is_err());
        assert!(backend.save_preset("新预设".into(), changed).is_err());
        assert!(backend.delete_preset(&preset.id).is_err());
        assert!(backend.remove_photos(vec![photos[0].id.clone()]).is_err());
        assert!(backend.import_photos(vec![paths[2].clone()]).is_err());
        let current = backend.snapshot().unwrap();
        assert_eq!(
            current.settings.border_percent,
            previous.settings.border_percent
        );
        assert_eq!(current.photos.len(), 2);
        assert_eq!(current.presets.len(), 1);
        assert_eq!(current.presets[0].id, preset.id);
        assert_eq!(
            serde_json::to_value(&current.batch).unwrap(),
            serde_json::to_value(&previous.batch).unwrap()
        );
        fs::remove_dir(&journal).unwrap();
        fs::rename(held_journal, journal).unwrap();
        backend.resume_batch(false, Arc::new(|_| {})).unwrap();
        wait_for_worker(&backend);
        assert!(backend
            .snapshot()
            .unwrap()
            .batch
            .unwrap()
            .items
            .iter()
            .all(|item| item.status == "completed"));
        assert_eq!(fs::read_dir(output).unwrap().count(), 2);
    }
}
