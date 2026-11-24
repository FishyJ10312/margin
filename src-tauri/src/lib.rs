// Rust image buffers use a dedicated allocator: macOS retained nearly 1 GiB of
// empty large malloc regions across 24 exports despite a ~142 MiB live heap.
// Foreign C/OS allocations keep their own allocator; no cross-allocator frees.
#[global_allocator]
static IMAGE_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app_models;
pub mod art_ai;
pub mod art_direction;
pub mod artwork;
pub mod backdrop;
pub mod camera_info;
pub mod chatgpt_auth;
#[cfg(unix)]
mod chatgpt_file_vault;
mod chatgpt_stream;
mod chatgpt_vault;
pub mod codex_image;
pub mod decoration;
pub mod designs;
pub mod film;
pub mod frame_art;
mod frame_opening;
pub mod generated_art;
pub mod image_api;
pub mod imaging;
mod jobs;
mod catalog_history;
pub mod layout;
pub mod model;
pub mod mcp;
pub mod pi_bridge;
mod refinement;
pub mod stickers;
mod storage;
pub mod source_io;
#[cfg(all(test, unix))]
mod test_support;

use app_models::{AppSnapshot, BatchSnapshot, ImportResult, Preset};
use jobs::{Backend, ProgressSink};
use model::{PreviewResult, RenderSettings};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type BackendState<'a> = State<'a, Arc<Backend>>;

async fn blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("操作意外中断，请重试：{error}"))?
}

fn progress_sink(app: AppHandle) -> ProgressSink {
    Arc::new(move |snapshot| {
        if let Err(error) = app.emit("batch-progress", snapshot) {
            eprintln!("无法通知导出进度：{error}");
        }
    })
}

#[tauri::command]
async fn get_state(state: BackendState<'_>) -> Result<AppSnapshot, String> {
    let backend = state.inner().clone();
    blocking(move || backend.snapshot()).await
}

