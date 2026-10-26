use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

pub const SETTINGS_VERSION: u32 = 2;
pub const DEFAULT_LLM_PORT: u16 = 11435;
pub const BUILTIN_SERVICES: [(&str, u16); 4] = [
    ("redis", 6379),
    ("postgres", 5432),
    ("mysql", 3306),
    ("mongodb", 27017),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmModel {
    pub id: String,
    pub display_name: String,
    pub repo: String,
    pub file_name: String,
    pub params: String,
    pub quant: String,
    pub context_window: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomService {
    pub id: String,
    pub name: String,
    pub image: String,
    pub container_port: u16,
    pub host_port: u16,
    pub url_template: String,
    #[serde(default)]
    pub volume_path: Option<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub health_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub version: u32,
    pub hide_popup_on_blur: bool,
    pub service_ports: BTreeMap<String, u16>,
    pub custom_services: Vec<CustomService>,
    pub llm_models: Vec<LlmModel>,
    pub llm_port: u16,
    pub llm_api_key: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            hide_popup_on_blur: true,
            service_ports: default_service_ports(),
            custom_services: Vec::new(),
            llm_models: Vec::new(),
            llm_port: DEFAULT_LLM_PORT,
            llm_api_key: String::new(),
        }
    }
}

impl AppSettings {
    pub fn port_for(&self, id: &str) -> Option<u16> {
        if let Some(port) = self.service_ports.get(id) {
            return Some(*port);
        }
        BUILTIN_SERVICES
            .iter()
            .find(|(service_id, _)| *service_id == id)
            .map(|(_, port)| *port)
    }
}

pub fn default_service_ports() -> BTreeMap<String, u16> {
    BUILTIN_SERVICES
        .iter()
        .map(|(id, port)| ((*id).to_string(), *port))
        .collect()
}

pub fn load(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Couldn't read settings from {}: {error}", path.display()))?;
    let settings: AppSettings = serde_json::from_str(&contents)
        .map_err(|error| format!("Couldn't parse {}: {error}", path.display()))?;

    validate_and_normalize(settings)
}

pub fn save(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Couldn't create the settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Couldn't serialize settings: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Couldn't write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Couldn't replace {}: {error}", path.display()))?;

    Ok(())
}

pub fn validate_and_normalize(mut settings: AppSettings) -> Result<AppSettings, String> {
    settings.version = SETTINGS_VERSION;
    settings.service_ports = normalize_builtin_ports(&settings.service_ports)?;

    let mut service_ids = BUILTIN_SERVICES
        .iter()
        .map(|(id, _)| (*id).to_string())
        .collect::<HashSet<_>>();
    let mut ports = settings
        .service_ports
        .values()
        .copied()
        .collect::<HashSet<_>>();
    let mut names = HashSet::new();

    for service in &mut settings.custom_services {
        service.id = service.id.trim().to_ascii_lowercase();
        service.name = service.name.trim().to_string();
        service.image = service.image.trim().to_string();
        service.url_template = service.url_template.trim().to_string();
        service.volume_path = service
            .volume_path
            .take()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty());
        service.health_command = service
            .health_command
            .take()
            .map(|command| command.trim().to_string())
            .filter(|command| !command.is_empty());

        validate_service_id(&service.id)?;
        if !service_ids.insert(service.id.clone()) {
            return Err(format!(
                "The service ID `{}` is already in use.",
                service.id
            ));
        }

        validate_name(&service.name, "service name")?;
        let name_key = service.name.to_ascii_lowercase();
        if !names.insert(name_key) {
            return Err(format!(
                "The service name `{}` is already in use.",
                service.name
            ));
        }

        validate_image(&service.image)?;
        validate_port(service.host_port, "host port")?;
        validate_port(service.container_port, "container port")?;
        if !ports.insert(service.host_port) {
            return Err(format!(
                "Host port {} is already assigned to another service.",
                service.host_port
            ));
        }

        validate_url_template(&service.url_template)?;

        if let Some(path) = &service.volume_path {
            validate_container_path(path)?;
        }

        let mut env_keys = HashSet::new();
        for env in &mut service.env {
            env.key = env.key.trim().to_string();
            env.value = env.value.trim_end_matches(['\r', '\n']).to_string();
            validate_env_var(env)?;
            if !env_keys.insert(env.key.clone()) {
                return Err(format!(
                    "Environment variable `{}` is repeated for {}.",
                    env.key, service.name
                ));
            }
        }

        if let Some(command) = &service.health_command {
            if command.chars().count() > 500 || command.contains('\0') || command.contains('\n') {
                return Err(format!(
                    "The health check for {} is too long or contains unsupported characters.",
                    service.name
                ));
            }
        }
    }

    validate_port(settings.llm_port, "local LLM port")?;

    let mut llm_ids = HashSet::new();
    for model in &mut settings.llm_models {
        model.id = model.id.trim().to_ascii_lowercase();
        model.display_name = model.display_name.trim().to_string();
        model.repo = model.repo.trim().to_string();
        model.file_name = model.file_name.trim().to_string();
        model.params = model.params.trim().to_string();
        model.quant = model.quant.trim().to_string();

        if model.id.len() < 2
            || model.id.len() > 48
            || !model.id.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
            || model.id.starts_with('-')
            || model.id.ends_with('-')
        {
            return Err(
                "A local LLM ID must be 2–48 lowercase letters, numbers, or hyphens.".into(),
            );
        }
        if !llm_ids.insert(model.id.clone()) {
            return Err(format!(
                "The local LLM ID `{}` is already in use.",
                model.id
            ));
        }

        validate_name(&model.display_name, "model name")?;
        validate_model_repo(&model.repo)?;
        validate_model_file(&model.file_name)?;

        if model.context_window == 0 || model.context_window > 1_048_576 {
            return Err(format!(
                "The context size for {} must be between 1 and 1048576 tokens.",
                model.display_name
            ));
        }
        if model.params.chars().count() > 24 || model.quant.chars().count() > 24 {
            return Err(format!(
                "The model details for {} are too long.",
                model.display_name
            ));
        }
    }

    if !settings.llm_api_key.is_empty() {
        let key = &settings.llm_api_key;
        if key.len() < 16
            || key.len() > 256
            || key
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err("The local LLM API key is invalid.".to_string());
        }
    }

    Ok(settings)
}

