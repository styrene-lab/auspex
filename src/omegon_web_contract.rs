#![allow(dead_code)]

use serde::Deserialize;
use serde_json::Value;

pub const HEADER_PRINCIPAL_ISSUER: &str = "Omegon-Principal-Issuer";
pub const HEADER_PRINCIPAL_SUBJECT: &str = "Omegon-Principal-Subject";
pub const HEADER_PRINCIPAL_ROLE: &str = "Omegon-Principal-Role";
pub const HEADER_PRINCIPAL_DISPLAY_NAME: &str = "Omegon-Principal-Display-Name";
pub const HEADER_PRINCIPAL_SESSION_ID: &str = "Omegon-Principal-Session-Id";
pub const HEADER_PRINCIPAL_CLIENT_ID: &str = "Omegon-Principal-Client-Id";
/// Auspex return URL honored by `/api/web/launch-context` when proxied.
pub const HEADER_BACK_URL: &str = "Omegon-Back-Url";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedPrincipalHeaders {
    pub issuer: String,
    pub subject: String,
    pub role: String,
    pub display_name: Option<String>,
    pub session_id: Option<String>,
    pub client_id: Option<String>,
    pub back_url: Option<String>,
}

impl TrustedPrincipalHeaders {
    pub fn auspex_operator(subject: impl Into<String>) -> Self {
        Self {
            issuer: "auspex".to_string(),
            subject: subject.into(),
            role: "operator".to_string(),
            display_name: None,
            session_id: None,
            client_id: None,
            back_url: None,
        }
    }

    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn back_url(mut self, back_url: impl Into<String>) -> Self {
        self.back_url = Some(back_url.into());
        self
    }

    pub fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            (HEADER_PRINCIPAL_ISSUER, self.issuer.clone()),
            (HEADER_PRINCIPAL_SUBJECT, self.subject.clone()),
            (HEADER_PRINCIPAL_ROLE, self.role.clone()),
        ];
        if let Some(display_name) = &self.display_name {
            pairs.push((HEADER_PRINCIPAL_DISPLAY_NAME, display_name.clone()));
        }
        if let Some(session_id) = &self.session_id {
            pairs.push((HEADER_PRINCIPAL_SESSION_ID, session_id.clone()));
        }
        if let Some(client_id) = &self.client_id {
            pairs.push((HEADER_PRINCIPAL_CLIENT_ID, client_id.clone()));
        }
        if let Some(back_url) = &self.back_url {
            pairs.push((HEADER_BACK_URL, back_url.clone()));
        }
        pairs
    }
}

/// Browser-native session envelope returned by
/// `GET /api/web/sessions/{session_id}` in omegon-secundus.
///
/// This is intentionally a narrow client-side DTO: it names only the fields the
/// Auspex web surface needs to bootstrap and bind the current mock. Unknown
/// backend fields are ignored by serde, so the frontend can move incrementally.
#[derive(Debug, Clone, Deserialize)]
pub struct BackendSessionShowResponse {
    pub schema_version: u8,
    pub session: BackendSessionSummary,
    pub allocation_mode: String,
    pub links: BackendSessionLinks,
    pub snapshot: BackendSurfacesSnapshot,
}

