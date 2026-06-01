//! COP Display Surface — state model for the Common Operating Picture.
//!
//! The COP is a fleet-wide shared display, not per-instance.  The primary
//! omegon agent writes structured content to named **regions** via tool calls
//! (`cop_write`, `cop_clear`, `cop_layout`).  Auspex intercepts these tool
//! events and updates this state; the Dioxus rendering layer reads it.
//!
//! Layout follows the classic **segmenta** model: a dominant center region
//! with four quadrants radiating outward (North, South, East, West).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ── Region identifiers ─────────────────────────────────────

/// Named region in the segmenta layout.
///
/// Center is the dominant region.  Quadrants radiate outward.
/// `Named(String)` allows the agent to define subdivisions in the future.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopRegion {
    Center,
    North,
    South,
    East,
    West,
    Named(String),
}

impl CopRegion {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "center" => Self::Center,
            "north" => Self::North,
            "south" => Self::South,
            "east" => Self::East,
            "west" => Self::West,
            other => Self::Named(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Center => "center",
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::Named(name) => name.as_str(),
        }
    }
}

// ── Content types ──────────────────────────────────────────

/// The kind of structured content a region can hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Table,
    StatusCard,
    AlertFeed,
    KvGrid,
    TextBlock,
    CodeBlock,
    Metric,
}

impl ContentType {
    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().replace('-', "_").trim() {
            "table" => Some(Self::Table),
            "status_card" => Some(Self::StatusCard),
            "alert_feed" => Some(Self::AlertFeed),
            "kv_grid" => Some(Self::KvGrid),
            "text_block" => Some(Self::TextBlock),
            "code_block" => Some(Self::CodeBlock),
            "metric" => Some(Self::Metric),
            _ => None,
        }
    }

    /// Whether this content type appends new data rather than replacing.
    pub fn is_append_mode(&self) -> bool {
        matches!(self, Self::AlertFeed)
    }
}

// ── Region content ─────────────────────────────────────────

/// Content occupying a single COP region.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionContent {
    pub content_type: ContentType,
    pub title: Option<String>,
    pub data: serde_json::Value,
    /// Monotonic sequence number — bumped on every write so renderers can
    /// detect changes cheaply.
    pub seq: u64,
}

// ── Table data model ───────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
}

// ── Status card data model ─────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusCardData {
    pub label: String,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
}

// ── Alert feed data model ──────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertEntry {
    pub message: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

// ── KV grid data model ─────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KvPair {
    pub key: String,
    pub value: serde_json::Value,
}

// ── Metric data model ──────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricData {
    pub label: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub trend: Option<String>,
}

// ── COP display state ──────────────────────────────────────

/// Maximum number of alert-feed entries retained per region.
const MAX_ALERT_FEED_ENTRIES: usize = 100;

/// The full COP display state — one per auspex instance (fleet-wide).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CopDisplayState {
    regions: BTreeMap<CopRegion, RegionContent>,
    write_seq: u64,
    /// Which regions are enabled in the current layout.
    /// If empty, all standard segmenta regions are shown.
    active_regions: Vec<CopRegion>,
}

impl CopDisplayState {
    /// Write content to a region.
    ///
    /// - For append-mode content types (AlertFeed), new data is appended to
    ///   existing entries (up to `MAX_ALERT_FEED_ENTRIES`).
    /// - For all other types, the region content is replaced entirely.
    pub fn write(
        &mut self,
        region: CopRegion,
        content_type: ContentType,
        title: Option<String>,
        data: serde_json::Value,
    ) {
        self.write_seq += 1;
        let seq = self.write_seq;

        if content_type.is_append_mode()
            && let Some(existing) = self.regions.get_mut(&region)
            && existing.content_type == content_type
        {
            existing.seq = seq;

            let new_items = data
                .get("items")
                .and_then(|v| v.as_array())
                .or_else(|| data.as_array())
                .cloned();

            let existing_items = existing
                .data
                .get_mut("items")
                .and_then(|v| v.as_array_mut());

            if let (Some(existing_arr), Some(new_arr)) = (existing_items, &new_items) {
                existing_arr.extend(new_arr.iter().cloned());
                if existing_arr.len() > MAX_ALERT_FEED_ENTRIES {
                    let drain_count = existing_arr.len() - MAX_ALERT_FEED_ENTRIES;
                    existing_arr.drain(..drain_count);
                }
            } else if let Some(new_arr) = new_items {
                existing.data = serde_json::json!({ "items": new_arr });
            }
            return;
        }

        self.regions.insert(
            region,
            RegionContent {
                content_type,
                title,
                data,
                seq,
            },
        );
    }

