use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::docker::{self, DockerError};
use crate::settings::{AppSettings, CustomService, EnvVar};

const MANAGED_LABEL: &str = "com.docklet.managed";
const SERVICE_LABEL: &str = "com.docklet.service";
const CONFIG_LABEL: &str = "com.docklet.config-hash";
const READY_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    Running,
    Stopped,
    Missing,
    Starting,
    DockerUnavailable,
    Unmanaged,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub kind: String,
    pub image: String,
    pub state: ServiceState,
    pub container_state: Option<String>,
    pub port: Option<u16>,
    pub host_port: u16,
    pub container_port: u16,
    pub url: Option<String>,
    pub managed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicesSnapshot {
    pub docker_available: bool,
    pub services: Vec<ServiceStatus>,
}

#[derive(Debug, Clone)]
pub struct ServiceDefinition {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub kind: String,
    pub image: String,
    pub host_port: u16,
    pub container_port: u16,
    pub container_name: String,
    pub volume_name: String,
    pub volume_path: Option<String>,
    pub env: Vec<EnvVar>,
    pub url_template: String,
    pub health_command: Option<String>,
}

#[derive(Debug, Clone)]
struct ContainerInfo {
    name: String,
    state: String,
    running: bool,
    managed: bool,
    service: Option<String>,
    config_hash: Option<String>,
    health: Option<String>,
    ports: Vec<(u16, u16)>,
}

impl ContainerInfo {
    fn host_port_for(&self, container_port: u16) -> Option<u16> {
        self.ports
            .iter()
            .find(|(container, _)| *container == container_port)
            .map(|(_, host)| *host)
    }
}

pub fn definitions(settings: &AppSettings) -> Vec<ServiceDefinition> {
    let mut services = vec![
        ServiceDefinition {
            id: "redis".into(),
            name: "Redis".into(),
            icon: "redis".into(),
            kind: "builtin".into(),
            image: "redis:8-alpine".into(),
            host_port: settings.port_for("redis").unwrap_or(6379),
            container_port: 6379,
            container_name: "docklet-redis".into(),
            volume_name: "docklet-redis-data".into(),
            volume_path: Some("/data".into()),
            env: vec![],
            url_template: "redis://127.0.0.1:{port}".into(),
            health_command: Some("redis-cli ping".into()),
        },
        ServiceDefinition {
            id: "postgres".into(),
            name: "PostgreSQL".into(),
            icon: "postgresql".into(),
            kind: "builtin".into(),
            image: "postgres:17-alpine".into(),
            host_port: settings.port_for("postgres").unwrap_or(5432),
            container_port: 5432,
            container_name: "docklet-postgres".into(),
            volume_name: "docklet-postgres-data".into(),
            volume_path: Some("/var/lib/postgresql/data".into()),
            env: vec![
                env("POSTGRES_DB", "docklet"),
                env("POSTGRES_USER", "docklet"),
                env("POSTGRES_PASSWORD", "docklet"),
            ],
            url_template: "postgresql://docklet:docklet@127.0.0.1:{port}/docklet".into(),
            health_command: Some("pg_isready -U docklet -d docklet".into()),
        },
        ServiceDefinition {
            id: "mysql".into(),
            name: "MySQL".into(),
            icon: "mysql".into(),
            kind: "builtin".into(),
            image: "mysql:8.4".into(),
            host_port: settings.port_for("mysql").unwrap_or(3306),
            container_port: 3306,
            container_name: "docklet-mysql".into(),
            volume_name: "docklet-mysql-data".into(),
            volume_path: Some("/var/lib/mysql".into()),
            env: vec![
                env("MYSQL_ROOT_PASSWORD", "docklet"),
                env("MYSQL_DATABASE", "docklet"),
                env("MYSQL_USER", "docklet"),
                env("MYSQL_PASSWORD", "docklet"),
            ],
            url_template: "mysql://docklet:docklet@127.0.0.1:{port}/docklet".into(),
            health_command: Some("mysqladmin ping -h 127.0.0.1 --silent".into()),
        },
        ServiceDefinition {
            id: "mongodb".into(),
            name: "MongoDB".into(),
            icon: "mongodb".into(),
            kind: "builtin".into(),
            image: "mongo:8".into(),
            host_port: settings.port_for("mongodb").unwrap_or(27017),
            container_port: 27017,
            container_name: "docklet-mongodb".into(),
            volume_name: "docklet-mongodb-data".into(),
            volume_path: Some("/data/db".into()),
            env: vec![env("MONGO_INITDB_DATABASE", "docklet")],
            url_template: "mongodb://127.0.0.1:{port}/docklet".into(),
            health_command: Some(
                "mongosh --quiet --eval \"db.adminCommand({ ping: 1 }).ok\"".into(),
            ),
        },
    ];

    services.extend(settings.custom_services.iter().map(definition_for_custom));
    services
}

