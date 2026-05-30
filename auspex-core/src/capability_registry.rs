#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CapabilityKey {
    pub kind: CapabilityKind,
    pub name: String,
}

impl CapabilityKey {
    pub fn new(kind: CapabilityKind, name: impl Into<String>) -> Self {
        Self { kind, name: name.into() }
    }

    pub fn binary(name: impl Into<String>) -> Self {
        Self::new(CapabilityKind::Binary, name)
    }

    pub fn extension(name: impl Into<String>) -> Self {
        Self::new(CapabilityKind::Extension, name)
    }

    pub fn host_action(name: impl Into<String>) -> Self {
        Self::new(CapabilityKind::HostAction, name)
    }

    pub fn tool(name: impl Into<String>) -> Self {
        Self::new(CapabilityKind::Tool, name)
    }

    pub fn package(name: impl Into<String>) -> Self {
        Self::new(CapabilityKind::Package, name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Binary,
    Extension,
    HostAction,
    Tool,
    Runtime,
    Service,
    Package,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEvidence {
    pub key: CapabilityKey,
    pub status: CapabilityStatus,
    pub source: CapabilitySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

impl CapabilityEvidence {
    pub fn present(key: CapabilityKey, source: CapabilitySource) -> Self {
        Self { key, status: CapabilityStatus::Present, source, detail: None, observed_at: None }
    }

    pub fn installable(key: CapabilityKey, source: CapabilitySource, detail: impl Into<String>) -> Self {
        Self { key, status: CapabilityStatus::Installable, source, detail: Some(detail.into()), observed_at: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Present,
    Missing,
    Installable,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CapabilitySource {
    InstanceDescriptor,
    AcpInitializeMetadata { extension: String },
    NexResolver,
    ArmoryArtifact { artifact: String },
    HostActionPolicy,
    OperatorDeclared,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceCapabilitySnapshot {
    pub instance_id: String,
    #[serde(default)]
    pub evidence: Vec<CapabilityEvidence>,
}

impl InstanceCapabilitySnapshot {
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self { instance_id: instance_id.into(), evidence: Vec::new() }
    }

    pub fn empty(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id)
    }

    pub fn add(&mut self, evidence: CapabilityEvidence) {
        self.evidence.push(evidence);
    }

    pub fn has_present(&self, key: &CapabilityKey) -> bool {
        self.evidence
            .iter()
            .any(|e| &e.key == key && e.status == CapabilityStatus::Present)
    }

    pub fn from_instance_descriptor_capabilities(
        instance_id: impl Into<String>,
        capabilities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut snapshot = Self::new(instance_id);
        for capability in capabilities {
            snapshot.add(CapabilityEvidence::present(
                CapabilityKey::tool(capability.into()),
                CapabilitySource::InstanceDescriptor,
            ));
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_capabilities_become_present_tool_evidence() {
        let snapshot = InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
            "agent-1",
            ["state.snapshot", "events.stream"],
        );

        assert!(snapshot.has_present(&CapabilityKey::tool("state.snapshot")));
        assert!(snapshot.has_present(&CapabilityKey::tool("events.stream")));
        assert!(!snapshot.has_present(&CapabilityKey::tool("package.install@1")));
    }

    #[test]
    fn installable_evidence_preserves_source_detail() {
        let evidence = CapabilityEvidence::installable(
            CapabilityKey::binary("d2"),
            CapabilitySource::NexResolver,
            "create project Nex overlay with package d2",
        );

        assert_eq!(evidence.status, CapabilityStatus::Installable);
        assert_eq!(evidence.detail.as_deref(), Some("create project Nex overlay with package d2"));
    }
}