#[tauri::command]
async fn get_agent_status(state: State<'_, Arc<mcp::AgentService>>) -> Result<mcp::AgentStatus, String> { Ok(state.status().await) }

#[tauri::command]
async fn set_agent_access(enabled: bool, allow_writes: bool, state: State<'_, Arc<mcp::AgentService>>) -> Result<mcp::AgentStatus, String> {
    state.inner().set_access(enabled, allow_writes).await
}

#[tauri::command]
fn complete_agent_command(request_id: String, result: Option<serde_json::Value>, error: Option<String>, state: State<'_, Arc<mcp::AgentService>>) -> Result<(), String> {
    state.complete(request_id, result, error)
}

#[tauri::command]
fn agent_window_ready(ready: bool, state: State<'_, Arc<mcp::AgentService>>) { state.mark_ready(ready); }

#[tauri::command]
async fn get_frame_template_previews(
    state: BackendState<'_>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let backend = state.inner().clone();
    blocking(move || backend.frame_template_previews()).await
}

#[tauri::command]
async fn import_photos(
    paths: Vec<String>,
    state: BackendState<'_>,
) -> Result<ImportResult, String> {
    let backend = state.inner().clone();
    blocking(move || backend.import_photos(paths)).await
}

#[tauri::command]
async fn remove_photos(ids: Vec<String>, selection: Option<app_models::PhotoSelection>, state: BackendState<'_>) -> Result<Option<app_models::PhotoRemovalReceipt>, String> {
    let backend = state.inner().clone();
    blocking(move || backend.remove_photos_with_selection(ids, selection)).await
}

#[tauri::command]
async fn undo_photo_removal(undo_id: String, state: BackendState<'_>) -> Result<app_models::RestoredPhotos, String> {
    let backend = state.inner().clone();
    blocking(move || backend.undo_photo_removal(undo_id)).await
}

#[tauri::command]
async fn redo_photo_removal(redo_id: String, selection: Option<app_models::PhotoSelection>, state: BackendState<'_>) -> Result<app_models::PhotoRemovalReceipt, String> {
    let backend = state.inner().clone();
    blocking(move || backend.redo_photo_removal_with_selection(redo_id, selection)).await
}

#[tauri::command]
async fn save_photo_selection(selection: app_models::PhotoSelection, catalog_revision: u64, state: BackendState<'_>) -> Result<bool, String> {
    let backend = state.inner().clone();
    blocking(move || backend.save_photo_selection(selection, catalog_revision)).await
}

#[tauri::command]
async fn get_preview(
    photo_id: String,
    settings: RenderSettings,
    state: BackendState<'_>,
) -> Result<PreviewResult, String> {
    let backend = state.inner().clone();
    blocking(move || backend.preview(&photo_id, &settings)).await
}

#[tauri::command]
async fn list_fonts() -> Result<Vec<model::FontOption>, String> {
    blocking(|| Ok(imaging::list_fonts())).await
}

#[tauri::command]
async fn sample_color(
    photo_id: String,
    x: u32,
    y: u32,
    state: BackendState<'_>,
) -> Result<model::ColorSample, String> {
    let backend = state.inner().clone();
    blocking(move || backend.sample_color(&photo_id, x, y)).await
}

#[tauri::command]
async fn get_sampler_image(
    photo_id: String,
    x: Option<u32>,
    y: Option<u32>,
    state: BackendState<'_>,
) -> Result<model::SampleImage, String> {
    let backend = state.inner().clone();
    blocking(move || backend.sampler_image(&photo_id, x, y)).await
}
#[tauri::command]
async fn set_photo_settings(
    updates: std::collections::HashMap<String, Option<RenderSettings>>,
    shared: Option<RenderSettings>,
    state: BackendState<'_>,
) -> Result<app_models::SettingsCommit, String> {
    let backend = state.inner().clone();
    blocking(move || backend.set_photo_settings(updates, shared)).await
}

#[tauri::command]
fn quick_frame_styles() -> Vec<RenderSettings> {
    std::iter::once(RenderSettings::default())
        .chain(["keyline", "scan-35mm"].into_iter().filter_map(designs::settings))
        .collect()
}

#[tauri::command]
async fn get_camera_info(
    photo_id: String,
    state: BackendState<'_>,
) -> Result<camera_info::CameraInfo, String> {
    let backend = state.inner().clone();
    blocking(move || backend.camera_info(&photo_id)).await
}
#[tauri::command]
async fn get_watermark_detail(
    photo_id: String,
    settings: RenderSettings,
    state: BackendState<'_>,
) -> Result<model::WatermarkDetail, String> {
    let backend = state.inner().clone();
    blocking(move || backend.watermark_detail(&photo_id, &settings)).await
}
#[tauri::command]
async fn get_ai_state(state: State<'_, Arc<art_ai::ArtAi>>) -> Result<art_ai::AiState, String> {
    let service = state.inner().clone();
    blocking(move || service.state()).await
}
#[tauri::command]
async fn get_chatgpt_state(
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<chatgpt_auth::ConnectionState, String> {
    let service = state.inner().clone();
    blocking(move || service.chatgpt.state()).await
}
#[tauri::command]
async fn get_pi_runtime(
    path: Option<String>,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<pi_bridge::RuntimeInfo, String> {
    let service = state.inner().clone();
    blocking(move || Ok(service.pi.info(path.as_deref()))).await
}
#[tauri::command]
async fn list_pi_models(
    path: Option<String>,
    provider: String,
    refresh: bool,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<pi_bridge::Catalog, String> {
    state.pi.catalog(path.as_deref(), &provider, refresh).await
}
fn pi_browser(app: AppHandle) -> pi_bridge::Browser {
    Arc::new(move |url| {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| "无法打开 Pi 授权页面".into())
    })
}
#[tauri::command]
fn open_pi_setup(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(
            "https://github.com/earendil-works/pi/tree/main/packages/coding-agent",
            None::<&str>,
        )
        .map_err(|_| "无法打开 Pi 安装说明".into())
}
#[tauri::command]
async fn connect_pi_subscription(
    app: AppHandle,
    path: Option<String>,
    provider: String,
    session_id: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<pi_bridge::Catalog, String> {
    let event_app = app.clone();
    let sink: pi_bridge::EventSink = Arc::new(move |e| {
        let _ = event_app.emit("pi-auth-event", e);
    });
    state
        .pi
        .login(
            path.as_deref(),
            &provider,
            &session_id,
            sink,
            pi_browser(app),
        )
        .await
}
#[tauri::command]
async fn answer_pi_prompt(
    session_id: String,
    prompt_id: String,
    value: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<(), String> {
    state.pi.answer(&session_id, &prompt_id, value).await
}
#[tauri::command]
fn cancel_pi_connection(session_id: String, state: State<'_, Arc<art_ai::ArtAi>>) {
    state.pi.cancel(Some(&session_id));
}
#[tauri::command]
fn reopen_pi_authorization(
    app: AppHandle,
    session_id: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<(), String> {
    state.pi.reopen(&session_id, pi_browser(app))
}
#[tauri::command]
async fn disconnect_pi_subscription(
    path: Option<String>,
    provider: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<pi_bridge::Catalog, String> {
    state.pi.logout(path.as_deref(), &provider).await
}
#[tauri::command]
async fn chatgpt_sign_in(
    app: AppHandle,
    account_id: Option<String>,
    consent: bool,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<chatgpt_auth::ConnectionState, String> {
    let opener = app.clone();
    let result = state
        .chatgpt
        .sign_in(account_id, consent, move |url| {
            opener
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|_| "无法打开官方登录页面，请检查默认浏览器".into())
        })
        .await;
    if result.is_ok() {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    result
}
#[tauri::command]
fn cancel_chatgpt_sign_in(state: State<'_, Arc<art_ai::ArtAi>>) {
    state.chatgpt.cancel_login();
}
#[tauri::command]
async fn select_chatgpt_account(
    account_id: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<chatgpt_auth::ConnectionState, String> {
    let service = state.inner().clone();
    blocking(move || service.chatgpt.select(&account_id)).await
}
#[tauri::command]
async fn list_chatgpt_models(
    account_id: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<Vec<chatgpt_auth::Model>, String> {
    state.chatgpt.models(&account_id).await
}
#[tauri::command]
async fn disconnect_chatgpt(
    account_id: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<chatgpt_auth::Disconnect, String> {
    state.cancel();
    state.chatgpt.disconnect(&account_id).await
}
#[tauri::command]
fn open_chatgpt_usage(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://chatgpt.com/settings/usage", None::<&str>)
        .map_err(|_| "无法打开 ChatGPT 用量设置".into())
}
#[tauri::command]
async fn prepare_ai_photo(
    photo_id: String,
    settings: Option<RenderSettings>,
    state: BackendState<'_>,
    ai: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<art_ai::AiPhoto, String> {
    let backend = state.inner().clone();
    let ai = ai.inner().clone();
    blocking(move || {
        let (data, w, h) = backend.analysis_image(&photo_id)?;
        if let Some(settings) = settings {
            let g = backend.design_geometry(&photo_id, &settings)?;
            ai.prepare_with_geometry(data, w, h, g)
        } else {
            ai.prepare(data, w, h)
        }
    })
    .await
}
#[tauri::command]
async fn list_builtin_frames() -> Result<Vec<designs::BuiltinFrame>, String> {
    blocking(designs::library).await
}
#[tauri::command]
async fn prepare_art_refinement(
    photo_id: String,
    settings: RenderSettings,
    state: BackendState<'_>,
    ai: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<art_ai::AiPhoto, String> {
    let backend = state.inner().clone();
    let ai = ai.inner().clone();
    blocking(move || {
        let (data, w, h) = backend.analysis_image(&photo_id)?;
        let geometry = backend.design_geometry(&photo_id, &settings)?;
        let mut safe = settings.clone();
        safe.watermark.kind = "none".into();
        safe.export.long_edge = Some(768);
        let before = backend.preview(&photo_id, &safe)?.data_url;
        let current = art_ai::FrameSuggestion {
            title: "当前配框".into(),
            rationale: String::new(),
            frame_color: settings.frame_color,
            border_percent: settings.border_percent,
            decoration: settings.decoration,
            artwork: settings.artwork.unwrap_or_default(),
        };
        ai.prepare_refinement(data, w, h, geometry, before, current)
    })
    .await
}
#[tauri::command]
async fn list_image_models(
    config: image_api::ImageConfig,
    api_key: String,
) -> Result<Vec<String>, String> {
    image_api::list_models(&config, &api_key).await
}
#[tauri::command]
async fn generate_handdrawn(
    token: String,
    config: art_ai::AiConfig,
    reference: bool,
    api_key: Option<String>,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<codex_image::GeneratedArt, String> {
    state
        .handdrawn_with_key(&token, config, reference, api_key.unwrap_or_default())
        .await
}
#[tauri::command]
async fn generate_complete_frame(
    token: String,
    config: art_ai::AiConfig,
    api_key: Option<String>,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<generated_art::GeneratedArt, String> {
    state
        .complete_frame(&token, config, api_key.unwrap_or_default())
        .await
}
#[tauri::command]
async fn generate_art_frames(
    token: String,
    config: art_ai::AiConfig,
    api_key: String,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<Vec<art_ai::FrameSuggestion>, String> {
    state.generate(&token, config, api_key).await
}
#[tauri::command]
fn save_ai_config(
    config: art_ai::AiConfig,
    state: State<'_, Arc<art_ai::ArtAi>>,
) -> Result<(), String> {
    state.save_config(&config)
}
#[tauri::command]
fn cancel_art_frames(state: State<'_, Arc<art_ai::ArtAi>>) {
    state.cancel();
}

#[tauri::command]
async fn save_settings(settings: RenderSettings, state: BackendState<'_>) -> Result<RenderSettings, String> {
    let backend = state.inner().clone();
    blocking(move || backend.save_settings(settings)).await
}

#[tauri::command]
async fn save_preset(
    name: String,
    settings: RenderSettings,
    state: BackendState<'_>,
) -> Result<Preset, String> {
    let backend = state.inner().clone();
    blocking(move || backend.save_preset(name, settings)).await
}

#[tauri::command]
async fn save_frame_template(
    name: String,
    settings: RenderSettings,
    state: BackendState<'_>,
) -> Result<app_models::FrameTemplate, String> {
    let backend = state.inner().clone();
    blocking(move || backend.save_frame_template(&name, &settings)).await
}
#[tauri::command]
async fn delete_frame_template(id: String, state: BackendState<'_>) -> Result<(), String> {
    let backend = state.inner().clone();
    blocking(move || backend.delete_frame_template(&id)).await
}

#[tauri::command]
async fn delete_preset(id: String, state: BackendState<'_>) -> Result<(), String> {
    let backend = state.inner().clone();
    blocking(move || backend.delete_preset(&id)).await
}

#[tauri::command]
async fn start_batch(
    output_directory: String,
    settings: RenderSettings,
    photo_ids: Vec<String>,
    uniform_ratio: Option<model::Ratio>,
    app: AppHandle,
    state: BackendState<'_>,
) -> Result<BatchSnapshot, String> {
    let backend = state.inner().clone();
    blocking(move || backend.start_batch_with_ratio(output_directory, settings, photo_ids, uniform_ratio, progress_sink(app)))
        .await
}

#[tauri::command]
fn cancel_batch(state: BackendState<'_>) -> Result<(), String> {
    state.cancel_batch()
}

#[tauri::command]
fn open_export_directory(app: AppHandle, state: BackendState<'_>) -> Result<(), String> {
    let directory = state.export_directory()?;
    app.opener()
        .open_path(directory, None::<&str>)
        .map_err(|error| format!("无法打开导出文件夹：{error}"))
}

#[tauri::command]
fn open_exported_image(
    path: String,
    app: AppHandle,
    state: BackendState<'_>,
) -> Result<(), String> {
    let path = state.exported_image(&path)?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|error| format!("无法打开导出照片：{error}"))
}

#[tauri::command]
async fn retry_failed(app: AppHandle, state: BackendState<'_>) -> Result<BatchSnapshot, String> {
    let backend = state.inner().clone();
    blocking(move || backend.resume_batch(true, progress_sink(app))).await
}

#[tauri::command]
async fn resume_batch(app: AppHandle, state: BackendState<'_>) -> Result<BatchSnapshot, String> {
    let backend = state.inner().clone();
    blocking(move || backend.resume_batch(false, progress_sink(app))).await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _directory| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            },
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            app.manage(Arc::new(art_ai::ArtAi::new(directory.clone())));
            let backend = Backend::new(directory.clone()).map_err(std::io::Error::other)?;
            app.manage(backend);
            let agent = mcp::AgentService::new(directory, std::env::current_exe()?);
            let agent_app = app.handle().clone();
            agent.set_dispatcher(Arc::new(move |command| agent_app.emit_to("main", "agent-command", command).map_err(|_| "无法连接 Margin 窗口".into())));
            app.manage(agent.clone());
            let startup_app = app.handle().clone();
            tauri::async_runtime::spawn(async move { if let Err(error) = agent.start_saved().await { let _ = startup_app.emit("agent-status-error", error); } });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_agent_status,
            set_agent_access,
            complete_agent_command,
            agent_window_ready,
            get_frame_template_previews,
            get_ai_state,
            get_chatgpt_state,
            get_pi_runtime,
            open_pi_setup,
            list_pi_models,
            connect_pi_subscription,
            answer_pi_prompt,
            cancel_pi_connection,
            reopen_pi_authorization,
            disconnect_pi_subscription,
            chatgpt_sign_in,
            cancel_chatgpt_sign_in,
            select_chatgpt_account,
            list_chatgpt_models,
            disconnect_chatgpt,
            open_chatgpt_usage,
            prepare_ai_photo,
            generate_art_frames,
            list_builtin_frames,
            prepare_art_refinement,
            generate_handdrawn,
            generate_complete_frame,
            list_image_models,
            save_ai_config,
            cancel_art_frames,
            import_photos,
            remove_photos,
            undo_photo_removal,
            redo_photo_removal,
            save_photo_selection,
            get_preview,
            list_fonts,
            sample_color,
            get_sampler_image,
            get_watermark_detail,
            get_camera_info,
            set_photo_settings,
            quick_frame_styles,
            save_settings,
            save_preset,
            delete_preset,
            save_frame_template,
            delete_frame_template,
            start_batch,
            cancel_batch,
            open_export_directory,
            open_exported_image,
            retry_failed,
            resume_batch,
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Margin");
}