fn env(key: &str, value: &str) -> EnvVar {
    EnvVar {
        key: key.to_string(),
        value: value.to_string(),
    }
}

fn definition_for_custom(service: &CustomService) -> ServiceDefinition {
    ServiceDefinition {
        id: service.id.clone(),
        name: service.name.clone(),
        icon: "docker".into(),
        kind: "custom".into(),
        image: service.image.clone(),
        host_port: service.host_port,
        container_port: service.container_port,
        container_name: container_name(&service.id),
        volume_name: volume_name(&service.id),
        volume_path: service.volume_path.clone(),
        env: service.env.clone(),
        url_template: service.url_template.clone(),
        health_command: service.health_command.clone(),
    }
}

pub fn container_name(id: &str) -> String {
    format!("docklet-{id}")
}

pub fn volume_name(id: &str) -> String {
    format!("docklet-{id}-data")
}

pub fn find_definition(settings: &AppSettings, id: &str) -> Result<ServiceDefinition, String> {
    definitions(settings)
        .into_iter()
        .find(|service| service.id == id)
        .ok_or_else(|| format!("Unknown Docklet service `{id}`."))
}

impl ServiceDefinition {
    pub fn url_for_port(&self, port: u16) -> String {
        self.url_template.replace("{port}", &port.to_string())
    }

    fn config_hash(&self) -> String {
        let env = self
            .env
            .iter()
            .map(|item| format!("{}={}", item.key, item.value))
            .collect::<Vec<_>>()
            .join("\u{1f}");
        let source = format!(
            "{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}",
            self.image,
            self.host_port,
            self.container_port,
            self.volume_path.as_deref().unwrap_or(""),
            env,
            self.health_command.as_deref().unwrap_or(""),
            self.id
        );
        stable_hash(&source)
    }

    fn has_volume(&self) -> bool {
        self.volume_path.is_some()
    }
}

fn stable_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub async fn list_statuses(settings: &AppSettings) -> ServicesSnapshot {
    let services = definitions(settings);

    if let Err(error) = docker::ensure_docker().await {
        return ServicesSnapshot {
            docker_available: false,
            services: services
                .iter()
                .map(|service| unavailable_status(service, &error))
                .collect(),
        };
    }

    let mut statuses = Vec::with_capacity(services.len());
    for service in &services {
        match docker::inspect_container(&service.container_name).await {
            Ok(Some(value)) => match parse_container(&value) {
                Ok(container) => statuses.push(status_from_container(service, &container)),
                Err(error) => statuses.push(error_status(service, error.user_message())),
            },
            Ok(None) => statuses.push(missing_status(service)),
            Err(error) if error.is_unavailable() => {
                return ServicesSnapshot {
                    docker_available: false,
                    services: services
                        .iter()
                        .map(|service| unavailable_status(service, &error.user_message()))
                        .collect(),
                };
            }
            Err(error) => statuses.push(error_status(service, error.user_message())),
        }
    }

    ServicesSnapshot {
        docker_available: true,
        services: statuses,
    }
}

pub async fn start(settings: &AppSettings, id: &str) -> Result<(), String> {
    let service = find_definition(settings, id)?;
    docker::ensure_docker().await?;
    start_service(&service).await
}

async fn start_service(service: &ServiceDefinition) -> Result<(), String> {
    let existing = docker::inspect_container(&service.container_name)
        .await
        .map_err(|error| error.user_message())?
        .map(|value| parse_container(&value))
        .transpose()
        .map_err(|error| error.user_message())?;

    if let Some(container) = existing {
        ensure_owned(service, &container)?;

        if needs_recreate(service, &container) {
            remove_container(service).await?;
            create_fresh(service).await?;
        } else {
            if container.state == "paused" {
                docker::run_docker(&["unpause", &service.container_name])
                    .await
                    .map_err(|error| error.user_message())?;
            } else if !container.running {
                docker::run_docker(&["start", &service.container_name])
                    .await
                    .map_err(|error| error.user_message())?;
            }
        }
    } else {
        create_fresh(service).await?;
    }

    wait_until_ready(service).await
}

