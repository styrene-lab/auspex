#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const HEADER_PRINCIPAL_ISSUER: &str = "Omegon-Principal-Issuer";
pub const HEADER_PRINCIPAL_SUBJECT: &str = "Omegon-Principal-Subject";
pub const HEADER_PRINCIPAL_ROLE: &str = "Omegon-Principal-Role";
pub const HEADER_PRINCIPAL_DISPLAY_NAME: &str = "Omegon-Principal-Display-Name";
pub const HEADER_PRINCIPAL_SESSION_ID: &str = "Omegon-Principal-Session-Id";
pub const HEADER_PRINCIPAL_CLIENT_ID: &str = "Omegon-Principal-Client-Id";
/// Auspex return URL honored by `/api/web/launch-context` when proxied.
pub const HEADER_BACK_URL: &str = "Omegon-Back-Url";

/// Bearer token for RBAC-gated daemon routes (`/api/sessions/*`).
///
/// The omegon daemon issues an ephemeral bearer (or signed-attach) token at
/// startup and expects it as `Authorization: Bearer <token>` on HTTP and as a
/// `?token=` query parameter on WebSocket upgrades (browsers cannot set WS
/// headers). The web surface reads it from the page URL: `/?token=<token>`.
#[cfg(target_arch = "wasm32")]
pub fn page_query_token() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let query = search.strip_prefix('?').unwrap_or(&search);
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("token=") {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn page_query_token() -> Option<String> {
    None
}

/// Append the daemon query token to a stream endpoint, preserving existing
/// query parameters.
pub fn endpoint_with_token(endpoint: &str, token: Option<&str>) -> String {
    match token {
        Some(token) if !token.is_empty() => {
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            format!("{endpoint}{separator}token={token}")
        }
        _ => endpoint.to_string(),
    }
}

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
    pub description: String,
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
    pub request_id: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct BackendSurfaceStreamEnvelope {
    pub schema_version: u8,
    pub session_id: String,
    pub revision: u64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub surface: Option<String>,
    pub payload: Value,
}

pub const BACKEND_SESSION_FIXTURE: &str =
    include_str!("../fixtures/omegon-web-session-default.json");
pub const BACKEND_LAUNCH_CONTEXT_PROXIED_FIXTURE: &str =
    include_str!("../fixtures/omegon-web-launch-context-proxied.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebActionRequest {
    pub schema_version: u32,
    pub action_id: String,
    pub client_id: String,
    #[serde(default = "default_web_session_id")]
    pub session_id: String,
    pub action: WebActionPayload,
}

fn default_web_session_id() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebActionPayload {
    SubmitPrompt {
        text: String,
        #[serde(default)]
        attachments: Vec<String>,
    },
    CancelActiveTurn,
    RunSlashCommand {
        raw: String,
    },
    RespondPermission {
        request_id: String,
        allow: bool,
    },
    RespondOperatorWait {
        request_id: String,
        completed: bool,
    },
    CopyLatestResponse,
    SelectSegment {
        index: usize,
    },
    CopySegment {
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiActionOutcomeEnvelope {
    pub protocol_version: u32,
    pub session_id: String,
    pub action_id: String,
    pub status: UiActionOutcomeStatus,
    pub revision_after: Option<u64>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiActionOutcomeStatus {
    Accepted,
    Rejected,
    Noop,
    Deferred,
}

pub fn submit_prompt_action(
    action_id: impl Into<String>,
    client_id: impl Into<String>,
    session_id: impl Into<String>,
    text: impl Into<String>,
    attachments: Vec<String>,
) -> WebActionRequest {
    WebActionRequest {
        schema_version: 1,
        action_id: action_id.into(),
        client_id: client_id.into(),
        session_id: session_id.into(),
        action: WebActionPayload::SubmitPrompt {
            text: text.into(),
            attachments,
        },
    }
}

pub fn respond_permission_action(
    action_id: impl Into<String>,
    client_id: impl Into<String>,
    session_id: impl Into<String>,
    request_id: impl Into<String>,
    allow: bool,
) -> WebActionRequest {
    WebActionRequest {
        schema_version: 1,
        action_id: action_id.into(),
        client_id: client_id.into(),
        session_id: session_id.into(),
        action: WebActionPayload::RespondPermission {
            request_id: request_id.into(),
            allow,
        },
    }
}

pub fn parse_backend_session(input: &str) -> Result<BackendSessionShowResponse, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_launch_context(
    input: &str,
) -> Result<BackendLaunchContextResponse, serde_json::Error> {
    serde_json::from_str(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendLoadError {
    UnsupportedTarget,
    Fetch {
        endpoint: &'static str,
        reason: String,
    },
}

impl std::fmt::Display for BackendLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget => {
                write!(f, "live backend fetch is only available in wasm builds")
            }
            Self::Fetch { endpoint, reason } => write!(f, "{endpoint}: {reason}"),
        }
    }
}

impl std::error::Error for BackendLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionTransportError {
    UnsupportedTarget,
    Serialize(String),
    Request {
        endpoint: String,
        reason: String,
    },
    Http {
        endpoint: String,
        status: u16,
        body: String,
    },
    Decode {
        endpoint: String,
        reason: String,
    },
}

impl std::fmt::Display for ActionTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget => {
                write!(f, "live backend actions are only available in wasm builds")
            }
            Self::Serialize(reason) => write!(f, "serialize action request: {reason}"),
            Self::Request { endpoint, reason } => write!(f, "POST {endpoint} failed: {reason}"),
            Self::Http {
                endpoint,
                status,
                body,
            } => write!(f, "POST {endpoint} returned {status}: {body}"),
            Self::Decode { endpoint, reason } => {
                write!(f, "POST {endpoint} response decode failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ActionTransportError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceRefreshError {
    UnsupportedTarget,
    Request {
        endpoint: String,
        reason: String,
    },
    Http {
        endpoint: String,
        status: u16,
        body: String,
    },
    Decode {
        endpoint: String,
        reason: String,
    },
}

impl std::fmt::Display for SurfaceRefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget => {
                write!(f, "live surface refresh is only available in wasm builds")
            }
            Self::Request { endpoint, reason } => write!(f, "GET {endpoint} failed: {reason}"),
            Self::Http {
                endpoint,
                status,
                body,
            } => write!(f, "GET {endpoint} returned {status}: {body}"),
            Self::Decode { endpoint, reason } => {
                write!(f, "GET {endpoint} response decode failed: {reason}")
            }
        }
    }
}

impl std::error::Error for SurfaceRefreshError {}

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
pub async fn post_action_request(
    endpoint: &str,
    request: &WebActionRequest,
    principal: Option<&TrustedPrincipalHeaders>,
) -> Result<UiActionOutcomeEnvelope, ActionTransportError> {
    let body = serde_json::to_string(request)
        .map_err(|error| ActionTransportError::Serialize(error.to_string()))?;
    let mut builder =
        gloo_net::http::Request::post(endpoint).header("content-type", "application/json");
    if let Some(token) = page_query_token() {
        builder = builder.header("authorization", &format!("Bearer {token}"));
    }
    if let Some(principal) = principal {
        for (name, value) in principal.pairs() {
            builder = builder.header(name, &value);
        }
    }
    let request = builder
        .body(body)
        .map_err(|error| ActionTransportError::Request {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;
    let response = request
        .send()
        .await
        .map_err(|error| ActionTransportError::Request {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| ActionTransportError::Request {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;
    if !(200..300).contains(&status) {
        return Err(ActionTransportError::Http {
            endpoint: endpoint.to_string(),
            status,
            body: text,
        });
    }
    serde_json::from_str(&text).map_err(|error| ActionTransportError::Decode {
        endpoint: endpoint.to_string(),
        reason: error.to_string(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_action_request(
    _endpoint: &str,
    _request: &WebActionRequest,
    _principal: Option<&TrustedPrincipalHeaders>,
) -> Result<UiActionOutcomeEnvelope, ActionTransportError> {
    Err(ActionTransportError::UnsupportedTarget)
}

#[cfg(target_arch = "wasm32")]
pub async fn refresh_surfaces_snapshot(
    endpoint: &str,
) -> Result<BackendSurfacesSnapshot, SurfaceRefreshError> {
    let mut builder = gloo_net::http::Request::get(endpoint);
    if let Some(token) = page_query_token() {
        builder = builder.header("authorization", &format!("Bearer {token}"));
    }
    let response = builder
        .send()
        .await
        .map_err(|error| SurfaceRefreshError::Request {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| SurfaceRefreshError::Request {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;
    if !(200..300).contains(&status) {
        return Err(SurfaceRefreshError::Http {
            endpoint: endpoint.to_string(),
            status,
            body: text,
        });
    }
    serde_json::from_str(&text).map_err(|error| SurfaceRefreshError::Decode {
        endpoint: endpoint.to_string(),
        reason: error.to_string(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn refresh_surfaces_snapshot(
    _endpoint: &str,
) -> Result<BackendSurfacesSnapshot, SurfaceRefreshError> {
    Err(SurfaceRefreshError::UnsupportedTarget)
}

#[cfg(target_arch = "wasm32")]
pub async fn load_initial_session()
-> Result<(BackendSessionShowResponse, BackendLaunchContextResponse), BackendLoadError> {
    let launch_context =
        fetch_json::<BackendLaunchContextResponse>("/api/web/launch-context").await?;
    // Compatibility session endpoint is intentionally used for bootstrap: native
    // `/api/sessions/default` is RBAC-gated and belongs to the next action/auth
    // slice once Auspex forwards bearer + Omegon-Principal-* headers.
    let session = fetch_json::<BackendSessionShowResponse>("/api/web/sessions/default").await?;
    Ok((session, launch_context))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn load_initial_session()
-> Result<(BackendSessionShowResponse, BackendLaunchContextResponse), BackendLoadError> {
    Err(BackendLoadError::UnsupportedTarget)
}

pub fn fixture_session() -> BackendSessionShowResponse {
    parse_backend_session(BACKEND_SESSION_FIXTURE).expect(
        "fixtures/omegon-web-session-default.json must match omegon-secundus web session contract",
    )
}

pub fn proxied_launch_context_fixture() -> BackendLaunchContextResponse {
    parse_launch_context(BACKEND_LAUNCH_CONTEXT_PROXIED_FIXTURE)
        .expect("fixtures/omegon-web-launch-context-proxied.json must match omegon-secundus launch context contract")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_with_token_appends_query_parameter() {
        assert_eq!(
            endpoint_with_token("/api/sessions/default/surfaces/stream", Some("abc123")),
            "/api/sessions/default/surfaces/stream?token=abc123"
        );
        assert_eq!(
            endpoint_with_token("/stream?mode=live", Some("abc123")),
            "/stream?mode=live&token=abc123"
        );
        assert_eq!(endpoint_with_token("/stream", None), "/stream");
        assert_eq!(endpoint_with_token("/stream", Some("")), "/stream");
    }

    #[test]
    fn page_query_token_is_none_off_wasm() {
        assert_eq!(page_query_token(), None);
    }

    #[test]
    fn fixture_matches_native_session_surface_contract() {
        let session = fixture_session();
        assert_eq!(session.schema_version, 1);
        assert_eq!(session.session.session_id, "default");
        assert_eq!(session.session.description, "Current live session");
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

    #[test]
    fn action_requests_serialize_to_native_backend_shape() {
        let submit = submit_prompt_action(
            "act-1",
            "auspex-web",
            "default",
            "continue from the release candidate plan",
            vec!["att-1".to_string()],
        );
        let submit_json = serde_json::to_value(&submit).expect("serialize submit action");
        assert_eq!(submit_json["schema_version"], 1);
        assert_eq!(submit_json["action_id"], "act-1");
        assert_eq!(submit_json["client_id"], "auspex-web");
        assert_eq!(submit_json["session_id"], "default");
        assert_eq!(submit_json["action"]["type"], "submit_prompt");
        assert_eq!(
            submit_json["action"]["text"],
            "continue from the release candidate plan"
        );
        assert_eq!(submit_json["action"]["attachments"][0], "att-1");

        let approval = respond_permission_action("act-2", "auspex-web", "default", "perm-7", true);
        let approval_json = serde_json::to_value(&approval).expect("serialize approval action");
        assert_eq!(approval_json["action"]["type"], "respond_permission");
        assert_eq!(approval_json["action"]["request_id"], "perm-7");
        assert_eq!(approval_json["action"]["allow"], true);

        let defaulted: WebActionRequest = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "action_id": "act-3",
            "client_id": "auspex-web",
            "action": { "type": "cancel_active_turn" }
        }))
        .expect("session_id defaults for compatibility with backend");
        assert_eq!(defaulted.session_id, "default");
    }

    #[test]
    fn surface_stream_envelope_deserializes_snapshot_event() {
        let fixture: Value = serde_json::from_str(BACKEND_SESSION_FIXTURE).expect("fixture json");
        let envelope: BackendSurfaceStreamEnvelope = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "session_id": "default",
            "revision": 7,
            "type": "snapshot",
            "surface": null,
            "payload": fixture["snapshot"].clone()
        }))
        .expect("stream snapshot envelope");
        assert_eq!(envelope.event_type, "snapshot");
        assert_eq!(envelope.revision, 7);
        let snapshot: BackendSurfacesSnapshot = serde_json::from_value(envelope.payload)
            .expect("snapshot payload follows surfaces contract");
        assert_eq!(snapshot.session_id, "default");
    }

    #[test]
    fn ui_action_outcome_deserializes_backend_envelope() {
        let accepted: UiActionOutcomeEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "sessionId": "default",
            "actionId": "act-1",
            "status": "accepted",
            "revisionAfter": 42,
            "message": "queued",
            "error": null
        }))
        .expect("accepted outcome");
        assert_eq!(accepted.protocol_version, 1);
        assert_eq!(accepted.status, UiActionOutcomeStatus::Accepted);
        assert_eq!(accepted.revision_after, Some(42));
        assert_eq!(accepted.message.as_deref(), Some("queued"));

        let rejected: UiActionOutcomeEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "sessionId": "default",
            "actionId": "act-2",
            "status": "rejected",
            "revisionAfter": null,
            "message": null,
            "error": "capability_not_granted"
        }))
        .expect("rejected outcome");
        assert_eq!(rejected.status, UiActionOutcomeStatus::Rejected);
        assert_eq!(rejected.error.as_deref(), Some("capability_not_granted"));
    }
}
