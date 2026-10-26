use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::settings::{self, LlmModel};
use crate::state::AppState;

const ENGINE_BUILD: &str = "b11838";
const ENGINE_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b11838/llama-b11838-bin-win-cpu-x64.zip";
const ENGINE_SHA256: &str = "45462c0f6b0e9f9b841e3d2bc2a086b29925a9691d866458bbbf90cf564ae2b7";
const ENGINE_ZIP_BYTES: u64 = 24_081_183;
const ENGINE_EXE: &str = "llama-server.exe";
const ENGINE_VERSION_FILE: &str = "version.txt";
const SERVER_LOG: &str = "logs/llama-server.log";
const STATUS_EVENT: &str = "llm-status";
const PROGRESS_EVENT: &str = "llm-download-progress";
const START_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmEngineStatus {
    pub installed: bool,
    pub installing: bool,
    pub version: String,
    pub path: Option<String>,
    pub error: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LlmState {
    Stopped,
    Running,
    Downloading,
    Missing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelStatus {
    pub id: String,
    pub display_name: String,
    pub params: String,
    pub quant: String,
    pub size_bytes: u64,
    pub context_window: u32,
    pub state: LlmState,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmSnapshot {
    pub engine: LlmEngineStatus,
    pub models: Vec<LlmModelStatus>,
    pub port: u16,
    pub active_model_id: Option<String>,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub fit: String,
    pub params: String,
    pub quant: String,
    pub size_bytes: u64,
    pub context_window: u32,
    pub repo: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LlmProgressPayload {
    id: String,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct PendingInstall {
    model: LlmModel,
    downloaded: u64,
    total: u64,
}

#[derive(Debug, Clone, Default)]
pub struct LlmRuntimeState {
    pub installing_engine: bool,
    pub engine_error: Option<String>,
    pub engine_downloaded: u64,
    pub engine_total: u64,
    pub downloads: HashMap<String, PendingInstall>,
    pub model_errors: HashMap<String, String>,
    pub active_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModelRequest {
    pub catalog_id: Option<String>,
    pub repo: Option<String>,
    pub file_name: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
enum ProgressSlot {
    Engine,
    Model(String),
}

pub fn catalog() -> Vec<LlmCatalogEntry> {
    vec![
        LlmCatalogEntry {
            id: "qwen2-5-1-5b-instruct".into(),
            display_name: "Qwen 2.5 1.5B".into(),
            description: "A quick general chat model with solid instruction following.".into(),
            category: "Chat".into(),
            fit: "comfortable".into(),
            params: "1.5B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 986_048_768,
            context_window: 8192,
            repo: "bartowski/Qwen2.5-1.5B-Instruct-GGUF".into(),
            file_name: "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "qwen2-5-coder-1-5b".into(),
            display_name: "Qwen 2.5 Coder 1.5B".into(),
            description: "Fast code completion and small refactors without leaving your machine."
                .into(),
            category: "Coding".into(),
            fit: "comfortable".into(),
            params: "1.5B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 986_048_800,
            context_window: 8192,
            repo: "bartowski/Qwen2.5-Coder-1.5B-Instruct-GGUF".into(),
            file_name: "Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "llama-3-2-1b".into(),
            display_name: "Llama 3.2 1B".into(),
            description: "A tiny, dependable assistant for summaries and short answers.".into(),
            category: "Chat".into(),
            fit: "comfortable".into(),
            params: "1B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 807_694_464,
            context_window: 8192,
            repo: "bartowski/Llama-3.2-1B-Instruct-GGUF".into(),
            file_name: "Llama-3.2-1B-Instruct-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "smollm2-1-7b".into(),
            display_name: "SmolLM2 1.7B".into(),
            description: "Compact and lively, tuned for everyday chat and writing.".into(),
            category: "Chat".into(),
            fit: "comfortable".into(),
            params: "1.7B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 1_055_609_536,
            context_window: 8192,
            repo: "HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF".into(),
            file_name: "smollm2-1.7b-instruct-q4_k_m.gguf".into(),
        },
        LlmCatalogEntry {
            id: "deepseek-r1-1-5b".into(),
            display_name: "DeepSeek R1 Distill 1.5B".into(),
            description: "A small reasoning model that shows its thinking before answering.".into(),
            category: "Reasoning".into(),
            fit: "comfortable".into(),
            params: "1.5B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 1_117_320_800,
            context_window: 8192,
            repo: "bartowski/DeepSeek-R1-Distill-Qwen-1.5B-GGUF".into(),
            file_name: "DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "gemma-2-2b".into(),
            display_name: "Gemma 2 2B".into(),
            description: "A well-rounded 2B model with crisp, readable prose.".into(),
            category: "Chat".into(),
            fit: "tight".into(),
            params: "2B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 1_708_582_752,
            context_window: 8192,
            repo: "bartowski/gemma-2-2b-it-GGUF".into(),
            file_name: "gemma-2-2b-it-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "phi-3-5-mini".into(),
            display_name: "Phi 3.5 Mini".into(),
            description: "A 3.8B model that punches above its weight on explanations.".into(),
            category: "Chat".into(),
            fit: "tight".into(),
            params: "3.8B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 2_393_232_672,
            context_window: 4096,
            repo: "bartowski/Phi-3.5-mini-instruct-GGUF".into(),
            file_name: "Phi-3.5-mini-instruct-Q4_K_M.gguf".into(),
        },
        LlmCatalogEntry {
            id: "qwen2-5-7b-instruct".into(),
            display_name: "Qwen 2.5 7B".into(),
            description:
                "The strongest option here. Expect slower, deliberate replies on the CPU engine."
                    .into(),
            category: "Chat".into(),
            fit: "partial".into(),
            params: "7B".into(),
            quant: "Q4_K_M".into(),
            size_bytes: 4_683_074_240,
            context_window: 4096,
            repo: "bartowski/Qwen2.5-7B-Instruct-GGUF".into(),
            file_name: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
        },
    ]
}

fn llm_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("llm"))
        .map_err(|error| format!("Couldn't resolve Docklet's data folder. {error}"))
}

fn engine_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(llm_root(app)?.join("engine"))
}

fn engine_exe(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(engine_dir(app)?.join(ENGINE_EXE))
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(llm_root(app)?.join("models"))
}

fn api_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(llm_root(app)?.join("api-key.txt"))
}

fn pid_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(llm_root(app)?.join("server.pid"))
}

fn log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(llm_root(app)?.join(SERVER_LOG))
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("Couldn't create {}: {error}", path.display()))
}

