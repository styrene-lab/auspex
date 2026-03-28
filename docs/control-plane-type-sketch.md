# ControlPlaneStateV1 Type Sketch

## Purpose

Provide an implementation-facing type sketch for the Omegon-side contract Auspex needs.

This is the bridge from high-level API requirements to concrete Rust-side structure.

## Design constraints

- Must be stable enough to become a public client contract.
- Must be shaped around existing Omegon runtime state where possible.
- Must carry both Omegon version identity and control-plane schema identity.
- Must define the current Omegon-side HTTP/WS JSON representation without implying that every downstream transport also uses JSON.

## Top-level shape

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ControlPlaneStateV1 {
    pub schema_version: u32,
    pub omegon_version: String,
    pub session: SessionSection,
    pub design_tree: DesignTreeSection,
    pub openspec: OpenSpecSection,
    pub cleave: CleaveSection,
    pub harness: HarnessStatus,
    pub health: HealthSection,
}
```

### Current Omegon JSON form

```json
{
  "schemaVersion": 1,
  "omegonVersion": "0.16.0",
  "session": {},
  "designTree": {},
  "openspec": {},
  "cleave": {},
  "harness": {},
  "health": {}
}
```

## Session section

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SessionSection {
    pub cwd: String,
    pub pid: u32,
    pub started_at: String,
    pub server: ServerBinding,
    pub stats: SessionStats,
    pub git: GitStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerBinding {
    pub addr: String,
    pub http_base: String,
    pub ws_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStats {
    pub turns: u32,
    pub tool_calls: u32,
    pub compactions: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitStatus {
    pub branch: Option<String>,
    pub detached: bool,
}
```

## DesignTree section

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DesignTreeSection {
    pub counts: DesignCounts,
    pub focused: Option<FocusedNode>,
    pub implementing: Vec<NodeBrief>,
    pub actionable: Vec<NodeBrief>,
    pub nodes: Vec<NodeBrief>,
}
```

This should be derived from the existing Omegon `web/api.rs` snapshot types, with public naming cleaned up.

## OpenSpec section

```rust
#[derive(Debug, Clone, Serialize)]
pub struct OpenSpecSection {
    pub changes: Vec<ChangeSnapshot>,
    pub total_tasks: usize,
    pub done_tasks: usize,
}
```

This can mostly reuse the current shape.

## Cleave section

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CleaveSection {
    pub active: bool,
    pub total_children: usize,
    pub completed: usize,
    pub failed: usize,
    pub children: Vec<ChildSnapshot>,
}
```

This can mostly reuse the current shape.

## Harness section

Use Omegon's existing `HarnessStatus` directly where possible.

```rust
pub type HarnessSection = HarnessStatus;
```

That is preferable to prematurely splitting `models`, `memory`, and other already-coherent status groups into separate top-level sections.

## Health section

```rust
#[derive(Debug, Clone, Serialize)]
pub struct HealthSection {
    pub status: String,
    pub last_updated_at: String,
    pub api_available: bool,
    pub websocket_available: bool,
}
```

This should stay minimal in v1.

## Startup/discovery payload sketch

Auspex also needs machine-readable startup metadata.

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ControlPlaneStartupInfo {
    pub pid: u32,
    pub cwd: String,
    pub addr: String,
    pub token: String,
    pub omegon_version: String,
    pub schema_version: u32,
}
```

## Suggested serde naming policy

Use Rust snake_case internally, with JSON camelCase externally for the Omegon HTTP/WS boundary.

That implies:
- `schema_version` -> `schemaVersion`
- `omegon_version` -> `omegonVersion`
- `design_tree` -> `designTree`
- `started_at` -> `startedAt`
- `tool_calls` -> `toolCalls`
- `last_updated_at` -> `lastUpdatedAt`

## Suggested Omegon implementation approach

### Step 1
Add a new public snapshot type alongside the existing internal snapshot.

### Step 2
Migrate `/api/state` to emit `ControlPlaneStateV1`.

### Step 3
Keep `GraphData` as a separate route, but document it as part of the public control-plane contract.

### Step 4
Expose `omegonVersion` and `schemaVersion` in both:
- startup/discovery output
- `/api/state`

## File targets in Omegon

Primary likely targets:
- `omegon/core/crates/omegon/src/web/api.rs`
- `omegon/core/crates/omegon/src/web/mod.rs`
- `omegon/core/crates/omegon/src/status.rs`
- `omegon/core/crates/omegon/src/main.rs`

## Boundary note

`ControlPlaneStateV1` is the Omegon-side public contract and current JSON representation for the local control-plane.

That does **not** mean every later transport must use JSON on the wire. In particular, the Styrene phone relay should define a semantic message schema that can be encoded using transport-native formats such as MessagePack over LXMF.

## Guiding rule

Do not make Auspex normalize this shape client-side first. The public contract belongs in Omegon.
