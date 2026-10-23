use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::sleep;

pub const CONTAINER_NAME: &str = "docklet-redis";
pub const VOLUME_NAME: &str = "docklet-redis-data";
pub const IMAGE: &str = "redis:8-alpine";
pub const HOST_PORT: u16 = 6379;
pub const MANAGED_LABEL: &str = "com.docklet.managed";

const DOCKER_TIMEOUT: Duration = Duration::from_secs(20);
const READY_TIMEOUT: Duration = Duration::from_secs(25);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RedisState {
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
pub struct RedisStatus {
    pub state: RedisState,
    pub container_state: Option<String>,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub docker_available: bool,
    pub managed: bool,
    pub error: Option<String>,
}

impl RedisStatus {
    fn new(state: RedisState) -> Self {
        Self {
            state,
            container_state: None,
            port: None,
            url: None,
            docker_available: false,
            managed: false,
            error: None,
        }
    }

    fn missing() -> Self {
        Self {
            docker_available: true,
            ..Self::new(RedisState::Missing)
        }
    }
}

#[derive(Debug)]
struct ContainerInfo {
    state: String,
    running: bool,
    managed: bool,
    port: Option<u16>,
}

#[derive(Debug)]
struct VolumeInfo {
    managed: bool,
}

#[derive(Debug)]
enum DockerError {
    CliMissing,
    Timeout(String),
    Io(String),
    Failed { _command: String, detail: String },
    Parse(String),
    Conflict(String),
}

impl DockerError {
    fn user_message(&self) -> String {
        match self {
            Self::CliMissing => {
                "Docker CLI wasn't found. Install Docker Desktop and make sure `docker` is on PATH."
                    .to_string()
            }
            Self::Timeout(context) => format!(
                "Docker didn't respond in time ({context}). Start Docker Desktop, then try again."
            ),
            Self::Io(detail) | Self::Parse(detail) => {
                format!("Docker returned an unexpected response. {detail}")
            }
            Self::Conflict(message) => message.clone(),
            Self::Failed { detail, .. } if is_docker_unavailable(detail) => {
                "Docker Desktop isn't running. Start it, then try again.".to_string()
            }
            Self::Failed { detail, .. } if is_port_conflict(detail) => {
                "Port 6379 is already in use. Free the port, then try again.".to_string()
            }
            Self::Failed { detail, .. } => clean_detail(detail),
        }
    }

    fn is_unavailable(&self) -> bool {
        match self {
            Self::CliMissing | Self::Timeout(_) => true,
            Self::Failed { detail, .. } => is_docker_unavailable(detail),
            _ => false,
        }
    }
}

fn is_docker_unavailable(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    [
        "failed to connect to the docker api",
        "error during connect",
        "cannot connect to the docker daemon",
        "is the docker daemon running",
        "dockerdesktoplinuxengine",
    ]
    .iter()
    .any(|marker| detail.contains(marker))
}

fn is_port_conflict(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("port is already allocated")
        || detail.contains("bind for 127.0.0.1:6379 failed")
        || detail.contains("ports are not available")
}

fn is_missing_error(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("no such object") || detail.contains("no such container")
}

fn clean_detail(detail: &str) -> String {
    let mut detail = detail.replace("\r\n", "\n");
    for prefix in [
        "Error response from daemon: ",
        "Error response from daemon:",
        "Error: ",
    ] {
        if let Some(stripped) = detail.strip_prefix(prefix) {
            detail = stripped.to_string();
        }
    }
    let detail = detail.trim();
    if detail.chars().count() > 600 {
        format!("{}…", detail.chars().take(600).collect::<String>())
    } else if detail.is_empty() {
        "Docker reported an error without additional details.".to_string()
    } else {
        detail.to_string()
    }
}

async fn run_docker_with_timeout(args: &[&str], timeout: Duration) -> Result<String, DockerError> {
    let mut command = Command::new("docker");
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let context = format!("docker {}", args.join(" "));
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| DockerError::Timeout(context.clone()))?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                DockerError::CliMissing
            } else {
                DockerError::Io(error.to_string())
            }
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };

    Err(DockerError::Failed {
        _command: context,
        detail,
    })
}

async fn run_docker(args: &[&str]) -> Result<String, DockerError> {
    run_docker_with_timeout(args, DOCKER_TIMEOUT).await
}

async fn ensure_docker() -> Result<(), String> {
    run_docker_with_timeout(
        &["info", "--format", "{{.ServerVersion}}"],
        Duration::from_secs(6),
    )
    .await
    .map(|_| ())
    .map_err(|error| error.user_message())
}