    /// Clear a single region.
    pub fn clear(&mut self, region: &CopRegion) {
        self.regions.remove(region);
    }

    /// Clear all regions.
    pub fn clear_all(&mut self) {
        self.regions.clear();
    }

    /// Set the active layout — which regions are shown and in what order.
    /// An empty list means "show all standard segmenta regions."
    pub fn set_layout(&mut self, regions: Vec<CopRegion>) {
        self.active_regions = regions;
    }

    /// Get the active layout regions.
    /// Returns the standard segmenta set if none explicitly configured.
    pub fn active_regions(&self) -> &[CopRegion] {
        if self.active_regions.is_empty() {
            // Caller should use default_segmenta_regions() instead
            &[]
        } else {
            &self.active_regions
        }
    }

    /// Get content for a specific region.
    pub fn region(&self, region: &CopRegion) -> Option<&RegionContent> {
        self.regions.get(region)
    }

    /// Iterate over all populated regions.
    pub fn regions(&self) -> impl Iterator<Item = (&CopRegion, &RegionContent)> {
        self.regions.iter()
    }

    /// Whether the COP has any content to display.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Current write sequence number.
    pub fn write_seq(&self) -> u64 {
        self.write_seq
    }

    // ── Tool event interception ────────────────────────────

    /// Try to handle a tool event as a COP surface command.
    /// Returns `true` if the tool name was a cop_* tool and was handled.
    pub fn try_apply_tool_start(
        &mut self,
        tool_name: &str,
        args: Option<&serde_json::Value>,
    ) -> bool {
        match tool_name {
            "cop_write" => {
                if let Some(args) = args {
                    self.apply_cop_write(args);
                }
                true
            }
            "cop_clear" => {
                if let Some(args) = args {
                    self.apply_cop_clear(args);
                }
                true
            }
            "cop_layout" => {
                if let Some(args) = args {
                    self.apply_cop_layout(args);
                }
                true
            }
            _ => false,
        }
    }

    fn apply_cop_write(&mut self, args: &serde_json::Value) {
        let region_str = args
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("center");
        let content_type_str = args
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("text_block");
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let data = args.get("data").cloned().unwrap_or(serde_json::Value::Null);

        let region = CopRegion::from_str_lossy(region_str);
        if let Some(content_type) = ContentType::from_str_lossy(content_type_str) {
            self.write(region, content_type, title, data);
        }
    }

    fn apply_cop_clear(&mut self, args: &serde_json::Value) {
        if let Some(region_str) = args.get("region").and_then(|v| v.as_str()) {
            let region = CopRegion::from_str_lossy(region_str);
            self.clear(&region);
        } else {
            self.clear_all();
        }
    }

    fn apply_cop_layout(&mut self, args: &serde_json::Value) {
        if let Some(regions) = args.get("regions").and_then(|v| v.as_array()) {
            let layout: Vec<CopRegion> = regions
                .iter()
                .filter_map(|v| v.as_str())
                .map(CopRegion::from_str_lossy)
                .collect();
            self.set_layout(layout);
        }
    }
}

