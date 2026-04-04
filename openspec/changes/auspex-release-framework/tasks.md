# auspex-release-framework — Tasks

Dependencies: 1 → 2 → 3. Documentation can finalize after scripts and workflow land.

## 1. Release metadata and local tooling
<!-- specs: release -->
- [ ] Create `CHANGELOG.md` with Keep a Changelog structure and initial `Unreleased` / `0.0.1-rc.6` sections
- [ ] Add `scripts/release_manifest.py` to generate `release-manifest.json` from checksums and release metadata
- [ ] Add tests for manifest generation behavior
- [ ] Add `scripts/release_preflight.py` to validate branch, clean tree, RC-to-stable promotion, changelog coverage, and workflow/script presence
- [ ] Add tests for release preflight behavior

## 2. CI release workflow
<!-- specs: release -->
- [ ] Add `.github/workflows/release.yml` triggered by `v*` tags and manual dispatch
- [ ] Build the Auspex release binary for the supported target set
- [ ] Archive release artifacts and generate checksums
- [ ] Generate `release-manifest.json` in CI
- [ ] Publish prereleases for RC tags and normal releases for stable tags

## 3. Release documentation and policy alignment
<!-- specs: release -->
- [ ] Add `docs/release-candidate-system.md` describing version progression toward `0.1.0`
- [ ] Update `docs/release-coordination.md` to reflect current Auspex/Omegon compatibility metadata instead of stale schema/version examples
- [ ] Document which release steps are local preflight versus CI automation
- [ ] Verify the documented commands and file references match the repository layout
