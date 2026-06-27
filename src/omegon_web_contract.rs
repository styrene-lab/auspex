#![allow(dead_code)]

use serde::Deserialize;
use serde_json::Value;

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

pub fn parse_backend_session(input: &str) -> Result<BackendSessionShowResponse, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn fixture_session() -> BackendSessionShowResponse {
    parse_backend_session(BACKEND_SESSION_FIXTURE)
        .expect("fixtures/omegon-web-session-default.json must match omegon-secundus web session contract")
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
}
