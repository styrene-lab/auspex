# Auspex release candidate framework

## Intent

Establish a minimal RC/stable release framework for Auspex modeled on Omegon's release invariants: changelog discipline, release preflight, manifest generation, and tag-driven GitHub release workflow for Rust desktop binaries.

## Scope

This change sets up the first real release discipline for Auspex:

- `CHANGELOG.md` with Keep a Changelog structure and an `Unreleased` section
- a tag-driven GitHub Actions release workflow for the Auspex desktop binary
- a release preflight script that enforces repo cleanliness and release metadata coherence
- a release manifest generator so downstream automation can consume structured release metadata
- release-candidate documentation describing RC progression toward `0.1.0`

This change does **not** attempt to clone every Omegon release concern. It deliberately skips:

- Homebrew automation
- Apple signing / notarization
- SBOM / provenance / cosign integration
- multi-package publishing

Those can come later once Auspex actually has a stable release cadence.

## Constraints

- Keep the framework small enough to ship in one pass.
- Reuse Omegon's release invariants, not its repo-specific complexity.
- The result must work for the current single-binary Rust app layout.
- RC and stable releases must remain explicit via semver tags.
