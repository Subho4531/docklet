use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition};

use crate::state::AppState;

pub const MAIN_WINDOW: &str = "main";

const POPUP_GAP: i32 = 10;
const SCREEN_MARGIN: i32 = 8;

pub fn remember_anchor(app: &AppHandle, position: PhysicalPosition<f64>) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut anchor) = state.last_tray_anchor.lock() {
            *anchor = Some(position);
        }
    }
}

pub fn cancel_pending_hide(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.focus_generation.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn show_popup(app: &AppHandle, anchor: Option<PhysicalPosition<f64>>) {
    cancel_pending_hide(app);

    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    let anchor = anchor
        .or_else(|| {
            app.try_state::<AppState>().and_then(|state| {
                state
                    .last_tray_anchor
                    .lock()
                    .ok()
                    .and_then(|anchor| *anchor)
            })
        })
        .unwrap_or_else(|| default_anchor(app, &window));

    position_popup(app, &window, anchor);
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn toggle_popup(app: &AppHandle, anchor: PhysicalPosition<f64>) {
    remember_anchor(app, anchor);
    cancel_pending_hide(app);

    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        hide_popup(app);
    } else {
        show_popup(app, Some(anchor));
    }
}

pub fn hide_popup(app: &AppHandle) {
    cancel_pending_hide(app);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.hide();
    }
}

pub fn schedule_auto_hide(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let generation = state.focus_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(140)).await;

        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        if state.focus_generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
            return;
        };
        if window.is_visible().unwrap_or(false) && !window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        }
    });
}

fn default_anchor(app: &AppHandle, window: &tauri::WebviewWindow) -> PhysicalPosition<f64> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        PhysicalPosition::new(
            (area.position.x + area.size.width as i32 - 20) as f64,
            (area.position.y + area.size.height as i32 - 20) as f64,
        )
    } else {
        PhysicalPosition::new(1200.0, 800.0)
    }
}

fn position_popup(app: &AppHandle, window: &tauri::WebviewWindow, anchor: PhysicalPosition<f64>) {
    let window_size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(380, 410));
    let monitor = app
        .monitor_from_point(anchor.x, anchor.y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    let (x, y) = if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let left = area.position.x;
        let top = area.position.y;
        let right = left + area.size.width as i32;
        let bottom = top + area.size.height as i32;
        let width = window_size.width as i32;
        let height = window_size.height as i32;

        let min_x = left + SCREEN_MARGIN;
        let max_x = (right - width - SCREEN_MARGIN).max(min_x);
        let x = ((anchor.x.round() as i32) - width / 2).clamp(min_x, max_x);

        let above = (anchor.y.round() as i32) - height - POPUP_GAP;
        let min_y = top + SCREEN_MARGIN;
        let max_y = (bottom - height - SCREEN_MARGIN).max(min_y);
        let y = if above >= min_y {
            above.clamp(min_y, max_y)
        } else {
            ((anchor.y.round() as i32) + POPUP_GAP).clamp(min_y, max_y)
        };

        (x, y)
    } else {
        (
            (anchor.x.round() as i32) - window_size.width as i32 / 2,
            (anchor.y.round() as i32) - window_size.height as i32 - POPUP_GAP,
        )
    };

    let _ = window.set_position(PhysicalPosition::new(x, y));
}
