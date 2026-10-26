use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::docker;
use crate::services::{self, ServiceState, ServicesSnapshot};
use crate::settings::{self, AppSettings};
use crate::state::AppState;
use crate::tray;
use crate::window;

#[derive(Clone, Copy)]
enum Operation {
    Start,
    Stop,
    Reset,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceErrorPayload {
    service_id: String,
    message: String,
}

pub fn publish_snapshot(app: &AppHandle, snapshot: &ServicesSnapshot) {
    tray::update(app, snapshot);
    let _ = app.emit("services-status", snapshot);
}

pub fn publish_error(app: &AppHandle, service_id: &str, message: &str) {
    let _ = app.emit(
        "service-error",
        ServiceErrorPayload {
            service_id: service_id.to_string(),
            message: message.to_string(),
        },
    );
}

pub async fn refresh_and_publish(app: &AppHandle) -> ServicesSnapshot {
    let settings = app.state::<AppState>().settings();
    let snapshot = services::list_statuses(&settings).await;
    publish_snapshot(app, &snapshot);
    snapshot
}

async fn run_operation(
    app: &AppHandle,
    service_id: String,
    operation: Operation,
) -> Result<ServicesSnapshot, String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    services::find_definition(&settings, &service_id)?;

    let lock = state.operation_lock(&service_id);
    let _guard = lock.lock().await;

    let result = match operation {
        Operation::Start => services::start(&settings, &service_id).await,
        Operation::Stop => services::stop(&settings, &service_id).await,
        Operation::Reset => services::reset(&settings, &service_id).await,
    };

    let snapshot = services::list_statuses(&state.settings()).await;
    publish_snapshot(app, &snapshot);

    match result {
        Ok(()) => Ok(snapshot),
        Err(error) => {
            publish_error(app, &service_id, &error);
            Err(error)
        }
    }
}

pub async fn copy_url(app: &AppHandle, service_id: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    let snapshot = services::list_statuses(&state.settings()).await;
    let service = snapshot
        .services
        .iter()
        .find(|service| service.id == service_id)
        .ok_or_else(|| format!("Unknown Docklet service `{service_id}`."))?;

    if service.state != ServiceState::Running {
        return Err(format!(
            "Start {} before copying its connection URL.",
            service.name
        ));
    }
    let Some(url) = service.url.clone() else {
        return Err(format!(
            "{} doesn't have a published local URL yet.",
            service.name
        ));
    };

    app.clipboard()
        .write_text(url.clone())
        .map_err(|error| format!("Couldn't copy the {} URL. {error}", service.name))?;
    Ok(url)
}

#[tauri::command]
pub async fn get_services_status(app: AppHandle) -> ServicesSnapshot {
    refresh_and_publish(&app).await
}

#[tauri::command]
pub async fn start_service(app: AppHandle, service_id: String) -> Result<ServicesSnapshot, String> {
    run_operation(&app, service_id, Operation::Start).await
}

#[tauri::command]
pub async fn stop_service(app: AppHandle, service_id: String) -> Result<ServicesSnapshot, String> {
    run_operation(&app, service_id, Operation::Stop).await
}

#[tauri::command]
pub async fn reset_service(app: AppHandle, service_id: String) -> Result<ServicesSnapshot, String> {
    run_operation(&app, service_id, Operation::Reset).await
}

#[tauri::command]
pub async fn copy_service_url(app: AppHandle, service_id: String) -> Result<String, String> {
    copy_url(&app, &service_id).await
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("There's nothing to copy.".to_string());
    }
    if text.len() > 4096 {
        return Err("That value is too long to copy.".to_string());
    }

    app.clipboard()
        .write_text(text)
        .map_err(|error| format!("Couldn't copy to the clipboard. {error}"))
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    app.state::<AppState>().settings()
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let state = app.state::<AppState>();
    let _guard = state.settings_lock.lock().await;

    let normalized = settings::validate_and_normalize(settings)?;
    let previous = state.settings();
    let previous_ids = previous
        .custom_services
        .iter()
        .map(|service| service.id.as_str())
        .collect::<HashSet<_>>();
    let next_ids = normalized
        .custom_services
        .iter()
        .map(|service| service.id.as_str())
        .collect::<HashSet<_>>();

    if let Some(removed) = previous_ids.difference(&next_ids).next() {
        return Err(format!(
            "Use Remove service to delete `{removed}` and its managed data."
        ));
    }
    if previous.llm_models != normalized.llm_models {
        return Err("Use the Models page to add or remove local LLMs.".to_string());
    }

    settings::save(state.settings_path(), &normalized)?;
    state.replace_settings(normalized.clone());
    let _ = app.emit("settings-changed", &normalized);
    refresh_and_publish(&app).await;
    Ok(normalized)
}

#[tauri::command]
pub async fn remove_custom_service(
    app: AppHandle,
    service_id: String,
) -> Result<AppSettings, String> {
    let state = app.state::<AppState>();
    let _settings_guard = state.settings_lock.lock().await;
    let lock = state.operation_lock(&service_id);
    let _operation_guard = lock.lock().await;

    let current = state.settings();
    let service = services::find_definition(&current, &service_id)?;
    if service.kind != "custom" {
        return Err("Built-in services can't be removed.".to_string());
    }

    services::remove_custom_resources(&current, &service_id).await?;

    let mut next = current;
    next.custom_services
        .retain(|service| service.id != service_id);
    let normalized = settings::validate_and_normalize(next)?;
    settings::save(state.settings_path(), &normalized)?;
    state.replace_settings(normalized.clone());
    state.forget_operation_lock(&service_id);

    let _ = app.emit("settings-changed", &normalized);
    refresh_and_publish(&app).await;
    Ok(normalized)
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    window::hide_popup(&app);
}

#[tauri::command]
pub fn resize_popup(app: AppHandle, height: f64) -> Result<(), String> {
    let height = height.clamp(180.0, 700.0);
    let app_handle = app.clone();

    app.run_on_main_thread(move || {
        let Some(window) = app_handle.get_webview_window(window::POPUP_WINDOW) else {
            return;
        };
        let _ = window.set_size(tauri::LogicalSize::new(window::POPUP_WIDTH, height));
        window::apply_popup_shape(&window);
        window::reposition_popup(&app_handle);
    })
    .map_err(|error| format!("Couldn't resize the quick panel. {error}"))
}

#[tauri::command]
pub async fn search_docker_images(
    query: String,
) -> Result<Vec<docker::DockerSearchResult>, String> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    if query.chars().count() > 80
        || query
            .chars()
            .any(|character| character.is_control() || character == '\0')
    {
        return Err("Search terms must be 2–80 visible characters.".to_string());
    }

    docker::search_images(query)
        .await
        .map_err(|error| error.user_message())
}

#[tauri::command]
pub fn open_main(app: AppHandle, page: Option<String>) {
    let page = match page.as_deref() {
        Some("models") => "models",
        Some("settings") => "settings",
        Some("library") => "library",
        _ => "services",
    };
    window::show_main(&app, page);
}

#[tauri::command]
pub fn get_main_page(app: AppHandle) -> String {
    app.state::<AppState>().take_pending_main_page()
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
