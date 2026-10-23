use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use tauri::PhysicalPosition;

#[derive(Default)]
pub struct AppState {
    pub operation: tokio::sync::Mutex<()>,
    pub focus_generation: AtomicU64,
    pub last_tray_anchor: Mutex<Option<PhysicalPosition<f64>>>,
}