async fn create_fresh(service: &ServiceDefinition) -> Result<(), String> {
    ensure_volume(service).await?;
    docker::ensure_image(&service.image)
        .await
        .map_err(|error| error.user_message())?;
    create_container(service)
        .await
        .map_err(|error| error.user_message())
}

pub async fn stop(settings: &AppSettings, id: &str) -> Result<(), String> {
    let service = find_definition(settings, id)?;
    docker::ensure_docker().await?;

    let Some(container) = docker::inspect_container(&service.container_name)
        .await
        .map_err(|error| error.user_message())?
        .map(|value| parse_container(&value))
        .transpose()
        .map_err(|error| error.user_message())?
    else {
        return Ok(());
    };

    ensure_owned(&service, &container)?;
    if matches!(container.state.as_str(), "exited" | "created") {
        return Ok(());
    }

    if container.state == "paused" {
        docker::run_docker(&["unpause", &service.container_name])
            .await
            .map_err(|error| error.user_message())?;
    }

    docker::run_docker_with_timeout(
        &["stop", "--time", "15", &service.container_name],
        Duration::from_secs(30),
    )
    .await
    .map_err(|error| error.user_message())?;

    Ok(())
}

pub async fn reset(settings: &AppSettings, id: &str) -> Result<(), String> {
    let service = find_definition(settings, id)?;
    docker::ensure_docker().await?;
    remove_container(&service).await?;
    remove_volume(&service).await?;
    start_service(&service).await
}

pub async fn remove_custom_resources(settings: &AppSettings, id: &str) -> Result<(), String> {
    let service = find_definition(settings, id)?;
    if service.kind != "custom" {
        return Err("Built-in services can't be removed.".to_string());
    }

    docker::ensure_docker().await?;
    remove_container(&service).await?;
    remove_volume(&service).await?;
    Ok(())
}

async fn ensure_volume(service: &ServiceDefinition) -> Result<(), String> {
    if !service.has_volume() {
        return Ok(());
    }

    match docker::inspect_volume(&service.volume_name)
        .await
        .map_err(|error| error.user_message())?
    {
        Some(volume) => {
            let managed = volume
                .pointer(&format!("/Labels/{MANAGED_LABEL}"))
                .and_then(Value::as_str)
                == Some("true");
            if !managed {
                return Err(format!(
                    "A Docker volume named `{}` exists, but Docklet doesn't manage it. Rename or remove it before starting {}.",
                    service.volume_name, service.name
                ));
            }
            Ok(())
        }
        None => {
            let managed_label = format!("{MANAGED_LABEL}=true");
            let service_label = format!("{SERVICE_LABEL}={}", service.id);
            docker::run_docker(&[
                "volume",
                "create",
                "--label",
                &managed_label,
                "--label",
                &service_label,
                &service.volume_name,
            ])
            .await
            .map_err(|error| error.user_message())?;
            Ok(())
        }
    }
}

async fn create_container(service: &ServiceDefinition) -> Result<(), DockerError> {
    let managed_label = format!("{MANAGED_LABEL}=true");
    let service_label = format!("{SERVICE_LABEL}={}", service.id);
    let config_label = format!("{CONFIG_LABEL}={}", service.config_hash());
    let port_binding = format!("127.0.0.1:{}:{}", service.host_port, service.container_port);

    let mut args = vec![
        "run".to_string(),
        "--detach".to_string(),
        "--name".to_string(),
        service.container_name.clone(),
        "--label".to_string(),
        managed_label,
        "--label".to_string(),
        service_label,
        "--label".to_string(),
        config_label,
        "--restart".to_string(),
        "unless-stopped".to_string(),
        "--publish".to_string(),
        port_binding,
        "--log-driver".to_string(),
        "json-file".to_string(),
        "--log-opt".to_string(),
        "max-size=5m".to_string(),
        "--log-opt".to_string(),
        "max-file=2".to_string(),
    ];

    if let (Some(path), true) = (&service.volume_path, service.has_volume()) {
        args.push("--volume".to_string());
        args.push(format!("{}:{path}", service.volume_name));
    }

    for item in &service.env {
        args.push("--env".to_string());
        args.push(format!("{}={}", item.key, item.value));
    }

    if let Some(health) = &service.health_command {
        args.extend([
            "--health-cmd".to_string(),
            health.clone(),
            "--health-interval".to_string(),
            "5s".to_string(),
            "--health-timeout".to_string(),
            "4s".to_string(),
            "--health-retries".to_string(),
            "10".to_string(),
            "--health-start-period".to_string(),
            "20s".to_string(),
        ]);
    }

    args.push(service.image.clone());

    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    docker::run_docker(&refs).await?;
    Ok(())
}