fn valid_api_key(key: &str) -> bool {
    key.len() >= 16
        && key.len() <= 256
        && key
            .chars()
            .all(|character| !character.is_whitespace() && !character.is_control())
}

fn generate_api_key() -> String {
    use std::fmt::Write as _;

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sk-docklet-{hex}")
}

async fn ensure_api_key(app: &AppHandle) -> Result<(String, PathBuf), String> {
    let state = app.state::<AppState>();
    let existing = state.settings().llm_api_key;
    let key = if valid_api_key(&existing) {
        existing
    } else {
        let _guard = state.settings_lock.lock().await;
        let mut next = state.settings();
        if valid_api_key(&next.llm_api_key) {
            next.llm_api_key.clone()
        } else {
            next.llm_api_key = generate_api_key();
            settings::save(state.settings_path(), &next)?;
            state.replace_settings(next.clone());
            let _ = app.emit("settings-changed", &next);
            next.llm_api_key
        }
    };

    let path = write_api_key_file(app, &key)?;
    Ok((key, path))
}

fn write_api_key_file(app: &AppHandle, key: &str) -> Result<PathBuf, String> {
    let path = api_key_path(app)?;
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }
    let current = fs::read_to_string(&path).unwrap_or_default();
    if current.trim() != key {
        fs::write(&path, key)
            .map_err(|error| format!("Couldn't write the local LLM API key. {error}"))?;
    }
    Ok(path)
}