/// Render the read-only gateway fleet projection into COP regions.
pub fn apply_gateway_fleet_projection(
    state: &mut CopDisplayState,
    projection: &crate::gateway_projection::GatewayProjectionResponse,
) {
    state.set_layout(vec![
        CopRegion::North,
        CopRegion::Center,
        CopRegion::East,
        CopRegion::South,
    ]);
    state.write(
        CopRegion::North,
        ContentType::StatusCard,
        Some("Fleet Gateway".into()),
        serde_json::json!({
            "label": "Auspex Gateway",
            "status": format!("{:?}", projection.degradation.mode).to_ascii_lowercase(),
            "detail": projection.degradation.reasons.join("; "),
            "severity": match projection.degradation.mode {
                crate::gateway_projection::GatewayDegradationMode::Full => "healthy",
                crate::gateway_projection::GatewayDegradationMode::Degraded => "warning",
                crate::gateway_projection::GatewayDegradationMode::AcpOnly => "degraded",
                crate::gateway_projection::GatewayDegradationMode::Unsupported => "critical",
            }
        }),
    );
    state.write(
        CopRegion::Center,
        ContentType::Table,
        Some("Deployed Fleet".into()),
        serde_json::json!({
            "columns": ["Instance", "Role", "Profile", "Ready", "Compatibility", "Base URL"],
            "rows": projection.fleet.instances.iter().map(|instance| {
                serde_json::json!([
                    instance.instance_id,
                    instance.role,
                    instance.profile,
                    instance.ready,
                    instance.compatibility.as_ref().map(|c| format!("{:?}", c.status).to_ascii_lowercase()).unwrap_or_else(|| "unknown".into()),
                    instance.base_url,
                ])
            }).collect::<Vec<_>>()
        }),
    );
    state.write(
        CopRegion::East,
        ContentType::KvGrid,
        Some("Fleet Summary".into()),
        serde_json::json!({
            "pairs": [
                {"key": "Total", "value": projection.fleet.summary.total_instances},
                {"key": "Ready", "value": projection.fleet.summary.ready_instances},
                {"key": "Compatible", "value": projection.fleet.summary.compatible_instances},
                {"key": "Unsupported", "value": projection.fleet.summary.unsupported_instances},
                {"key": "Pending HostActions", "value": projection.fleet.summary.pending_host_actions}
            ]
        }),
    );
    state.write(
        CopRegion::South,
        ContentType::TextBlock,
        Some("Gateway Degradation".into()),
        serde_json::json!({
            "text": if projection.degradation.reasons.is_empty() {
                "No degradation detected".to_string()
            } else {
                projection.degradation.reasons.join("\n")
            },
            "unavailable_surfaces": projection.degradation.unavailable_surfaces,
            "fallback_surfaces": projection.degradation.fallback_surfaces
        }),
    );
}

/// Render local Omegon discovery candidates into COP regions.
pub fn apply_local_omegon_discovery(
    state: &mut CopDisplayState,
    candidates: &[crate::local_omegon_discovery::LocalOmegonCandidate],
) {
    apply_local_omegon_discovery_with_policy(state, candidates, None);
}

pub fn apply_local_omegon_discovery_with_policy(
    state: &mut CopDisplayState,
    candidates: &[crate::local_omegon_discovery::LocalOmegonCandidate],
    policy: Option<&styrene_policy::PolicyDecision>,
) {
    state.set_layout(vec![CopRegion::North, CopRegion::Center, CopRegion::East]);
    let owned = candidates
        .iter()
        .filter(|candidate| {
            candidate.ownership == crate::local_omegon_discovery::LocalOmegonOwnership::AuspexOwned
        })
        .count();
    let user_owned = candidates
        .iter()
        .filter(|candidate| {
            candidate.ownership == crate::local_omegon_discovery::LocalOmegonOwnership::UserOwned
        })
        .count();
    let unknown = candidates
        .iter()
        .filter(|candidate| {
            candidate.ownership == crate::local_omegon_discovery::LocalOmegonOwnership::Unknown
        })
        .count();

    state.write(
        CopRegion::North,
        ContentType::StatusCard,
        Some("Local Omegon Discovery".into()),
        serde_json::json!({
            "label": "Local Omegon",
            "status": if candidates.is_empty() { "none" } else { "discovered" },
            "detail": format!("{} local candidate(s): {owned} Auspex-owned, {user_owned} user-owned, {unknown} unknown", candidates.len()),
            "severity": if candidates.is_empty() { "warning" } else { "active" },
            "policy": policy.map(|decision| format!("{:?}", decision.effect)).unwrap_or_else(|| "not evaluated".into())
        }),
    );
    state.write(
        CopRegion::Center,
        ContentType::Table,
        Some("Local Candidates".into()),
        serde_json::json!({
            "columns": ["Source", "Ownership", "PID", "Startup URL", "State URL", "Command"],
            "rows": candidates.iter().map(|candidate| {
                serde_json::json!([
                    format!("{:?}", candidate.source),
                    format!("{:?}", candidate.ownership),
                    candidate.pid.map(|pid| pid.to_string()).unwrap_or_else(|| "—".into()),
                    candidate.startup_url.clone().unwrap_or_else(|| "—".into()),
                    candidate.state_url.clone().unwrap_or_else(|| "—".into()),
                    candidate.command.clone().unwrap_or_else(|| "—".into()),
                ])
            }).collect::<Vec<_>>()
        }),
    );
    state.write(
        CopRegion::East,
        ContentType::KvGrid,
        Some("Discovery Summary".into()),
        serde_json::json!({
            "pairs": [
                {"key": "Total", "value": candidates.len()},
                {"key": "Auspex-owned", "value": owned},
                {"key": "User-owned", "value": user_owned},
                {"key": "Unknown", "value": unknown},
                {"key": "Policy", "value": policy.map(|decision| format!("{:?}", decision.effect)).unwrap_or_else(|| "not evaluated".into())},
                {"key": "Obligations", "value": policy.map(|decision| decision.obligations.len()).unwrap_or(0)},
                {"key": "Reasons", "value": policy.map(|decision| decision.reasons.len()).unwrap_or(0)}
            ]
        }),
    );
}

