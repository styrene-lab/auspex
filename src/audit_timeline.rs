use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::fixtures::{AttributedText, ToolCard, TranscriptData, TurnBlock};

const AUDIT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTimelineStore {
    pub schema_version: u32,
    pub entries: Vec<AuditEntry>,
    #[serde(skip)]
    seen_ids: BTreeSet<String>,
}

impl Default for AuditTimelineStore {
    fn default() -> Self {
        Self {
            schema_version: AUDIT_SCHEMA_VERSION,
            entries: Vec::new(),
            seen_ids: BTreeSet::new(),
        }
    }
}

impl AuditTimelineStore {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut store = serde_json::from_str::<Self>(json)?;
        store.rebuild_seen_ids();
        Ok(store)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn append_transcript_snapshot(&mut self, session_key: &str, transcript: &TranscriptData) -> usize {
        let mut appended = 0;
        for turn in &transcript.turns {
            for (block_index, block) in turn.blocks.iter().enumerate() {
                let entry = AuditEntry::from_block(session_key, turn.number, block_index, block);
                if self.seen_ids.insert(entry.block_id.clone()) {
                    self.entries.push(entry);
                    appended += 1;
                }
            }
        }
        appended
    }

    fn rebuild_seen_ids(&mut self) {
        self.seen_ids = self.entries.iter().map(|entry| entry.block_id.clone()).collect();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub session_key: String,
    pub turn_number: u32,
    pub block_index: usize,
    pub block_id: String,
    pub kind: AuditEntryKind,
    pub label: String,
    pub content: String,
}

impl AuditEntry {
    fn from_block(session_key: &str, turn_number: u32, block_index: usize, block: &TurnBlock) -> Self {
        let (kind, label, content) = match block {
            TurnBlock::Thinking(thinking) => (
                AuditEntryKind::Thinking,
                "Thinking".to_string(),
                thinking.text.clone(),
            ),
            TurnBlock::Text(text) => (
                AuditEntryKind::Text,
                attributed_label(text, "Message"),
                text.text.clone(),
            ),
            TurnBlock::Tool(tool) => (
                AuditEntryKind::Tool,
                format!("Tool · {}", tool.name),
                tool_content(tool),
            ),
            TurnBlock::System(text) => (
                AuditEntryKind::System,
                attributed_label(text, "System"),
                text.text.clone(),
            ),
            TurnBlock::Aborted(text) => (
                AuditEntryKind::Aborted,
                "Aborted".to_string(),
                text.clone(),
            ),
        };

        Self {
            session_key: session_key.to_string(),
            turn_number,
            block_index,
            block_id: format!("{session_key}:turn-{turn_number}-block-{block_index}"),
            kind,
            label,
            content,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEntryKind {
    Thinking,
    Text,
    Tool,
    System,
    Aborted,
}

fn attributed_label(text: &AttributedText, fallback: &str) -> String {
    text.origin
        .as_ref()
        .map(|origin| origin.label.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn tool_content(tool: &ToolCard) -> String {
    let mut parts = Vec::new();
    if !tool.args.is_empty() {
        parts.push(format!("args:\n{}", tool.args));
    }
    if !tool.partial_output.is_empty() {
        parts.push(format!("partial_output:\n{}", tool.partial_output));
    }
    if let Some(result) = &tool.result {
        parts.push(format!("result:\n{result}"));
    }
    if parts.is_empty() {
        tool.name.clone()
    } else {
        parts.join("\n\n")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn default_audit_timeline_path() -> Option<std::path::PathBuf> {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let mut path = std::path::PathBuf::from(home);
                path.push(".config");
                path
            })
        })?;
    let mut path = config_root;
    path.push("auspex");
    path.push("audit-timeline.json");
    Some(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_or_default(path: &std::path::Path) -> AuditTimelineStore {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| AuditTimelineStore::from_json(&json).ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn persist(path: &std::path::Path, store: &AuditTimelineStore) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = store
        .to_json_pretty()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{AttributedText, BlockOrigin, OriginKind, SystemNoticeKind, TranscriptData, Turn, TurnBlock, TurnBlockText};

    #[test]
    fn append_transcript_snapshot_dedupes_by_stable_block_id() {
        let transcript = TranscriptData {
            turns: vec![Turn {
                number: 4,
                blocks: vec![
                    TurnBlock::Thinking(TurnBlockText {
                        text: "inspect state".into(),
                        collapsed: true,
                    }),
                    TurnBlock::System(AttributedText {
                        text: "Dispatcher switch confirmed".into(),
                        origin: Some(BlockOrigin {
                            kind: OriginKind::Dispatcher,
                            label: "Dispatcher".into(),
                        }),
                        notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
                    }),
                ],
            }],
            active_turn: None,
            context_tokens: None,
        };

        let mut store = AuditTimelineStore::default();
        assert_eq!(store.append_transcript_snapshot("remote:main", &transcript), 2);
        assert_eq!(store.append_transcript_snapshot("remote:main", &transcript), 0);
        assert_eq!(store.entries[0].block_id, "remote:main:turn-4-block-0");
        assert_eq!(store.entries[1].label, "Dispatcher");
    }

    #[test]
    fn store_round_trips_json() {
        let mut store = AuditTimelineStore::default();
        store.entries.push(AuditEntry {
            session_key: "mock:default".into(),
            turn_number: 1,
            block_index: 0,
            block_id: "mock:default:turn-1-block-0".into(),
            kind: AuditEntryKind::Text,
            label: "Message".into(),
            content: "hello".into(),
        });
        store.rebuild_seen_ids();

        let json = store.to_json_pretty().unwrap();
        let decoded = AuditTimelineStore::from_json(&json).unwrap();
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].content, "hello");
    }
}
