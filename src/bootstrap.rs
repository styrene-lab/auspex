use std::env;
use std::fs;

use crate::controller::AppController;

pub const SNAPSHOT_FILE_ENV: &str = "AUSPEX_REMOTE_SNAPSHOT_PATH";
pub const STATE_URL_ENV: &str = "AUSPEX_OMEGON_STATE_URL";
pub const DEFAULT_STATE_URL: &str = "http://127.0.0.1:7842/api/state";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapSource {
    MockDefault,
    SnapshotFile { path: String },
    HttpState { url: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapResult {
    pub controller: AppController,
    pub source: BootstrapSource,
    pub note: Option<String>,
}

impl BootstrapResult {
    fn mock(note: Option<String>) -> Self {
        Self {
            controller: AppController::default(),
            source: BootstrapSource::MockDefault,
            note,
        }
    }
}

pub fn bootstrap_controller_from_env() -> BootstrapResult {
    if let Some(path) = snapshot_path_from_env() {
        return bootstrap_from_snapshot_file(&path).unwrap_or_else(|error| {
            let note = format!(
                "Snapshot bootstrap failed for {}: {}. Falling back to mock local session.",
                path, error
            );
            BootstrapResult::mock(Some(note))
        });
    }

    if let Some(url) = state_url_from_env() {
        return bootstrap_from_http_state(&url).unwrap_or_else(|error| {
            let note = format!(
                "HTTP bootstrap failed for {}: {}. Falling back to mock local session.",
                url, error
            );
            BootstrapResult::mock(Some(note))
        });
    }

    BootstrapResult::mock(None)
}

pub fn snapshot_path_from_env() -> Option<String> {
    non_empty_env(SNAPSHOT_FILE_ENV)
}

pub fn state_url_from_env() -> Option<String> {
    non_empty_env(STATE_URL_ENV)
}

pub fn bootstrap_from_snapshot_file(path: &str) -> Result<BootstrapResult, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read snapshot file: {error}"))?;
    let controller = AppController::from_remote_snapshot_json(&contents)
        .map_err(|error| format!("invalid snapshot JSON: {error}"))?;

    Ok(BootstrapResult {
        controller,
        source: BootstrapSource::SnapshotFile {
            path: path.to_string(),
        },
        note: Some(format!("Loaded Omegon snapshot from {path}")),
    })
}

pub fn bootstrap_from_http_state(url: &str) -> Result<BootstrapResult, String> {
    let response = reqwest::blocking::get(url)
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("state endpoint returned error: {error}"))?;

    let body = response
        .text()
        .map_err(|error| format!("could not read response body: {error}"))?;
    let controller = AppController::from_remote_snapshot_json(&body)
        .map_err(|error| format!("invalid state payload: {error}"))?;

    Ok(BootstrapResult {
        controller,
        source: BootstrapSource::HttpState {
            url: url.to_string(),
        },
        note: Some(format!("Attached to Omegon state endpoint at {url}")),
    })
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const REMOTE_SNAPSHOT_JSON: &str = r#"{
        "design": {
            "focused": {
                "id": "auspex-remote",
                "title": "Remote session adapter",
                "status": "implementing",
                "open_questions": [],
                "decisions": 1,
                "children": 0
            },
            "implementing": [{"id": "auspex-remote", "title": "Remote session adapter", "status": "implementing"}],
            "actionable": []
        },
        "openspec": {"totalTasks": 5, "doneTasks": 2},
        "cleave": {"active": false, "totalChildren": 0, "completed": 0, "failed": 0},
        "session": {"turns": 12, "toolCalls": 34, "compactions": 1},
        "harness": {
            "gitBranch": "main",
            "gitDetached": false,
            "thinkingLevel": "medium",
            "capabilityTier": "victory",
            "providers": [{"name": "Anthropic", "authenticated": true, "authMethod": "api-key", "model": "claude-sonnet"}],
            "memoryAvailable": true,
            "cleaveAvailable": true,
            "memoryWarning": null,
            "activeDelegates": []
        }
    }"#;

    #[test]
    fn snapshot_file_bootstrap_builds_remote_controller() {
        let path = temp_snapshot_path("snapshot.json");
        fs::write(&path, REMOTE_SNAPSHOT_JSON).unwrap();

        let result = bootstrap_from_snapshot_file(path.to_str().unwrap()).unwrap();

        assert_eq!(result.source, BootstrapSource::SnapshotFile { path: path.to_string_lossy().to_string() });
        assert!(result.controller.is_remote());
        assert!(result.note.unwrap().contains("Loaded Omegon snapshot"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_snapshot_file_returns_error() {
        let path = temp_snapshot_path("invalid-snapshot.json");
        fs::write(&path, "not json").unwrap();

        let error = bootstrap_from_snapshot_file(path.to_str().unwrap()).unwrap_err();
        assert!(error.contains("invalid snapshot JSON"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn state_url_env_is_opt_in() {
        let key = format!("{}_TEST_ONLY", STATE_URL_ENV);
        assert_eq!(non_empty_env(&key), None);
        assert_eq!(state_url_from_env(), env::var(STATE_URL_ENV).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty()));
    }

    #[test]
    fn default_state_url_is_local_omegon_endpoint() {
        assert_eq!(DEFAULT_STATE_URL, "http://127.0.0.1:7842/api/state");
    }

    fn temp_snapshot_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("auspex-bootstrap-{}-{}", std::process::id(), name));
        path
    }
}
