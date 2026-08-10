//! Transport-neutral read-only Omegon runtime inventory requests and responses.
//!
//! These DTOs mirror Omegon's stable client control boundary without importing
//! Omegon internals into Auspex. The transport adapter maps each request to the
//! existing WebSocket/IPC control method names.

use serde::{Deserialize, Serialize};

use crate::runtime_types::{CommandTarget, TargetedCommand};

pub const OMEGON_CLIENT_API_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