pub async fn snapshot(app: &AppHandle) -> Result<LlmSnapshot, String> {
    reap_dead_server(app).await;

    let state = app.state::<AppState>();
    let settings = state.settings();
    let runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();

    let exe = engine_exe(app)?;
    let installed = exe.is_file();
    let models_root = models_dir(app)?;
    let port = settings.llm_port;
    let api_key = settings.llm_api_key.clone();

    let mut models = Vec::with_capacity(settings.llm_models.len() + runtime.downloads.len());
    for model in &settings.llm_models {
        let present = models_root.join(&model.id).join(&model.file_name).is_file();
        let pending = runtime.downloads.get(&model.id);
        let error = runtime.model_errors.get(&model.id).cloned();
        let state = if runtime.active_model.as_deref() == Some(model.id.as_str()) {
            LlmState::Running
        } else if pending.is_some() {
            LlmState::Downloading
        } else if error.is_some() {
            LlmState::Error
        } else if present {
            LlmState::Stopped
        } else {
            LlmState::Missing
        };
        models.push(model_status(model, state, pending, &error, port, &api_key));
    }

    for (id, pending) in &runtime.downloads {
        if settings.llm_models.iter().any(|model| &model.id == id) {
            continue;
        }
        models.push(model_status(
            &pending.model,
            LlmState::Downloading,
            Some(pending),
            &runtime.model_errors.get(id).cloned(),
            port,
            &api_key,
        ));
    }

    let engine = LlmEngineStatus {
        installed,
        installing: runtime.installing_engine,
        version: ENGINE_BUILD.to_string(),
        path: installed.then(|| exe.display().to_string()),
        error: runtime.engine_error.clone(),
        downloaded_bytes: runtime.engine_downloaded,
        total_bytes: runtime.engine_total,
    };

    Ok(LlmSnapshot {
        engine,
        models,
        port,
        active_model_id: runtime.active_model.clone(),
        api_key,
    })
}

fn model_status(
    model: &LlmModel,
    state: LlmState,
    pending: Option<&PendingInstall>,
    error: &Option<String>,
    port: u16,
    api_key: &str,
) -> LlmModelStatus {
    let downloaded_bytes =
        pending
            .map(|pending| pending.downloaded)
            .unwrap_or(if state == LlmState::Missing {
                0
            } else {
                model.size_bytes
            });
    let total_bytes = pending
        .map(|pending| pending.total)
        .unwrap_or(model.size_bytes);

    LlmModelStatus {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        params: model.params.clone(),
        quant: model.quant.clone(),
        size_bytes: model.size_bytes,
        context_window: model.context_window,
        state,
        downloaded_bytes,
        total_bytes,
        endpoint: Some(format!("http://127.0.0.1:{port}/v1")),
        api_key: valid_api_key(api_key).then(|| api_key.to_string()),
        model_name: Some(model.id.clone()),
        error: error.clone(),
    }
}

async fn reap_dead_server(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut server = state.llm_server.lock().await;
    let Some(child) = server.as_mut() else {
        return;
    };

    let exited = match child.try_wait() {
        Ok(status) => status,
        Err(_) => None,
    };
    let Some(status) = exited else {
        return;
    };

    server.take();
    if let Ok(path) = pid_path(app) {
        let _ = fs::remove_file(path);
    }

    let mut runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(model_id) = runtime.active_model.take() {
        runtime.model_errors.insert(
            model_id,
            format!("llama-server stopped unexpectedly ({status}). Check the server log."),
        );
    }
}

pub async fn publish(app: &AppHandle) {
    if let Ok(snapshot) = snapshot(app).await {
        let _ = app.emit(STATUS_EVENT, snapshot);
    }
}

#[tauri::command]
pub async fn get_llm_status(app: AppHandle) -> Result<LlmSnapshot, String> {
    let _ = ensure_api_key(&app).await?;
    snapshot(&app).await
}

#[tauri::command]
pub fn get_llm_catalog() -> Vec<LlmCatalogEntry> {
    catalog()
}

#[tauri::command]
pub async fn install_llm_engine(app: AppHandle) -> Result<LlmSnapshot, String> {
    let state = app.state::<AppState>();

    if engine_exe(&app)?.is_file() {
        return snapshot(&app).await;
    }

    let _guard = state.llm_engine_lock.lock().await;
    if engine_exe(&app)?.is_file() {
        return snapshot(&app).await;
    }

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.installing_engine = true;
        runtime.engine_error = None;
        runtime.engine_downloaded = 0;
        runtime.engine_total = ENGINE_ZIP_BYTES;
    }
    publish(&app).await;

    let result = install_engine_files(&app).await;

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.installing_engine = false;
        match &result {
            Ok(()) => {
                runtime.engine_error = None;
                runtime.engine_downloaded = ENGINE_ZIP_BYTES;
                runtime.engine_total = ENGINE_ZIP_BYTES;
            }
            Err(error) => runtime.engine_error = Some(error.clone()),
        }
    }
    publish(&app).await;

    result?;
    snapshot(&app).await
}

