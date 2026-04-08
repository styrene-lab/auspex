use crate::audit_timeline::{AuditTimelineQuery, AuditTimelineStore, AuditTimelineView};
use crate::fixtures::{
    AppSurfaceKind, AppSurfaceNotice, ChatMessage, ComposerState, DevScenario, GraphData,
    HostSessionSummary, MockHostSession, SessionData, SessionTelemetryData, ShellState, WorkData,
};
use crate::instance_registry::{
    InstanceRegistryStore, default_instance_registry_path, persist as persist_instance_registry,
};
use crate::remote_session::{DispatcherSwitchCommandOutcome, RemoteHostSession};
use crate::runtime_types::{CommandTarget, TargetedCommand};
use crate::session_event::SessionEvent;
use crate::session_model::HostSessionModel;
use crate::state_engine::{AttachedInstanceRecord, AttachedInstanceStateEngine};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRouteOption {
    pub route_id: String,
    pub label: String,
    pub detail: String,
    pub target: CommandTarget,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsAuthState {
    pub providers: Vec<crate::fixtures::ProviderInfo>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub inventory_refreshed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashCommandResult {
    pub name: String,
    pub args: String,
    pub accepted: bool,
    pub output: String,
}

fn provider_status_key(name: &str) -> Option<String> {
    let normalized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    normalized
        .split_whitespace()
        .next()
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_inventory_key(name: &str) -> String {
    provider_status_key(name).unwrap_or_else(|| name.trim().to_ascii_lowercase())
}

#[cfg(not(target_arch = "wasm32"))]
fn merge_provider_inventory(
    runtime_providers: &[crate::fixtures::ProviderInfo],
    settings_providers: &[crate::fixtures::ProviderInfo],
) -> Vec<crate::fixtures::ProviderInfo> {
    let mut merged = Vec::with_capacity(runtime_providers.len().max(settings_providers.len()));
    let mut seen = std::collections::HashSet::new();

    for runtime in runtime_providers {
        let key = provider_inventory_key(&runtime.name);
        let settings = settings_providers
            .iter()
            .find(|candidate| provider_inventory_key(&candidate.name) == key);
        merged.push(crate::fixtures::ProviderInfo {
            name: runtime.name.clone(),
            authenticated: runtime.authenticated,
            auth_method: settings
                .and_then(|provider| provider.auth_method.clone())
                .or_else(|| runtime.auth_method.clone()),
            model: runtime
                .model
                .clone()
                .or_else(|| settings.and_then(|provider| provider.model.clone())),
        });
        seen.insert(key);
    }

    for settings in settings_providers {
        let key = provider_inventory_key(&settings.name);
        if seen.insert(key) {
            merged.push(settings.clone());
        }
    }

    merged
}

const DEMO_REMOTE_SNAPSHOT_JSON: &str = r#"{
    "design": {
        "focused": {
            "id": "auspex-remote",
            "title": "Remote session adapter",
            "status": "implementing",
            "open_questions": ["How should reconnect work?"],
            "decisions": 1,
            "children": 2
        },
        "implementing": [{"id": "auspex-remote", "title": "Remote session adapter", "status": "implementing"}],
        "actionable": [{"id": "compat-handshake", "title": "Compatibility handshake", "status": "ready"}]
    },
    "openspec": {"total_tasks": 5, "done_tasks": 2},
    "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0},
    "session": {"turns": 12, "tool_calls": 34, "compactions": 1},
    "dispatcher": {
        "session_id": "session_01HVDEMO",
        "dispatcher_instance_id": "omg_primary_01HVDEMO",
        "expected_role": "primary-driver",
        "expected_profile": "primary-interactive",
        "expected_model": "anthropic:claude-sonnet-4-6",
        "control_plane_schema": 2,
        "token_ref": "secret://auspex/instances/omg_primary_01HVDEMO/token",
        "observed_base_url": "http://127.0.0.1:7842",
        "last_verified_at": "2026-04-04T12:00:00Z",
        "available_options": [
            {"profile": "primary-interactive", "label": "Primary Interactive", "model": "anthropic:claude-sonnet-4-6"},
            {"profile": "supervisor-heavy", "label": "Supervisor Heavy", "model": "openai:gpt-4.1"}
        ],
        "switch_state": {
            "requested_profile": null,
            "requested_model": null,
            "status": "idle",
            "note": null
        }
    },
    "harness": {
        "git_branch": "main",
        "git_detached": false,
        "thinking_level": "medium",
        "capability_tier": "victory",
        "providers": [{"name": "Anthropic", "authenticated": true, "auth_method": "api-key", "model": "claude-sonnet"}],
        "memory_available": true,
        "cleave_available": true,
        "memory_warning": null,
        "active_delegates": []
    }
}"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMode {
    Mock,
    Live,
}

impl SessionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "Mock (offline)",
            Self::Live => "Live",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionSource {
    Mock(MockHostSession),
    Remote(Box<RemoteHostSession>),
}

impl Default for SessionSource {
    fn default() -> Self {
        Self::Mock(MockHostSession::default())
    }
}

impl SessionSource {
    fn model(&self) -> &dyn HostSessionModel {
        match self {
            Self::Mock(session) => session,
            Self::Remote(session) => session.as_ref(),
        }
    }

    fn model_mut(&mut self) -> &mut dyn HostSessionModel {
        match self {
            Self::Mock(session) => session,
            Self::Remote(session) => session.as_mut(),
        }
    }

    fn mode(&self) -> SessionMode {
        match self {
            Self::Mock(_) => SessionMode::Mock,
            Self::Remote(_) => SessionMode::Live,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppController {
    session: SessionSource,
    bootstrap_note: Option<String>,
    transcript_auto_expand: bool,
    audit_timeline: AuditTimelineStore,
    instance_registry: InstanceRegistryStore,
    attached_instance_engine: AttachedInstanceStateEngine,
    telemetry_snapshot: SessionTelemetryData,
    last_audited_telemetry_snapshot: SessionTelemetryData,
    telemetry_audit_sequence: u64,
    #[cfg(not(target_arch = "wasm32"))]
    settings_auth_state: SettingsAuthState,
}

impl Default for AppController {
    fn default() -> Self {
        let mut controller = Self {
            session: SessionSource::default(),
            bootstrap_note: None,
            transcript_auto_expand: true,
            audit_timeline: AuditTimelineStore::default(),
            instance_registry: InstanceRegistryStore::default(),
            attached_instance_engine: AttachedInstanceStateEngine::default(),
            telemetry_snapshot: SessionTelemetryData::default(),
            last_audited_telemetry_snapshot: SessionTelemetryData::default(),
            telemetry_audit_sequence: 0,
            #[cfg(not(target_arch = "wasm32"))]
            settings_auth_state: SettingsAuthState {
                providers: vec![crate::fixtures::ProviderInfo {
                    name: "Anthropic/Claude".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: Some("claude-sonnet".into()),
                }],
                last_error: None,
                last_action: None,
                inventory_refreshed: true,
            },
        };
        controller.refresh_telemetry_snapshot();
        controller
    }
}

impl AppController {
    pub fn from_remote_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        Self::from_remote_snapshot_json_with_registry(json, InstanceRegistryStore::default())
    }

    pub fn from_remote_snapshot_json_with_registry(
        json: &str,
        instance_registry: InstanceRegistryStore,
    ) -> Result<Self, serde_json::Error> {
        let session = RemoteHostSession::from_snapshot_json(json)?;
        let mut controller = Self {
            session: SessionSource::Remote(Box::new(session)),
            bootstrap_note: None,
            transcript_auto_expand: true,
            audit_timeline: AuditTimelineStore::default(),
            attached_instance_engine: AttachedInstanceStateEngine::default(),
            instance_registry,
            telemetry_snapshot: SessionTelemetryData::default(),
            last_audited_telemetry_snapshot: SessionTelemetryData::default(),
            telemetry_audit_sequence: 0,
            #[cfg(not(target_arch = "wasm32"))]
            settings_auth_state: SettingsAuthState {
                providers: vec![],
                last_error: None,
                last_action: None,
                inventory_refreshed: false,
            },
        };
        controller.rebuild_attached_instances();
        controller.refresh_telemetry_snapshot();
        controller.refresh_audit_timeline();
        Ok(controller)
    }

    #[allow(dead_code)]
    pub fn remote_demo() -> Self {
        Self::from_remote_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
            .expect("embedded remote demo snapshot must stay valid")
    }

    pub fn with_audit_timeline(mut self, audit_timeline: AuditTimelineStore) -> Self {
        self.audit_timeline = audit_timeline;
        self.refresh_audit_timeline();
        self
    }

    pub fn with_instance_registry(mut self, instance_registry: InstanceRegistryStore) -> Self {
        self.instance_registry = instance_registry;
        self.rebuild_attached_instances();
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
        self
    }

    pub fn session_mode(&self) -> SessionMode {
        self.session.mode()
    }

    pub fn available_command_routes(&self) -> Vec<CommandRouteOption> {
        self.attached_instance_engine
            .available_command_routes()
            .into_iter()
            .map(|route| CommandRouteOption {
                route_id: route.route_id,
                label: route.label,
                detail: route.detail,
                target: route.target,
            })
            .collect()
    }

    pub fn selected_command_route_id(&self) -> String {
        self.attached_instance_engine.selected_command_route_id()
    }

    pub fn select_command_route(&mut self, route_id: &str) {
        self.attached_instance_engine
            .select_command_route(route_id.to_string());
        self.refresh_telemetry_snapshot();
    }

    #[allow(dead_code)]
    pub fn attached_instances(&self) -> &[AttachedInstanceRecord] {
        self.attached_instance_engine.attached_instances()
    }

    pub fn evaluate_instance_lifecycle(&mut self, now_epoch_seconds: u64) {
        self.attached_instance_engine
            .evaluate_lifecycle_policy(now_epoch_seconds);
        self.instance_registry = self.attached_instance_engine.registry_store().clone();
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
    }

    #[allow(dead_code)]
    pub fn attach_instance_record(&mut self, instance: AttachedInstanceRecord) {
        self.attached_instance_engine.attach_instance(instance);
        self.instance_registry = self.attached_instance_engine.registry_store().clone();
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
    }

    #[allow(dead_code)]
    pub fn detach_instance_record(&mut self, instance_id: &str) {
        self.attached_instance_engine.detach_instance(instance_id);
        self.instance_registry = self.attached_instance_engine.registry_store().clone();
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
    }

    #[allow(dead_code)]
    pub fn purge_stale_instance_records(&mut self, active_instance_ids: &[String]) {
        self.attached_instance_engine
            .purge_stale_instances(active_instance_ids);
        self.instance_registry = self.attached_instance_engine.registry_store().clone();
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
    }

    #[allow(dead_code)]
    pub fn surface_notice(&self) -> Option<AppSurfaceNotice> {
        match self.shell_state() {
            ShellState::StartingOmegon => Some(AppSurfaceNotice {
                kind: AppSurfaceKind::Startup,
                body: "Launching the Omegon engine. The conversation shell will activate automatically once ready.".into(),
                detail: self.bootstrap_note.clone(),
            }),
            ShellState::CompatibilityChecking => Some(AppSurfaceNotice {
                kind: AppSurfaceKind::Reconnecting,
                body: "The connection to the host is being restored. New input is temporarily paused. Cached session state is shown.".into(),
                detail: None,
            }),
            ShellState::Failed => Some(AppSurfaceNotice {
                kind: if self.scenario() == DevScenario::CompatibilityFailure {
                    AppSurfaceKind::CompatibilityFailure
                } else {
                    AppSurfaceKind::StartupFailure
                },
                body: self.summary().connection.clone(),
                detail: Some(
                    if self.scenario() == DevScenario::CompatibilityFailure {
                        self.bootstrap_note.clone().unwrap_or_else(|| {
                            "Auspex cannot operate with the detected host. Update Omegon to a compatible version and restart.".into()
                        })
                    } else {
                        self.bootstrap_note.clone().unwrap_or_else(|| {
                            "Auspex requires its embedded Omegon backend for local operation. Fix backend startup and relaunch, or explicitly attach to a remote Omegon control plane.".into()
                        })
                    }
                ),
            }),
            ShellState::Ready | ShellState::Degraded => self.bootstrap_note.clone().map(|body| {
                AppSurfaceNotice {
                    kind: AppSurfaceKind::BootstrapNote,
                    body,
                    detail: None,
                }
            }),
        }
    }

    pub fn set_bootstrap_note(&mut self, note: Option<String>) {
        self.bootstrap_note = note;
    }

    pub fn is_remote(&self) -> bool {
        self.session_mode() == SessionMode::Live
    }

    pub fn switch_session_mode(&mut self, raw: &str) {
        let previous_session_key = self.session_audit_key();
        let previous_instance_ids: Vec<String> = self
            .attached_instances()
            .iter()
            .filter(|instance| instance.session_key == previous_session_key)
            .map(|instance| instance.instance_id.clone())
            .collect();
        self.session = match raw {
            "live" => SessionSource::Remote(Box::new(
                RemoteHostSession::from_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
                    .expect("embedded remote demo snapshot must stay valid"),
            )),
            _ => SessionSource::Mock(MockHostSession::default()),
        };
        if raw != "live" {
            for instance_id in &previous_instance_ids {
                self.detach_instance_record(instance_id);
            }
        }
        self.rebuild_attached_instances();
        self.refresh_telemetry_snapshot();
        self.bootstrap_note = None;
        self.refresh_audit_timeline();
    }

    pub fn shell_state(&self) -> ShellState {
        self.session.model().shell_state()
    }

    pub fn scenario(&self) -> DevScenario {
        self.session.model().scenario()
    }

    pub fn summary(&self) -> &HostSessionSummary {
        self.session.model().summary()
    }

    pub fn messages(&self) -> &[ChatMessage] {
        self.session.model().messages()
    }

    pub fn composer(&self) -> &ComposerState {
        self.session.model().composer()
    }

    pub fn can_submit(&self) -> bool {
        self.session.model().can_submit()
    }

    pub fn operator_readiness(&self) -> crate::fixtures::OperatorReadinessData {
        use crate::fixtures::{OperatorReadinessData, ReadinessStepData, ReadinessStepState};

        let shell_state = self.shell_state();
        let remote_attached = self.is_remote();
        #[cfg(not(target_arch = "wasm32"))]
        let auth_inventory_refreshed = self.settings_auth_state.inventory_refreshed;
        #[cfg(target_arch = "wasm32")]
        let auth_inventory_refreshed = true;
        let session_data = self.session_data();
        let authenticated_provider = session_data
            .providers
            .iter()
            .any(|provider| provider.authenticated);

        let host_step = match shell_state {
            crate::fixtures::ShellState::StartingOmegon => ReadinessStepData {
                label: "Embedded Omegon".into(),
                detail: "Launching embedded Omegon process".into(),
                state: ReadinessStepState::Active,
            },
            crate::fixtures::ShellState::CompatibilityChecking => ReadinessStepData {
                label: "Embedded Omegon".into(),
                detail: "Embedded host discovered; validating compatibility".into(),
                state: ReadinessStepState::Active,
            },
            crate::fixtures::ShellState::Failed => ReadinessStepData {
                label: "Embedded Omegon".into(),
                detail: "Embedded host failed to reach an operational state".into(),
                state: ReadinessStepState::Blocked,
            },
            _ => ReadinessStepData {
                label: "Embedded Omegon".into(),
                detail: "Embedded host is running".into(),
                state: ReadinessStepState::Complete,
            },
        };

        let session_step = if remote_attached {
            ReadinessStepData {
                label: "Session snapshot".into(),
                detail: "Remote session state is attached".into(),
                state: ReadinessStepState::Complete,
            }
        } else if matches!(
            shell_state,
            crate::fixtures::ShellState::StartingOmegon
                | crate::fixtures::ShellState::CompatibilityChecking
        ) {
            ReadinessStepData {
                label: "Session snapshot".into(),
                detail: "Waiting for host control-plane session state".into(),
                state: ReadinessStepState::Pending,
            }
        } else {
            ReadinessStepData {
                label: "Session snapshot".into(),
                detail: "Using fallback local session state".into(),
                state: ReadinessStepState::Complete,
            }
        };

        let auth_step = if !auth_inventory_refreshed {
            ReadinessStepData {
                label: "Auth inventory".into(),
                detail: "Loading provider auth inventory".into(),
                state: ReadinessStepState::Active,
            }
        } else if self.settings_auth_state().last_error.as_deref().is_some() {
            ReadinessStepData {
                label: "Auth inventory".into(),
                detail: self
                    .settings_auth_state()
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Auth inventory refresh failed".into()),
                state: ReadinessStepState::Blocked,
            }
        } else {
            ReadinessStepData {
                label: "Auth inventory".into(),
                detail: format!("{} provider record(s) loaded", session_data.providers.len()),
                state: ReadinessStepState::Complete,
            }
        };

        let prompt_step = if self.can_submit() {
            ReadinessStepData {
                label: "Prompt execution".into(),
                detail: "At least one authenticated provider is available".into(),
                state: ReadinessStepState::Complete,
            }
        } else if !auth_inventory_refreshed {
            ReadinessStepData {
                label: "Prompt execution".into(),
                detail: "Waiting for provider auth inventory".into(),
                state: ReadinessStepState::Pending,
            }
        } else if !authenticated_provider {
            ReadinessStepData {
                label: "Prompt execution".into(),
                detail: "No authenticated providers are available yet".into(),
                state: ReadinessStepState::Blocked,
            }
        } else {
            ReadinessStepData {
                label: "Prompt execution".into(),
                detail: "Prompt execution is temporarily unavailable".into(),
                state: ReadinessStepState::Blocked,
            }
        };

        let ready = !matches!(
            shell_state,
            crate::fixtures::ShellState::StartingOmegon
                | crate::fixtures::ShellState::CompatibilityChecking
        ) && auth_inventory_refreshed;

        let (title, detail) = if !ready {
            if !auth_inventory_refreshed {
                (
                    "Preparing operator controls".into(),
                    "Auspex is attached, but provider auth inventory is still loading so promptability and remediation paths can converge.".into(),
                )
            } else {
                (
                    "Starting embedded Omegon".into(),
                    "Auspex is waiting for the embedded host and control plane to become operational.".into(),
                )
            }
        } else {
            (
                "Ready".into(),
                "Operator controls are fully converged.".into(),
            )
        };

        OperatorReadinessData {
            ready,
            title,
            detail,
            steps: vec![host_step, session_step, auth_step, prompt_step],
        }
    }

    pub fn is_run_active(&self) -> bool {
        self.session.model().is_run_active()
    }

    pub fn work_data(&self) -> WorkData {
        self.session.model().work_data()
    }

    pub fn session_data(&self) -> SessionData {
        let mut data = self.session.model().session_data();
        #[cfg(not(target_arch = "wasm32"))]
        if !self.settings_auth_state.providers.is_empty() {
            for provider in &mut data.providers {
                if let Some(settings_provider) =
                    self.settings_auth_state.providers.iter().find(|candidate| {
                        provider_status_key(&candidate.name) == provider_status_key(&provider.name)
                    })
                {
                    provider.auth_method = settings_provider.auth_method.clone();
                    if provider.model.is_none() {
                        provider.model = settings_provider.model.clone();
                    }
                }
            }
        }
        data.telemetry = self.telemetry_snapshot.clone();
        data
    }

    pub fn graph_data(&self) -> GraphData {
        self.session.model().graph_data()
    }

    pub fn transcript(&self) -> &crate::fixtures::TranscriptData {
        self.session.model().transcript()
    }

    pub fn transcript_auto_expand(&self) -> bool {
        self.transcript_auto_expand
    }

    pub fn audit_timeline(&self) -> &AuditTimelineStore {
        &self.audit_timeline
    }

    #[allow(dead_code)]
    pub fn query_audit_timeline(&self, query: &AuditTimelineQuery) -> AuditTimelineView<'_> {
        let mut view = self.audit_timeline.query(query);
        let current_session_key = self.session_audit_key();
        if !view
            .sessions
            .iter()
            .any(|session| session == &current_session_key)
        {
            view.sessions.push(current_session_key);
            view.sessions.sort();
        }
        view
    }

    #[allow(dead_code)]
    pub fn current_audit_session_key(&self) -> String {
        self.session_audit_key()
    }

    pub fn set_transcript_auto_expand(&mut self, enabled: bool) {
        self.transcript_auto_expand = enabled;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn settings_auth_state(&self) -> &SettingsAuthState {
        &self.settings_auth_state
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn ensure_settings_auth_status(&mut self) {
        if !self.settings_auth_state.inventory_refreshed {
            let _ = self.refresh_settings_auth_status();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn refresh_settings_auth_status(&mut self) -> Result<(), String> {
        match crate::bootstrap::load_desktop_auth_snapshot() {
            Ok(snapshot) => {
                let existing_providers = merge_provider_inventory(
                    &self.session.model().session_data().providers,
                    &self.settings_auth_state.providers,
                );
                let providers: Vec<crate::fixtures::ProviderInfo> = snapshot
                    .providers
                    .iter()
                    .map(|provider| {
                        let existing = existing_providers.iter().find(|existing| {
                            provider_status_key(&existing.name)
                                == provider_status_key(&provider.name)
                        });
                        crate::fixtures::ProviderInfo {
                            name: provider.name.clone(),
                            authenticated: provider.authenticated,
                            auth_method: provider.auth_method.clone(),
                            model: existing.and_then(|provider| provider.model.clone()),
                        }
                    })
                    .collect();
                self.settings_auth_state.providers = providers;
                if let SessionSource::Remote(session) = &mut self.session {
                    session.refresh_provider_auth(
                        snapshot
                            .providers
                            .into_iter()
                            .map(|provider| {
                                let existing = existing_providers.iter().find(|existing| {
                                    provider_status_key(&existing.name)
                                        == provider_status_key(&provider.name)
                                });
                                crate::omegon_control::ProviderStatusSnapshot {
                                    name: provider.name,
                                    authenticated: provider.authenticated,
                                    auth_method: provider.auth_method,
                                    model: existing.and_then(|provider| provider.model.clone()),
                                }
                            })
                            .collect(),
                    );
                }
                self.settings_auth_state.last_error = None;
                self.settings_auth_state.last_action = Some("auth.refresh".into());
                self.settings_auth_state.inventory_refreshed = true;
                self.refresh_telemetry_snapshot();
                Ok(())
            }
            Err(error) => {
                self.settings_auth_state.last_error = Some(error.clone());
                self.settings_auth_state.inventory_refreshed = true;
                Err(error)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_settings_auth_action(
        &mut self,
        action: crate::bootstrap::DesktopAuthAction,
        provider: Option<&str>,
    ) -> Result<(), String> {
        match crate::bootstrap::run_desktop_auth_action(action, provider) {
            Ok(snapshot) => {
                let existing_providers = merge_provider_inventory(
                    &self.session.model().session_data().providers,
                    &self.settings_auth_state.providers,
                );
                let providers: Vec<crate::fixtures::ProviderInfo> = snapshot
                    .providers
                    .iter()
                    .map(|provider| {
                        let existing = existing_providers.iter().find(|existing| {
                            provider_status_key(&existing.name)
                                == provider_status_key(&provider.name)
                        });
                        crate::fixtures::ProviderInfo {
                            name: provider.name.clone(),
                            authenticated: provider.authenticated,
                            auth_method: provider.auth_method.clone(),
                            model: existing.and_then(|provider| provider.model.clone()),
                        }
                    })
                    .collect();
                self.settings_auth_state.providers = providers;
                if let SessionSource::Remote(session) = &mut self.session {
                    session.refresh_provider_auth(
                        snapshot
                            .providers
                            .into_iter()
                            .map(|provider| {
                                let existing = existing_providers.iter().find(|existing| {
                                    provider_status_key(&existing.name)
                                        == provider_status_key(&provider.name)
                                });
                                crate::omegon_control::ProviderStatusSnapshot {
                                    name: provider.name,
                                    authenticated: provider.authenticated,
                                    auth_method: provider.auth_method,
                                    model: existing.and_then(|provider| provider.model.clone()),
                                }
                            })
                            .collect(),
                    );
                }
                self.settings_auth_state.last_error = None;
                self.settings_auth_state.last_action =
                    Some(format!("auth.{}", action.subcommand()));
                self.settings_auth_state.inventory_refreshed = true;
                self.refresh_telemetry_snapshot();
                Ok(())
            }
            Err(error) => {
                self.settings_auth_state.last_error = Some(error.clone());
                self.settings_auth_state.inventory_refreshed = true;
                Err(error)
            }
        }
    }

    #[allow(dead_code)]
    pub fn as_model(&self) -> &dyn HostSessionModel {
        self.session.model()
    }

    pub fn set_scenario(&mut self, scenario: DevScenario) {
        self.session.model_mut().set_scenario(scenario);
        self.refresh_telemetry_snapshot();
        self.refresh_audit_timeline();
    }

    pub fn select_scenario(&mut self, raw: &str) {
        let next = match raw {
            "booting" => DevScenario::Booting,
            "degraded" => DevScenario::Degraded,
            "startup-failure" => DevScenario::StartupFailure,
            "compat-failure" => DevScenario::CompatibilityFailure,
            "reconnecting" => DevScenario::Reconnecting,
            "local-dev-quiet" => DevScenario::LocalDevQuiet,
            "local-dev-busy" => DevScenario::LocalDevBusy,
            "homelab-fleet" => DevScenario::HomelabFleet,
            "enterprise-incident" => DevScenario::EnterpriseIncident,
            _ => DevScenario::Ready,
        };
        self.set_scenario(next);
    }

    pub fn update_draft(&mut self, value: impl Into<String>) {
        self.session.model_mut().composer_mut().set_draft(value);
    }

    pub fn current_command_target(&self) -> CommandTarget {
        self.attached_instance_engine.current_command_target()
    }

    fn command_target(&self) -> CommandTarget {
        self.current_command_target()
    }

    fn session_audit_key(&self) -> String {
        match &self.session {
            SessionSource::Remote(session) => session
                .session_data()
                .dispatcher_binding
                .as_ref()
                .map(|binding| format!("remote:{}", binding.session_id))
                .unwrap_or_else(|| "remote:detached".into()),
            SessionSource::Mock(_) => format!("mock:{}", self.scenario().key()),
        }
    }

    fn refresh_audit_timeline(&mut self) {
        let session_key = self.session_audit_key();
        let transcript = self.transcript().clone();
        self.audit_timeline
            .append_transcript_snapshot(&session_key, &transcript);
        self.append_telemetry_audit_entries(&session_key);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = crate::audit_timeline::default_audit_timeline_path() {
            let _ = crate::audit_timeline::persist(&path, &self.audit_timeline);
        }
    }

    fn append_telemetry_audit_entries(&mut self, session_key: &str) {
        if self.telemetry_snapshot == self.last_audited_telemetry_snapshot {
            return;
        }

        self.telemetry_audit_sequence += 1;
        let sequence = self.telemetry_audit_sequence;
        let telemetry = &self.telemetry_snapshot;
        let mut entries = vec![
            crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("provider-summary-{sequence}"),
                "Telemetry · Provider summary",
                telemetry.provider_summary.clone(),
            ),
            crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("lifecycle-summary-{sequence}"),
                "Telemetry · Lifecycle summary",
                telemetry.lifecycle_summary.clone(),
            ),
            crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("route-summary-{sequence}"),
                "Telemetry · Route summary",
                telemetry.route_summary.clone(),
            ),
            crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("latest-turn-summary-{sequence}"),
                "Telemetry · Latest turn summary",
                telemetry.latest_turn_summary.clone(),
            ),
        ];

        for (index, provider) in telemetry.provider_rollups.iter().enumerate() {
            entries.push(crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("provider-rollup-{sequence}-{index}"),
                format!(
                    "Telemetry · Provider rollup · {}",
                    provider.route_id.as_deref().unwrap_or("unrouted")
                ),
                format!(
                    "provider: {}\nsource: {}\ninstance: {}\nrole: {}\nprofile: {}\nmodel: {}",
                    provider.provider,
                    provider.source,
                    provider.instance_id.as_deref().unwrap_or("not reported"),
                    provider.role.as_deref().unwrap_or("not reported"),
                    provider.profile.as_deref().unwrap_or("not reported"),
                    provider.model.as_deref().unwrap_or("model not reported"),
                ),
            ));
        }

        for (index, control_plane) in telemetry.control_plane_rollups.iter().enumerate() {
            entries.push(crate::audit_timeline::AuditEntry::telemetry(
                session_key,
                &format!("control-plane-rollup-{sequence}-{index}"),
                format!(
                    "Telemetry · Control-plane rollup · {}",
                    control_plane.route_id.as_deref().unwrap_or("unrouted")
                ),
                format!(
                    "instance: {}\nrole: {}\nprofile: {}\nbase_url: {}\nauth_mode: {}",
                    control_plane
                        .instance_id
                        .as_deref()
                        .unwrap_or("not reported"),
                    control_plane.role.as_deref().unwrap_or("not reported"),
                    control_plane.profile.as_deref().unwrap_or("not reported"),
                    control_plane.base_url.as_deref().unwrap_or("not reported"),
                    control_plane.auth_mode.as_deref().unwrap_or("not reported"),
                ),
            ));
        }

        for entry in entries {
            let _ = self.audit_timeline.append_entry(entry);
        }

        self.last_audited_telemetry_snapshot = telemetry.clone();
    }

    fn rebuild_attached_instances(&mut self) {
        let session_key = self.session_audit_key();
        let session = self.session.model().session_data();
        let selected_route = self.attached_instance_engine.selected_command_route_id();
        self.attached_instance_engine = AttachedInstanceStateEngine::from_registry_and_session(
            self.instance_registry.clone(),
            session_key,
            &session,
        );
        self.attached_instance_engine
            .select_command_route(selected_route);
        self.instance_registry = self.attached_instance_engine.registry_store().clone();
    }

    fn effective_provider_inventory(
        &self,
        model_data: &SessionData,
    ) -> Vec<crate::fixtures::ProviderInfo> {
        #[cfg(not(target_arch = "wasm32"))]
        if !self.settings_auth_state.providers.is_empty() {
            return merge_provider_inventory(
                &model_data.providers,
                &self.settings_auth_state.providers,
            );
        }
        model_data.providers.clone()
    }

    fn refresh_telemetry_snapshot(&mut self) {
        let model_data = self.session.model().session_data();
        let mut telemetry = model_data.telemetry.clone();
        let providers = self.effective_provider_inventory(&model_data);
        telemetry.provider_summary = crate::telemetry::summarize_provider_inventory(&providers);
        telemetry.provider_rollups = crate::telemetry::aggregate_provider_rollups(
            self.attached_instance_engine.attached_instances(),
            &providers,
            &self.attached_instance_engine.selected_command_route_id(),
            telemetry.latest_provider_telemetry.as_ref(),
        );
        let lifecycle = crate::telemetry::aggregate_lifecycle_telemetry(
            self.attached_instance_engine.attached_instances(),
            self.attached_instance_engine.registry_store(),
            &self.attached_instance_engine.selected_command_route_id(),
        );
        telemetry.lifecycle_summary = lifecycle.summary.clone();
        telemetry.lifecycle = lifecycle;
        telemetry.control_plane_rollups = crate::telemetry::aggregate_control_plane_rollups(
            self.attached_instance_engine.attached_instances(),
            &self.attached_instance_engine.selected_command_route_id(),
            telemetry.control_plane.as_ref(),
        );
        self.telemetry_snapshot = telemetry;
    }

    fn persist_instance_registry(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = default_instance_registry_path() {
            let _ = persist_instance_registry(&path, &self.instance_registry);
        }
    }

    #[allow(dead_code)]
    pub fn submit_prompt(&mut self) -> bool {
        let submitted = self.session.model_mut().submit();
        if submitted {
            self.refresh_audit_timeline();
        }
        submitted
    }

    pub fn submit_prompt_command(&mut self) -> Option<TargetedCommand> {
        let target = self.command_target();
        match &mut self.session {
            SessionSource::Remote(session) => {
                let trimmed = session.composer().draft().trim().to_string();
                if trimmed.is_empty() || !session.can_submit() {
                    return None;
                }
                if !session.submit() {
                    return None;
                }
                self.refresh_audit_timeline();
                Some(TargetedCommand::legacy_json(
                    target,
                    serde_json::json!({
                        "type": "user_prompt",
                        "text": trimmed,
                    })
                    .to_string(),
                ))
            }
            SessionSource::Mock(session) => {
                let submitted = session.submit();
                if submitted {
                    self.refresh_audit_timeline();
                }
                submitted.then(|| TargetedCommand::legacy_json(target, String::new()))
            }
        }
        .filter(|command| !command.command_json.is_empty())
    }

    pub fn cancel_command(&self) -> Option<TargetedCommand> {
        match &self.session {
            SessionSource::Remote(session) if session.is_run_active() => {
                Some(TargetedCommand::legacy_json(
                    self.command_target(),
                    serde_json::json!({ "type": "cancel" }).to_string(),
                ))
            }
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn request_dispatcher_switch_command(
        &mut self,
        profile: &str,
        model: Option<&str>,
    ) -> Option<TargetedCommand> {
        let target = self.command_target();
        match &mut self.session {
            SessionSource::Remote(session) => {
                match session.request_dispatcher_switch(profile, model)? {
                    DispatcherSwitchCommandOutcome::Issued { request_id } => {
                        Some(TargetedCommand::legacy_json(
                            target,
                            serde_json::json!({
                                "type": "switch_dispatcher",
                                "request_id": request_id,
                                "profile": profile,
                                "model": model,
                            })
                            .to_string(),
                        ))
                    }
                    DispatcherSwitchCommandOutcome::Noop => None,
                }
            }
            SessionSource::Mock(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn request_dispatcher_switch_command_json(
        &mut self,
        profile: &str,
        model: Option<&str>,
    ) -> Option<String> {
        self.request_dispatcher_switch_command(profile, model)
            .map(|command| command.command_json)
    }

    pub fn apply_remote_event_json(&mut self, json: &str) -> Result<bool, serde_json::Error> {
        match &mut self.session {
            SessionSource::Remote(session) => {
                let applied = session.apply_event_json(json)?;
                if applied {
                    self.handle_session_mutation();
                }
                Ok(applied)
            }
            SessionSource::Mock(_) => Ok(false),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub fn apply_ipc_event(
        &mut self,
        client: &crate::ipc_client::IpcCommandClient,
        event: omegon_traits::IpcEventPayload,
    ) -> Result<bool, String> {
        match &mut self.session {
            SessionSource::Remote(session) => {
                let normalized: SessionEvent = event.into();
                let applied = match &normalized {
                    SessionEvent::HarnessChanged | SessionEvent::StateChanged { .. } => {
                        let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
                            format!("tokio runtime unavailable for IPC state refresh: {error}")
                        })?;
                        let snapshot =
                            tokio::task::block_in_place(|| runtime.block_on(client.get_state()))?;
                        session.refresh_from_ipc_state(&snapshot)
                    }
                    _ => session.apply_session_event(normalized),
                };
                if applied {
                    self.handle_session_mutation();
                }
                Ok(applied)
            }
            SessionSource::Mock(_) => Ok(false),
        }
    }

    fn handle_session_mutation(&mut self) {
        self.rebuild_attached_instances();
        let active_instance_ids: Vec<String> = self
            .attached_instances()
            .iter()
            .filter(|instance| instance.session_key == self.session_audit_key())
            .map(|instance| instance.instance_id.clone())
            .collect();
        self.purge_stale_instance_records(&active_instance_ids);
        self.refresh_telemetry_snapshot();
        self.persist_instance_registry();
        self.refresh_audit_timeline();
    }

    pub fn parse_slash_command_result(json: &str) -> Option<SlashCommandResult> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        if value.get("type")?.as_str()? != "slash_command_result" {
            return None;
        }

        Some(SlashCommandResult {
            name: value.get("name")?.as_str()?.to_string(),
            args: value
                .get("args")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            accepted: value.get("accepted")?.as_bool()?,
            output: value
                .get("output")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_timeline::AuditEntryKind;
    use crate::fixtures::ActivityKind;
    use crate::runtime_types::{InstanceFreshness, InstanceRecord};

    const REMOTE_SNAPSHOT_JSON: &str = DEMO_REMOTE_SNAPSHOT_JSON;

    #[test]
    fn default_controller_uses_mock_session_source() {
        let controller = AppController::default();

        assert_eq!(controller.scenario(), DevScenario::Ready);
        assert_eq!(controller.messages().len(), 1);
        assert_eq!(
            controller.summary().connection,
            "Connected to local host session"
        );
    }

    #[test]
    fn remote_controller_uses_remote_session_source() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert_eq!(controller.scenario(), DevScenario::Ready);
        assert!(controller.summary().connection.contains("main"));
        assert_eq!(controller.messages().len(), 1);
        let session = controller.session_data();
        let dispatcher = session.dispatcher_binding.as_ref().unwrap();
        assert_eq!(dispatcher.dispatcher_instance_id, "omg_primary_01HVDEMO");
        assert_eq!(dispatcher.expected_role, "primary-driver");
        assert_eq!(dispatcher.expected_profile, "primary-interactive");
        assert_eq!(
            dispatcher.expected_model.as_deref(),
            Some("anthropic:claude-sonnet-4-6")
        );
        assert_eq!(dispatcher.session_id, "session_01HVDEMO");
        assert_eq!(dispatcher.available_options.len(), 2);
        assert_eq!(dispatcher.switch_state.as_ref().unwrap().status, "idle");
        assert_eq!(dispatcher.switch_state.as_ref().unwrap().request_id, None);
    }

    #[test]
    fn available_command_routes_prefer_dispatcher_by_default() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let routes = controller.available_command_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(controller.attached_instances().len(), 1);
        assert!(
            routes
                .iter()
                .any(|route| route.route_id == "session-dispatcher")
        );
        assert_eq!(controller.selected_command_route_id(), "session-dispatcher");
    }

    #[test]
    fn registry_backed_controller_hydrates_host_route_from_persisted_record() {
        let registry = InstanceRegistryStore {
            schema_version: 1,
            instances: vec![InstanceRecord {
                schema_version: 1,
                identity: crate::runtime_types::WorkerIdentity {
                    instance_id: "omg_host_01HVDEMO".into(),
                    role: crate::runtime_types::WorkerRole::PrimaryDriver,
                    profile: "control-plane".into(),
                    status: crate::runtime_types::WorkerLifecycleState::Ready,
                    created_at: "2026-04-06T00:00:00Z".into(),
                    updated_at: "2026-04-06T00:00:01Z".into(),
                },
                ownership: crate::runtime_types::WorkerOwnership {
                    owner_kind: crate::runtime_types::OwnerKind::AuspexSession,
                    owner_id: "session_01HVDEMO".into(),
                    parent_instance_id: None,
                },
                desired: crate::runtime_types::DesiredWorkerState {
                    backend: crate::runtime_types::BackendConfig {
                        kind: crate::runtime_types::BackendKind::LocalProcess,
                        image: None,
                        namespace: None,
                        resources: None,
                    },
                    workspace: crate::runtime_types::WorkspaceBinding {
                        cwd: "/repo".into(),
                        workspace_id: "repo:demo".into(),
                        branch: Some("main".into()),
                    },
                    task: None,
                    policy: crate::runtime_types::PolicyOverrides {
                        model: Some("openai:gpt-4.1".into()),
                        ..Default::default()
                    },
                },
                observed: crate::runtime_types::ObservedWorkerState {
                    placement: crate::runtime_types::ObservedPlacement {
                        placement_id: "pid/9001".into(),
                        host: "desktop:local".into(),
                        pid: Some(9001),
                        namespace: None,
                        pod_name: None,
                        container_name: None,
                    },
                    control_plane: crate::runtime_types::ObservedControlPlane {
                        schema_version: 2,
                        omegon_version: "0.15.10-rc.17".into(),
                        base_url: "http://127.0.0.1:7842".into(),
                        startup_url: "http://127.0.0.1:7842/api/startup".into(),
                        health_url: "http://127.0.0.1:7842/api/healthz".into(),
                        ready_url: "http://127.0.0.1:7842/api/readyz".into(),
                        ws_url: "ws://127.0.0.1:7842/ws".into(),
                        auth_mode: "ephemeral-bearer".into(),
                        token_ref: None,
                        last_ready_at: Some("2026-04-06T00:00:02Z".into()),
                    },
                    health: crate::runtime_types::ObservedHealth {
                        ready: true,
                        degraded_reason: None,
                        last_heartbeat_at: Some("2026-04-06T00:00:03Z".into()),
                        last_seen_at: Some("2026-04-06T00:00:03Z".into()),
                        freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
                    },
                    exit: crate::runtime_types::ObservedExit {
                        exited: false,
                        exit_code: None,
                        exit_reason: None,
                        exited_at: None,
                    },
                },
            }],
        };

        let controller =
            AppController::from_remote_snapshot_json_with_registry(REMOTE_SNAPSHOT_JSON, registry)
                .unwrap();

        let routes = controller.available_command_routes();
        assert!(
            routes
                .iter()
                .any(|route| route.route_id == "host-control-plane")
        );
        assert!(
            routes
                .iter()
                .any(|route| route.route_id == "session-dispatcher")
        );
    }

    #[test]
    fn selecting_host_control_plane_changes_command_target() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        controller.select_command_route("host-control-plane");
        controller.update_draft("ship it");

        let command = controller
            .submit_prompt_command()
            .expect("targeted command");
        assert_eq!(command.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            command.target.dispatcher_instance_id,
            Some("omg_primary_01HVDEMO".into())
        );
    }

    #[test]
    fn controller_attach_and_detach_instance_persist_registry_state() {
        let mut controller = AppController::default();
        controller.attach_instance_record(AttachedInstanceRecord {
            instance_id: "omg_host_01HVTEST".into(),
            route_id: crate::state_engine::HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "host".into(),
            profile: "control-plane".into(),
            session_key: "mock:ready".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("openai:gpt-4.1".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        assert!(
            controller
                .attached_instances()
                .iter()
                .any(|instance| instance.instance_id == "omg_host_01HVTEST")
        );

        controller.detach_instance_record("omg_host_01HVTEST");
        assert!(
            !controller
                .attached_instances()
                .iter()
                .any(|instance| instance.instance_id == "omg_host_01HVTEST")
        );
    }

    #[test]
    fn controller_purges_stale_instance_records() {
        let mut controller = AppController::default();
        controller.attach_instance_record(AttachedInstanceRecord {
            instance_id: "omg_host_01HVTEST".into(),
            route_id: crate::state_engine::HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "host".into(),
            profile: "control-plane".into(),
            session_key: "mock:ready".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("openai:gpt-4.1".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        controller.attach_instance_record(AttachedInstanceRecord {
            instance_id: "omg_dispatcher_01HVTEST".into(),
            route_id: crate::state_engine::SESSION_DISPATCHER_ROUTE_ID.into(),
            role: "primary-driver".into(),
            profile: "primary-interactive".into(),
            session_key: "mock:ready".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("anthropic:claude-sonnet-4-6".into()),
            dispatcher_instance_id: Some("omg_dispatcher_01HVTEST".into()),
            registry_record: None,
        });

        controller.purge_stale_instance_records(&["omg_dispatcher_01HVTEST".into()]);
        assert_eq!(controller.attached_instances().len(), 1);
        assert_eq!(
            controller.attached_instances()[0].instance_id,
            "omg_dispatcher_01HVTEST"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn session_data_merges_settings_auth_metadata_without_overriding_runtime_auth() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        controller.settings_auth_state.providers = vec![crate::fixtures::ProviderInfo {
            name: "Anthropic/Claude".into(),
            authenticated: false,
            auth_method: Some("api-key".into()),
            model: None,
        }];

        let session = controller.session_data();
        assert_eq!(session.providers.len(), 1);
        assert_eq!(session.providers[0].name, "Anthropic");
        assert!(session.providers[0].authenticated);
        assert_eq!(session.providers[0].auth_method.as_deref(), Some("api-key"));
        assert!(controller.can_submit());
    }

    #[test]
    fn remote_runtime_authority_blocks_submit_even_if_settings_inventory_lists_auth() {
        let mut controller = AppController::from_remote_snapshot_json_with_registry(
            r#"{
              "design": {"focused": null, "implementing": [], "actionable": [], "all_nodes": [], "counts": {"total":0,"seed":0,"exploring":0,"resolved":0,"decided":0,"implementing":0,"implemented":0,"blocked":0,"deferred":0,"open_questions":0}},
              "openspec": {"changes": [], "total_tasks": 0, "done_tasks": 0},
              "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0, "children": []},
              "session": {"turns": 0, "tool_calls": 0, "compactions": 0},
              "harness": null,
              "instance": {
                "identity": {"instance_id": "web-compat", "role": "primary_driver", "profile": "primary-interactive", "status": "ready"},
                "workspace": {"cwd": "/repo", "workspace_id": "repo:detached", "branch": "detached"},
                "runtime": {"health": "ready", "provider_ok": false, "memory_ok": true, "cleave_available": false, "thinking_level": "Medium", "capability_tier": "victory"}
              }
            }"#,
            InstanceRegistryStore::default(),
        )
        .unwrap();
        controller.settings_auth_state.providers = vec![crate::fixtures::ProviderInfo {
            name: "OpenAI/Codex".into(),
            authenticated: true,
            auth_method: Some("oauth".into()),
            model: None,
        }];
        controller.settings_auth_state.inventory_refreshed = true;

        assert!(!controller.can_submit());
        let session = controller.session_data();
        assert!(session.providers.is_empty());
    }

    #[test]
    fn remote_auth_refresh_rehydrates_prompt_execution_state() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        assert!(controller.can_submit());

        if let SessionSource::Remote(session) = &mut controller.session {
            session.refresh_provider_auth(Vec::new());
        }
        controller.settings_auth_state.providers = vec![];
        controller.settings_auth_state.inventory_refreshed = true;
        controller.refresh_telemetry_snapshot();
        assert!(!controller.can_submit());

        if let SessionSource::Remote(session) = &mut controller.session {
            session.refresh_provider_auth(vec![crate::omegon_control::ProviderStatusSnapshot {
                name: "OpenAI/Codex".into(),
                authenticated: true,
                auth_method: Some("oauth".into()),
                model: None,
            }]);
        }
        controller.settings_auth_state.providers = vec![crate::fixtures::ProviderInfo {
            name: "OpenAI/Codex".into(),
            authenticated: true,
            auth_method: Some("oauth".into()),
            model: None,
        }];
        controller.refresh_telemetry_snapshot();
        assert!(controller.can_submit());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn effective_provider_inventory_preserves_runtime_models_and_appends_settings_only_providers() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        controller.settings_auth_state.providers = vec![
            crate::fixtures::ProviderInfo {
                name: "Anthropic/Claude".into(),
                authenticated: true,
                auth_method: Some("oauth".into()),
                model: None,
            },
            crate::fixtures::ProviderInfo {
                name: "OpenAI/Codex".into(),
                authenticated: true,
                auth_method: Some("oauth".into()),
                model: Some("gpt-4.1".into()),
            },
        ];

        let providers =
            controller.effective_provider_inventory(&controller.session.model().session_data());

        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name, "Anthropic");
        assert_eq!(providers[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(providers[0].auth_method.as_deref(), Some("oauth"));
        assert_eq!(providers[1].name, "OpenAI/Codex");
        assert_eq!(providers[1].model.as_deref(), Some("gpt-4.1"));
    }

    #[test]
    fn parse_slash_command_result_extracts_structured_fields() {
        let result = AppController::parse_slash_command_result(
            r#"{"type":"slash_command_result","name":"login","args":"anthropic","accepted":true,"output":"done"}"#,
        )
        .expect("slash result");

        assert_eq!(result.name, "login");
        assert_eq!(result.args, "anthropic");
        assert!(result.accepted);
        assert_eq!(result.output, "done");
    }

    #[test]
    fn select_scenario_maps_known_values() {
        let mut controller = AppController::default();

        controller.select_scenario("degraded");
        assert_eq!(controller.scenario(), DevScenario::Degraded);

        controller.select_scenario("startup-failure");
        assert_eq!(controller.scenario(), DevScenario::StartupFailure);

        controller.select_scenario("compat-failure");
        assert_eq!(controller.scenario(), DevScenario::CompatibilityFailure);
    }

    #[test]
    fn select_scenario_maps_fixture_pack_values() {
        let mut controller = AppController::default();

        controller.select_scenario("local-dev-busy");
        assert_eq!(controller.scenario(), DevScenario::LocalDevBusy);

        controller.select_scenario("homelab-fleet");
        assert_eq!(controller.scenario(), DevScenario::HomelabFleet);

        controller.select_scenario("enterprise-incident");
        assert_eq!(controller.scenario(), DevScenario::EnterpriseIncident);
    }

    #[test]
    fn select_scenario_defaults_unknown_values_to_ready() {
        let mut controller = AppController::default();
        controller.select_scenario("not-a-real-scenario");

        assert_eq!(controller.scenario(), DevScenario::Ready);
    }

    #[test]
    fn update_draft_and_submit_route_through_session_source() {
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert_eq!(controller.composer().draft(), "hello world");
        assert!(controller.submit_prompt());
        assert_eq!(controller.messages().len(), 3);
    }

    #[test]
    fn remote_submit_emits_user_prompt_command_json() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        controller.update_draft("ship it");

        let command = controller.submit_prompt_command().unwrap();

        assert_eq!(
            command.command_json,
            r#"{"text":"ship it","type":"user_prompt"}"#
        );
        assert_eq!(command.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            command.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );
        assert_eq!(controller.messages().len(), 1);
        assert_eq!(
            controller.summary().activity,
            "Submitting prompt to Omegon remote session"
        );
        assert_eq!(controller.summary().activity_kind, ActivityKind::Waiting);
        assert_eq!(controller.composer().draft(), "");
        assert_eq!(
            command.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"legacy_json","command_json":"{\"text\":\"ship it\",\"type\":\"user_prompt\"}"}}"#
        );
    }

    #[test]
    fn remote_events_route_only_for_remote_session_source() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
                .unwrap()
        );
        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_chunk","text":"hello remote"}"#)
                .unwrap()
        );
        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_end"}"#)
                .unwrap()
        );
        assert_eq!(controller.messages().last().unwrap().text, "hello remote");

        let mut mock_controller = AppController::default();
        assert!(
            !mock_controller
                .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
                .unwrap()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn ipc_invalidation_events_require_runtime_refresh() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        let client = crate::ipc_client::IpcCommandClient::new("/definitely/not/here.sock");

        let error = controller
            .apply_ipc_event(&client, omegon_traits::IpcEventPayload::HarnessChanged)
            .expect_err("missing IPC socket should fail refresh");
        assert!(
            error.contains("IPC")
                || error.contains("ipc")
                || error.contains("socket")
                || error.contains("No such file")
                || error.contains("os error")
                || error.contains("tokio runtime unavailable")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn ipc_non_invalidation_events_apply_without_refresh_client() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        let client = crate::ipc_client::IpcCommandClient::new("/definitely/not/here.sock");

        assert!(
            controller
                .apply_ipc_event(
                    &client,
                    omegon_traits::IpcEventPayload::TurnStarted { turn: 9 }
                )
                .unwrap()
        );
        if let SessionSource::Remote(session) = &mut controller.session {
            assert!(session.apply_session_event(SessionEvent::MessageStart {
                role: "assistant".into(),
            }));
        } else {
            panic!("expected remote session source");
        }
        assert!(
            controller
                .apply_ipc_event(
                    &client,
                    omegon_traits::IpcEventPayload::MessageDelta {
                        text: "hello".into()
                    }
                )
                .unwrap()
        );
        assert!(
            controller
                .apply_ipc_event(&client, omegon_traits::IpcEventPayload::MessageCompleted)
                .unwrap()
        );

        assert_eq!(controller.transcript().active_turn, Some(9));
        assert_eq!(controller.messages().last().unwrap().text, "hello");
    }

    #[test]
    fn switch_session_mode_swaps_between_mock_and_remote_demo() {
        let mut controller = AppController::default();
        assert_eq!(controller.session_mode(), SessionMode::Mock);

        controller.switch_session_mode("live");
        assert_eq!(controller.session_mode(), SessionMode::Live);
        assert!(
            controller
                .summary()
                .connection
                .contains("Attached to Omegon host")
        );
        assert_eq!(controller.messages().len(), 1);

        controller.switch_session_mode("mock");
        assert_eq!(controller.session_mode(), SessionMode::Mock);
        assert_eq!(
            controller.summary().connection,
            "Connected to local host session"
        );
        assert!(
            controller
                .attached_instances()
                .iter()
                .all(|instance| instance.session_key == "mock:ready")
        );
        assert!(
            controller
                .available_command_routes()
                .iter()
                .all(|route| route.route_id == "local-shell")
        );
    }

    #[test]
    fn leaving_live_mode_detaches_live_session_owned_instances() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        assert!(!controller.attached_instances().is_empty());

        controller.switch_session_mode("mock");

        assert!(
            controller
                .attached_instances()
                .iter()
                .all(|instance| instance.session_key == "mock:ready")
        );
        assert!(
            controller
                .available_command_routes()
                .iter()
                .all(|route| route.route_id == "local-shell")
        );
    }

    #[test]
    fn transcript_auto_expand_defaults_on_and_is_mutable() {
        let mut controller = AppController::default();
        assert!(controller.transcript_auto_expand());

        controller.set_transcript_auto_expand(false);
        assert!(!controller.transcript_auto_expand());

        controller.set_transcript_auto_expand(true);
        assert!(controller.transcript_auto_expand());
    }

    #[test]
    fn controller_retains_transcript_blocks_in_audit_timeline() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());

        assert_eq!(controller.audit_timeline().entries.len(), 6);
        assert_eq!(
            controller.audit_timeline().entries[0].block_id,
            "mock:ready:turn-1-block-0"
        );
        assert!(
            controller.audit_timeline().entries[1]
                .content
                .contains("scaffold only proves")
        );
        assert!(
            controller
                .audit_timeline()
                .entries
                .iter()
                .any(|entry| entry.kind == AuditEntryKind::Telemetry
                    && entry.label == "Telemetry · Provider summary")
        );
        assert!(
            controller
                .audit_timeline()
                .entries
                .iter()
                .any(|entry| entry.kind == AuditEntryKind::Telemetry
                    && entry.label == "Telemetry · Route summary")
        );
    }

    #[test]
    fn audit_timeline_query_filters_current_session_entries() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());

        let filtered = controller.query_audit_timeline(&AuditTimelineQuery {
            session_key: Some(controller.current_audit_session_key()),
            turn_number: Some(1),
            kind: Some(AuditEntryKind::Text),
            text: "scaffold".into(),
        });

        assert_eq!(filtered.entries.len(), 1);
        assert_eq!(filtered.entries[0].turn_number, 1);
        assert_eq!(filtered.entries[0].kind, AuditEntryKind::Text);
        assert_eq!(filtered.entries[0].session_key, "mock:ready");
    }

    #[test]
    fn audit_timeline_query_updates_session_options_after_mode_switch() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());
        controller.switch_session_mode("live");

        let filtered = controller.query_audit_timeline(&AuditTimelineQuery::default());

        assert!(filtered.sessions.contains(&"mock:ready".to_string()));
        assert!(
            filtered
                .sessions
                .contains(&"remote:session_01HVDEMO".to_string())
        );
    }

    #[test]
    fn is_run_active_false_by_default() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn is_run_active_becomes_true_on_turn_start_and_false_on_turn_end() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        assert!(controller.is_run_active());

        controller
            .apply_remote_event_json(r#"{"type":"turn_end","turn":1}"#)
            .unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn run_active_blocks_submit() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();

        assert!(!controller.can_submit());

        controller.update_draft("rush message");
        let result = controller.submit_prompt_command();
        assert!(
            result.is_none(),
            "submit must be blocked while run is active"
        );
    }

    #[test]
    fn cancel_command_json_produced_when_run_active() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(controller.cancel_command().is_none());

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();

        let cancel = controller
            .cancel_command()
            .expect("cancel command expected during active run");
        assert_eq!(cancel.command_json, r#"{"type":"cancel"}"#);
        assert_eq!(cancel.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            cancel.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );
        assert_eq!(
            cancel.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"legacy_json","command_json":"{\"type\":\"cancel\"}"}}"#
        );
    }

    #[test]
    fn session_reset_clears_run_active() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        assert!(controller.is_run_active());

        controller
            .apply_remote_event_json(r#"{"type":"session_reset"}"#)
            .unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn state_snapshot_without_live_instances_purges_session_registry_entries() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        assert!(!controller.attached_instances().is_empty());

        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":0,"done_tasks":0},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":0,"tool_calls":0,"compactions":0},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        assert!(controller.attached_instances().is_empty());
        assert!(
            controller
                .available_command_routes()
                .iter()
                .all(|route| route.route_id == "local-shell")
        );
    }

    #[test]
    fn remote_dispatcher_switch_emits_command_and_updates_pending_state() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let command = controller
            .request_dispatcher_switch_command("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        assert_eq!(
            command.command_json,
            r#"{"model":"openai:gpt-4.1","profile":"supervisor-heavy","request_id":"dispatcher-switch-1","type":"switch_dispatcher"}"#
        );
        assert_eq!(command.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            command.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );

        let session = controller.session_data();
        let switch_state = &session.dispatcher_binding.as_ref().unwrap().switch_state;
        let switch_state = switch_state.as_ref().unwrap();
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.requested_profile.as_deref(),
            Some("supervisor-heavy")
        );
        assert_eq!(
            switch_state.requested_model.as_deref(),
            Some("openai:gpt-4.1")
        );
        assert_eq!(switch_state.status, "pending");
    }

    #[test]
    fn dispatcher_switch_becomes_active_when_snapshot_confirms_binding() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"supervisor-heavy","expected_model":"openai:gpt-4.1","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}]},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let session = controller.session_data();
        let dispatcher = session.dispatcher_binding.as_ref().unwrap();
        assert_eq!(dispatcher.expected_profile, "supervisor-heavy");
        assert_eq!(dispatcher.expected_model.as_deref(), Some("openai:gpt-4.1"));
        let switch_state = dispatcher.switch_state.as_ref().unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.note.as_deref(),
            Some("Dispatcher switch confirmed by snapshot")
        );
        assert!(controller.messages().last().unwrap().text.contains(
            "Dispatcher switch confirmed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1"
        ));
    }

    #[test]
    fn startup_surface_uses_typed_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::Booting);
        controller.set_bootstrap_note(Some("Starting Omegon at /tmp/omegon…".into()));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::Startup);
        assert!(surface.body.contains("Launching the Omegon engine"));
        assert_eq!(
            surface.detail.as_deref(),
            Some("Starting Omegon at /tmp/omegon…")
        );
    }

    #[test]
    fn operator_readiness_blocks_until_auth_inventory_is_loaded() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        let readiness = controller.operator_readiness();

        assert!(!readiness.ready);
        assert_eq!(readiness.title, "Preparing operator controls");
        assert!(
            readiness
                .steps
                .iter()
                .any(|step| step.label == "Auth inventory"
                    && step.state == crate::fixtures::ReadinessStepState::Active)
        );
    }

    #[test]
    fn operator_readiness_becomes_ready_after_auth_inventory_refresh() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        let _ = controller.refresh_settings_auth_status();
        let readiness = controller.operator_readiness();

        assert!(readiness.ready);
        assert_eq!(readiness.title, "Ready");
        assert!(
            readiness
                .steps
                .iter()
                .any(|step| step.label == "Auth inventory"
                    && step.state == crate::fixtures::ReadinessStepState::Complete)
        );
    }

    #[test]
    fn reconnecting_surface_uses_typed_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::Reconnecting);

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::Reconnecting);
        assert!(
            surface
                .body
                .contains("connection to the host is being restored")
        );
        assert_eq!(surface.detail, None);
    }

    #[test]
    fn failed_surface_uses_typed_failure_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::CompatibilityFailure);
        controller.set_bootstrap_note(Some(
            "Update Omegon to a compatible version and restart.".into(),
        ));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::CompatibilityFailure);
        assert_eq!(surface.body, "Host incompatible");
        assert!(surface.detail.as_deref().unwrap().contains("Update Omegon"));
    }

    #[test]
    fn ready_state_uses_bootstrap_note_surface() {
        let mut controller = AppController::default();
        controller.set_bootstrap_note(Some("Attached via startup discovery".into()));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::BootstrapNote);
        assert_eq!(surface.body, "Attached via startup discovery");
        assert_eq!(surface.detail, None);
    }

    #[test]
    fn snapshot_active_state_for_different_request_id_does_not_confirm_local_pending_switch() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"supervisor-heavy","expected_model":"openai:gpt-4.1","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}],"switch_state":{"request_id":"dispatcher-switch-999","requested_profile":"supervisor-heavy","requested_model":"openai:gpt-4.1","status":"active","failure_code":null,"note":"Different request became active"}},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-999")
        );
        assert!(controller.messages().iter().any(|message| {
            message
                .text
                .contains("Dispatcher reports a different active request (dispatcher-switch-999): supervisor-heavy · openai:gpt-4.1")
        }));
        assert!(controller.messages().iter().all(|message| {
            !message
                .text
                .contains("Dispatcher switch confirmed (dispatcher-switch-1)")
        }));
    }

    #[test]
    fn dispatcher_switch_to_current_binding_is_noop() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let command = controller.request_dispatcher_switch_command_json(
            "primary-interactive",
            Some("anthropic:claude-sonnet-4-6"),
        );

        assert!(command.is_none());
        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.note.as_deref(),
            Some("Dispatcher already active: primary-interactive")
        );
    }

    #[test]
    fn repeated_dispatcher_switch_requests_supersede_prior_pending_request() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();
        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", None)
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "pending");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-2")
        );
        assert_eq!(
            switch_state.requested_profile.as_deref(),
            Some("supervisor-heavy")
        );
        assert_eq!(switch_state.requested_model, None);
        assert!(
            controller
                .messages()
                .iter()
                .any(|message| message.text.contains("Dispatcher switch superseded"))
        );
    }

    #[test]
    fn explicit_snapshot_failure_wins_for_dispatcher_switch() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();
        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"primary-interactive","expected_model":"anthropic:claude-sonnet-4-6","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}],"switch_state":{"request_id":"dispatcher-switch-1","requested_profile":"supervisor-heavy","requested_model":"openai:gpt-4.1","status":"failed","failure_code":"backend_rejected","note":"Backend rejected dispatcher switch"}},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "failed");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.failure_code.as_deref(),
            Some("backend_rejected")
        );
        assert!(controller
            .messages()
            .last()
            .unwrap()
            .text
            .contains("Dispatcher switch failed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1 [backend_rejected]"));
    }

    #[test]
    fn session_data_telemetry_includes_selected_route_lifecycle_freshness() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller.evaluate_instance_lifecycle(100);

        let session = controller.session_data();
        assert_eq!(session.telemetry.lifecycle.attached_count, 1);
        assert_eq!(
            session
                .telemetry
                .lifecycle
                .selected_instance
                .as_ref()
                .map(|instance| instance.instance_id.as_str()),
            Some("omg_primary_01HVDEMO")
        );
        assert_eq!(
            session
                .telemetry
                .lifecycle
                .selected_instance
                .as_ref()
                .and_then(|instance| instance.freshness.as_deref()),
            Some("fresh")
        );
        assert_eq!(
            session.telemetry.lifecycle_summary,
            "1 attached instance(s) · ready · freshness fresh"
        );
    }

    #[test]
    fn session_data_telemetry_updates_when_selected_route_becomes_stale() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let instance_id = controller.attached_instances()[0].instance_id.clone();
        controller.attach_instance_record(AttachedInstanceRecord {
            instance_id: instance_id.clone(),
            route_id: "session-dispatcher".into(),
            role: "primary-driver".into(),
            profile: "primary-interactive".into(),
            session_key: "remote:session_01HVDEMO".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("anthropic:claude-sonnet-4-6".into()),
            dispatcher_instance_id: Some(instance_id.clone()),
            registry_record: Some(InstanceRecord {
                schema_version: 1,
                identity: crate::runtime_types::WorkerIdentity {
                    instance_id: instance_id.clone(),
                    role: crate::runtime_types::WorkerRole::PrimaryDriver,
                    profile: "primary-interactive".into(),
                    status: crate::runtime_types::WorkerLifecycleState::Ready,
                    created_at: "2026-04-06T00:00:00Z".into(),
                    updated_at: "2026-04-06T00:00:01Z".into(),
                },
                ownership: crate::runtime_types::WorkerOwnership {
                    owner_kind: crate::runtime_types::OwnerKind::AuspexSession,
                    owner_id: "session_01HVDEMO".into(),
                    parent_instance_id: None,
                },
                desired: crate::runtime_types::DesiredWorkerState::default(),
                observed: crate::runtime_types::ObservedWorkerState {
                    health: crate::runtime_types::ObservedHealth {
                        ready: true,
                        degraded_reason: None,
                        last_heartbeat_at: None,
                        last_seen_at: Some("100".into()),
                        freshness: Some(InstanceFreshness::Fresh),
                    },
                    ..Default::default()
                },
            }),
        });

        controller.evaluate_instance_lifecycle(401);

        let session = controller.session_data();
        assert_eq!(
            session
                .telemetry
                .lifecycle
                .selected_instance
                .as_ref()
                .and_then(|instance| instance.freshness.as_deref()),
            Some("fresh")
        );
        assert_eq!(
            session
                .telemetry
                .lifecycle
                .selected_instance
                .as_ref()
                .and_then(|instance| instance.status.as_deref()),
            Some("ready")
        );
        assert_eq!(
            session.telemetry.lifecycle_summary,
            "1 attached instance(s) · ready · freshness fresh"
        );
    }
}
