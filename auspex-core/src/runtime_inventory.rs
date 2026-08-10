//! Transport-neutral read-only Omegon runtime inventory requests and responses.
//!
//! These DTOs mirror Omegon's stable client control boundary without importing
//! Omegon internals into Auspex. The transport adapter maps each request to the
//! existing WebSocket/IPC control method names.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::runtime_types::{CommandTarget, TargetedCommand};

pub const OMEGON_CLIENT_API_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeInventoryRequest {
    RuntimeInventoryStatus,
    ProfileView,
    WorkspaceStatusView,
    WorkspaceListView,
    SkillsView,
    ExtensionView,
    ArmoryBrowse,
    CatalogView,
    PluginView,
    PermissionsView,
}

impl RuntimeInventoryRequest {
    pub const ALL: [Self; 10] = [
        Self::RuntimeInventoryStatus,
        Self::ProfileView,
        Self::WorkspaceStatusView,
        Self::WorkspaceListView,
        Self::SkillsView,
        Self::ExtensionView,
        Self::ArmoryBrowse,
        Self::CatalogView,
        Self::PluginView,
        Self::PermissionsView,
    ];

    pub const fn method(self) -> &'static str {
        match self {
            Self::RuntimeInventoryStatus => "runtime-inventory-status",
            Self::ProfileView => "profile-view",
            Self::WorkspaceStatusView => "workspace-status-view",
            Self::WorkspaceListView => "workspace-list-view",
            Self::SkillsView => "skills-view",
            Self::ExtensionView => "extension-view",
            Self::ArmoryBrowse => "armory-browse",
            Self::CatalogView => "catalog-view",
            Self::PluginView => "plugin-view",
            Self::PermissionsView => "permissions-view",
        }
    }

    pub fn targeted_command(self, target: CommandTarget) -> TargetedCommand {
        TargetedCommand::control_method(
            target,
            self.method(),
            serde_json::json!({"client_api_version": OMEGON_CLIENT_API_VERSION}),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInventoryResponse {
    pub method: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, flatten)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

impl RuntimeInventoryResponse {
    pub fn decode(
        expected: RuntimeInventoryRequest,
        value: serde_json::Value,
    ) -> Result<Self, String> {
        let response: Self = serde_json::from_value(value)
            .map_err(|error| format!("invalid Omegon inventory response: {error}"))?;
        if response.method != expected.method() {
            return Err(format!(
                "mismatched Omegon inventory response: expected {}, received {}",
                expected.method(),
                response.method
            ));
        }
        Ok(response)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RuntimeInventoryResourceState {
    Loading,
    Ready { response: RuntimeInventoryResponse },
    Stale { response: RuntimeInventoryResponse },
    Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInventoryResource {
    pub instance_id: String,
    pub request: RuntimeInventoryRequest,
    pub state: RuntimeInventoryResourceState,
    pub observed_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeInventoryStore {
    resources: BTreeMap<(String, RuntimeInventoryRequest), RuntimeInventoryResource>,
}

impl RuntimeInventoryStore {
    pub fn mark_loading(
        &mut self,
        instance_id: impl Into<String>,
        request: RuntimeInventoryRequest,
    ) {
        let instance_id = instance_id.into();
        self.resources.insert(
            (instance_id.clone(), request),
            RuntimeInventoryResource {
                instance_id,
                request,
                state: RuntimeInventoryResourceState::Loading,
                observed_at_unix_ms: None,
            },
        );
    }

    pub fn apply_response(
        &mut self,
        instance_id: impl Into<String>,
        request: RuntimeInventoryRequest,
        value: serde_json::Value,
        observed_at_unix_ms: u64,
    ) -> Result<(), String> {
        let instance_id = instance_id.into();
        let response = RuntimeInventoryResponse::decode(request, value)?;
        let state = if response.accepted {
            RuntimeInventoryResourceState::Ready { response }
        } else {
            RuntimeInventoryResourceState::Error {
                message: response
                    .output
                    .unwrap_or_else(|| "Omegon rejected the inventory request".into()),
            }
        };
        self.resources.insert(
            (instance_id.clone(), request),
            RuntimeInventoryResource {
                instance_id,
                request,
                state,
                observed_at_unix_ms: Some(observed_at_unix_ms),
            },
        );
        Ok(())
    }

    pub fn mark_error(
        &mut self,
        instance_id: impl Into<String>,
        request: RuntimeInventoryRequest,
        error: impl Into<String>,
        observed_at_unix_ms: u64,
    ) {
        let instance_id = instance_id.into();
        self.resources.insert(
            (instance_id.clone(), request),
            RuntimeInventoryResource {
                instance_id,
                request,
                state: RuntimeInventoryResourceState::Error {
                    message: error.into(),
                },
                observed_at_unix_ms: Some(observed_at_unix_ms),
            },
        );
    }

    pub fn mark_instance_stale(&mut self, instance_id: &str) {
        for ((resource_instance, _), resource) in &mut self.resources {
            if resource_instance != instance_id {
                continue;
            }
            if let RuntimeInventoryResourceState::Ready { response } = &resource.state {
                resource.state = RuntimeInventoryResourceState::Stale {
                    response: response.clone(),
                };
            }
        }
    }

    pub fn resource(
        &self,
        instance_id: &str,
        request: RuntimeInventoryRequest,
    ) -> Option<&RuntimeInventoryResource> {
        self.resources.get(&(instance_id.to_string(), request))
    }

    pub fn resources_for(&self, instance_id: &str) -> Vec<&RuntimeInventoryResource> {
        self.resources
            .iter()
            .filter_map(|((resource_instance, _), resource)| {
                (resource_instance == instance_id).then_some(resource)
            })
            .collect()
    }

    pub fn remove_instance(&mut self, instance_id: &str) {
        self.resources
            .retain(|(resource_instance, _), _| resource_instance != instance_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> CommandTarget {
        CommandTarget {
            session_key: "remote:primary".into(),
            dispatcher_instance_id: Some("omegon-primary".into()),
        }
    }

    #[test]
    fn every_inventory_request_maps_to_a_versioned_control_command() {
        for request in RuntimeInventoryRequest::ALL {
            let command = request.targeted_command(target());
            let value: serde_json::Value =
                serde_json::from_str(&command.web_command_json()).unwrap();
            assert_eq!(value["type"], request.method());
            assert_eq!(value["client_api_version"], OMEGON_CLIENT_API_VERSION);
            assert!(value["command_id"].is_string());
        }
    }

    #[test]
    fn store_tracks_freshness_error_and_instance_ownership() {
        let mut store = RuntimeInventoryStore::default();
        store.mark_loading("primary-1", RuntimeInventoryRequest::SkillsView);
        assert!(matches!(
            store
                .resource("primary-1", RuntimeInventoryRequest::SkillsView)
                .unwrap()
                .state,
            RuntimeInventoryResourceState::Loading
        ));

        store
            .apply_response(
                "primary-1",
                RuntimeInventoryRequest::SkillsView,
                serde_json::json!({
                    "method": "skills-view",
                    "accepted": true,
                    "skills": [{"name": "rust"}]
                }),
                42,
            )
            .unwrap();
        store.mark_instance_stale("primary-1");
        assert!(matches!(
            store
                .resource("primary-1", RuntimeInventoryRequest::SkillsView)
                .unwrap()
                .state,
            RuntimeInventoryResourceState::Stale { .. }
        ));

        store.mark_error(
            "primary-1",
            RuntimeInventoryRequest::ProfileView,
            "disconnected",
            43,
        );
        assert_eq!(store.resources_for("primary-1").len(), 2);
        store.remove_instance("primary-1");
        assert!(store.resources_for("primary-1").is_empty());
    }

    #[test]
    fn response_decoder_preserves_resource_details_and_checks_method() {
        let response = RuntimeInventoryResponse::decode(
            RuntimeInventoryRequest::SkillsView,
            serde_json::json!({
                "method": "skills-view",
                "accepted": true,
                "output": "2 skills",
                "skills": [{"name": "rust"}, {"name": "security"}]
            }),
        )
        .unwrap();
        assert_eq!(response.details["skills"].as_array().unwrap().len(), 2);

        let error = RuntimeInventoryResponse::decode(
            RuntimeInventoryRequest::ProfileView,
            serde_json::json!({"method": "skills-view", "accepted": true}),
        )
        .unwrap_err();
        assert!(error.contains("mismatched"));
    }
}
