# auspex-release-framework — Tasks

Dependencies: 1 → 2 → 3. Documentation can finalize after scripts and workflow land.

## 1. Release metadata and local tooling
<!-- specs: release -->
- [x] Create `CHANGELOG.md` with Keep a Changelog structure and initial `Unreleased` / `0.0.1-rc.6` sections
- [x] Add `scripts/release_manifest.py` to generate `release-manifest.json` from checksums and release metadata
- [x] Add tests for manifest generation behavior
- [x] Add `scripts/release_preflight.py` to validate branch, clean tree, RC-to-stable promotion, changelog coverage, and workflow/script presence
- [x] Add tests for release preflight behavior

## 2. CI release workflow
<!-- specs: release -->
- [x] Add `.github/workflows/release.yml` triggered by `v*` tags and manual dispatch
- [x] Build the Auspex release binary for the supported target set
- [x] Archive release artifacts and generate checksums
- [x] Generate `release-manifest.json` in CI
- [x] Publish prereleases for RC tags and normal releases for stable tags

## 3. Release documentation and policy alignment
<!-- specs: release -->
- [x] Add `docs/release-candidate-system.md` describing version progression toward `0.1.0`
- [x] Update `docs/release-coordination.md` to reflect current Auspex/Omegon compatibility metadata instead of stale schema/version examples
- [x] Document which release steps are local preflight versus CI automation
- [x] Verify the documented commands and file references match the repository layout
