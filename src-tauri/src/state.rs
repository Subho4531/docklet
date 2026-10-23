use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};

use tauri::PhysicalPosition;

use crate::settings::AppSettings;

pub struct AppState {
    settings: RwLock<AppSettings>,
    settings_path: PathBuf,
    operation_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    pub settings_lock: tokio::sync::Mutex<()>,
    pub focus_generation: AtomicU64,
    pub last_tray_anchor: Mutex<Option<PhysicalPosition<f64>>>,
    pub pending_main_page: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(settings: AppSettings, settings_path: PathBuf) -> Self {
        Self {
            settings: RwLock::new(settings),
            settings_path,
            operation_locks: Mutex::new(HashMap::new()),
            settings_lock: tokio::sync::Mutex::new(()),
            focus_generation: AtomicU64::new(0),
            last_tray_anchor: Mutex::new(None),
            pending_main_page: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn replace_settings(&self, settings: AppSettings) {
        *self
            .settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
    }

    pub fn settings_path(&self) -> &PathBuf {
        &self.settings_path
    }

    pub fn operation_lock(&self, id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .operation_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    pub fn forget_operation_lock(&self, id: &str) {
        let mut locks = self
            .operation_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.remove(id);
    }

    pub fn set_pending_main_page(&self, page: &str) {
        let mut pending = self
            .pending_main_page
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *pending = Some(page.to_string());
    }

    pub fn take_pending_main_page(&self) -> String {
        let mut pending = self
            .pending_main_page
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.take().unwrap_or_else(|| "services".to_string())
    }
}
