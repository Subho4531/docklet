use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::services::{ServiceState, ServicesSnapshot};
use crate::window;

pub struct TrayHandles {
    tray: TrayIcon<tauri::Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayHandles> {
    let open = MenuItem::with_id(app, "open", "Open Docklet", true, None::<&str>)?;
    let quick = MenuItem::with_id(app, "quick", "Quick panel", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Docklet", true, None::<&str>)?;

    let items: [&dyn tauri::menu::IsMenuItem<tauri::Wry>; 5] =
        [&open, &quick, &settings, &separator, &quit];
    let menu = Menu::with_items(app, &items)?;

    let mut builder = TrayIconBuilder::with_id("docklet-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Docklet · local services")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => window::show_main(app, "services"),
            "quick" => window::show_popup(app, None),
            "settings" => window::show_main(app, "settings"),
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
    Ok(TrayHandles { tray })
}

pub fn update(app: &AppHandle, snapshot: &ServicesSnapshot) {
    let Some(handles) = app.try_state::<TrayHandles>() else {
        return;
    };

    let _ = handles.tray.set_tooltip(Some(tooltip(snapshot).as_str()));
}

fn tooltip(snapshot: &ServicesSnapshot) -> String {
    if !snapshot.docker_available {
        return "Docklet · Docker Desktop offline".to_string();
    }

    let running = snapshot
        .services
        .iter()
        .filter(|service| service.state == ServiceState::Running)
        .count();
    let starting = snapshot
        .services
        .iter()
        .filter(|service| service.state == ServiceState::Starting)
        .count();
    let attention = snapshot
        .services
        .iter()
        .filter(|service| matches!(service.state, ServiceState::Error | ServiceState::Unmanaged))
        .count();

    if attention > 0 {
        format!("Docklet · {attention} service(s) need attention")
    } else if starting > 0 {
        format!("Docklet · {starting} service(s) starting")
    } else if running == 0 {
        "Docklet · all services stopped".to_string()
    } else {
        format!("Docklet · {running} service(s) running")
    }
}
