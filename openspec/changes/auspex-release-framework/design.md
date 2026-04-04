# auspex-release-framework — Design

## Goal

Give Auspex a real release skeleton so release candidates and stable versions are deliberate, reproducible, and reviewable.

## Decisions

1. Copy Omegon's release invariants, not its full release complexity.
2. Use tag-driven GitHub Releases from `v*` tags.
3. Treat `-rc.N` versions as prereleases and plain semver versions as stable releases.
4. Keep the first framework limited to the single Auspex binary and checksum/manifest artifacts.
5. Add scripted preflight checks before stable promotion.

## File scope

- `CHANGELOG.md` — Keep a Changelog baseline and release entries
- `.github/workflows/release.yml` — tag-driven artifact build and GitHub release
- `scripts/release_preflight.py` — repo coherence checks for stable promotion
- `scripts/release_manifest.py` — structured manifest generation from checksums
- `docs/release-candidate-system.md` — RC/stable workflow for Auspex
- `docs/release-coordination.md` — update stale compatibility examples and align with current Omegon bounds

## Constraints

- Keep CI implementation minimal and repo-local.
- Do not block initial RC adoption on signing/notarization.
- Emit artifacts that future downstream automation can consume without scraping release text.
- Keep all release checks runnable locally before CI.

## Acceptance criteria

- Auspex has a repository-level changelog.
- A `vX.Y.Z` or `vX.Y.Z-rc.N` tag can trigger a release workflow.
- RC tags publish as prereleases.
- Stable promotion is guarded by scripted preflight.
- Release output includes a `release-manifest.json`.
- Documentation explains how to progress from RCs to `0.1.0`.
