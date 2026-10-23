use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;

const DOCKER_TIMEOUT: Duration = Duration::from_secs(25);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerSearchResult {
    pub name: String,
    pub description: String,
    pub star_count: u64,
    pub is_official: bool,
}

#[derive(Debug, Deserialize)]
struct DockerSearchRow {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "StarCount", default)]
    star_count: String,
    #[serde(rename = "IsOfficial", default)]
    is_official: String,
}

impl DockerSearchRow {
    fn into_result(self) -> DockerSearchResult {
        DockerSearchResult {
            name: self.name,
            description: self.description,
            star_count: self.star_count.parse().unwrap_or(0),
            is_official: self.is_official.eq_ignore_ascii_case("true"),
        }
    }
}

#[derive(Debug)]
pub enum DockerError {
    CliMissing,
    Timeout(String),
    Io(String),
    Failed { command: String, detail: String },
    Parse(String),
}

impl DockerError {
    pub fn user_message(&self) -> String {
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
            Self::Failed { detail, .. } if is_docker_unavailable(detail) => {
                "Docker Desktop isn't running. Start it, then try again.".to_string()
            }
            Self::Failed { command, detail } if is_port_conflict(detail) => {
                format!(
                    "A local port needed by this service is already in use. Free the port, then try again. (docker {command})"
                )
            }
            Self::Failed { detail, .. } => clean_detail(detail),
        }
    }

    pub fn is_unavailable(&self) -> bool {
        match self {
            Self::CliMissing | Self::Timeout(_) => true,
            Self::Failed { detail, .. } => is_docker_unavailable(detail),
            _ => false,
        }
    }
}

pub fn is_docker_unavailable(detail: &str) -> bool {
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

pub fn is_port_conflict(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("port is already allocated")
        || detail.contains("bind for 127.0.0.1")
        || detail.contains("ports are not available")
        || detail.contains("address already in use")
}

pub fn is_missing_error(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("no such object")
        || detail.contains("no such container")
        || detail.contains("no such volume")
        || detail.contains("no such image")
}

pub fn clean_detail(detail: &str) -> String {
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

pub async fn run_docker_with_timeout(
    args: &[&str],
    timeout: Duration,
) -> Result<String, DockerError> {
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
        command: context,
        detail,
    })
}

pub async fn run_docker(args: &[&str]) -> Result<String, DockerError> {
    run_docker_with_timeout(args, DOCKER_TIMEOUT).await
}

pub async fn search_images(query: &str) -> Result<Vec<DockerSearchResult>, DockerError> {
    let output = run_docker_with_timeout(
        &["search", query, "--limit", "30", "--format", "{{json .}}"],
        Duration::from_secs(20),
    )
    .await?;

    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let row: DockerSearchRow = serde_json::from_str(line).map_err(|error| {
            DockerError::Parse(format!("Couldn't read Docker Hub search results: {error}"))
        })?;
        if seen.insert(row.name.clone()) {
            results.push(row.into_result());
        }
    }

    Ok(results)
}

pub async fn ensure_docker() -> Result<(), String> {
    run_docker_with_timeout(
        &["info", "--format", "{{.ServerVersion}}"],
        Duration::from_secs(6),
    )
    .await
    .map(|_| ())
    .map_err(|error| error.user_message())
}

pub async fn inspect_container(name: &str) -> Result<Option<Value>, DockerError> {
    let output = match run_docker(&["inspect", "--type", "container", name]).await {
        Ok(output) => output,
        Err(DockerError::Failed { detail, .. }) if is_missing_error(&detail) => return Ok(None),
        Err(error) => return Err(error),
    };

    let values: Vec<Value> = serde_json::from_str(&output)
        .map_err(|error| DockerError::Parse(format!("Couldn't read container details: {error}")))?;

    Ok(values.into_iter().next())
}

pub async fn inspect_volume(name: &str) -> Result<Option<Value>, DockerError> {
    let output = match run_docker(&["volume", "inspect", name]).await {
        Ok(output) => output,
        Err(DockerError::Failed { detail, .. }) if is_missing_error(&detail) => return Ok(None),
        Err(error) => return Err(error),
    };

    let values: Vec<Value> = serde_json::from_str(&output)
        .map_err(|error| DockerError::Parse(format!("Couldn't read volume details: {error}")))?;

    Ok(values.into_iter().next())
}

pub async fn ensure_image(image: &str) -> Result<(), DockerError> {
    if run_docker(&["image", "inspect", image]).await.is_ok() {
        return Ok(());
    }

    run_docker_with_timeout(&["pull", image], Duration::from_secs(300)).await?;
    Ok(())
}

pub async fn container_logs(name: &str) -> String {
    match run_docker_with_timeout(&["logs", "--tail", "20", name], Duration::from_secs(5)).await {
        Ok(logs) if !logs.is_empty() => {
            let logs = clean_detail(&logs);
            format!("\n\nRecent service output:\n{logs}")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_docker_daemon_failures() {
        let error = DockerError::Failed {
            command: "docker info".into(),
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
    fn recognises_missing_resource_errors() {
        assert!(is_missing_error("Error: No such object: docklet-redis"));
        assert!(is_missing_error("Error: No such container: docklet-redis"));
        assert!(is_missing_error(
            "Error response from daemon: get docklet-redis-data: no such volume"
        ));
        assert!(is_missing_error("No such image: redis:8-alpine"));
        assert!(!is_missing_error("permission denied"));
    }

    #[test]
    fn recognises_port_conflicts() {
        assert!(is_port_conflict(
            "Bind for 127.0.0.1:6379 failed: port is already allocated"
        ));
        assert!(is_port_conflict("address already in use"));
    }

    #[test]
    fn parses_docker_search_rows() {
        let row: DockerSearchRow = serde_json::from_str(
            r#"{"Description":"Official Redis","IsOfficial":"true","Name":"redis","StarCount":"13624"}"#,
        )
        .expect("search row should parse");
        let result = row.into_result();

        assert_eq!(result.name, "redis");
        assert_eq!(result.description, "Official Redis");
        assert_eq!(result.star_count, 13624);
        assert!(result.is_official);
    }
}
