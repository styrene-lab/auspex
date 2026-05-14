use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use omegon_flow::catalog::{
    KIND_ACP_SESSION, KIND_FLEET_DISPATCH, KIND_HUMAN_APPROVAL, KIND_MCP_TOOL, KIND_RESULT_OUTPUT,
    KIND_SENTRY_SUBMIT, KIND_WAIT_FOR_RESULT, KIND_WEBHOOK_INPUT, NodeCatalog, WorkflowRole,
    builtin_workflow_catalog, kind_name,
};
use omegon_flow::{Flow, FlowEdge, FlowEndpoint, FlowMeta, FlowNode, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub const WORKFLOW_PATH: &str = ".auspex/workflows/default.flow";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowState {
    pub path: String,
    pub doc_id: Option<Uuid>,
    pub flow_json: String,
    pub validation: WorkflowValidation,
    pub plan: Option<DispatchPlanPreview>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalogNode {
    pub kind: String,
    pub label: String,
    pub category: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowValidation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl WorkflowValidation {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchPlanPreview {
    pub trigger: String,
    pub steps: Vec<DispatchPlanStep>,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchPlanStep {
    pub node_id: String,
    pub kind: String,
    pub label: String,
    pub action: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_or_create_workflow() -> WorkflowState {
    let path = workflow_abs_path();
    match omegon_flow::load_flow(&path) {
        Ok(doc) => build_state(doc.flow, Some(doc.id), "loaded"),
        Err(_) => {
            let flow = starter_flow();
            let doc_id = Uuid::new_v4();
            let _ = omegon_flow::save_flow(&path, &flow, Some(doc_id));
            build_state(flow, Some(doc_id), "created")
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn load_or_create_workflow() -> WorkflowState {
    build_state(starter_flow(), None, "local")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_workflow(flow: &Flow, doc_id: Option<Uuid>) -> anyhow::Result<Uuid> {
    let doc_id = doc_id.unwrap_or_else(Uuid::new_v4);
    omegon_flow::save_flow(&workflow_abs_path(), flow, Some(doc_id))?;
    Ok(doc_id)
}

pub fn state_from_json(
    flow_json: &str,
    doc_id: Option<Uuid>,
    status: impl Into<String>,
) -> Result<WorkflowState, String> {
    let flow: Flow = serde_json::from_str(flow_json)
        .map_err(|error| format!("Flow editor emitted invalid JSON: {error}"))?;
    Ok(build_state(flow, doc_id, status))
}

pub fn workflow_catalog_nodes() -> Vec<WorkflowCatalogNode> {
    let mut nodes: Vec<_> = builtin_workflow_catalog()
        .definitions()
        .map(|definition| WorkflowCatalogNode {
            kind: definition.kind.clone(),
            label: definition.label.clone(),
            category: definition.category.clone(),
            description: definition.description.clone(),
        })
        .collect();
    nodes.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.label.cmp(&b.label))
    });
    nodes
}

pub fn add_catalog_node(state: &WorkflowState, kind: &str) -> Result<WorkflowState, String> {
    let mut flow: Flow = serde_json::from_str(&state.flow_json)
        .map_err(|error| format!("Stored workflow JSON is invalid: {error}"))?;
    let catalog = builtin_workflow_catalog();
    let definition = catalog
        .get(kind)
        .ok_or_else(|| format!("Unknown workflow node kind `{kind}`."))?;
    let index = flow.nodes.len() as f32;
    let x = 180.0 + (index % 4.0) * 280.0;
    let y = 180.0 + (index / 4.0).floor() * 150.0;

    flow.nodes.push(workflow_node(
        &catalog,
        Uuid::new_v4(),
        kind,
        x,
        y,
        default_node_data(kind, &definition.label),
    ));

    Ok(build_state(flow, state.doc_id, "edited"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_workflow_state(state: &mut WorkflowState) -> anyhow::Result<()> {
    let flow: Flow = serde_json::from_str(&state.flow_json)?;
    let saved_id = save_workflow(&flow, state.doc_id)?;
    state.doc_id = Some(saved_id);
    Ok(())
}

pub fn validate_workflow(flow: &Flow) -> WorkflowValidation {
    let catalog = builtin_workflow_catalog();
    let base = flow.validate();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for edge_id in base.dangling_edges {
        errors.push(format!("Edge {edge_id} points at a missing node."));
    }
    for edge_id in base.edges_with_unknown_sockets {
        errors.push(format!(
            "Edge {edge_id} uses a socket that its node does not declare."
        ));
    }
    for node_id in base.duplicate_node_ids {
        errors.push(format!("Node id {node_id} appears more than once."));
    }
    for edge_id in base.duplicate_edge_ids {
        errors.push(format!("Edge id {edge_id} appears more than once."));
    }
    for (node_id, socket) in base.duplicate_socket_names {
        errors.push(format!(
            "Node {node_id} declares socket `{socket}` more than once."
        ));
    }

    let catalog_report = catalog.validate_flow(flow);
    for (node_id, kind) in catalog_report.unknown_node_kinds {
        errors.push(format!(
            "Node {node_id} uses unknown workflow kind `{kind}`."
        ));
    }
    for (node_id, kind, socket) in catalog_report.missing_required_sockets {
        errors.push(format!(
            "Node {node_id} (`{kind}`) is missing required socket `{socket}`."
        ));
    }
    for (node_id, kind, socket) in catalog_report.unexpected_sockets {
        warnings.push(format!(
            "Node {node_id} (`{kind}`) declares socket `{socket}` outside the shared catalog contract."
        ));
    }

    let shape = catalog.validate_workflow_shape(flow);
    if let Some(count) = shape.trigger_count {
        errors.push(format!(
            "Expected exactly one workflow trigger, found {count}."
        ));
    }
    if shape.output_missing {
        errors.push("Expected at least one workflow output.".to_string());
    }
    for node_id in shape.unreachable_nodes {
        if let Some(node) = flow.node(&node_id) {
            errors.push(format!(
                "Workflow node `{}` is not reachable from the workflow trigger.",
                node_label(node)
            ));
        }
    }

    WorkflowValidation { errors, warnings }
}

pub fn compile_dispatch_plan(flow: &Flow) -> Option<DispatchPlanPreview> {
    let validation = validate_workflow(flow);
    if !validation.is_valid() {
        return None;
    }

    let catalog = builtin_workflow_catalog();
    let input = flow
        .nodes
        .iter()
        .find(|node| catalog.role_for(node) == Some(WorkflowRole::Trigger))?;
    let output = flow
        .nodes
        .iter()
        .find(|node| catalog.role_for(node) == Some(WorkflowRole::Output))?;
    let reachable = reachable_nodes(flow, vec![input.id]);
    let mut indexed: BTreeMap<Uuid, &FlowNode> =
        flow.nodes.iter().map(|node| (node.id, node)).collect();
    let mut steps = Vec::new();
    let mut cursor = input.id;
    let mut visited = BTreeSet::new();

    while visited.insert(cursor) {
        let next = flow
            .edges
            .iter()
            .find(|edge| edge.source.node == cursor)
            .map(|edge| edge.target.node);
        let Some(next_id) = next else { break };
        let Some(node) = indexed.remove(&next_id) else {
            break;
        };
        if catalog.role_for(node) == Some(WorkflowRole::Output) {
            break;
        }
        if reachable.contains(&node.id)
            && matches!(
                catalog.role_for(node),
                Some(WorkflowRole::Executable | WorkflowRole::Branch)
            )
        {
            steps.push(DispatchPlanStep {
                node_id: node.id.to_string(),
                kind: kind_name(&node.kind),
                label: node_label(node),
                action: action_for_node(node),
            });
        }
        cursor = node.id;
    }

    Some(DispatchPlanPreview {
        trigger: node_label(input),
        steps,
        output: node_label(output),
    })
}

fn build_state(flow: Flow, doc_id: Option<Uuid>, status: impl Into<String>) -> WorkflowState {
    let validation = validate_workflow(&flow);
    let plan = compile_dispatch_plan(&flow);
    let flow_json = serde_json::to_string(&flow).unwrap_or_else(|_| "{}".to_string());
    WorkflowState {
        path: WORKFLOW_PATH.to_string(),
        doc_id,
        flow_json,
        validation,
        plan,
        status: status.into(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn workflow_abs_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(WORKFLOW_PATH)
}

fn starter_flow() -> Flow {
    let catalog = builtin_workflow_catalog();
    let input = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let dispatch = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let sentry = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    let output = Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap();

    Flow {
        meta: FlowMeta {
            title: Some("Auspex starter workflow".to_string()),
            description: Some("Webhook to fleet dispatch to Sentry submit.".to_string()),
        },
        nodes: vec![
            workflow_node(
                &catalog,
                input,
                KIND_WEBHOOK_INPUT,
                140.0,
                180.0,
                json!({"label": "Webhook input", "method": "POST", "path": "/api/workflows/default"}),
            ),
            workflow_node(
                &catalog,
                dispatch,
                KIND_FLEET_DISPATCH,
                460.0,
                180.0,
                json!({"label": "Dispatch to fleet", "target": "detached-service", "endpoint": "/api/fleet/dispatch"}),
            ),
            workflow_node(
                &catalog,
                sentry,
                KIND_SENTRY_SUBMIT,
                780.0,
                180.0,
                json!({"label": "Submit Sentry task", "endpoint": "/api/sentry/submit"}),
            ),
            workflow_node(
                &catalog,
                output,
                KIND_RESULT_OUTPUT,
                1100.0,
                180.0,
                json!({"label": "Result output", "topic": "aether.task.result"}),
            ),
        ],
        edges: vec![
            workflow_edge(input, "event", dispatch, "event"),
            workflow_edge(dispatch, "task", sentry, "task"),
            workflow_edge(sentry, "result", output, "result"),
        ],
    }
}

fn default_node_data(kind: &str, label: &str) -> serde_json::Value {
    match kind {
        KIND_WEBHOOK_INPUT => {
            json!({"label": label, "method": "POST", "path": "/api/workflows/default"})
        }
        KIND_FLEET_DISPATCH => {
            json!({"label": label, "target": "detached-service", "endpoint": "/api/fleet/dispatch"})
        }
        KIND_SENTRY_SUBMIT => json!({"label": label, "endpoint": "/api/sentry/submit"}),
        KIND_MCP_TOOL => json!({"label": label, "tool": ""}),
        KIND_ACP_SESSION => json!({"label": label, "session": "default"}),
        KIND_WAIT_FOR_RESULT => json!({"label": label, "timeout_seconds": 300}),
        KIND_HUMAN_APPROVAL => json!({"label": label, "prompt": "Approve workflow continuation"}),
        KIND_RESULT_OUTPUT => json!({"label": label, "topic": "aether.task.result"}),
        _ => json!({"label": label}),
    }
}

fn workflow_node(
    catalog: &NodeCatalog,
    id: Uuid,
    kind: &str,
    x: f32,
    y: f32,
    data: serde_json::Value,
) -> FlowNode {
    let sockets = catalog
        .get(kind)
        .map(|definition| definition.sockets())
        .unwrap_or_default();
    FlowNode {
        id,
        kind: NodeKind::Custom(kind.to_string()),
        position: (x, y),
        data,
        sockets,
    }
}

fn workflow_edge(source: Uuid, source_socket: &str, target: Uuid, target_socket: &str) -> FlowEdge {
    FlowEdge {
        id: Uuid::new_v4(),
        source: FlowEndpoint {
            node: source,
            socket: source_socket.to_string(),
        },
        target: FlowEndpoint {
            node: target,
            socket: target_socket.to_string(),
        },
    }
}

fn reachable_nodes(flow: &Flow, starts: Vec<Uuid>) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = std::collections::VecDeque::from(starts);
    while let Some(node_id) = queue.pop_front() {
        if !seen.insert(node_id) {
            continue;
        }
        for edge in flow.edges.iter().filter(|edge| edge.source.node == node_id) {
            queue.push_back(edge.target.node);
        }
    }
    seen
}

fn node_label(node: &FlowNode) -> String {
    node.data
        .get("label")
        .or_else(|| node.data.get("name"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| kind_name(&node.kind))
}

fn action_for_node(node: &FlowNode) -> String {
    match kind_name(&node.kind).as_str() {
        KIND_FLEET_DISPATCH => "POST /api/fleet/dispatch".to_string(),
        KIND_SENTRY_SUBMIT => "POST /api/sentry/submit".to_string(),
        "mcp_tool" => "invoke MCP tool".to_string(),
        "acp_session" => "open ACP session".to_string(),
        "wait_for_result" => "wait for MQTT/Aether result".to_string(),
        "branch" => "branch on result status".to_string(),
        other => format!("run {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_flow_validates_and_compiles() {
        let flow = starter_flow();
        let validation = validate_workflow(&flow);
        assert_eq!(validation.errors, Vec::<String>::new());
        let plan = compile_dispatch_plan(&flow).expect("starter flow compiles");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "POST /api/fleet/dispatch");
        assert_eq!(plan.steps[1].action, "POST /api/sentry/submit");
    }

    #[test]
    fn unreachable_executable_node_is_actionable_error() {
        let mut flow = starter_flow();
        let catalog = builtin_workflow_catalog();
        flow.nodes.push(workflow_node(
            &catalog,
            Uuid::new_v4(),
            KIND_FLEET_DISPATCH,
            0.0,
            0.0,
            json!({"label": "Detached dispatch"}),
        ));

        let validation = validate_workflow(&flow);
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("Detached dispatch"))
        );
        assert!(compile_dispatch_plan(&flow).is_none());
    }
}
