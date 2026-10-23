mod commands;
mod docker;
mod services;
mod settings;
mod state;
mod tray;
mod window;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use settings::AppSettings;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|argument| argument == "--hidden") {
                window::show_main(app, "services");
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = match settings::load(&settings_path) {
                Ok(settings) => settings,
                Err(error) => {
                    eprintln!("{error} Using default Docklet settings.");
                    AppSettings::default()
                }
            };

            app.manage(AppState::new(settings, settings_path));

            let handles = tray::create(app.handle())?;
            app.manage(handles);

            if let Some(popup) = app.get_webview_window(window::POPUP_WINDOW) {
                window::apply_popup_shape(&popup);
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::refresh_and_publish(&app_handle).await;
            });

            let launched_hidden = std::env::args().any(|argument| argument == "--hidden");
            if !launched_hidden {
                window::show_main(app.handle(), "services");
            }

            Ok(())
        })
        .on_window_event(|window, event| match window.label() {
            window::POPUP_WINDOW => match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    window::hide_popup(window.app_handle());
                }
                tauri::WindowEvent::Focused(false) => {
                    if window.is_visible().unwrap_or(false) {
                        window::schedule_auto_hide(window.app_handle());
                    }
                }
                tauri::WindowEvent::Resized(_) => {
                    if let Some(popup) =
                        window.app_handle().get_webview_window(window::POPUP_WINDOW)
                    {
                        window::apply_popup_shape(&popup);
                    }
                }
                _ => {}
            },
            window::MAIN_WINDOW => {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_services_status,
            commands::start_service,
            commands::stop_service,
            commands::reset_service,
            commands::copy_service_url,
            commands::get_settings,
            commands::save_settings,
            commands::remove_custom_service,
            commands::search_docker_images,
            commands::hide_popup,
            commands::resize_popup,
            commands::open_main,
            commands::get_main_page,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Docklet");
}