async fn inspect_container() -> Result<Option<ContainerInfo>, DockerError> {
    let output = match run_docker(&["inspect", "--type", "container", CONTAINER_NAME]).await {
        Ok(output) => output,
        Err(DockerError::Failed { detail, .. }) if is_missing_error(&detail) => return Ok(None),
        Err(error) => return Err(error),
    };

    let values: Vec<Value> = serde_json::from_str(&output)
        .map_err(|error| DockerError::Parse(format!("Couldn't read container details: {error}")))?;
    let Some(value) = values.first() else {
        return Ok(None);
    };

    Ok(Some(parse_container(value)?))
}

fn parse_container(value: &Value) -> Result<ContainerInfo, DockerError> {
    let state = value
        .pointer("/State/Status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let running = value
        .pointer("/State/Running")
        .and_then(Value::as_bool)
        .unwrap_or(state == "running");
    let managed = value
        .pointer(&format!("/Config/Labels/{MANAGED_LABEL}"))
        .and_then(Value::as_str)
        == Some("true");
    let port = parse_port_bindings(value);

    Ok(ContainerInfo {
        state,
        running,
        managed,
        port,
    })
}

fn parse_port_bindings(value: &Value) -> Option<u16> {
    let port_prefix = format!("{HOST_PORT}/");
    for pointer in ["/HostConfig/PortBindings", "/NetworkSettings/Ports"] {
        let Some(bindings) = value.pointer(pointer).and_then(Value::as_object) else {
            continue;
        };
        for (container_port, hosts) in bindings {
            if !container_port.starts_with(&port_prefix) {
                continue;
            }
            let Some(hosts) = hosts.as_array() else {
                continue;
            };
            for host in hosts {
                if let Some(port) = host
                    .get("HostPort")
                    .and_then(Value::as_str)
                    .and_then(|port| port.parse::<u16>().ok())
                {
                    return Some(port);
                }
            }
        }
    }
    None
}

async fn inspect_volume() -> Result<Option<VolumeInfo>, DockerError> {
    let output = match run_docker(&["volume", "inspect", VOLUME_NAME]).await {
        Ok(output) => output,
        Err(DockerError::Failed { detail, .. }) if is_missing_error(&detail) => return Ok(None),
        Err(error) => return Err(error),
    };

    let values: Vec<Value> = serde_json::from_str(&output)
        .map_err(|error| DockerError::Parse(format!("Couldn't read volume details: {error}")))?;
    let Some(value) = values.first() else {
        return Ok(None);
    };
    let managed = value
        .pointer(&format!("/Labels/{MANAGED_LABEL}"))
        .and_then(Value::as_str)
        == Some("true");

    Ok(Some(VolumeInfo { managed }))
}

async fn ensure_volume() -> Result<(), DockerError> {
    match inspect_volume().await? {
        Some(volume) if volume.managed => Ok(()),
        Some(_) => Err(DockerError::Conflict(format!(
            "A Docker volume named `{VOLUME_NAME}` already exists, but Docklet doesn't manage it. Rename that volume before starting Redis."
        ))),
        None => {
            let managed_label = format!("{MANAGED_LABEL}=true");
            run_docker(&["volume", "create", "--label", &managed_label, VOLUME_NAME]).await?;
            Ok(())
        }
    }
}

async fn ensure_image() -> Result<(), DockerError> {
    if run_docker(&["image", "inspect", IMAGE]).await.is_ok() {
        return Ok(());
    }

    run_docker_with_timeout(&["pull", IMAGE], Duration::from_secs(240)).await?;
    Ok(())
}

async fn create_container() -> Result<(), DockerError> {
    let managed_label = format!("{MANAGED_LABEL}=true");
    let port_binding = format!("127.0.0.1:{HOST_PORT}:{HOST_PORT}");
    let volume_binding = format!("{VOLUME_NAME}:/data");

    run_docker(&[
        "run",
        "--detach",
        "--name",
        CONTAINER_NAME,
        "--label",
        &managed_label,
        "--restart",
        "unless-stopped",
        "--publish",
        &port_binding,
        "--volume",
        &volume_binding,
        "--log-driver",
        "json-file",
        "--log-opt",
        "max-size=5m",
        "--log-opt",
        "max-file=2",
        "--health-cmd",
        "redis-cli ping",
        "--health-interval",
        "5s",
        "--health-timeout",
        "3s",
        "--health-retries",
        "5",
        IMAGE,
    ])
    .await?;
    Ok(())
}

async fn wait_until_ready() -> Result<(), String> {
    let deadline = Instant::now() + READY_TIMEOUT;

    while Instant::now() < deadline {
        match run_docker_with_timeout(
            &["exec", CONTAINER_NAME, "redis-cli", "ping"],
            Duration::from_secs(3),
        )
        .await
        {
            Ok(output) if output.contains("PONG") => return Ok(()),
            Ok(_) => {}
            Err(error) if error.is_unavailable() => return Err(error.user_message()),
            Err(_) => {}
        }

        match inspect_container().await {
            Ok(Some(info))
                if !info.running && info.state != "restarting" && info.state != "created" =>
            {
                let logs = container_logs().await;
                return Err(format!("Redis stopped before it became ready.{logs}"));
            }
            Err(error) if error.is_unavailable() => return Err(error.user_message()),
            _ => {}
        }

        sleep(Duration::from_millis(300)).await;
    }

    Err(format!(
        "Redis is running, but it didn't become ready within {} seconds.",
        READY_TIMEOUT.as_secs()
    ))
}

async fn container_logs() -> String {
    match run_docker_with_timeout(
        &["logs", "--tail", "20", CONTAINER_NAME],
        Duration::from_secs(5),
    )
    .await
    {
        Ok(logs) if !logs.is_empty() => {
            let logs = clean_detail(&logs);
            format!("\n\nRecent Redis output:\n{logs}")
        }
        _ => String::new(),
    }
}

fn conflict_from(error: DockerError) -> String {
    error.user_message()
}

pub async fn status() -> RedisStatus {
    match inspect_container().await {
        Ok(Some(info)) => {
            let port = info.port;
            let url = port.map(url_for_port);
            let mut status = RedisStatus {
                state: RedisState::Running,
                container_state: Some(info.state.clone()),
                port,
                url,
                docker_available: true,
                managed: info.managed,
                error: None,
            };

            if !info.managed {
                status.state = RedisState::Unmanaged;
                status.error = Some(format!(
                    "A container named `{CONTAINER_NAME}` exists, but Docklet doesn't manage it. Rename or remove it to continue."
                ));
            } else if info.state == "restarting" {
                status.state = RedisState::Starting;
            } else if info.state == "paused" {
                status.state = RedisState::Error;
                status.error = Some(
                    "The Docklet Redis container is paused. Reset it or resume it from Docker Desktop."
                        .to_string(),
                );
            } else if info.state == "dead" {
                status.state = RedisState::Error;
                status.error =
                    Some("The Docklet Redis container is in a failed state. Reset it.".to_string());
            } else if info.running {
                status.state = RedisState::Running;
                if port.is_none() {
                    status.state = RedisState::Error;
                    status.error = Some(
                        "Redis is running without a published local port. Reset it.".to_string(),
                    );
                }
            } else if matches!(info.state.as_str(), "created" | "exited") {
                status.state = RedisState::Stopped;
            } else {
                status.state = RedisState::Error;
                status.error = Some(format!("Redis container state: {}.", info.state));
            }

            status
        }
        Ok(None) => RedisStatus::missing(),
        Err(error) => {
            let available = !error.is_unavailable();
            RedisStatus {
                state: if available {
                    RedisState::Error
                } else {
                    RedisState::DockerUnavailable
                },
                docker_available: available,
                error: Some(error.user_message()),
                ..RedisStatus::new(RedisState::Error)
            }
        }
    }
}

pub async fn start() -> Result<RedisStatus, String> {
    ensure_docker().await?;
    let container = inspect_container().await.map_err(conflict_from)?;

    match container {
        Some(info) if !info.managed => Err(format!(
            "A container named `{CONTAINER_NAME}` exists, but Docklet doesn't manage it. Rename or remove it to continue."
        )),
        Some(info) => {
            if info.state == "paused" {
                run_docker(&["unpause", CONTAINER_NAME])
                    .await
                    .map_err(conflict_from)?;
            } else if !info.running {
                run_docker(&["start", CONTAINER_NAME])
                    .await
                    .map_err(conflict_from)?;
            }
            wait_until_ready().await?;
            Ok(status().await)
        }
        None => {
            ensure_volume().await.map_err(conflict_from)?;
            ensure_image().await.map_err(conflict_from)?;
            create_container().await.map_err(conflict_from)?;
            wait_until_ready().await?;
            Ok(status().await)
        }
    }
}

pub async fn stop() -> Result<RedisStatus, String> {
    ensure_docker().await?;
    let container = inspect_container().await.map_err(conflict_from)?;

    match container {
        Some(info) if !info.managed => Err(format!(
            "A container named `{CONTAINER_NAME}` exists, but Docklet doesn't manage it. Rename or remove it to continue."
        )),
        Some(info) if info.state != "exited" && info.state != "created" => {
            run_docker_with_timeout(
                &["stop", "--time", "10", CONTAINER_NAME],
                Duration::from_secs(30),
            )
            .await
            .map_err(conflict_from)?;
            Ok(status().await)
        }
        _ => Ok(status().await),
    }
}

pub async fn reset() -> Result<RedisStatus, String> {
    ensure_docker().await?;

    let container = inspect_container().await.map_err(conflict_from)?;
    if let Some(info) = &container {
        if !info.managed {
            return Err(format!(
                "A container named `{CONTAINER_NAME}` exists, but Docklet doesn't manage it. Reset was cancelled."
            ));
        }
    }

    let volume = inspect_volume().await.map_err(conflict_from)?;
    if let Some(volume) = &volume {
        if !volume.managed {
            return Err(format!(
                "A Docker volume named `{VOLUME_NAME}` exists, but Docklet doesn't manage it. Reset was cancelled."
            ));
        }
    }

    if container.is_some() {
        run_docker_with_timeout(&["rm", "--force", CONTAINER_NAME], Duration::from_secs(30))
            .await
            .map_err(conflict_from)?;
    }

    if volume.is_some() {
        run_docker_with_timeout(&["volume", "rm", VOLUME_NAME], Duration::from_secs(30))
            .await
            .map_err(conflict_from)?;
    }

    start().await
}

pub fn url_for_port(port: u16) -> String {
    format!("redis://127.0.0.1:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_inspect() -> Value {
        json!([{
            "State": { "Status": "running", "Running": true },
            "Config": { "Labels": { "com.docklet.managed": "true" } },
            "HostConfig": {
                "PortBindings": {
                    "6379/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "6379" }]
                }
            },
            "NetworkSettings": { "Ports": {} }
        }])
    }

    #[test]
    fn parses_managed_running_container() {
        let value = sample_inspect();
        let info = parse_container(&value[0]).expect("container should parse");

        assert_eq!(info.state, "running");
        assert!(info.running);
        assert!(info.managed);
        assert_eq!(info.port, Some(6379));
    }

    #[test]
    fn honours_the_actual_mapped_port() {
        let mut value = sample_inspect();
        value[0]["HostConfig"]["PortBindings"]["6379/tcp"][0]["HostPort"] = json!("49153");

        let info = parse_container(&value[0]).expect("container should parse");
        assert_eq!(info.port, Some(49153));
        assert_eq!(url_for_port(49153), "redis://127.0.0.1:49153");
    }

    #[test]
    fn falls_back_to_network_settings_ports() {
        let value = json!({
            "State": { "Status": "running" },
            "NetworkSettings": {
                "Ports": {
                    "6379/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "6380" }]
                }
            }
        });

        let info = parse_container(&value).expect("container should parse");
        assert_eq!(info.port, Some(6380));
    }

    #[test]
    fn recognises_an_unmanaged_container() {
        let mut value = sample_inspect();
        value[0]["Config"]["Labels"] = json!({});
        let info = parse_container(&value[0]).expect("container should parse");
        assert!(!info.managed);
    }

    #[test]
    fn recognises_docker_daemon_failures() {
        let error = DockerError::Failed {
            _command: "docker info".into(),
            detail:
                "failed to connect to the docker API at npipe:////./pipe/dockerDesktopLinuxEngine"
                    .into(),
        };
        assert!(is_docker_unavailable(
            "failed to connect to the docker API at npipe:////./pipe/dockerDesktopLinuxEngine"
        ));
        assert!(error.is_unavailable());
        assert_eq!(
            error.user_message(),
            "Docker Desktop isn't running. Start it, then try again."
        );
    }

    #[test]
    fn recognises_missing_container_errors() {
        assert!(is_missing_error("Error: No such object: docklet-redis"));
        assert!(is_missing_error("Error: No such container: docklet-redis"));
        assert!(!is_missing_error("permission denied"));
    }

    #[test]
    fn recognises_port_conflicts() {
        assert!(is_port_conflict(
            "Bind for 127.0.0.1:6379 failed: port is already allocated"
        ));
    }
}