/// The five standard segmenta regions.
pub fn default_segmenta_regions() -> Vec<CopRegion> {
    vec![
        CopRegion::North,
        CopRegion::West,
        CopRegion::Center,
        CopRegion::East,
        CopRegion::South,
    ]
}

// ── Tool definitions for runtime injection ─────────────────

/// JSON schema definitions for the cop_* tools, suitable for injection
/// into an omegon agent's tool surface at runtime.
pub fn cop_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "cop_write",
            "label": "COP Write",
            "description": "Write structured content to a named region of the Common Operating Picture. Content types: table, status_card, alert_feed, kv_grid, text_block, code_block, metric. Regions: center, north, south, east, west.",
            "parameters": {
                "type": "object",
                "properties": {
                    "region": {
                        "type": "string",
                        "description": "Target region: center, north, south, east, west",
                        "default": "center"
                    },
                    "content_type": {
                        "type": "string",
                        "enum": ["table", "status_card", "alert_feed", "kv_grid", "text_block", "code_block", "metric"],
                        "description": "The type of structured content to render"
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional title displayed above the region content"
                    },
                    "data": {
                        "type": "object",
                        "description": "Content payload — schema depends on content_type"
                    }
                },
                "required": ["content_type", "data"]
            }
        }),
        serde_json::json!({
            "name": "cop_clear",
            "label": "COP Clear",
            "description": "Clear a named COP region, or clear all regions if no region specified.",
            "parameters": {
                "type": "object",
                "properties": {
                    "region": {
                        "type": "string",
                        "description": "Region to clear. Omit to clear all regions."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "cop_layout",
            "label": "COP Layout",
            "description": "Configure which regions are active and their arrangement. Default segmenta: center with north/south/east/west quadrants.",
            "parameters": {
                "type": "object",
                "properties": {
                    "regions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Ordered list of region names to activate"
                    }
                },
                "required": ["regions"]
            }
        }),
    ]
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_local_omegon_discovery_writes_candidate_rows() {
        let candidates = vec![
            crate::local_omegon_discovery::LocalOmegonCandidate::from_process_table(
                202,
                "/Users/wilson/.cargo/bin/omegon serve --control-port 7842 --strict-port",
            ),
        ];
        let mut state = CopDisplayState::default();

        apply_local_omegon_discovery(&mut state, &candidates);

        let center = state.region(&CopRegion::Center).unwrap();
        let rows = center
            .data
            .get("rows")
            .and_then(|value| value.as_array())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][1], "UserOwned");
        assert_eq!(rows[0][2], "202");
        assert_eq!(rows[0][3], "http://127.0.0.1:7842/api/startup");
        assert_eq!(
            state.active_regions(),
            &[CopRegion::North, CopRegion::Center, CopRegion::East]
        );
    }

    #[test]
    fn gateway_fleet_projection_renders_non_empty_rows() {
        let projection = crate::gateway_projection::GatewayProjectionResponse::fleet_status(
            crate::fleet_projection::FleetRuntimeProjection::from_instances(&[
                crate::gateway_projection::fixtures::demo_instance(
                    "primary",
                    "0.25.6",
                    true,
                    true,
                    &["state.snapshot"],
                ),
            ]),
        );
        let mut state = CopDisplayState::default();

        apply_gateway_fleet_projection(&mut state, &projection);

        let center = state.region(&CopRegion::Center).unwrap();
        let rows = center.data.get("rows").and_then(|v| v.as_array()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "primary");
        assert_eq!(rows[0][3], true);
        assert_eq!(rows[0][4], "compatible");
    }

    #[test]
    fn gateway_fleet_projection_refresh_is_idempotent_for_degradation_region() {
        let projection = crate::gateway_projection::GatewayProjectionResponse::fleet_status(
            crate::fleet_projection::FleetRuntimeProjection::from_instances(&[]),
        );
        let mut state = CopDisplayState::default();

        apply_gateway_fleet_projection(&mut state, &projection);
        let first = state.region(&CopRegion::South).unwrap().data.clone();
        apply_gateway_fleet_projection(&mut state, &projection);
        let second = state.region(&CopRegion::South).unwrap().data.clone();

        assert_eq!(first, second);
        assert_eq!(
            state.region(&CopRegion::South).unwrap().content_type,
            ContentType::TextBlock
        );
    }

    #[test]
    fn apply_gateway_fleet_projection_writes_owned_cop_regions() {
        let projection = crate::gateway_projection::GatewayProjectionResponse::fleet_status(
            crate::fleet_projection::FleetRuntimeProjection::from_instances(&[]),
        );
        let mut state = CopDisplayState::default();

        apply_gateway_fleet_projection(&mut state, &projection);

        assert!(state.region(&CopRegion::North).is_some());
        assert!(state.region(&CopRegion::Center).is_some());
        assert!(state.region(&CopRegion::East).is_some());
        assert!(state.region(&CopRegion::South).is_some());
        assert!(state.region(&CopRegion::West).is_none());
        assert_eq!(
            state.active_regions(),
            &[
                CopRegion::North,
                CopRegion::Center,
                CopRegion::East,
                CopRegion::South
            ]
        );
    }

    #[test]
    fn gateway_fleet_projection_center_table_uses_projection_not_registry_shape() {
        let projection = crate::gateway_projection::GatewayProjectionResponse::fleet_status(
            crate::fleet_projection::FleetRuntimeProjection::from_instances(&[]),
        );
        let mut state = CopDisplayState::default();

        apply_gateway_fleet_projection(&mut state, &projection);

        let center = state.region(&CopRegion::Center).unwrap();
        assert_eq!(center.content_type, ContentType::Table);
        assert_eq!(center.title.as_deref(), Some("Deployed Fleet"));
        assert!(center.data.get("columns").is_some());
        assert!(center.data.get("rows").is_some());
        assert!(center.data.get("instances").is_none());
    }

    #[test]
    fn write_replaces_region_content() {
        let mut state = CopDisplayState::default();
        state.write(
            CopRegion::Center,
            ContentType::TextBlock,
            Some("Hello".into()),
            serde_json::json!({"text": "first"}),
        );
        assert_eq!(state.region(&CopRegion::Center).unwrap().seq, 1);

        state.write(
            CopRegion::Center,
            ContentType::TextBlock,
            Some("Hello".into()),
            serde_json::json!({"text": "second"}),
        );
        let content = state.region(&CopRegion::Center).unwrap();
        assert_eq!(content.seq, 2);
        assert_eq!(content.data, serde_json::json!({"text": "second"}));
    }

    #[test]
    fn alert_feed_appends() {
        let mut state = CopDisplayState::default();
        state.write(
            CopRegion::North,
            ContentType::AlertFeed,
            Some("Alerts".into()),
            serde_json::json!({"items": [{"message": "first"}]}),
        );
        state.write(
            CopRegion::North,
            ContentType::AlertFeed,
            None,
            serde_json::json!({"items": [{"message": "second"}]}),
        );

        let content = state.region(&CopRegion::North).unwrap();
        let items = content.data["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["message"], "first");
        assert_eq!(items[1]["message"], "second");
    }

    #[test]
    fn alert_feed_trims_to_max() {
        let mut state = CopDisplayState::default();
        let big_feed: Vec<serde_json::Value> = (0..MAX_ALERT_FEED_ENTRIES + 10)
            .map(|i| serde_json::json!({"message": format!("alert-{i}")}))
            .collect();
        state.write(
            CopRegion::East,
            ContentType::AlertFeed,
            None,
            serde_json::json!({"items": big_feed}),
        );
        // Now append more
        state.write(
            CopRegion::East,
            ContentType::AlertFeed,
            None,
            serde_json::json!({"items": [{"message": "overflow"}]}),
        );

        let items = state.region(&CopRegion::East).unwrap().data["items"]
            .as_array()
            .unwrap();
        assert_eq!(items.len(), MAX_ALERT_FEED_ENTRIES);
        // Last entry should be the overflow
        assert_eq!(items.last().unwrap()["message"], "overflow");
    }

    #[test]
    fn clear_removes_region() {
        let mut state = CopDisplayState::default();
        state.write(
            CopRegion::Center,
            ContentType::Metric,
            None,
            serde_json::json!({"value": 42}),
        );
        assert!(!state.is_empty());

        state.clear(&CopRegion::Center);
        assert!(state.is_empty());
    }

    #[test]
    fn clear_all_removes_everything() {
        let mut state = CopDisplayState::default();
        state.write(
            CopRegion::Center,
            ContentType::TextBlock,
            None,
            serde_json::json!({"text": "a"}),
        );
        state.write(
            CopRegion::North,
            ContentType::Metric,
            None,
            serde_json::json!({"value": 1}),
        );
        state.clear_all();
        assert!(state.is_empty());
    }

    #[test]
    fn set_layout_updates_active_regions() {
        let mut state = CopDisplayState::default();
        assert!(state.active_regions().is_empty());

        state.set_layout(vec![CopRegion::Center, CopRegion::South]);
        assert_eq!(
            state.active_regions(),
            &[CopRegion::Center, CopRegion::South]
        );
    }

    #[test]
    fn try_apply_tool_start_cop_write() {
        let mut state = CopDisplayState::default();
        let args = serde_json::json!({
            "region": "north",
            "content_type": "status_card",
            "title": "Fleet Status",
            "data": {"label": "Primary", "status": "healthy"}
        });
        assert!(state.try_apply_tool_start("cop_write", Some(&args)));
        assert!(state.region(&CopRegion::North).is_some());
    }

    #[test]
    fn try_apply_tool_start_cop_clear() {
        let mut state = CopDisplayState::default();
        state.write(
            CopRegion::West,
            ContentType::TextBlock,
            None,
            serde_json::json!({"text": "data"}),
        );
        let args = serde_json::json!({"region": "west"});
        assert!(state.try_apply_tool_start("cop_clear", Some(&args)));
        assert!(state.region(&CopRegion::West).is_none());
    }

    #[test]
    fn try_apply_tool_start_cop_layout() {
        let mut state = CopDisplayState::default();
        let args = serde_json::json!({"regions": ["center", "east", "west"]});
        assert!(state.try_apply_tool_start("cop_layout", Some(&args)));
        assert_eq!(
            state.active_regions(),
            &[CopRegion::Center, CopRegion::East, CopRegion::West]
        );
    }

    #[test]
    fn non_cop_tool_returns_false() {
        let mut state = CopDisplayState::default();
        assert!(!state.try_apply_tool_start("read_file", None));
    }

    #[test]
    fn region_from_str_lossy_case_insensitive() {
        assert_eq!(CopRegion::from_str_lossy("CENTER"), CopRegion::Center);
        assert_eq!(CopRegion::from_str_lossy("North"), CopRegion::North);
        assert_eq!(
            CopRegion::from_str_lossy("custom"),
            CopRegion::Named("custom".into())
        );
    }

    #[test]
    fn content_type_from_str_lossy_handles_variants() {
        assert_eq!(
            ContentType::from_str_lossy("status_card"),
            Some(ContentType::StatusCard)
        );
        assert_eq!(
            ContentType::from_str_lossy("status-card"),
            Some(ContentType::StatusCard)
        );
        assert_eq!(
            ContentType::from_str_lossy("TABLE"),
            Some(ContentType::Table)
        );
        assert_eq!(ContentType::from_str_lossy("unknown"), None);
    }

    #[test]
    fn cop_tool_definitions_are_valid_json() {
        let defs = cop_tool_definitions();
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0]["name"], "cop_write");
        assert_eq!(defs[1]["name"], "cop_clear");
        assert_eq!(defs[2]["name"], "cop_layout");
    }

    #[test]
    fn write_seq_increments_monotonically() {
        let mut state = CopDisplayState::default();
        assert_eq!(state.write_seq(), 0);

        state.write(
            CopRegion::Center,
            ContentType::TextBlock,
            None,
            serde_json::json!({}),
        );
        assert_eq!(state.write_seq(), 1);

        state.write(
            CopRegion::North,
            ContentType::Metric,
            None,
            serde_json::json!({}),
        );
        assert_eq!(state.write_seq(), 2);

        // Appending also increments
        state.write(
            CopRegion::South,
            ContentType::AlertFeed,
            None,
            serde_json::json!({"items": []}),
        );
        assert_eq!(state.write_seq(), 3);
    }
}
