use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::redis::{RedisState, RedisStatus};
use crate::window;

pub struct TrayHandles {
    tray: TrayIcon<tauri::Wry>,
    start: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
    copy_url: MenuItem<tauri::Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayHandles> {
    let open = MenuItem::with_id(app, "open", "Open Docklet", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start", "Start Redis", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop Redis", false, None::<&str>)?;
    let copy_url = MenuItem::with_id(app, "copy", "Copy Redis URL", false, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Docklet", true, None::<&str>)?;

    let menu_items: [&dyn tauri::menu::IsMenuItem<tauri::Wry>; 7] = [
        &open,
        &separator_one,
        &start,
        &stop,
        &copy_url,
        &separator_two,
        &quit,
    ];
    let menu = Menu::with_items(app, &menu_items)?;

    let mut builder = TrayIconBuilder::with_id("docklet-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Docklet · Redis")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => window::show_popup(app, None),
            "start" => spawn_action(app, TrayAction::Start),
            "stop" => spawn_action(app, TrayAction::Stop),
            "copy" => spawn_copy(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                position,
                button,
                button_state,
                ..
            } = event
            {
                if matches!(button_state, MouseButtonState::Up) {
                    window::remember_anchor(tray.app_handle(), position);
                    if matches!(button, MouseButton::Left) {
                        window::toggle_popup(tray.app_handle(), position);
                    }
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let tray = builder.build(app)?;

    Ok(TrayHandles {
        tray,
        start,
        stop,
        copy_url,
    })
}

pub fn update(app: &AppHandle, status: &RedisStatus) {
    let Some(handles) = app.try_state::<TrayHandles>() else {
        return;
    };

    let running = matches!(status.state, RedisState::Running | RedisState::Starting);
    let has_url = status.url.is_some();

    let _ = handles.start.set_enabled(!running);
    let _ = handles.stop.set_enabled(running);
    let _ = handles
        .copy_url
        .set_enabled(status.state == RedisState::Running && has_url);
    let _ = handles.tray.set_tooltip(Some(tooltip(status)));
}

fn tooltip(status: &RedisStatus) -> String {
    match status.state {
        RedisState::Running => match status.port {
            Some(port) => format!("Docklet · Redis running on {port}"),
            None => "Docklet · Redis running".to_string(),
        },
        RedisState::Stopped => "Docklet · Redis stopped".to_string(),
        RedisState::Missing => "Docklet · Redis not set up".to_string(),
        RedisState::Starting => "Docklet · Redis starting".to_string(),
        RedisState::DockerUnavailable => "Docklet · Docker Desktop offline".to_string(),
        RedisState::Unmanaged => "Docklet · Container name conflict".to_string(),
        RedisState::Error => "Docklet · Redis needs attention".to_string(),
    }
}

#[derive(Clone, Copy)]
enum TrayAction {
    Start,
    Stop,
}

fn spawn_action(app: &AppHandle, action: TrayAction) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match action {
            TrayAction::Start => commands::start(&app).await,
            TrayAction::Stop => commands::stop(&app).await,
        };

        if let Err(error) = result {
            commands::tray_failure(&app, error);
        }
    });
}

fn spawn_copy(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match commands::copy_current_url(&app).await {
            Ok(url) => {
                let _ = app.emit("redis-toast", format!("Copied {url}"));
            }
            Err(error) => commands::tray_failure(&app, error),
        }
    });
}
