use std::env;
use std::fs;

use crate::controller::AppController;
use crate::event_stream::{
    EventStreamHandle, WS_TOKEN_ENV, WS_URL_ENV, apply_ws_auth_token, derive_authenticated_ws_url,
    spawn_websocket_event_stream,
};
use crate::omegon_control::OmegonStartupInfo;

pub const SNAPSHOT_FILE_ENV: &str = "AUSPEX_REMOTE_SNAPSHOT_PATH";
pub const STATE_URL_ENV: &str = "AUSPEX_OMEGON_STATE_URL";
pub const STARTUP_URL_ENV: &str = "AUSPEX_OMEGON_STARTUP_URL";
pub const EXPECTED_CONTROL_PLANE_SCHEMA: u32 = 1;
pub const DEFAULT_STATE_URL: &str = "http://127.0.0.1:7842/api/state";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapSource {
    MockDefault,
    SnapshotFile { path: String },
    HttpState { url: String },
}

#[derive(Clone, Debug)]
pub struct BootstrapResult {
    pub controller: AppController,
    pub source: BootstrapSource,
    pub note: Option<String>,
    pub event_stream: Option<EventStreamHandle>,
}

impl BootstrapResult {
    fn mock(note: Option<String>) -> Self {
        Self {
            controller: AppController::default(),
            source: BootstrapSource::MockDefault,
            note,
            event_stream: None,
        }
    }

    fn compatibility_failure(note: String) -> Self {
        let mut controller = AppController::default();
        controller.set_scenario(crate::fixtures::DevScenario::CompatibilityFailure);
        controller.set_bootstrap_note(Some(note.clone()));
        Self {
            controller,
            source: BootstrapSource::MockDefault,
            note: Some(note),
            event_stream: None,
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
            if error.contains("control-plane schema") {
                return BootstrapResult::compatibility_failure(error);
            }
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

pub fn startup_url_from_env() -> Option<String> {
    non_empty_env(STARTUP_URL_ENV)
}

pub fn websocket_url_from_env() -> Option<String> {
    non_empty_env(WS_URL_ENV)
}

pub fn websocket_token_from_env() -> Option<String> {
    non_empty_env(WS_TOKEN_ENV)
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
        event_stream: None,
    })
}

pub fn bootstrap_from_http_state(url: &str) -> Result<BootstrapResult, String> {
    let startup = fetch_startup_info(url).ok();
    if let Some(startup) = startup.as_ref() {
        validate_startup_info(startup)?;
    }
    let state_url = startup
        .as_ref()
        .map(|startup| startup.state_url.as_str())
        .filter(|state_url| !state_url.is_empty())
        .unwrap_or(url);

    let response = reqwest::blocking::get(state_url)
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("state endpoint returned error: {error}"))?;

    let body = response
        .text()
        .map_err(|error| format!("could not read response body: {error}"))?;
    let controller = AppController::from_remote_snapshot_json(&body)
        .map_err(|error| format!("invalid state payload: {error}"))?;
    let ws_token = websocket_token_from_env();
    let ws_url = startup
        .as_ref()
        .map(|startup| startup.ws_url.clone())
        .filter(|ws_url| !ws_url.is_empty())
        .or_else(|| {
            websocket_url_from_env()
                .map(|url| apply_ws_auth_token(&url, ws_token.as_deref()))
                .transpose()
                .ok()
                .flatten()
        })
        .or_else(|| derive_authenticated_ws_url(state_url, ws_token.as_deref()).ok())
        .unwrap_or_else(|| {
            DEFAULT_STATE_URL
                .replace("http://", "ws://")
                .replace("https://", "wss://")
                .replace("/api/state", "/ws")
        });
    let event_stream = Some(spawn_websocket_event_stream(&ws_url));
    let note = startup
        .as_ref()
        .map(|startup| {
            format!(
                "Attached via Omegon startup discovery at {} (auth: {} via {}). Streaming events from {}",
                startup_url_from_state_url(url),
                startup.auth_mode,
                startup.auth_source,
                ws_url
            )
        })
        .unwrap_or_else(|| {
            format!("Attached to Omegon state endpoint at {state_url}. Streaming events from {ws_url}")
        });

    Ok(BootstrapResult {
        controller,
        source: BootstrapSource::HttpState {
            url: state_url.to_string(),
        },
        note: Some(note),
        event_stream,
    })
}

fn fetch_startup_info(state_url: &str) -> Result<OmegonStartupInfo, String> {
    let startup_url =
        startup_url_from_env().unwrap_or_else(|| startup_url_from_state_url(state_url));
    let response = reqwest::blocking::get(&startup_url)
        .map_err(|error| format!("startup discovery request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("startup discovery returned error: {error}"))?;
    let body = response
        .text()
        .map_err(|error| format!("could not read startup discovery response: {error}"))?;
    serde_json::from_str::<OmegonStartupInfo>(&body)
        .map_err(|error| format!("invalid startup discovery payload: {error}"))
}

fn startup_url_from_state_url(state_url: &str) -> String {
    state_url.replace("/api/state", "/api/startup")
}

fn validate_startup_info(startup: &OmegonStartupInfo) -> Result<(), String> {
    if startup.schema_version != EXPECTED_CONTROL_PLANE_SCHEMA {
        return Err(format!(
            "Auspex requires control-plane schema {}, but Omegon reported schema {}.",
            EXPECTED_CONTROL_PLANE_SCHEMA, startup.schema_version
        ));
    }

    Ok(())
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

        assert_eq!(
            result.source,
            BootstrapSource::SnapshotFile {
                path: path.to_string_lossy().to_string()
            }
        );
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
        assert_eq!(
            state_url_from_env(),
            env::var(STATE_URL_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        );
    }

    #[test]
    fn startup_url_derives_from_state_url() {
        assert_eq!(
            startup_url_from_state_url("http://127.0.0.1:7842/api/state"),
            "http://127.0.0.1:7842/api/startup"
        );
    }

    #[test]
    fn startup_schema_mismatch_is_rejected() {
        let error = validate_startup_info(&OmegonStartupInfo {
            schema_version: 2,
            addr: "127.0.0.1:7842".into(),
            http_base: "http://127.0.0.1:7842".into(),
            state_url: "http://127.0.0.1:7842/api/state".into(),
            ws_url: "ws://127.0.0.1:7842/ws?token=test".into(),
            token: "test".into(),
            auth_mode: "signed-attach".into(),
            auth_source: "keyring".into(),
        })
        .unwrap_err();

        assert!(error.contains("requires control-plane schema 1"));
    }

    #[test]
    fn default_state_url_is_local_omegon_endpoint() {
        assert_eq!(DEFAULT_STATE_URL, "http://127.0.0.1:7842/api/state");
    }

    #[test]
    fn websocket_token_env_is_opt_in() {
        let key = format!("{}_TEST_ONLY", WS_TOKEN_ENV);
        assert_eq!(non_empty_env(&key), None);
        assert_eq!(
            websocket_token_from_env(),
            env::var(WS_TOKEN_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        );
    }

    #[test]
    fn explicit_websocket_url_can_be_tokenized() {
        let ws_url = apply_ws_auth_token("ws://127.0.0.1:7842/ws", Some("secret-token")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=secret-token");
    }

    fn temp_snapshot_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("auspex-bootstrap-{}-{}", std::process::id(), name));
        path
    }
}