async fn install_engine_files(app: &AppHandle) -> Result<(), String> {
    let root = llm_root(app)?;
    ensure_directory(&root)?;
    let archive = root.join("llama-engine.zip");
    if archive.exists() {
        let _ = fs::remove_file(&archive);
    }

    download_file(
        app,
        ENGINE_URL,
        &archive,
        ProgressSlot::Engine,
        ENGINE_ZIP_BYTES,
    )
    .await?;

    let length = fs::metadata(&archive)
        .map_err(|error| format!("Couldn't inspect the llama.cpp download. {error}"))?
        .len();
    if length != ENGINE_ZIP_BYTES {
        let _ = fs::remove_file(&archive);
        return Err(format!(
            "The llama.cpp download was {length} bytes; expected {ENGINE_ZIP_BYTES}. Try again."
        ));
    }
    let digest = sha256_file(&archive)?;
    if !digest.eq_ignore_ascii_case(ENGINE_SHA256) {
        let _ = fs::remove_file(&archive);
        return Err(
            "The llama.cpp download failed its integrity check and was discarded. Try again."
                .to_string(),
        );
    }

    let engine = engine_dir(app)?;
    if engine.exists() {
        fs::remove_dir_all(&engine)
            .map_err(|error| format!("Couldn't replace the old llama.cpp engine. {error}"))?;
    }
    ensure_directory(&engine)?;

    let extracted = extract_zip(&archive, &engine);
    let _ = fs::remove_file(&archive);
    if let Err(error) = extracted {
        let _ = fs::remove_dir_all(&engine);
        return Err(error);
    }

    if !engine.join(ENGINE_EXE).is_file() {
        let _ = fs::remove_dir_all(&engine);
        return Err("The llama.cpp archive didn't contain llama-server.exe.".to_string());
    }
    fs::write(engine.join(ENGINE_VERSION_FILE), ENGINE_BUILD)
        .map_err(|error| format!("Couldn't record the llama.cpp version. {error}"))?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("Couldn't read the download. {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Couldn't read the download. {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn extract_zip(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive)
        .map_err(|error| format!("Couldn't open the engine archive. {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("The engine archive is invalid. {error}"))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Couldn't read the engine archive. {error}"))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let output = destination.join(&relative);

        if entry.is_dir() {
            ensure_directory(&output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            ensure_directory(parent)?;
        }
        let mut target = File::create(&output)
            .map_err(|error| format!("Couldn't extract {}: {error}", output.display()))?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|error| format!("Couldn't extract {}: {error}", output.display()))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn install_model(
    app: AppHandle,
    request: InstallModelRequest,
) -> Result<LlmSnapshot, String> {
    let state = app.state::<AppState>();
    let model = resolve_model(&app, &request)?;

    let directory = models_dir(&app)?.join(&model.id);
    ensure_directory(&directory)?;
    let destination = directory.join(&model.file_name);
    let partial = directory.join(format!("{}.part", model.file_name));

    if destination.is_file() {
        return Err(format!(
            "{} is already on disk. Remove it from Models before reinstalling.",
            model.display_name
        ));
    }

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.downloads.insert(
            model.id.clone(),
            PendingInstall {
                model: model.clone(),
                downloaded: 0,
                total: model.size_bytes,
            },
        );
        runtime.model_errors.remove(&model.id);
    }
    publish(&app).await;

    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}?download=true",
        model.repo, model.file_name
    );
    if let Err(error) = download_file(
        &app,
        &url,
        &partial,
        ProgressSlot::Model(model.id.clone()),
        model.size_bytes,
    )
    .await
    {
        clear_pending(&state, &model.id);
        let _ = fs::remove_dir_all(&directory);
        publish(&app).await;
        return Err(error);
    }

    if let Err(error) = tokio::fs::rename(&partial, &destination).await {
        clear_pending(&state, &model.id);
        let _ = fs::remove_dir_all(&directory);
        publish(&app).await;
        return Err(format!(
            "The model downloaded but couldn't be finalized. {error}"
        ));
    }

    let actual_size = fs::metadata(&destination)
        .map(|metadata| metadata.len())
        .unwrap_or(model.size_bytes);
    let mut saved = model.clone();
    saved.size_bytes = actual_size;
    let saved_id = saved.id.clone();

    if let Err(error) = persist_model(&app, saved).await {
        clear_pending(&state, &saved_id);
        let _ = fs::remove_file(&destination);
        let _ = fs::remove_dir_all(&directory);
        publish(&app).await;
        return Err(error);
    }

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.downloads.remove(&saved_id);
        runtime.model_errors.remove(&saved_id);
    }
    publish(&app).await;
    snapshot(&app).await
}

