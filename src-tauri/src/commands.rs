use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::redis::{self, RedisState, RedisStatus};
use crate::state::AppState;
use crate::tray;
use crate::window;

#[derive(Clone, Copy)]
enum Operation {
    Start,
    Stop,
    Reset,
}

pub fn publish_status(app: &AppHandle, status: &RedisStatus) {
    tray::update(app, status);
    let _ = app.emit("redis-status", status);
}

pub fn publish_error(app: &AppHandle, message: &str) {
    let _ = app.emit("redis-error", message);
}

async fn run_operation(app: &AppHandle, operation: Operation) -> Result<RedisStatus, String> {
    let state = app.state::<AppState>();
    let _guard = state.operation.lock().await;

    let result = match operation {
        Operation::Start => redis::start().await,
        Operation::Stop => redis::stop().await,
        Operation::Reset => redis::reset().await,
    };

    match &result {
        Ok(status) => publish_status(app, status),
        Err(_) => {
            let status = redis::status().await;
            publish_status(app, &status);
        }
    }

    result
}

pub async fn start(app: &AppHandle) -> Result<RedisStatus, String> {
    run_operation(app, Operation::Start).await
}

pub async fn stop(app: &AppHandle) -> Result<RedisStatus, String> {
    run_operation(app, Operation::Stop).await
}

pub async fn reset(app: &AppHandle) -> Result<RedisStatus, String> {
    run_operation(app, Operation::Reset).await
}

pub async fn copy_current_url(app: &AppHandle) -> Result<String, String> {
    let status = redis::status().await;
    let Some(url) = status.url else {
        return Err("Redis doesn't have a local URL yet. Start Redis first.".to_string());
    };
    if status.state != RedisState::Running {
        return Err("Start Redis before copying its connection URL.".to_string());
    }

    app.clipboard()
        .write_text(url.clone())
        .map_err(|error| format!("Couldn't copy the Redis URL. {error}"))?;
    Ok(url)
}

pub fn tray_failure(app: &AppHandle, message: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = redis::status().await;
        publish_status(&app, &status);
        window::show_popup(&app, None);
        publish_error(&app, &message);
    });
}

#[tauri::command]
pub async fn get_redis_status(app: AppHandle) -> RedisStatus {
    let status = redis::status().await;
    tray::update(&app, &status);
    status
}

#[tauri::command]
pub async fn start_redis(app: AppHandle) -> Result<RedisStatus, String> {
    start(&app).await
}

#[tauri::command]
pub async fn stop_redis(app: AppHandle) -> Result<RedisStatus, String> {
    stop(&app).await
}

#[tauri::command]
pub async fn reset_redis(app: AppHandle) -> Result<RedisStatus, String> {
    reset(&app).await
}

#[tauri::command]
pub async fn copy_redis_url(app: AppHandle) -> Result<String, String> {
    copy_current_url(&app).await
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    window::hide_popup(&app);
}

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|error| format!("Couldn't read the launch-at-login setting. {error}"))
}

#[tauri::command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();

    if enabled {
        manager
            .enable()
            .map_err(|error| format!("Couldn't enable launch at login. {error}"))?;
    } else {
        manager
            .disable()
            .map_err(|error| format!("Couldn't disable launch at login. {error}"))?;
    }

    manager
        .is_enabled()
        .map_err(|error| format!("Couldn't verify the launch-at-login setting. {error}"))
}