pub(crate) fn validate_model_repo(repo: &str) -> Result<(), String> {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err("A model repository must look like `owner/repo`.".to_string());
    };
    if owner.is_empty()
        || name.is_empty()
        || owner.len() > 96
        || name.len() > 96
        || !owner.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err("That Hugging Face repository name isn't valid.".to_string());
    }
    Ok(())
}

pub(crate) fn validate_model_file(file_name: &str) -> Result<(), String> {
    if file_name.len() < 6
        || file_name.len() > 160
        || !file_name.to_ascii_lowercase().ends_with(".gguf")
        || file_name.contains("..")
        || !file_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(
            "A model file must be a `.gguf` file name using letters, numbers, dots, dashes, or underscores."
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_builtin_ports(
    configured: &BTreeMap<String, u16>,
) -> Result<BTreeMap<String, u16>, String> {
    let mut normalized = BTreeMap::new();
    let mut seen = HashSet::new();

    for (id, default_port) in BUILTIN_SERVICES {
        let port = configured.get(id).copied().unwrap_or(default_port);
        validate_port(port, "host port")?;
        if !seen.insert(port) {
            return Err(format!(
                "Host port {port} is assigned to more than one built-in service."
            ));
        }
        normalized.insert(id.to_string(), port);
    }

    Ok(normalized)
}

fn validate_service_id(id: &str) -> Result<(), String> {
    if id.len() < 2 || id.len() > 32 {
        return Err("A custom service ID must be 2–32 characters.".to_string());
    }
    if !id.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Err(
            "A custom service ID can contain only lowercase letters, numbers, and hyphens."
                .to_string(),
        );
    }
    if id.starts_with('-') || id.ends_with('-') {
        return Err("A custom service ID can't start or end with a hyphen.".to_string());
    }
    if BUILTIN_SERVICES.iter().any(|(builtin, _)| *builtin == id) {
        return Err(format!("`{id}` is reserved for a built-in service."));
    }
    Ok(())
}

pub(crate) fn validate_name(value: &str, label: &str) -> Result<(), String> {
    let length = value.chars().count();
    if length == 0 || length > 48 {
        return Err(format!("The {label} must be 1–48 characters."));
    }
    if value.contains(['\0', '\r', '\n']) {
        return Err(format!("The {label} contains unsupported characters."));
    }
    Ok(())
}

fn validate_image(image: &str) -> Result<(), String> {
    if image.is_empty() || image.len() > 200 {
        return Err("A Docker image reference must be 1–200 characters.".to_string());
    }
    if image.starts_with('-') || image.chars().any(char::is_whitespace) {
        return Err("That Docker image reference isn't valid.".to_string());
    }
    Ok(())
}

fn validate_port(port: u16, label: &str) -> Result<(), String> {
    if port == 0 {
        return Err(format!("The {label} must be between 1 and 65535."));
    }
    Ok(())
}

fn validate_url_template(template: &str) -> Result<(), String> {
    if template.is_empty() || template.len() > 300 {
        return Err("The URL template must be 1–300 characters.".to_string());
    }
    if !template.contains("{port}") {
        return Err("The URL template must contain `{port}`.".to_string());
    }
    if template.matches('{').count() != 1
        || template.matches('}').count() != 1
        || template.replace("{port}", "").contains(['{', '}'])
    {
        return Err("The URL template can only use the `{port}` placeholder.".to_string());
    }
    if template.chars().any(char::is_whitespace) {
        return Err("The URL template can't contain spaces.".to_string());
    }

    let Some((scheme, rest)) = template.split_once("://") else {
        return Err("The URL template must include a scheme such as `http://`.".to_string());
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        || !scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
    {
        return Err("The URL template has an invalid scheme.".to_string());
    }
    if !rest.contains("127.0.0.1") && !rest.contains("localhost") {
        return Err("Custom service URLs must point at 127.0.0.1 or localhost.".to_string());
    }
    if !rest.contains("{port}") {
        return Err("The URL template must put `{port}` in the local address.".to_string());
    }

    Ok(())
}

fn validate_container_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("A container volume path must be absolute (start with `/`).".to_string());
    }
    if path.contains('\0') || path.contains("\r") || path.contains("\n") {
        return Err("The container volume path contains unsupported characters.".to_string());
    }
    if path
        .split('/')
        .any(|component| component == ".." || component == ".")
    {
        return Err("The container volume path can't use `.` or `..` segments.".to_string());
    }
    Ok(())
}