fn clear_pending(state: &AppState, id: &str) {
    let mut runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime.downloads.remove(id);
}

async fn persist_model(app: &AppHandle, model: LlmModel) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = state.settings_lock.lock().await;
    let mut next = state.settings();
    if next
        .llm_models
        .iter()
        .any(|existing| existing.id == model.id)
    {
        return Err(format!("{} is already installed.", model.display_name));
    }
    next.llm_models.push(model);
    let next = settings::validate_and_normalize(next)?;
    settings::save(state.settings_path(), &next)?;
    state.replace_settings(next.clone());
    let _ = app.emit("settings-changed", &next);
    Ok(())
}

fn resolve_model(app: &AppHandle, request: &InstallModelRequest) -> Result<LlmModel, String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let installed = |id: &str| {
        settings.llm_models.iter().any(|model| model.id == id) || runtime.downloads.contains_key(id)
    };

    if let Some(catalog_id) = request
        .catalog_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let entry = catalog()
            .into_iter()
            .find(|entry| entry.id == catalog_id)
            .ok_or_else(|| format!("Unknown catalog model `{catalog_id}`."))?;
        if installed(&entry.id) {
            return Err(format!(
                "{} is already installed or downloading.",
                entry.display_name
            ));
        }
        return Ok(LlmModel {
            id: entry.id,
            display_name: entry.display_name,
            repo: entry.repo,
            file_name: entry.file_name,
            params: entry.params,
            quant: entry.quant,
            context_window: entry.context_window,
            size_bytes: entry.size_bytes,
        });
    }

    let repo = request.repo.as_deref().unwrap_or("").trim();
    let file_name = request.file_name.as_deref().unwrap_or("").trim();
    settings::validate_model_repo(repo)?;
    settings::validate_model_file(file_name)?;

    let stem = file_name
        .get(..file_name.len().saturating_sub(5))
        .unwrap_or(file_name);
    let requested_name = request
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(stem);
    settings::validate_name(requested_name, "model name")?;

    let base = {
        let slug = slugify(requested_name);
        if slug.len() >= 2 {
            slug
        } else {
            "local-model".to_string()
        }
    };
    let mut id = base.clone();
    let mut suffix = 2;
    while installed(&id) {
        id = format!("{base}-{suffix}");
        suffix += 1;
        if suffix > 999 {
            return Err(
                "Couldn't choose an ID for this model. Rename it and try again.".to_string(),
            );
        }
    }

    Ok(LlmModel {
        id,
        display_name: requested_name.to_string(),
        repo: repo.to_string(),
        file_name: file_name.to_string(),
        params: "Custom".to_string(),
        quant: quant_from_file(file_name),
        context_window: 4096,
        size_bytes: 0,
    })
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug.truncate(40);
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn quant_from_file(file_name: &str) -> String {
    const QUANTS: [&str; 13] = [
        "IQ4_XS", "Q4_K_M", "Q4_K_S", "Q5_K_M", "Q5_K_S", "Q6_K", "Q8_0", "Q3_K_M", "Q2_K", "BF16",
        "F16", "Q4_0", "Q5_0",
    ];
    let upper = file_name.to_ascii_uppercase();
    for quant in QUANTS {
        if upper.contains(quant) {
            return quant.to_string();
        }
    }
    "GGUF".to_string()
}