async fn wait_until_ready(service: &ServiceDefinition) -> Result<(), String> {
    let deadline = Instant::now() + READY_TIMEOUT;

    while Instant::now() < deadline {
        let container = match docker::inspect_container(&service.container_name).await {
            Ok(Some(value)) => parse_container(&value).map_err(|error| error.user_message())?,
            Ok(None) => {
                return Err(format!(
                    "{} disappeared before it became ready. Try starting it again.",
                    service.name
                ));
            }
            Err(error) if error.is_unavailable() => return Err(error.user_message()),
            Err(_) => {
                sleep(Duration::from_millis(350)).await;
                continue;
            }
        };

        if !container.running && !matches!(container.state.as_str(), "created" | "restarting") {
            let logs = docker::container_logs(&service.container_name).await;
            return Err(format!(
                "{} stopped before it became ready.{logs}",
                service.name
            ));
        }

        match container.health.as_deref() {
            Some("healthy") => return Ok(()),
            Some("unhealthy") => {}
            Some(_) => {}
            None => {
                if tcp_ready(service.host_port).await {
                    return Ok(());
                }
            }
        }

        sleep(Duration::from_millis(350)).await;
    }

    let logs = docker::container_logs(&service.container_name).await;
    Err(format!(
        "{} is running, but it didn't become ready within {} seconds.{logs}",
        service.name,
        READY_TIMEOUT.as_secs()
    ))
}