fn validate_env_var(env: &EnvVar) -> Result<(), String> {
    let mut chars = env.key.chars();
    let Some(first) = chars.next() else {
        return Err("Environment variable names can't be empty.".to_string());
    };
    if !(first.is_ascii_uppercase() || first == '_')
        || !chars.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(format!(
            "Environment variable `{}` must use uppercase letters, numbers, and underscores.",
            env.key
        ));
    }
    if env.value.len() > 4096 || env.value.contains('\0') {
        return Err(format!(
            "Environment variable `{}` has an invalid value.",
            env.key
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_the_builtin_ports() {
        let settings = AppSettings::default();
        assert_eq!(settings.port_for("redis"), Some(6379));
        assert_eq!(settings.port_for("postgres"), Some(5432));
        assert_eq!(settings.port_for("mysql"), Some(3306));
        assert_eq!(settings.port_for("mongodb"), Some(27017));
    }

    #[test]
    fn normalizes_missing_and_unknown_builtin_ports() {
        let mut settings = AppSettings::default();
        settings.service_ports.clear();
        settings.service_ports.insert("redis".into(), 6380);
        settings.service_ports.insert("unknown".into(), 9999);

        let normalized = validate_and_normalize(settings).unwrap();
        assert_eq!(normalized.port_for("redis"), Some(6380));
        assert_eq!(normalized.port_for("postgres"), Some(5432));
        assert!(!normalized.service_ports.contains_key("unknown"));
    }

    #[test]
    fn rejects_duplicate_host_ports() {
        let mut settings = AppSettings::default();
        settings.service_ports.insert("redis".into(), 5432);

        assert!(
            validate_and_normalize(settings)
                .unwrap_err()
                .contains("more than one")
        );
    }

    #[test]
    fn accepts_a_valid_custom_service() {
        let mut settings = AppSettings::default();
        settings.custom_services.push(CustomService {
            id: "http-echo".into(),
            name: "HTTP Echo".into(),
            image: "hashicorp/http-echo:1.0".into(),
            container_port: 5678,
            host_port: 5678,
            url_template: "http://127.0.0.1:{port}".into(),
            volume_path: None,
            env: vec![EnvVar {
                key: "ECHO_TEXT".into(),
                value: "hello".into(),
            }],
            health_command: None,
        });

        let normalized = validate_and_normalize(settings).unwrap();
        assert_eq!(normalized.custom_services.len(), 1);
        assert_eq!(normalized.custom_services[0].id, "http-echo");
    }

    #[test]
    fn rejects_reserved_service_ids() {
        let mut settings = AppSettings::default();
        settings.custom_services.push(CustomService {
            id: "redis".into(),
            name: "Another Redis".into(),
            image: "redis:8-alpine".into(),
            container_port: 6379,
            host_port: 6381,
            url_template: "redis://127.0.0.1:{port}".into(),
            volume_path: Some("/data".into()),
            env: vec![],
            health_command: Some("redis-cli ping".into()),
        });

        assert!(
            validate_and_normalize(settings)
                .unwrap_err()
                .contains("reserved")
        );
    }

    #[test]
    fn rejects_remote_custom_urls() {
        let mut settings = AppSettings::default();
        settings.custom_services.push(CustomService {
            id: "remote".into(),
            name: "Remote".into(),
            image: "nginx:alpine".into(),
            container_port: 80,
            host_port: 8080,
            url_template: "http://example.com:{port}".into(),
            volume_path: None,
            env: vec![],
            health_command: None,
        });

        assert!(
            validate_and_normalize(settings)
                .unwrap_err()
                .contains("127.0.0.1")
        );
    }

    #[test]
    fn persists_and_reloads_settings_atomically() {
        let directory =
            std::env::temp_dir().join(format!("docklet-settings-test-{}", std::process::id()));
        let path = directory.join("settings.json");
        let mut settings = AppSettings::default();
        settings.hide_popup_on_blur = false;
        settings.service_ports.insert("redis".into(), 6380);

        save(&path, &settings).unwrap();
        let loaded = load(&path).unwrap();

        assert!(!loaded.hide_popup_on_blur);
        assert_eq!(loaded.port_for("redis"), Some(6380));
        assert!(!directory.join("settings.json.tmp").exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn loads_legacy_settings_without_llm_fields() {
        let legacy = r#"{
            "version": 1,
            "hidePopupOnBlur": true,
            "servicePorts": {
                "redis": 6379,
                "postgres": 5432,
                "mysql": 3306,
                "mongodb": 27017
            },
            "customServices": []
        }"#;
        let parsed: AppSettings = serde_json::from_str(legacy).unwrap();
        let normalized = validate_and_normalize(parsed).unwrap();

        assert_eq!(normalized.version, SETTINGS_VERSION);
        assert!(normalized.llm_models.is_empty());
        assert_eq!(normalized.llm_port, DEFAULT_LLM_PORT);
        assert!(normalized.llm_api_key.is_empty());
    }

    #[test]
    fn validates_model_repositories() {
        assert!(validate_model_repo("owner/repo").is_ok());
        assert!(validate_model_repo("owner.name/model_repo").is_ok());
        assert!(validate_model_repo("owner").is_err());
        assert!(validate_model_repo("owner/repo/extra").is_err());
        assert!(validate_model_repo("/repo").is_err());
        assert!(validate_model_repo("own er/repo").is_err());
        assert!(validate_model_repo("owner/re po").is_err());
    }

    #[test]
    fn validates_model_file_names() {
        assert!(validate_model_file("Model-Q4_K_M.gguf").is_ok());
        assert!(validate_model_file("model.gguf").is_ok());
        assert!(validate_model_file("model.bin").is_err());
        assert!(validate_model_file("model.gguf.txt").is_err());
        assert!(validate_model_file("../model.gguf").is_err());
        assert!(validate_model_file("model file.gguf").is_err());
        assert!(validate_model_file("m.ggu").is_err());
    }

    #[test]
    fn rejects_duplicate_llm_model_ids() {
        let mut settings = AppSettings::default();
        settings.llm_models.push(LlmModel {
            id: "same-id".into(),
            display_name: "First".into(),
            repo: "owner/repo".into(),
            file_name: "First-Q4_K_M.gguf".into(),
            params: "1B".into(),
            quant: "Q4_K_M".into(),
            context_window: 4096,
            size_bytes: 100,
        });
        settings.llm_models.push(LlmModel {
            id: "same-id".into(),
            display_name: "Second".into(),
            repo: "owner/repo".into(),
            file_name: "Second-Q4_K_M.gguf".into(),
            params: "1B".into(),
            quant: "Q4_K_M".into(),
            context_window: 4096,
            size_bytes: 100,
        });

        assert!(
            validate_and_normalize(settings)
                .unwrap_err()
                .contains("already in use")
        );
    }
}