#[tauri::command]
pub async fn remove_model(app: AppHandle, model_id: String) -> Result<LlmSnapshot, String> {
    let state = app.state::<AppState>();

    {
        let runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if runtime.downloads.contains_key(&model_id) {
            return Err(
                "Wait for this model to finish downloading before removing it.".to_string(),
            );
        }
    }

    let _guard = state.llm_lock.lock().await;

    let model = state
        .settings()
        .llm_models
        .into_iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("Unknown local model `{model_id}`."))?;

    let active = {
        let runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.active_model.clone()
    };
    if active.as_deref() == Some(model_id.as_str()) {
        stop_locked(&app, &state).await;
    }

    // Delete the files first so a failed delete leaves the model registered
    // and the user can try again.
    let directory = models_dir(&app)?.join(&model_id);
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| {
            format!(
                "Couldn't delete the files for {}: {error}",
                model.display_name
            )
        })?;
    }

    {
        let _settings_guard = state.settings_lock.lock().await;
        let mut next = state.settings();
        next.llm_models.retain(|existing| existing.id != model_id);
        let next = settings::validate_and_normalize(next)?;
        settings::save(state.settings_path(), &next)?;
        state.replace_settings(next.clone());
        let _ = app.emit("settings-changed", &next);
    }

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.model_errors.remove(&model_id);
        runtime.downloads.remove(&model_id);
    }

    let directory = models_dir(&app)?.join(&model_id);
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| {
            format!(
                "{} was unregistered but its files couldn't be deleted. {error}",
                model.display_name
            )
        })?;
    }

    publish(&app).await;
    snapshot(&app).await
}

#[tauri::command]
pub async fn start_model(app: AppHandle, model_id: String) -> Result<LlmSnapshot, String> {
    let state = app.state::<AppState>();
    let _guard = state.llm_lock.lock().await;

    if !engine_exe(&app)?.is_file() {
        return Err("Install the llama.cpp engine from the Models page first.".to_string());
    }

    let settings = state.settings();
    let model = settings
        .llm_models
        .iter()
        .find(|model| model.id == model_id)
        .cloned()
        .ok_or_else(|| format!("Unknown local model `{model_id}`."))?;
    let model_file = models_dir(&app)?.join(&model.id).join(&model.file_name);
    if !model_file.is_file() {
        return Err(format!(
            "{} isn't fully downloaded. Reinstall it from Models.",
            model.display_name
        ));
    }

    let (api_key, key_file) = ensure_api_key(&app).await?;
    stop_locked(&app, &state).await;

    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.model_errors.remove(&model_id);
    }

    let port = settings.llm_port;
    if std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
        return Err(format!(
            "Port {port} is already in use. Close whatever is using it and try again."
        ));
    }

    let log = log_path(&app)?;
    if let Some(parent) = log.parent() {
        ensure_directory(parent)?;
    }
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|error| format!("Couldn't open the llama-server log. {error}"))?;
    let log_error = log_file
        .try_clone()
        .map_err(|error| format!("Couldn't prepare the llama-server log. {error}"))?;

    let engine = engine_dir(&app)?;
    let mut command = tokio::process::Command::new(engine_exe(&app)?);
    command
        .current_dir(&engine)
        .arg("--model")
        .arg(&model_file)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--alias")
        .arg(&model.id)
        .arg("--api-key-file")
        .arg(&key_file)
        .arg("--ctx-size")
        .arg(model.context_window.to_string())
        .arg("--no-webui")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_error));
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let mut child = command
        .spawn()
        .map_err(|error| format!("Couldn't start llama-server. {error}"))?;
    if let Some(pid) = child.id() {
        if let Ok(path) = pid_path(&app) {
            let _ = fs::write(path, pid.to_string());
        }
    }

    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| format!("Couldn't prepare the llama-server check. {error}"))?;
    let health = format!("http://127.0.0.1:{port}/health");
    let deadline = Instant::now() + START_TIMEOUT;

    loop {
        if let Ok(Some(status)) = child.try_wait() {
            cleanup_pid(&app);
            let tail = log_tail(&log);
            let message = if tail.is_empty() {
                format!("llama-server exited before it was ready ({status}).")
            } else {
                format!("llama-server exited before it was ready. {tail}")
            };
            set_model_error(&state, &model_id, &message);
            publish(&app).await;
            return Err(message);
        }

        let ready = client
            .get(&health)
            .bearer_auth(&api_key)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false);
        if ready {
            break;
        }

        if Instant::now() >= deadline {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            cleanup_pid(&app);
            let tail = log_tail(&log);
            let message = if tail.is_empty() {
                "llama-server didn't become ready within 3 minutes.".to_string()
            } else {
                format!("llama-server didn't become ready within 3 minutes. {tail}")
            };
            set_model_error(&state, &model_id, &message);
            publish(&app).await;
            return Err(message);
        }

        tokio::time::sleep(Duration::from_millis(600)).await;
    }

    state.llm_server.lock().await.replace(child);
    {
        let mut runtime = state
            .llm_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.active_model = Some(model.id.clone());
        runtime.model_errors.remove(&model.id);
    }
    publish(&app).await;
    snapshot(&app).await
}

