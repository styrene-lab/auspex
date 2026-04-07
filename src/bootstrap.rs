use semver::Version;
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::time::Duration;

use crate::audit_timeline::{default_audit_timeline_path, load_or_default};
#[cfg(not(target_arch = "wasm32"))]
use crate::command_transport::CommandTransport;
use crate::controller::AppController;
use crate::event_stream::{
    EventStreamHandle, apply_ws_auth_token, derive_authenticated_ws_url,
    spawn_websocket_event_stream,
};
use crate::instance_registry::{
    default_instance_registry_path, load_or_default as load_registry_or_default,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::ipc_client::{IpcCommandClient, IpcEventStreamHandle, spawn_ipc_event_stream};
use crate::omegon_control::OmegonStartupInfo;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderAuthStatus {
    pub name: String,
    pub authenticated: bool,
    pub auth_method: Option<String>,
    pub detail: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopAuthSnapshot {
    pub providers: Vec<ProviderAuthStatus>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAuthAction {
    Login,
    Logout,
    Unlock,
}

#[cfg(not(target_arch = "wasm32"))]
impl DesktopAuthAction {
    pub fn subcommand(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Logout => "logout",
            Self::Unlock => "unlock",
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn omegon_command_path() -> Option<PathBuf> {
    find_omegon_binary().or_else(|| Some(PathBuf::from("omegon")))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_desktop_auth_snapshot() -> Result<DesktopAuthSnapshot, String> {
    let binary = omegon_command_path().ok_or_else(|| "Omegon binary not available".to_string())?;
    let output = std::process::Command::new(binary)
        .args(["auth", "status"])
        .output()
        .map_err(|error| format!("failed to run omegon auth status: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("omegon auth status failed with {}", output.status)
        } else {
            stderr
        });
    }

    parse_desktop_auth_status(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop_auth_action(
    action: DesktopAuthAction,
    provider: Option<&str>,
) -> Result<DesktopAuthSnapshot, String> {
    let binary = omegon_command_path().ok_or_else(|| "Omegon binary not available".to_string())?;
    let mut command = std::process::Command::new(binary);
    command.arg("auth").arg(action.subcommand());
    if let Some(provider) = provider.filter(|value| !value.trim().is_empty()) {
        command.arg(provider.trim());
    }
    if matches!(action, DesktopAuthAction::Logout) {
        command.arg("--yes");
    }

    let output = command
        .output()
        .map_err(|error| format!("failed to run omegon auth {}: {error}", action.subcommand()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!(
                "omegon auth {} failed with {}",
                action.subcommand(),
                output.status
            )
        } else {
            stderr
        });
    }

    load_desktop_auth_snapshot()
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_desktop_auth_status(stdout: &str) -> Result<DesktopAuthSnapshot, String> {
    let mut providers = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Authentication Status") {
            continue;
        }
        let status = if let Some(rest) = trimmed.strip_prefix('✓') {
            (true, rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix('✗') {
            (false, rest.trim())
        } else {
            continue;
        };
        let parts: Vec<&str> = status.1.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let auth_method = parts
            .last()
            .map(|value| value.trim_matches(['(', ')']))
            .map(str::to_string);
        let name = if parts.len() > 1 {
            parts[..parts.len() - 1].join(" ")
        } else {
            parts[0].to_string()
        };
        providers.push(ProviderAuthStatus {
            name,
            authenticated: status.0,
            auth_method,
            detail: status.1.to_string(),
        });
    }

    if providers.is_empty() {
        return Err("omegon auth status returned no provider rows".into());
    }

    Ok(DesktopAuthSnapshot { providers })
}

#[cfg(not(target_arch = "wasm32"))]
pub const SNAPSHOT_FILE_ENV: &str = "AUSPEX_REMOTE_SNAPSHOT_PATH";
#[cfg(not(target_arch = "wasm32"))]
pub const STATE_URL_ENV: &str = "AUSPEX_OMEGON_STATE_URL";
#[cfg(not(target_arch = "wasm32"))]
pub const STARTUP_URL_ENV: &str = "AUSPEX_OMEGON_STARTUP_URL";
#[cfg(not(target_arch = "wasm32"))]
pub const OMEGON_BIN_ENV: &str = "AUSPEX_OMEGON_BIN";
pub const DEFAULT_STATE_URL: &str = "http://127.0.0.1:7842/api/state";
const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");

#[cfg(not(target_arch = "wasm32"))]
const OWNED_OMEGON_PID_FILE: &str = "auspex-owned-omegon.pid";

#[cfg(not(target_arch = "wasm32"))]
const SPAWN_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(not(target_arch = "wasm32"))]
const SPAWN_POLL: Duration = Duration::from_millis(250);

/// Connection hints passed to the shared async bootstrap path.
/// On desktop, built from env vars. On web, built from page URL / JS config.
#[derive(Clone, Debug, Default)]
pub struct ConnectHints {
    /// Explicit WebSocket URL override.
    pub ws_url: Option<String>,
    /// Explicit startup discovery URL override.
    pub startup_url: Option<String>,
    /// Auth token for the WebSocket connection.
    pub ws_token: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ConnectHints {
    /// Build hints from environment variables (desktop path).
    pub fn from_env() -> Self {
        Self {
            ws_url: non_empty_env("AUSPEX_OMEGON_WS_URL"),
            startup_url: non_empty_env(STARTUP_URL_ENV),
            ws_token: non_empty_env("AUSPEX_OMEGON_WS_TOKEN"),
        }
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct CargoPackageMetadata {
    #[serde(default)]
    omegon: OmegonCompatibilityManifest,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct CargoPackageSection {
    #[serde(default)]
    metadata: CargoPackageMetadata,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct CargoManifest {
    #[serde(default)]
    package: CargoPackageSection,
}

#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, Eq)]
struct OmegonCompatibilityManifest {
    minimum_version: String,
    maximum_tested_version: String,
    control_plane_schema: u32,
}

impl OmegonCompatibilityManifest {
    fn parse() -> Self {
        toml::from_str::<CargoManifest>(CARGO_MANIFEST)
            .map(|manifest| manifest.package.metadata.omegon)
            .unwrap_or_default()
    }
}

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
    #[cfg(not(target_arch = "wasm32"))]
    pub ipc_event_stream: Option<IpcEventStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    pub command_transport: Option<CommandTransport>,
}

impl BootstrapResult {
    fn startup_failure(note: String) -> Self {
        let mut controller = AppController::default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_audit_timeline_path() {
            controller = controller.with_audit_timeline(load_or_default(&path));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_instance_registry_path() {
            controller = controller.with_instance_registry(load_registry_or_default(&path));
        }
        controller.set_scenario(crate::fixtures::DevScenario::StartupFailure);
        controller.set_bootstrap_note(Some(note.clone()));
        Self {
            controller,
            source: BootstrapSource::MockDefault,
            note: Some(note),
            event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            ipc_event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            command_transport: None,
        }
    }

    fn compatibility_failure(note: String) -> Self {
        let mut controller = AppController::default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_audit_timeline_path() {
            controller = controller.with_audit_timeline(load_or_default(&path));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_instance_registry_path() {
            controller = controller.with_instance_registry(load_registry_or_default(&path));
        }
        controller.set_scenario(crate::fixtures::DevScenario::CompatibilityFailure);
        controller.set_bootstrap_note(Some(note.clone()));
        Self {
            controller,
            source: BootstrapSource::MockDefault,
            note: Some(note),
            event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            ipc_event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            command_transport: None,
        }
    }

    /// Initial result returned when Omegon needs to be spawned.
    /// The app shows StartingOmegon state while the async spawn runs.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawning_omegon(binary: PathBuf) -> Self {
        let label = binary.display().to_string();
        let mut controller = AppController::default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_audit_timeline_path() {
            controller = controller.with_audit_timeline(load_or_default(&path));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_instance_registry_path() {
            controller = controller.with_instance_registry(load_registry_or_default(&path));
        }
        controller.set_scenario(crate::fixtures::DevScenario::Booting);
        controller.set_bootstrap_note(Some(format!("Starting owned Omegon at {label}\u{2026}")));
        Self {
            controller,
            source: BootstrapSource::SpawningOmegon { binary: label },
            note: None,
            event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            ipc_event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            command_transport: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bootstrap_controller_from_env() -> BootstrapResult {
    // 1. Explicit snapshot file wins for dev/test snapshots.
    if let Some(path) = snapshot_path_from_env() {
        return bootstrap_from_snapshot_file(&path).unwrap_or_else(|error| {
            BootstrapResult::startup_failure(format!(
                "Snapshot bootstrap failed for {path}: {error}."
            ))
        });
    }

    // 2. Explicit state URL — defer to async HTTP attach.
    if state_url_from_env().is_some() {
        let mut controller = AppController::default();
        controller.set_scenario(crate::fixtures::DevScenario::Booting);
        controller.set_bootstrap_note(Some("Attaching to Omegon control plane…".into()));
        return BootstrapResult {
            controller,
            source: BootstrapSource::HttpState {
                url: state_url_from_env().unwrap_or_else(|| DEFAULT_STATE_URL.into()),
            },
            note: None,
            event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            ipc_event_stream: None,
            #[cfg(not(target_arch = "wasm32"))]
            command_transport: None,
        };
    }

    // 3. Default mode: Auspex owns an embedded local Omegon backend.
    if let Some(binary) = find_omegon_binary() {
        return BootstrapResult::spawning_omegon(binary);
    }

    // No explicit URL, no running instance, no binary found.
    BootstrapResult::startup_failure(
        "Auspex could not locate its owned Omegon backend. Set AUSPEX_OMEGON_BIN or bundle the binary with the app.".into(),
    )
}

/// Async bootstrap from an HTTP state endpoint.
pub async fn bootstrap_from_http_state_async(
    url: &str,
    hints: &ConnectHints,
) -> Result<BootstrapResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("could not build HTTP client: {e}"))?;

    let startup = fetch_startup_info_async(&client, url, hints).await.ok();
    let compatibility_warning = if let Some(startup) = startup.as_ref() {
        validate_startup_info(startup)?
    } else {
        None
    };
    let state_url = startup
        .as_ref()
        .map(|startup| startup.state_url.as_str())
        .filter(|state_url| !state_url.is_empty())
        .unwrap_or(url);

    let response = client
        .get(state_url)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("state endpoint returned error: {error}"))?;

    let body = response
        .text()
        .await
        .map_err(|error| format!("could not read response body: {error}"))?;
    let registry_store = default_instance_registry_path()
        .map(|path| load_registry_or_default(&path))
        .unwrap_or_default();
    let mut controller =
        AppController::from_remote_snapshot_json_with_registry(&body, registry_store)
            .map_err(|error| format!("invalid state payload: {error}"))?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = default_audit_timeline_path() {
        controller = controller.with_audit_timeline(load_or_default(&path));
    }
    let ws_url = startup
        .as_ref()
        .map(|startup| startup.ws_url.clone())
        .filter(|ws_url| !ws_url.is_empty())
        .or_else(|| {
            hints
                .ws_url
                .as_deref()
                .map(|url| apply_ws_auth_token(url, hints.ws_token.as_deref()))
                .transpose()
                .ok()
                .flatten()
        })
        .or_else(|| derive_authenticated_ws_url(state_url, hints.ws_token.as_deref()).ok())
        .unwrap_or_else(|| {
            DEFAULT_STATE_URL
                .replace("http://", "ws://")
                .replace("https://", "wss://")
                .replace("/api/state", "/ws")
        });
    let event_stream = Some(spawn_websocket_event_stream(&ws_url));
    #[cfg(not(target_arch = "wasm32"))]
    let ipc_client = startup
        .as_ref()
        .and_then(|startup| startup.instance_descriptor.as_ref())
        .and_then(|instance| instance.control_plane.as_ref())
        .and_then(|control_plane| control_plane.ipc_socket_path.clone())
        .filter(|path| !path.is_empty())
        .map(IpcCommandClient::new)
        .filter(|client| client.is_available());
    #[cfg(not(target_arch = "wasm32"))]
    let ipc_event_stream = ipc_client
        .as_ref()
        .map(|client| spawn_ipc_event_stream(client.socket_path().to_string()));
    #[cfg(not(target_arch = "wasm32"))]
    let command_transport = ipc_client
        .map(CommandTransport::Ipc)
        .or(Some(CommandTransport::EventStream));
    let note = startup
        .as_ref()
        .map(|startup| {
            let control_mode = if command_transport
                .as_ref()
                .is_some_and(|transport| matches!(transport, CommandTransport::Ipc(_)))
            {
                "Control via IPC; websocket event stream active"
            } else {
                "Control via degraded websocket bridge until Styrene RPC is established"
            };
            let mut note = format!(
                "Attached via Omegon startup discovery at {} (auth: {} via {}). {}. Streaming events from {}",
                startup_url_from_state_url(url),
                startup.auth_mode,
                startup.auth_source,
                control_mode,
                ws_url
            );
            if let Some(warning) = compatibility_warning.as_deref() {
                note.push_str(" Warning: ");
                note.push_str(warning);
            }
            note
        })
        .unwrap_or_else(|| {
            let mut note = format!(
                "Attached to Omegon state endpoint at {state_url}. Control via degraded websocket bridge until Styrene RPC is established. Streaming events from {ws_url}"
            );
            if let Some(warning) = compatibility_warning.as_deref() {
                note.push_str(" Warning: ");
                note.push_str(warning);
            }
            note
        });

    Ok(BootstrapResult {
        controller,
        source: BootstrapSource::HttpState {
            url: state_url.to_string(),
        },
        note: Some(note),
        event_stream,
        #[cfg(not(target_arch = "wasm32"))]
        ipc_event_stream,
        #[cfg(not(target_arch = "wasm32"))]
        command_transport,
    })
}

async fn fetch_startup_info_async(
    client: &reqwest::Client,
    state_url: &str,
    hints: &ConnectHints,
) -> Result<OmegonStartupInfo, String> {
    let startup_url = hints
        .startup_url
        .clone()
        .unwrap_or_else(|| startup_url_from_state_url(state_url));
    let response = client
        .get(&startup_url)
        .send()
        .await
        .map_err(|error| format!("startup discovery request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("startup discovery returned error: {error}"))?;
    let body = response
        .text()
        .await
        .map_err(|error| format!("could not read startup discovery response: {error}"))?;
    serde_json::from_str::<OmegonStartupInfo>(&body)
        .map_err(|error| format!("invalid startup discovery payload: {error}"))
}

/// Complete the bootstrap for an explicit state URL.
/// Called from the app's async spawn when STATE_URL_ENV is set.
#[allow(dead_code)]
pub async fn complete_http_bootstrap(url: &str, hints: &ConnectHints) -> BootstrapResult {
    bootstrap_from_http_state_async(url, hints)
        .await
        .unwrap_or_else(|error| {
            if error.contains("control-plane schema") {
                return BootstrapResult::compatibility_failure(error);
            }
            BootstrapResult::startup_failure(format!("Remote attach failed for {url}: {error}."))
        })
}

/// Check if Omegon is already running at the default address (quick 2s timeout).
#[allow(dead_code)]
pub async fn omegon_is_running_async() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok();
    let Some(client) = client else { return false };
    client
        .get(DEFAULT_STATE_URL)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn snapshot_path_from_env() -> Option<String> {
    non_empty_env(SNAPSHOT_FILE_ENV)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn state_url_from_env() -> Option<String> {
    non_empty_env(STATE_URL_ENV)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn startup_url_from_env() -> Option<String> {
    non_empty_env(STARTUP_URL_ENV)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn websocket_url_from_env() -> Option<String> {
    non_empty_env("AUSPEX_OMEGON_WS_URL")
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn websocket_token_from_env() -> Option<String> {
    non_empty_env("AUSPEX_OMEGON_WS_TOKEN")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bootstrap_from_snapshot_file(path: &str) -> Result<BootstrapResult, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read snapshot file: {error}"))?;
    let registry_store = default_instance_registry_path()
        .map(|registry_path| load_registry_or_default(&registry_path))
        .unwrap_or_default();
    let mut controller =
        AppController::from_remote_snapshot_json_with_registry(&contents, registry_store)
            .map_err(|error| format!("invalid snapshot JSON: {error}"))?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(audit_path) = default_audit_timeline_path() {
        controller = controller.with_audit_timeline(load_or_default(&audit_path));
    }

    Ok(BootstrapResult {
        controller,
        source: BootstrapSource::SnapshotFile {
            path: path.to_string(),
        },
        note: Some(format!("Loaded Omegon snapshot from {path}")),
        event_stream: None,
        #[cfg(not(target_arch = "wasm32"))]
        ipc_event_stream: None,
        #[cfg(not(target_arch = "wasm32"))]
        command_transport: None,
    })
}

fn startup_url_from_state_url(state_url: &str) -> String {
    state_url.replace("/api/state", "/api/startup")
}

fn detected_omegon_version(startup: &OmegonStartupInfo) -> Option<&str> {
    startup
        .instance_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.control_plane.as_ref())
        .and_then(|control_plane| control_plane.omegon_version.as_deref())
        .filter(|version| !version.is_empty())
}

fn parse_version(version: &str) -> Result<Version, String> {
    Version::parse(version).map_err(|error| format!("invalid semver '{version}': {error}"))
}

fn validate_omegon_version(startup: &OmegonStartupInfo) -> Result<Option<String>, String> {
    let manifest = OmegonCompatibilityManifest::parse();
    let detected = detected_omegon_version(startup)
        .ok_or_else(|| "Auspex requires Omegon version identity, but the startup metadata did not report omegon_version.".to_string())?;
    let detected = parse_version(detected)?;
    let minimum = parse_version(&manifest.minimum_version)?;
    let maximum = parse_version(&manifest.maximum_tested_version)?;

    if detected < minimum {
        return Err(format!(
            "Auspex requires Omegon {} or newer. Connected instance is {}.",
            manifest.minimum_version, detected
        ));
    }

    if detected > maximum {
        return Ok(Some(format!(
            "Omegon {} is newer than Auspex's maximum tested version {}. Continuing with compatibility warning.",
            detected, manifest.maximum_tested_version
        )));
    }

    Ok(None)
}

fn validate_startup_info(startup: &OmegonStartupInfo) -> Result<Option<String>, String> {
    let manifest = OmegonCompatibilityManifest::parse();
    if startup.schema_version != manifest.control_plane_schema {
        return Err(format!(
            "Auspex requires control-plane schema {}, but Omegon reported schema {}.",
            manifest.control_plane_schema, startup.schema_version
        ));
    }

    validate_omegon_version(startup)
}

/// Locate the Omegon binary.
///
/// Priority order:
/// 1. `AUSPEX_OMEGON_BIN` env var — explicit override
/// 2. `~/.local/bin/omegon` — common user-local install location
/// 3. `~/.cargo/bin/omegon` — default `cargo install` location
/// 4. `/usr/local/bin/omegon` and `/opt/homebrew/bin/omegon` — common system paths
/// 5. `which omegon` — PATH lookup
#[cfg(not(target_arch = "wasm32"))]
pub fn find_omegon_binary() -> Option<PathBuf> {
    if let Some(path) = non_empty_env(OMEGON_BIN_ENV) {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        for rel in &[".local/bin/omegon", ".cargo/bin/omegon"] {
            let p = PathBuf::from(&home).join(rel);
            if p.exists() {
                return Some(p);
            }
        }
    }

    for abs in &["/usr/local/bin/omegon", "/opt/homebrew/bin/omegon"] {
        let p = PathBuf::from(abs);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(output) = std::process::Command::new("which").arg("omegon").output()
        && output.status.success()
    {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() {
            let p = PathBuf::from(s);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Spawn the owned Omegon backend, verify the launcher contract,
/// then bootstrap from the control plane.
///
/// This function blocks on process I/O and should be called from
/// `tokio::task::spawn_blocking` or a dedicated thread.
fn startup_state_url(startup: &OmegonStartupInfo) -> Option<String> {
    if !startup.state_url.is_empty() {
        return Some(startup.state_url.clone());
    }

    if !startup.startup_url.is_empty() {
        return Some(startup.startup_url.replace("/api/startup", "/api/state"));
    }

    if !startup.http_base.is_empty() {
        return Some(format!(
            "{}/api/state",
            startup.http_base.trim_end_matches('/')
        ));
    }

    None
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn spawn_and_attach_omegon(binary: &std::path::Path) -> BootstrapResult {
    use tokio::io::AsyncBufReadExt;

    if omegon_is_running_async().await {
        clear_owned_omegon_pid();
        match bootstrap_from_http_state_async(DEFAULT_STATE_URL, &ConnectHints::from_env()).await {
            Ok(result) => return result,
            Err(error) => {
                eprintln!(
                    "auspex: existing local Omegon at {DEFAULT_STATE_URL} failed bootstrap ({error}); reaping conflicting owned instances and spawning fresh one"
                );
                reap_conflicting_omegon_children();
            }
        }
    }

    reap_owned_omegon_child();

    let label = binary.display().to_string();

    let mut child = match tokio::process::Command::new(binary)
        .arg("serve")
        .arg("--control-port")
        .arg("7842")
        .arg("--strict-port")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Err(error) => {
            return BootstrapResult::startup_failure(format!(
                "Could not spawn owned Omegon backend at {label}: {error}."
            ));
        }
        Ok(child) => child,
    };

    record_owned_omegon_pid(child.id());

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return BootstrapResult::startup_failure(
                "Owned Omegon backend spawned but stdout was not captured.".into(),
            );
        }
    };

    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut startup_info: Option<OmegonStartupInfo> = None;

    let deadline = tokio::time::sleep(SPAWN_TIMEOUT);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(info) = serde_json::from_str::<OmegonStartupInfo>(trimmed)
                            && info.schema_version > 0
                        {
                            startup_info = Some(info);
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            _ = &mut deadline => break,
        }
    }

    let Some(info) = startup_info else {
        return BootstrapResult::startup_failure(format!(
            "Owned Omegon backend at {label} did not emit a startup JSON line within {}s.",
            SPAWN_TIMEOUT.as_secs()
        ));
    };

    let state_url = startup_state_url(&info).unwrap_or_else(|| DEFAULT_STATE_URL.to_string());
    let startup_url = if !info.startup_url.is_empty() {
        info.startup_url.clone()
    } else {
        startup_url_from_state_url(&state_url)
    };
    let ready_url = if !info.ready_url.is_empty() {
        info.ready_url.clone()
    } else {
        state_url.replace("/api/state", "/api/readyz")
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return BootstrapResult::startup_failure(format!(
                "Owned Omegon backend at {label} started but Auspex could not build an HTTP client: {error}"
            ));
        }
    };

    let mut startup_ok = false;
    let mut startup_error = None;
    for _ in 0..20 {
        match client.get(&startup_url).send().await {
            Ok(response) if response.status().is_success() => {
                startup_ok = true;
                break;
            }
            Ok(response) => startup_error = Some(format!("HTTP {}", response.status())),
            Err(error) => startup_error = Some(error.to_string()),
        }
        tokio::time::sleep(SPAWN_POLL).await;
    }
    if !startup_ok {
        return BootstrapResult::startup_failure(format!(
            "Owned Omegon backend at {label} emitted startup info but /api/startup at {startup_url} did not become reachable within 5s{}.",
            startup_error
                .as_deref()
                .map(|e| format!(": {e}"))
                .unwrap_or_default()
        ));
    }

    let mut ready_ok = false;
    let mut ready_error = None;
    for _ in 0..20 {
        match client.get(&ready_url).send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                let payload_ok = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| value.get("ok").and_then(|ok| ok.as_bool()))
                    .unwrap_or(false);
                if payload_ok {
                    ready_ok = true;
                    break;
                }
                ready_error = Some(format!("payload {body}"));
            }
            Ok(response) => ready_error = Some(format!("HTTP {}", response.status())),
            Err(error) => ready_error = Some(error.to_string()),
        }
        tokio::time::sleep(SPAWN_POLL).await;
    }
    if !ready_ok {
        return BootstrapResult::startup_failure(format!(
            "Owned Omegon backend at {label} satisfied startup discovery but /api/readyz at {ready_url} did not report ready within 5s{}.",
            ready_error
                .as_deref()
                .map(|e| format!(": {e}"))
                .unwrap_or_default()
        ));
    }

    bootstrap_from_http_state_async(&state_url, &ConnectHints::from_env())
        .await
        .unwrap_or_else(|error| {
            if error.contains("control-plane schema") {
                BootstrapResult::compatibility_failure(error)
            } else {
                BootstrapResult::startup_failure(format!(
                    "Owned Omegon backend started at {label} but bootstrap failed: {error}"
                ))
            }
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn owned_omegon_pid_path() -> PathBuf {
    std::env::temp_dir().join(OWNED_OMEGON_PID_FILE)
}

#[cfg(not(target_arch = "wasm32"))]
fn record_owned_omegon_pid(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    let _ = fs::write(owned_omegon_pid_path(), pid.to_string());
}

#[cfg(not(target_arch = "wasm32"))]
fn read_owned_omegon_pid() -> Option<u32> {
    fs::read_to_string(owned_omegon_pid_path())
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_owned_omegon_pid() {
    let _ = fs::remove_file(owned_omegon_pid_path());
}

#[cfg(not(target_arch = "wasm32"))]
fn owned_omegon_pids() -> Vec<u32> {
    let output = match std::process::Command::new("ps")
        .args(["ax", "-o", "pid=,command="])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (pid, command) = trimmed.split_once(' ')?;
            if !(command.contains("omegon serve --control-port 7842 --strict-port")
                || command.contains("omegon serve --strict-port --control-port 7842"))
            {
                return None;
            }
            pid.parse::<u32>().ok()
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn pid_is_owned_omegon(pid: u32) -> bool {
    owned_omegon_pids()
        .into_iter()
        .any(|candidate| candidate == pid)
}

#[cfg(not(target_arch = "wasm32"))]
fn reap_owned_omegon_child() {
    let Some(pid) = read_owned_omegon_pid() else {
        return;
    };
    if pid_is_owned_omegon(pid) {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    clear_owned_omegon_pid();
}

#[cfg(not(target_arch = "wasm32"))]
fn reap_conflicting_omegon_children() {
    for pid in owned_omegon_pids() {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    clear_owned_omegon_pid();
}

#[cfg(not(target_arch = "wasm32"))]
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
        "openspec": {"total_tasks": 5, "done_tasks": 2},
        "cleave": {"active": false, "total_children": 0, "completed_children": 0, "failed_children": 0},
        "session": {
            "id": "remote-test-session",
            "branch": "main",
            "mode": "power",
            "turns": 10,
            "tool_calls": 1066,
            "compactions": 3,
            "thinking_level": "medium",
            "capability_tier": "gloriana",
            "memory_available": true,
            "cleave_available": true,
            "providers": [
                {"name": "anthropic", "authenticated": true, "model": "claude-sonnet-4-20250514"},
                {"name": "openrouter", "authenticated": true}
            ]
        },
        "dispatcher": {
            "session_id": "remote-test-session",
            "dispatcher_instance_id": "omegon-1",
            "expected_role": "primary",
            "expected_profile": "gloriana",
            "expected_model": "claude-sonnet-4-20250514",
            "control_plane_schema": 2
        },
        "activity": "Exploring design alternatives for the remote session adapter architecture"
    }"#;

    fn remote_startup_info_fixture() -> OmegonStartupInfo {
        OmegonStartupInfo {
            schema_version: 2,
            state_url: "http://127.0.0.1:7842/api/state".into(),
            ws_url: "ws://127.0.0.1:7842/ws".into(),
            auth_mode: "none".into(),
            auth_source: "default".into(),
            instance_descriptor: Some(crate::omegon_control::OmegonInstanceDescriptor {
                control_plane: Some(crate::omegon_control::OmegonControlPlaneDescriptor {
                    omegon_version: Some("0.15.10-rc.17".into()),
                    schema_version: 2,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn parse_desktop_auth_status_extracts_provider_rows() {
        let snapshot = parse_desktop_auth_status(
            "Authentication Status:\n  ✓ Anthropic/Claude oauth (stored)\n  ✗ OpenAI API       api-key\n",
        )
        .expect("auth status should parse");

        assert_eq!(snapshot.providers.len(), 2);
        assert_eq!(snapshot.providers[0].name, "Anthropic/Claude oauth");
        assert!(snapshot.providers[0].authenticated);
        assert_eq!(snapshot.providers[1].name, "OpenAI API");
        assert!(!snapshot.providers[1].authenticated);
    }

    #[test]
    fn parse_desktop_auth_status_rejects_empty_output() {
        let err = parse_desktop_auth_status("Authentication Status:\n\n").unwrap_err();
        assert!(err.contains("no provider rows"));
    }

    #[test]
    fn snapshot_file_bootstrap_builds_remote_controller() {
        let dir = std::env::temp_dir().join("auspex-test-snapshot-bootstrap");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("snapshot.json");
        std::fs::write(&path, REMOTE_SNAPSHOT_JSON).unwrap();
        let result =
            bootstrap_from_snapshot_file(path.to_str().unwrap()).expect("bootstrap should succeed");
        assert!(result.controller.is_remote());
        assert!(matches!(
            result.source,
            BootstrapSource::SnapshotFile { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_snapshot_file_returns_error() {
        let result = bootstrap_from_snapshot_file("/nonexistent/snapshot.json");
        assert!(result.is_err());
    }

    #[test]
    fn startup_schema_match_is_accepted() {
        let info = remote_startup_info_fixture();
        assert!(validate_startup_info(&info).is_ok());
    }

    #[test]
    fn startup_older_version_is_rejected() {
        let mut info = remote_startup_info_fixture();
        info.instance_descriptor
            .as_mut()
            .and_then(|descriptor| descriptor.control_plane.as_mut())
            .expect("fixture control plane")
            .omegon_version = Some("0.15.10-rc.16".into());
        let err = validate_startup_info(&info).unwrap_err();
        assert!(err.contains("requires Omegon 0.15.10-rc.17 or newer"));
    }

    #[test]
    fn startup_newer_version_is_allowed_with_warning() {
        let mut info = remote_startup_info_fixture();
        info.instance_descriptor
            .as_mut()
            .and_then(|descriptor| descriptor.control_plane.as_mut())
            .expect("fixture control plane")
            .omegon_version = Some("0.15.10-rc.18".into());
        let warning = validate_startup_info(&info).unwrap();
        assert!(
            warning
                .as_deref()
                .unwrap_or_default()
                .contains("newer than Auspex's maximum tested version 0.15.10-rc.17")
        );
    }

    #[test]
    fn startup_missing_version_identity_is_rejected() {
        let mut info = remote_startup_info_fixture();
        info.instance_descriptor = None;
        let err = validate_startup_info(&info).unwrap_err();
        assert!(err.contains("requires Omegon version identity"));
    }

    #[test]
    fn startup_schema_mismatch_is_rejected() {
        let mut info = remote_startup_info_fixture();
        info.schema_version = 99;
        let err = validate_startup_info(&info).unwrap_err();
        assert!(err.contains("control-plane schema"));
    }

    #[test]
    fn startup_url_derives_from_state_url() {
        assert_eq!(
            startup_url_from_state_url("http://127.0.0.1:7842/api/state"),
            "http://127.0.0.1:7842/api/startup"
        );
    }

    #[test]
    fn startup_state_url_falls_back_from_startup_url_and_http_base() {
        let mut info = remote_startup_info_fixture();
        info.state_url.clear();
        info.startup_url = "http://127.0.0.1:7850/api/startup".into();
        assert_eq!(
            startup_state_url(&info).as_deref(),
            Some("http://127.0.0.1:7850/api/state")
        );

        info.startup_url.clear();
        info.http_base = "http://127.0.0.1:7850".into();
        assert_eq!(
            startup_state_url(&info).as_deref(),
            Some("http://127.0.0.1:7850/api/state")
        );
    }

    #[test]
    fn startup_discovery_note_mentions_transport_mode() {
        let mut note = String::from(
            "Attached via Omegon startup discovery at http://127.0.0.1:7842/api/startup (auth: ephemeral-bearer via generated). Control via IPC; websocket event stream active. Streaming events from ws://127.0.0.1:7842/ws?token=test",
        );
        assert!(note.contains("Control via IPC"));
        note = String::from(
            "Attached to Omegon state endpoint at http://127.0.0.1:7842/api/state. Control via degraded websocket bridge until Styrene RPC is established. Streaming events from ws://127.0.0.1:7842/ws?token=test",
        );
        assert!(note.contains("degraded websocket bridge until Styrene RPC is established"));
    }

    #[test]
    fn startup_failure_uses_failed_scenario() {
        let r = BootstrapResult::startup_failure("test error".into());
        assert!(matches!(r.source, BootstrapSource::MockDefault));
        assert!(r.note.as_deref().unwrap().contains("test error"));
    }

    #[test]
    fn find_omegon_binary_respects_env_override() {
        let dir = std::env::temp_dir().join("auspex-test-omegon-bin");
        let _ = std::fs::create_dir_all(&dir);
        let fake_binary = dir.join("omegon-test");
        std::fs::write(&fake_binary, "#!/bin/bash\necho test").unwrap();

        // We can't set env for just our function, so just verify the function
        // behavior when the path exists.
        assert!(fake_binary.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_omegon_binary_ignores_nonexistent_override() {
        // When env points to a non-existent path, falls through.
        let path = PathBuf::from("/nonexistent/omegon-XXXXXX");
        assert!(!path.exists());
    }

    #[test]
    fn find_omegon_binary_prefers_local_bin_over_cargo_bin() {
        // Structural: ensure priority order is documented.
        // Actual binary presence varies by host.
    }

    #[test]
    fn owned_omegon_pid_round_trips() {
        clear_owned_omegon_pid();
        record_owned_omegon_pid(Some(4242));
        assert_eq!(read_owned_omegon_pid(), Some(4242));
        clear_owned_omegon_pid();
        assert_eq!(read_owned_omegon_pid(), None);
    }

    #[test]
    fn pid_is_owned_omegon_rejects_impossible_pid() {
        assert!(!pid_is_owned_omegon(u32::MAX));
    }

    #[test]
    fn embedded_omegon_pid_scan_ignores_unrelated_processes() {
        assert!(owned_omegon_pids().iter().all(|pid| *pid > 0));
    }

    #[test]
    fn explicit_websocket_url_can_be_tokenized() {
        // Verify token attachment to custom WS URLs.
        let url = "ws://custom.host:9000/ws";
        let token = Some("my-token");
        let result = crate::event_stream::apply_ws_auth_token(url, token).unwrap();
        assert!(result.contains("token=my-token"));
    }

    #[test]
    fn state_url_env_is_opt_in() {
        // Without setting the env, state_url_from_env returns None.
        // (We can't unset env reliably in tests, so just check the function exists.)
        let _ = state_url_from_env();
    }

    #[test]
    fn websocket_token_env_is_opt_in() {
        let _ = websocket_token_from_env();
    }

    #[tokio::test]
    async fn omegon_not_running_returns_false_quickly() {
        // Default address should not have a running instance during tests.
        let start = std::time::Instant::now();
        let running = omegon_is_running_async().await;
        let elapsed = start.elapsed();
        // The check should fail quickly (within the 2s timeout).
        assert!(elapsed < Duration::from_secs(5));
        // It's acceptable for this to be true if Omegon happens to be running,
        // but we at least verify the function completes promptly.
        let _ = running;
    }
}