async fn tcp_ready(port: u16) -> bool {
    tokio::time::timeout(
        Duration::from_millis(700),
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}

async fn remove_container(service: &ServiceDefinition) -> Result<(), String> {
    let Some(container) = docker::inspect_container(&service.container_name)
        .await
        .map_err(|error| error.user_message())?
        .map(|value| parse_container(&value))
        .transpose()
        .map_err(|error| error.user_message())?
    else {
        return Ok(());
    };

    ensure_owned(service, &container)?;
    docker::run_docker_with_timeout(
        &["rm", "--force", &service.container_name],
        Duration::from_secs(30),
    )
    .await
    .map_err(|error| error.user_message())?;
    Ok(())
}

async fn remove_volume(service: &ServiceDefinition) -> Result<(), String> {
    if !service.has_volume() {
        return Ok(());
    }

    let Some(volume) = docker::inspect_volume(&service.volume_name)
        .await
        .map_err(|error| error.user_message())?
    else {
        return Ok(());
    };

    let managed = volume
        .pointer(&format!("/Labels/{MANAGED_LABEL}"))
        .and_then(Value::as_str)
        == Some("true");
    if !managed {
        return Err(format!(
            "A Docker volume named `{}` exists, but Docklet doesn't manage it. Reset was cancelled.",
            service.volume_name
        ));
    }

    docker::run_docker_with_timeout(
        &["volume", "rm", &service.volume_name],
        Duration::from_secs(30),
    )
    .await
    .map_err(|error| error.user_message())?;
    Ok(())
}

fn ensure_owned(service: &ServiceDefinition, container: &ContainerInfo) -> Result<(), String> {
    if !container.managed {
        return Err(format!(
            "A container named `{}` exists, but Docklet doesn't manage it. Rename or remove it before continuing.",
            service.container_name
        ));
    }
    if let Some(owner) = &container.service {
        if owner != &service.id {
            return Err(format!(
                "The container `{}` belongs to another Docklet service (`{owner}`).",
                service.container_name
            ));
        }
    }
    Ok(())
}

fn needs_recreate(service: &ServiceDefinition, container: &ContainerInfo) -> bool {
    if let Some(hash) = &container.config_hash {
        return hash != &service.config_hash();
    }

    container.host_port_for(service.container_port) != Some(service.host_port)
}

fn parse_container(value: &Value) -> Result<ContainerInfo, DockerError> {
    let name = value
        .get("Name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();
    let state = value
        .pointer("/State/Status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let running = value
        .pointer("/State/Running")
        .and_then(Value::as_bool)
        .unwrap_or(state == "running");
    let health = value
        .pointer("/State/Health/Status")
        .and_then(Value::as_str)
        .map(str::to_string);
    let managed = value
        .pointer(&format!("/Config/Labels/{MANAGED_LABEL}"))
        .and_then(Value::as_str)
        == Some("true");
    let service = value
        .pointer(&format!("/Config/Labels/{SERVICE_LABEL}"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let config_hash = value
        .pointer(&format!("/Config/Labels/{CONFIG_LABEL}"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let ports = parse_port_bindings(value);

    Ok(ContainerInfo {
        name,
        state,
        running,
        managed,
        service,
        config_hash,
        health,
        ports,
    })
}

fn parse_port_bindings(value: &Value) -> Vec<(u16, u16)> {
    let mut ports = Vec::new();
    for pointer in ["/HostConfig/PortBindings", "/NetworkSettings/Ports"] {
        let Some(bindings) = value.pointer(pointer).and_then(Value::as_object) else {
            continue;
        };
        for (container_port, hosts) in bindings {
            let Some(container_port) = container_port
                .split('/')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
            else {
                continue;
            };
            let Some(hosts) = hosts.as_array() else {
                continue;
            };
            for host in hosts {
                if let Some(port) = host
                    .get("HostPort")
                    .and_then(Value::as_str)
                    .and_then(|port| port.parse::<u16>().ok())
                {
                    if !ports.contains(&(container_port, port)) {
                        ports.push((container_port, port));
                    }
                }
            }
        }
    }
    ports
}

fn status_from_container(service: &ServiceDefinition, container: &ContainerInfo) -> ServiceStatus {
    let port = container.host_port_for(service.container_port);
    let url = port.map(|port| service.url_for_port(port));
    let mut status = ServiceStatus {
        id: service.id.clone(),
        name: service.name.clone(),
        icon: service.icon.clone(),
        kind: service.kind.clone(),
        image: service.image.clone(),
        state: ServiceState::Running,
        container_state: Some(container.state.clone()),
        port,
        host_port: service.host_port,
        container_port: service.container_port,
        url,
        managed: container.managed,
        error: None,
    };

    if !container.managed {
        status.state = ServiceState::Unmanaged;
        status.error = Some(format!(
            "A container named `{}` exists, but Docklet doesn't manage it.",
            container.name
        ));
    } else if let Some(owner) = &container.service {
        if owner != &service.id {
            status.state = ServiceState::Unmanaged;
            status.error = Some(format!(
                "The container `{}` belongs to another Docklet service (`{owner}`).",
                container.name
            ));
        }
    }

    if status.state == ServiceState::Unmanaged {
        return status;
    }

    match container.state.as_str() {
        "running" if container.health.as_deref() == Some("starting") => {
            status.state = ServiceState::Starting;
        }
        "running" => {
            if container.health.as_deref() == Some("unhealthy") {
                status.state = ServiceState::Error;
                status.error = Some(format!(
                    "{} is running but failed its health check. Check Docker, then reset it if needed.",
                    service.name
                ));
            } else if port.is_none() {
                status.state = ServiceState::Error;
                status.error = Some(format!(
                    "{} is running without a published local port. Reset it.",
                    service.name
                ));
            } else {
                status.state = ServiceState::Running;
            }
        }
        "restarting" => status.state = ServiceState::Starting,
        "paused" => {
            status.state = ServiceState::Error;
            status.error = Some(format!(
                "{} is paused. Resume it, or reset it from settings.",
                service.name
            ));
        }
        "dead" => {
            status.state = ServiceState::Error;
            status.error = Some(format!(
                "{} is in a failed Docker state. Reset it from settings.",
                service.name
            ));
        }
        "created" | "exited" => status.state = ServiceState::Stopped,
        other => {
            status.state = ServiceState::Error;
            status.error = Some(format!("{} container state: {other}.", service.name));
        }
    }

    status
}

fn missing_status(service: &ServiceDefinition) -> ServiceStatus {
    ServiceStatus {
        id: service.id.clone(),
        name: service.name.clone(),
        icon: service.icon.clone(),
        kind: service.kind.clone(),
        image: service.image.clone(),
        state: ServiceState::Missing,
        container_state: None,
        port: None,
        host_port: service.host_port,
        container_port: service.container_port,
        url: None,
        managed: false,
        error: None,
    }
}

fn unavailable_status(service: &ServiceDefinition, error: &str) -> ServiceStatus {
    let mut status = missing_status(service);
    status.state = ServiceState::DockerUnavailable;
    status.error = Some(error.to_string());
    status
}

fn error_status(service: &ServiceDefinition, error: String) -> ServiceStatus {
    let mut status = missing_status(service);
    status.state = ServiceState::Error;
    status.error = Some(error);
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_inspect(container_port: u16, host_port: u16) -> Value {
        json!([{
            "Name": "/docklet-redis",
            "State": { "Status": "running", "Running": true, "Health": { "Status": "healthy" } },
            "Config": { "Labels": {
                "com.docklet.managed": "true",
                "com.docklet.service": "redis",
                "com.docklet.config-hash": "abc"
            }},
            "HostConfig": {
                "PortBindings": {
                    format!("{container_port}/tcp"): [{ "HostIp": "127.0.0.1", "HostPort": host_port.to_string() }]
                }
            },
            "NetworkSettings": { "Ports": {} }
        }])
    }

    #[test]
    fn parses_managed_running_container() {
        let value = sample_inspect(6379, 6379);
        let container = parse_container(&value[0]).unwrap();

        assert_eq!(container.name, "docklet-redis");
        assert_eq!(container.state, "running");
        assert!(container.running);
        assert!(container.managed);
        assert_eq!(container.service.as_deref(), Some("redis"));
        assert_eq!(container.host_port_for(6379), Some(6379));
        assert_eq!(container.health.as_deref(), Some("healthy"));
    }

    #[test]
    fn honours_the_actual_mapped_port() {
        let value = sample_inspect(6379, 49153);
        let service = find_definition(&AppSettings::default(), "redis").unwrap();
        let container = parse_container(&value[0]).unwrap();
        let status = status_from_container(&service, &container);

        assert_eq!(status.port, Some(49153));
        assert_eq!(status.url.as_deref(), Some("redis://127.0.0.1:49153"));
    }

    #[test]
    fn falls_back_to_network_settings_ports() {
        let value = json!({
            "Name": "/docklet-redis",
            "State": { "Status": "running", "Running": true },
            "NetworkSettings": {
                "Ports": {
                    "6379/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "6380" }]
                }
            }
        });
        let container = parse_container(&value).unwrap();
        assert_eq!(container.host_port_for(6379), Some(6380));
    }

    #[test]
    fn builtin_definitions_keep_the_existing_redis_resources() {
        let services = definitions(&AppSettings::default());
        let redis = services
            .iter()
            .find(|service| service.id == "redis")
            .unwrap();
        assert_eq!(redis.container_name, "docklet-redis");
        assert_eq!(redis.volume_name, "docklet-redis-data");
        assert_eq!(services.len(), 4);
    }

    #[test]
    fn config_hash_changes_when_a_port_changes() {
        let mut settings = AppSettings::default();
        let before = find_definition(&settings, "redis").unwrap().config_hash();
        settings.service_ports.insert("redis".into(), 6380);
        let after = find_definition(&settings, "redis").unwrap().config_hash();
        assert_ne!(before, after);
    }

    #[test]
    fn custom_services_get_managed_resource_names() {
        let mut settings = AppSettings::default();
        settings.custom_services.push(CustomService {
            id: "http-echo".into(),
            name: "HTTP Echo".into(),
            image: "hashicorp/http-echo:1.0".into(),
            container_port: 5678,
            host_port: 5678,
            url_template: "http://127.0.0.1:{port}".into(),
            volume_path: None,
            env: vec![],
            health_command: None,
        });
        let service = find_definition(&settings, "http-echo").unwrap();
        assert_eq!(service.container_name, "docklet-http-echo");
        assert_eq!(service.volume_name, "docklet-http-echo-data");
        assert_eq!(service.url_for_port(5678), "http://127.0.0.1:5678");
    }

    #[test]
    fn identifies_unmanaged_containers() {
        let mut value = sample_inspect(6379, 6379);
        value[0]["Config"]["Labels"] = json!({});
        let container = parse_container(&value[0]).unwrap();
        let service = find_definition(&AppSettings::default(), "redis").unwrap();
        let status = status_from_container(&service, &container);
        assert_eq!(status.state, ServiceState::Unmanaged);
        assert!(status.error.unwrap().contains("doesn't manage"));
    }

    #[test]
    fn recognises_starting_health_state() {
        let mut value = sample_inspect(6379, 6379);
        value[0]["State"]["Health"]["Status"] = json!("starting");
        let container = parse_container(&value[0]).unwrap();
        let service = find_definition(&AppSettings::default(), "redis").unwrap();
        let status = status_from_container(&service, &container);
        assert_eq!(status.state, ServiceState::Starting);
    }
}