fn set_model_error(state: &AppState, model_id: &str, message: &str) {
    let mut runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime
        .model_errors
        .insert(model_id.to_string(), message.to_string());
}

#[tauri::command]
pub async fn stop_model(app: AppHandle) -> Result<LlmSnapshot, String> {
    let state = app.state::<AppState>();
    let _guard = state.llm_lock.lock().await;
    stop_locked(&app, &state).await;
    publish(&app).await;
    snapshot(&app).await
}

async fn stop_locked(app: &AppHandle, state: &AppState) {
    let mut server = state.llm_server.lock().await;
    if let Some(mut child) = server.take() {
        let _ = child.start_kill();
        if tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .is_err()
        {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
    drop(server);

    cleanup_pid(app);
    let mut runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime.active_model = None;
}

fn cleanup_pid(app: &AppHandle) {
    if let Ok(path) = pid_path(app) {
        let _ = fs::remove_file(path);
    }
}

fn log_tail(path: &Path) -> String {
    const MAX_BYTES: u64 = 8192;
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(length.saturating_sub(MAX_BYTES)));
    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents);
    let lines = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>();
    lines.into_iter().rev().collect::<Vec<_>>().join(" ")
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    destination: &Path,
    slot: ProgressSlot,
    fallback_total: u64,
) -> Result<(), String> {
    let result = download_body(app, url, destination, &slot, fallback_total).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(destination).await;
    }
    result
}

async fn download_body(
    app: &AppHandle,
    url: &str,
    destination: &Path,
    slot: &ProgressSlot,
    fallback_total: u64,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("Docklet/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Couldn't prepare the download. {error}"))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Couldn't reach Hugging Face. {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = match status.as_u16() {
            401 | 403 => " This model may be gated or private; use a public GGUF repository.",
            404 => " That model file wasn't found in the repository.",
            _ => "",
        };
        return Err(format!("The download failed with HTTP {status}.{detail}"));
    }

    let total = response.content_length().unwrap_or(fallback_total);
    set_progress(app, slot, 0, total);
    emit_progress(app, slot, 0, total);

    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Couldn't create the model folder. {error}"))?;
    }
    let _ = tokio::fs::remove_file(destination).await;
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("Couldn't create the model file. {error}"))?;

    let mut stream = response.bytes_stream();
    let mut downloaded = 0u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    loop {
        let chunk = match tokio::time::timeout(Duration::from_secs(60), stream.next()).await {
            Ok(Some(chunk)) => {
                chunk.map_err(|error| format!("The download stopped unexpectedly. {error}"))?
            }
            Ok(None) => break,
            Err(_) => {
                return Err(
                    "The download stalled for 60 seconds. Check your connection and try again."
                        .to_string(),
                );
            }
        };

        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Couldn't write the model file. {error}"))?;
        downloaded += chunk.len() as u64;
        set_progress(app, slot, downloaded, total);
        if last_emit.elapsed() >= Duration::from_millis(250) {
            emit_progress(app, slot, downloaded, total);
            last_emit = Instant::now();
        }
    }
    file.flush()
        .await
        .map_err(|error| format!("Couldn't finish the model file. {error}"))?;
    drop(file);

    if total > 0 && downloaded != total {
        return Err(format!(
            "The download ended early ({downloaded} of {total} bytes). Try again."
        ));
    }

    set_progress(app, slot, downloaded, total);
    emit_progress(app, slot, downloaded, total);
    Ok(())
}

