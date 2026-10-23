mod commands;
mod redis;
mod state;
mod tray;
mod window;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|argument| argument == "--hidden") {
                window::show_popup(app, None);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .setup(|app| {
            let handles = tray::create(app.handle())?;
            app.manage(handles);

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let status = redis::status().await;
                commands::publish_status(&app_handle, &status);
            });

            let launched_hidden = std::env::args().any(|argument| argument == "--hidden");
            if !launched_hidden {
                window::show_popup(app.handle(), None);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != window::MAIN_WINDOW {
                return;
            }

            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    window::hide_popup(window.app_handle());
                }
                tauri::WindowEvent::Focused(false) => {
                    if window.is_visible().unwrap_or(false) {
                        window::schedule_auto_hide(window.app_handle());
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_redis_status,
            commands::start_redis,
            commands::stop_redis,
            commands::reset_redis,
            commands::copy_redis_url,
            commands::hide_popup,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Docklet");
}
