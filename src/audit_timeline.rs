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

    pub fn append_transcript_snapshot(
        &mut self,
        session_key: &str,
        transcript: &TranscriptData,
    ) -> usize {
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

    pub fn append_entry(&mut self, entry: AuditEntry) -> bool {
        if self.seen_ids.insert(entry.block_id.clone()) {
            self.entries.push(entry);
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn query(&self, query: &AuditTimelineQuery) -> AuditTimelineView<'_> {
        let sessions = self
            .entries
            .iter()
            .map(|entry| entry.session_key.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_string)
            .collect();

        let turns = self
            .entries
            .iter()
            .filter(|entry| query.matches_session(entry))
            .map(|entry| entry.turn_number)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let kinds = self
            .entries
            .iter()
            .filter(|entry| query.matches_session(entry) && query.matches_turn(entry))
            .map(|entry| entry.kind.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let entries = self
            .entries
            .iter()
            .filter(|entry| query.matches(entry))
            .collect();

        AuditTimelineView {
            sessions,
            turns,
            kinds,
            entries,
        }
    }

    fn rebuild_seen_ids(&mut self) {
        self.seen_ids = self
            .entries
            .iter()
            .map(|entry| entry.block_id.clone())
            .collect();
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditTimelineQuery {
    pub session_key: Option<String>,
    pub turn_number: Option<u32>,
    pub kind: Option<AuditEntryKind>,
    pub text: String,
}

#[allow(dead_code)]
impl AuditTimelineQuery {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    fn matches(&self, entry: &AuditEntry) -> bool {
        self.matches_session(entry)
            && self.matches_turn(entry)
            && self.matches_kind(entry)
            && self.matches_text(entry)
    }

    fn matches_session(&self, entry: &AuditEntry) -> bool {
        self.session_key
            .as_deref()
            .is_none_or(|session_key| entry.session_key == session_key)
    }

    fn matches_turn(&self, entry: &AuditEntry) -> bool {
        self.turn_number
            .is_none_or(|turn_number| entry.turn_number == turn_number)
    }

    fn matches_kind(&self, entry: &AuditEntry) -> bool {
        self.kind.as_ref().is_none_or(|kind| entry.kind == *kind)
    }

    fn matches_text(&self, entry: &AuditEntry) -> bool {
        let needle = self.text.trim();
        if needle.is_empty() {
            return true;
        }

        let needle = needle.to_ascii_lowercase();
        let haystack = format!("{} {}", entry.label, entry.content).to_ascii_lowercase();
        haystack.contains(&needle)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditTimelineView<'a> {
    pub sessions: Vec<String>,
    pub turns: Vec<u32>,
    pub kinds: Vec<AuditEntryKind>,
    pub entries: Vec<&'a AuditEntry>,
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
    fn from_block(
        session_key: &str,
        turn_number: u32,
        block_index: usize,
        block: &TurnBlock,
    ) -> Self {
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
            TurnBlock::Aborted(text) => {
                (AuditEntryKind::Aborted, "Aborted".to_string(), text.clone())
            }
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

    pub fn telemetry(
        session_key: &str,
        telemetry_key: &str,
        label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            session_key: session_key.to_string(),
            turn_number: 0,
            block_index: 0,
            block_id: format!("{session_key}:telemetry:{telemetry_key}"),
            kind: AuditEntryKind::Telemetry,
            label: label.into(),
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditEntryKind {
    Thinking,
    Text,
    Tool,
    System,
    Telemetry,
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
    use crate::fixtures::{
        AttributedText, BlockOrigin, OriginKind, SystemNoticeKind, TranscriptData, Turn, TurnBlock,
        TurnBlockText,
    };

    #[test]
    fn append_transcript_snapshot_dedupes_by_stable_block_id() {
        let transcript = TranscriptData {
            turns: vec![Turn {
                number: 4,
                user_prompt: None,
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
        assert_eq!(
            store.append_transcript_snapshot("remote:main", &transcript),
            2
        );
        assert_eq!(
            store.append_transcript_snapshot("remote:main", &transcript),
            0
        );
        assert_eq!(store.entries[0].block_id, "remote:main:turn-4-block-0");
        assert_eq!(store.entries[1].label, "Dispatcher");
    }

    #[test]
    fn query_filters_entries_by_session_turn_kind_and_text() {
        let store = AuditTimelineStore {
            schema_version: AUDIT_SCHEMA_VERSION,
            entries: vec![
                AuditEntry {
                    session_key: "mock:ready".into(),
                    turn_number: 1,
                    block_index: 0,
                    block_id: "mock:ready:turn-1-block-0".into(),
                    kind: AuditEntryKind::Thinking,
                    label: "Thinking".into(),
                    content: "inspect state".into(),
                },
                AuditEntry {
                    session_key: "mock:ready".into(),
                    turn_number: 2,
                    block_index: 0,
                    block_id: "mock:ready:turn-2-block-0".into(),
                    kind: AuditEntryKind::Tool,
                    label: "Tool · bash".into(),
                    content: "result:\nship status".into(),
                },
                AuditEntry {
                    session_key: "remote:session-7".into(),
                    turn_number: 2,
                    block_index: 0,
                    block_id: "remote:session-7:turn-2-block-0".into(),
                    kind: AuditEntryKind::System,
                    label: "Dispatcher".into(),
                    content: "Switched model".into(),
                },
            ],
            seen_ids: BTreeSet::from([
                "mock:ready:turn-1-block-0".into(),
                "mock:ready:turn-2-block-0".into(),
                "remote:session-7:turn-2-block-0".into(),
            ]),
        };

        let filtered = store.query(&AuditTimelineQuery {
            session_key: Some("mock:ready".into()),
            turn_number: Some(2),
            kind: Some(AuditEntryKind::Tool),
            text: "SHIP".into(),
        });

        assert_eq!(
            filtered.sessions,
            vec!["mock:ready".to_string(), "remote:session-7".to_string()]
        );
        assert_eq!(filtered.turns, vec![1, 2]);
        assert_eq!(filtered.kinds, vec![AuditEntryKind::Tool]);
        assert_eq!(filtered.entries.len(), 1);
        assert_eq!(filtered.entries[0].block_id, "mock:ready:turn-2-block-0");
    }

    #[test]
    fn query_text_search_matches_label_and_content_case_insensitively() {
        let mut store = AuditTimelineStore::default();
        store.entries.push(AuditEntry {
            session_key: "mock:default".into(),
            turn_number: 1,
            block_index: 0,
            block_id: "mock:default:turn-1-block-0".into(),
            kind: AuditEntryKind::System,
            label: "Dispatcher".into(),
            content: "Model switched to supervisor-heavy".into(),
        });
        store.rebuild_seen_ids();

        let by_label = store.query(&AuditTimelineQuery::with_text("dispatch"));
        assert_eq!(by_label.entries.len(), 1);

        let by_content = store.query(&AuditTimelineQuery::with_text("SUPERVISOR"));
        assert_eq!(by_content.entries.len(), 1);
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