fn set_progress(app: &AppHandle, slot: &ProgressSlot, downloaded: u64, total: u64) {
    let state = app.state::<AppState>();
    let mut runtime = state
        .llm_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match slot {
        ProgressSlot::Engine => {
            runtime.engine_downloaded = downloaded;
            runtime.engine_total = total;
        }
        ProgressSlot::Model(id) => {
            if let Some(pending) = runtime.downloads.get_mut(id) {
                pending.downloaded = downloaded;
                pending.total = total;
            }
        }
    }
}

fn emit_progress(app: &AppHandle, slot: &ProgressSlot, downloaded: u64, total: u64) {
    let id = match slot {
        ProgressSlot::Engine => "engine".to_string(),
        ProgressSlot::Model(id) => id.clone(),
    };
    let _ = app.emit(
        PROGRESS_EVENT,
        LlmProgressPayload {
            id,
            downloaded_bytes: downloaded,
            total_bytes: total,
        },
    );
}

pub fn cleanup_on_startup(app: &AppHandle) {
    if let Ok(path) = pid_path(app) {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if process_is_llama_server(pid) {
                    kill_pid(pid);
                }
            }
        }
        let _ = fs::remove_file(path);
    }

    if let Ok(models) = models_dir(app) {
        let Ok(entries) = fs::read_dir(&models) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Ok(files) = fs::read_dir(&path) else {
                continue;
            };
            for file in files.flatten() {
                if file
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".part")
                {
                    let _ = fs::remove_file(file.path());
                }
            }
        }
    }
}

pub fn shutdown(app: &AppHandle) {
    if let Ok(path) = pid_path(app) {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if process_is_llama_server(pid) {
                    kill_pid(pid);
                }
            }
        }
        let _ = fs::remove_file(path);
    }
}

#[cfg(windows)]
fn process_is_llama_server(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;

    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .creation_flags(0x0800_0000)
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("llama-server.exe")
        })
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn process_is_llama_server(_pid: u32) -> bool {
    false
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    use std::os::windows::process::CommandExt;

    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F", "/T"])
        .creation_flags(0x0800_0000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_pid(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_valid() {
        let entries = catalog();
        let mut ids = std::collections::HashSet::new();
        for entry in entries {
            assert!(ids.insert(entry.id.clone()), "duplicate id {}", entry.id);
            assert!(entry.id.len() >= 2 && entry.id.len() <= 48);
            assert!(
                entry
                    .id
                    .chars()
                    .all(|character| character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || character == '-')
            );
            assert!(entry.file_name.ends_with(".gguf"));
            assert!(entry.size_bytes > 0);
        }
    }

    #[test]
    fn catalog_entries_pass_settings_validation() {
        for entry in catalog() {
            assert!(
                crate::settings::validate_model_repo(&entry.repo).is_ok(),
                "invalid repo {}",
                entry.repo
            );
            assert!(
                crate::settings::validate_model_file(&entry.file_name).is_ok(),
                "invalid file {}",
                entry.file_name
            );
            assert!(
                crate::settings::validate_name(&entry.display_name, "model name").is_ok(),
                "invalid display name {}",
                entry.display_name
            );
        }
    }

    #[test]
    fn slugify_produces_safe_ids() {
        assert_eq!(slugify("Qwen 2.5 Coder!"), "qwen-2-5-coder");
        assert_eq!(slugify("  --  "), "");
        assert_eq!(slugify("a".repeat(80).as_str()).len(), 40);
    }

    #[test]
    fn quant_is_detected_from_the_file_name() {
        assert_eq!(quant_from_file("Qwen2.5-7B-Instruct-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(quant_from_file("model-f16.gguf"), "F16");
        assert_eq!(quant_from_file("model.gguf"), "GGUF");
    }
}