/// Returned by `GET /api/web/launch-context`; tells the web shell whether it is
/// direct Omegon-owned or proxied by Auspex, and who owns policy decisions.
#[derive(Debug, Clone, Deserialize)]
pub struct BackendLaunchContextResponse {
    pub mode: String,
    pub proxied_by: Option<String>,
    pub back_url: Option<String>,
    pub policy_owner: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub created_at: String,
    pub turns: u32,
    pub tool_calls: u32,
    pub last_prompt_snippet: String,
    pub current: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSessionLinks {
    pub surfaces: Option<String>,
    pub actions: Option<String>,
    pub stream: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSurfacesSnapshot {
    pub schema_version: u32,
    pub session_id: String,
    pub revision: u64,
    pub generated_at: String,
    pub surfaces: BackendSurfaceBundle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSurfaceBundle {
    pub conversation: BackendConversationSurface,
    pub editor: BackendEditorSurface,
    pub command: BackendCommandSurface,
    pub command_menu: BackendCommandMenuSurface,
    pub dashboard: BackendDashboardSurface,
    pub footer: BackendFooterSurface,
    pub instruments: BackendInstrumentsSurface,
    pub memory_status: BackendMemoryStatusSurface,
    pub operations: BackendOperationsSurface,
    pub plan: BackendPlanSurface,
    pub runtime: BackendRuntimeSurface,
    pub settings: BackendSettingsSurface,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConversationSurface {
    pub segments: Vec<BackendConversationSegment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConversationSegment {
    pub index: usize,
    pub role: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub complete: bool,
    pub copyable: bool,
    pub selectable: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendEditorSurface {
    pub accepts_prompt: bool,
    pub placeholder: String,
    pub queue_mode: String,
    pub supports_attachments: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendCommandSurface {
    pub pending_prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendCommandMenuSurface {
    pub available: bool,
    pub open: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendDashboardSurface {
    pub session: BackendDashboardSessionSurface,
    pub lifecycle_available: bool,
    pub cleave_available: bool,
    pub delegate_available: bool,
    pub harness_available: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendDashboardSessionSurface {
    pub turns: u32,
    pub tool_calls: u32,
    pub compactions: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendFooterSurface {
    pub busy: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendInstrumentsSurface {
    pub active_tool: Option<String>,
    pub tools: Vec<BackendToolRunSurface>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendToolRunSurface {
    pub id: String,
    pub name: String,
    pub status: String,
    pub args: Value,
    pub output_tail: Option<String>,
    pub result_summary: Option<String>,
    pub is_error: bool,
    pub elapsed_ms: Option<u64>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendMemoryStatusSurface {
    pub active_facts: usize,
    pub total_facts: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendOperationsSurface {
    pub active_child_runtimes: usize,
    pub kind: Option<String>,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub children: Vec<BackendOperationChild>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendOperationChild {
    pub label: String,
    pub status: String,
    pub activity: Option<String>,
    pub tasks_done: usize,
    pub tasks_total: usize,
    pub result_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendPlanSurface {
    pub active: Option<BackendPlanLane>,
    pub workstreams: Vec<BackendPlanWorkstream>,
    pub reconciliation_issues: usize,
    pub promotion_nudges: Vec<String>,
    pub resume_candidates: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendPlanLane {
    pub plan_id: String,
    pub mode: String,
    pub guidance: String,
    pub status: String,
    pub scope: String,
    pub source: String,
    pub completed: usize,
    pub total: usize,
    pub items: Vec<BackendPlanItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendPlanItem {
    pub id: Option<String>,
    pub label: String,
    pub status: String,
    pub intent: Option<String>,
    pub writable: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendPlanWorkstream {
    pub id: String,
    pub title: String,
    pub status: String,
    pub completed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendRuntimeSurface {
    pub context_class: Option<String>,
    pub thinking_level: Option<String>,
    pub capability_grade: Option<String>,
    pub posture: Option<String>,
    pub operating_profile: Option<String>,
    pub autonomy_mode: Option<String>,
    pub session_kind: Option<String>,
    pub git_branch: Option<String>,
    pub active_persona: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSettingsSurface {
    pub auth_mode: Option<String>,
    pub auth_source: Option<String>,
}

pub const BACKEND_SESSION_FIXTURE: &str = include_str!("../fixtures/omegon-web-session-default.json");
pub const BACKEND_LAUNCH_CONTEXT_PROXIED_FIXTURE: &str =
    include_str!("../fixtures/omegon-web-launch-context-proxied.json");

pub fn parse_backend_session(input: &str) -> Result<BackendSessionShowResponse, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_launch_context(input: &str) -> Result<BackendLaunchContextResponse, serde_json::Error> {
    serde_json::from_str(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendLoadError {
    UnsupportedTarget,
    Fetch { endpoint: &'static str, reason: String },
}

impl std::fmt::Display for BackendLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget => write!(f, "live backend fetch is only available in wasm builds"),
            Self::Fetch { endpoint, reason } => write!(f, "{endpoint}: {reason}"),
        }
    }
}

impl std::error::Error for BackendLoadError {}

#[cfg(target_arch = "wasm32")]
async fn fetch_json<T>(endpoint: &'static str) -> Result<T, BackendLoadError>
where
    T: for<'de> Deserialize<'de>,
{
    gloo_net::http::Request::get(endpoint)
        .send()
        .await
        .map_err(|error| BackendLoadError::Fetch {
            endpoint,
            reason: error.to_string(),
        })?
        .json::<T>()
        .await
        .map_err(|error| BackendLoadError::Fetch {
            endpoint,
            reason: error.to_string(),
        })
}

#[cfg(target_arch = "wasm32")]
pub async fn load_initial_session(
) -> Result<(BackendSessionShowResponse, BackendLaunchContextResponse), BackendLoadError> {
    let launch_context = fetch_json::<BackendLaunchContextResponse>("/api/web/launch-context").await?;
    // Compatibility session endpoint is intentionally used for bootstrap: native
    // `/api/sessions/default` is RBAC-gated and belongs to the next action/auth
    // slice once Auspex forwards bearer + Omegon-Principal-* headers.
    let session = fetch_json::<BackendSessionShowResponse>("/api/web/sessions/default").await?;
    Ok((session, launch_context))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn load_initial_session(
) -> Result<(BackendSessionShowResponse, BackendLaunchContextResponse), BackendLoadError> {
    Err(BackendLoadError::UnsupportedTarget)
}

pub fn fixture_session() -> BackendSessionShowResponse {
    parse_backend_session(BACKEND_SESSION_FIXTURE)
        .expect("fixtures/omegon-web-session-default.json must match omegon-secundus web session contract")
}

pub fn proxied_launch_context_fixture() -> BackendLaunchContextResponse {
    parse_launch_context(BACKEND_LAUNCH_CONTEXT_PROXIED_FIXTURE)
        .expect("fixtures/omegon-web-launch-context-proxied.json must match omegon-secundus launch context contract")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_matches_native_session_surface_contract() {
        let session = fixture_session();
        assert_eq!(session.schema_version, 1);
        assert_eq!(session.session.session_id, "default");
        assert_eq!(session.allocation_mode, "singleton-live");
        assert_eq!(
            session.links.surfaces.as_deref(),
            Some("/api/sessions/default/surfaces")
        );
        assert_eq!(
            session.links.actions.as_deref(),
            Some("/api/sessions/default/actions")
        );
        assert_eq!(
            session.links.stream.as_deref(),
            Some("/api/sessions/default/surfaces/stream")
        );
        assert_eq!(session.snapshot.schema_version, 1);
        assert_eq!(session.snapshot.surfaces.conversation.segments.len(), 2);
        assert_eq!(session.snapshot.surfaces.instruments.tools.len(), 2);
        assert_eq!(
            session.snapshot.surfaces.instruments.active_tool.as_deref(),
            Some("bash")
        );
        let plan = session
            .snapshot
            .surfaces
            .plan
            .active
            .as_ref()
            .expect("fixture should carry an active plan lane");
        assert_eq!(plan.mode, "executing");
        assert_eq!(plan.completed, 1);
        assert_eq!(plan.total, 4);
        assert_eq!(plan.items.len(), 4);
        assert_eq!(session.snapshot.surfaces.operations.children.len(), 3);
        assert_eq!(
            session.snapshot.surfaces.runtime.autonomy_mode.as_deref(),
            Some("Conservative")
        );
    }

    #[test]
    fn proxied_launch_context_fixture_matches_backend_contract() {
        let launch = proxied_launch_context_fixture();
        assert_eq!(launch.mode, "proxied");
        assert_eq!(launch.proxied_by.as_deref(), Some("auspex"));
        assert_eq!(launch.back_url.as_deref(), Some("http://127.0.0.1:7820/"));
        assert_eq!(launch.policy_owner, "auspex");
    }

    #[test]
    fn trusted_principal_headers_use_standard_backend_names() {
        let headers = TrustedPrincipalHeaders::auspex_operator("operator:wilson")
            .display_name("Wilson")
            .session_id("default")
            .client_id("auspex-web")
            .back_url("http://127.0.0.1:7820/")
            .pairs();

        assert_eq!(
            headers,
            vec![
                (HEADER_PRINCIPAL_ISSUER, "auspex".to_string()),
                (HEADER_PRINCIPAL_SUBJECT, "operator:wilson".to_string()),
                (HEADER_PRINCIPAL_ROLE, "operator".to_string()),
                (HEADER_PRINCIPAL_DISPLAY_NAME, "Wilson".to_string()),
                (HEADER_PRINCIPAL_SESSION_ID, "default".to_string()),
                (HEADER_PRINCIPAL_CLIENT_ID, "auspex-web".to_string()),
                (HEADER_BACK_URL, "http://127.0.0.1:7820/".to_string()),
            ]
        );
    }
}
