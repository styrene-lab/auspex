---
id: project-manifest-and-registry
title: "Project manifest and Auspex project registry"
status: resolved
tags: [architecture, lifecycle, ideas]
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
  - auspex-instance-registry-schema
---

# Project manifest and Auspex project registry

## Overview

Auspex needs a stable way to discover, identify, and relate the repos it manages. Each repo declares its own identity in `.omegon/project.json`. Auspex maintains a registry of project paths that reference these manifests.

This is the prerequisite for the idea layer, multi-project views, cross-project dependency tracking, and future workspace management.

## Origin

Designed in Omegon's design tree as part of the idea-layer node. Decisions captured here are upstream-authoritative from that node.

## Decisions

### Per-repo .omegon/project.json; Auspex registry is a manifest of project references

**Status:** accepted

**Rationale:** Each repo owns its identity via `.omegon/project.json`. Auspex maintains a registry that references these by path — it does not duplicate or shadow project metadata. Single source of truth per layer.

### Every project is git-backed; remotes are optional

**Status:** accepted

**Rationale:** `git.local: true` is always true — git backing is a hard requirement. The `remotes` map is informational; Auspex reads it for sync options but doesn't enforce any. Empty remotes is first-class, not degraded. A future lightweight git server becomes just another remote entry.

### Auspex discovers projects by scanning, then stores explicitly

**Status:** accepted

**Rationale:** Scan is a QoL import mechanism — finds `.omegon/` dirs under configured workspace roots. After discovery, projects are stored as explicit path entries in the Auspex registry. No re-scanning on every startup.

### New projects: Auspex inits a local git repo and scaffolds .omegon/

**Status:** accepted

**Rationale:** Preserves the invariant that every idea has a project home and every project is git-backed. Creating a project is cheap (`git init` + two files). No orphan ideas concept needed.

## Schemas

### project.json (per-repo, in .omegon/)

```json
{
  "name": "omegon",
  "description": "Systems engineering harness — agent loop, lifecycle, TUI",
  "version": "0.15.9",
  "tags": ["rust", "agent", "tui"],
  "git": {
    "local": true,
    "remotes": {
      "origin": "https://github.com/styrene-lab/omegon.git"
    }
  },
  "created": "2025-01-15T00:00:00Z"
}
```

Minimal required fields: `name`, `git.local`.

### Auspex project registry (~/.config/auspex/projects.json)

```json
{
  "schema_version": 1,
  "projects": [
    {
      "path": "/Users/cwilson/workspace/black-meridian/omegon",
      "added": "2026-04-04T00:00:00Z",
      "source": "scan"
    },
    {
      "path": "/Users/cwilson/workspace/black-meridian/auspex",
      "added": "2026-04-04T00:00:00Z",
      "source": "scan"
    },
    {
      "path": "/Users/cwilson/ideas/garden-tracker",
      "added": "2026-04-04T12:00:00Z",
      "source": "created"
    }
  ],
  "scan_roots": [
    "/Users/cwilson/workspace/black-meridian"
  ]
}
```

`scan_roots` records where Auspex has been told to look. `source` tracks whether a project was found by scan, manually added, or created fresh by Auspex.

### Local-only project (no upstream)

```json
{
  "name": "garden-tracker",
  "description": "Personal plant watering schedule and notes",
  "tags": ["personal", "local"],
  "git": {
    "local": true,
    "remotes": {}
  },
  "created": "2026-04-04T12:00:00Z"
}
```

## Interaction with instance registry

The existing [[auspex-instance-registry-schema]] tracks Omegon worker instances. The project registry is orthogonal — it tracks *repos*, not *processes*. A worker instance binds to a project via `desired.workspace.cwd`, which Auspex can resolve to a project registry entry.

```
Project registry       Instance registry
  (repos)                (workers)
    |                       |
    +-- project.path <---- desired.workspace.cwd
```

## Implementation notes

- Auspex reads `.omegon/project.json` on project list refresh, not cached indefinitely
- If `project.json` is missing at a registered path, project shows as "uninitialized" — Auspex offers scaffold
- If the path doesn't exist at all, project shows as "missing" — Auspex offers removal
- `scan_roots` is a convenience for the import UX; Auspex does NOT auto-scan on startup
