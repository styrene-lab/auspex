use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crate::controller::AppController;
use crate::event_stream::{
    EventStreamHandle, WS_TOKEN_ENV, WS_URL_ENV, apply_ws_auth_token, derive_authenticated_ws_url,
    spawn_websocket_event_stream,
};
use crate::omegon_control::OmegonStartupInfo;

pub const SNAPSHOT_FILE_ENV: &str = "AUSPEX_REMOTE_SNAPSHOT_PATH";
pub const STATE_URL_ENV: &str = "AUSPEX_OMEGON_STATE_URL";
pub const STARTUP_URL_ENV: &str = "AUSPEX_OMEGON_STARTUP_URL";
/// Explicit path to the Omegon binary. Overrides discovery.
pub const OMEGON_BIN_ENV: &str = "AUSPEX_OMEGON_BIN";
pub const EXPECTED_CONTROL_PLANE_SCHEMA: u32 = 1;
pub const DEFAULT_STATE_URL: &str = "http://127.0.0.1:7842/api/state";

const SPAWN_TIMEOUT: Duration = Duration::from_secs(15);
const SPAWN_POLL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapSource {
    MockDefault,
    SnapshotFile {
        path: String,
    },
    HttpState {
        url: String,
    },
    /// Bootstrap is deferred — Omegon binary found but not yet spawned.
    /// The app should start in StartingOmegon state and complete the
    /// spawn asynchronously via spawn_and_attach_omegon().
    SpawningOmegon {
        binary: String,
    },
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

    /// Initial result returned when Omegon needs to be spawned.
    /// The app shows StartingOmegon state while the async spawn runs.
    pub fn spawning_omegon(binary: PathBuf) -> Self {
        let label = binary.display().to_string();
        let mut controller = AppController::default();
        controller.set_scenario(crate::fixtures::DevScenario::Booting);
        controller.set_bootstrap_note(Some(format!("Starting Omegon at {label}\u{2026}")));
        Self {
            controller,
            source: BootstrapSource::SpawningOmegon { binary: label },
            note: None,
            event_stream: None,
        }
    }
}

pub fn bootstrap_controller_from_env() -> BootstrapResult {
    // 1. Explicit snapshot file wins.
    if let Some(path) = snapshot_path_from_env() {
        return bootstrap_from_snapshot_file(&path).unwrap_or_else(|error| {
            BootstrapResult::mock(Some(format!(
                "Snapshot bootstrap failed for {path}: {error}. Falling back to mock local session."
            )))
        });
    }

    // 2. Explicit state URL — attach without spawning.
    if let Some(url) = state_url_from_env() {
        return bootstrap_from_http_state(&url).unwrap_or_else(|error| {
            if error.contains("control-plane schema") {
                return BootstrapResult::compatibility_failure(error);
            }
            BootstrapResult::mock(Some(format!(
                "HTTP bootstrap failed for {url}: {error}. Falling back to mock local session."
            )))
        });
    }

    // 3. No explicit config — auto-discover or spawn.
    // Try to attach if Omegon is already running at the default address.
    if omegon_is_running() {
        return bootstrap_from_http_state(DEFAULT_STATE_URL).unwrap_or_else(|error| {
            if error.contains("control-plane schema") {
                return BootstrapResult::compatibility_failure(error);
            }
            BootstrapResult::mock(Some(format!(
                "Auto-attach to running Omegon failed: {error}."
            )))
        });
    }

    // Not running — find the binary and return a deferred spawn result.
    // The actual spawn happens asynchronously in the app component so
    // the UI can start immediately in StartingOmegon state.
    if let Some(binary) = find_omegon_binary() {
        return BootstrapResult::spawning_omegon(binary);
    }

    // Omegon not found anywhere.
    BootstrapResult::mock(Some(
        "Omegon not found. Install it or set AUSPEX_OMEGON_BIN to its path.".into(),
    ))
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

/// Check if Omegon is already running at the default address (quick 1s timeout).
fn omegon_is_running() -> bool {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .ok()
        .and_then(|client| client.get(DEFAULT_STATE_URL).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Locate the Omegon binary.
///
/// Priority order:
/// 1. `AUSPEX_OMEGON_BIN` env var — explicit override
/// 2. `~/.cargo/bin/omegon` — default `cargo install` location
/// 3. `which omegon` — PATH lookup
pub fn find_omegon_binary() -> Option<PathBuf> {
    if let Some(path) = non_empty_env(OMEGON_BIN_ENV) {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Check common user-local binary directories. Note: .exists() follows
    // symlinks, so broken symlinks correctly return false.
    if let Ok(home) = std::env::var("HOME") {
        for rel in &[".local/bin/omegon", ".cargo/bin/omegon"] {
            let p = PathBuf::from(&home).join(rel);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // Check common system-wide locations (useful when PATH is stripped in
    // a bundled .app launch context).
    for abs in &["/usr/local/bin/omegon", "/opt/homebrew/bin/omegon"] {
        let p = PathBuf::from(abs);
        if p.exists() {
            return Some(p);
        }
    }

    // Finally, try PATH via `which`.
    if let Ok(output) = std::process::Command::new("which").arg("omegon").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                let p = PathBuf::from(s);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Spawn Omegon, wait for it to accept connections, then bootstrap from it.
/// Called from the app component's use_future after returning SpawningOmegon.
pub fn spawn_and_attach_omegon(binary: &std::path::Path) -> BootstrapResult {
    let label = binary.display().to_string();

    match std::process::Command::new(binary).spawn() {
        Err(error) => BootstrapResult::mock(Some(format!(
            "Could not spawn Omegon at {label}: {error}. Running in mock mode."
        ))),
        Ok(_child) => {
            let startup_url = startup_url_from_state_url(DEFAULT_STATE_URL);
            let deadline = Instant::now() + SPAWN_TIMEOUT;

            while Instant::now() < deadline {
                let ready = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(1))
                    .build()
                    .ok()
                    .and_then(|c| c.get(&startup_url).send().ok())
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);

                if ready {
                    return bootstrap_from_http_state(DEFAULT_STATE_URL).unwrap_or_else(|error| {
                        if error.contains("control-plane schema") {
                            BootstrapResult::compatibility_failure(error)
                        } else {
                            BootstrapResult::mock(Some(format!(
                                "Spawned Omegon at {label} but bootstrap failed: {error}"
                            )))
                        }
                    });
                }

                thread::sleep(SPAWN_POLL);
            }

            BootstrapResult::mock(Some(format!(
                "Spawned Omegon at {label} but it did not become ready within {}s. \
                 Running in mock mode.",
                SPAWN_TIMEOUT.as_secs()
            )))
        }
    }
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

    #[test]
    fn omegon_not_running_returns_false_quickly() {
        assert!(!omegon_is_running());
    }

    #[test]
    fn find_omegon_binary_respects_env_override() {
        let me = std::env::current_exe().unwrap();
        // SAFETY: single-threaded test context
        unsafe { std::env::set_var(OMEGON_BIN_ENV, me.to_str().unwrap()) };
        let found = find_omegon_binary();
        unsafe { std::env::remove_var(OMEGON_BIN_ENV) };
        assert_eq!(found, Some(me));
    }

    #[test]
    fn find_omegon_binary_ignores_nonexistent_override() {
        // SAFETY: single-threaded test context
        unsafe { std::env::set_var(OMEGON_BIN_ENV, "/does/not/exist/omegon") };
        let _ = find_omegon_binary();
        unsafe { std::env::remove_var(OMEGON_BIN_ENV) };
    }

    fn temp_snapshot_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("auspex-bootstrap-{}-{}", std::process::id(), name));
        path
    }
}
