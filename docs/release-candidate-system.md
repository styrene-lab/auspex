---
title: Auspex release candidate system
status: implemented
tags: [release, rc, ci, versioning]
---

# Auspex release candidate system

## Goal

Give Auspex a small but real release cadence before `0.1.0`.

This framework intentionally mirrors Omegon's **release invariants** without copying its full release machinery. Auspex starts with:

- explicit semver prerelease versions
- a repository changelog
- scripted release preflight
- machine-readable release manifests
- tag-driven GitHub Releases

It intentionally defers:

- signing
- notarization
- Homebrew automation
- SBOM / provenance work

## Version progression

The RC line toward the first stable release should look like this:

```text
0.1.0-rc.1   ← first public RC toward stable
0.1.0-rc.2   ← follow-up RC if needed
0.1.0        ← first stable release
```

## Release policy

- RC tags use semver prerelease syntax: `v0.1.0-rc.1`
- stable tags use plain semver syntax: `v0.1.0`
- RC tags publish GitHub prereleases
- stable tags publish normal GitHub releases
- initial release surface is **macOS arm64 raw tarball archives only**
- initial RCs are **unsigned** by design

That scope is deliberately narrow. Pretending to support broader distribution before the release loop works would be process theater.

## Local workflow

### Cut an RC

1. Update `Cargo.toml` version to the intended RC version, for example `0.1.0-rc.1`
2. Add or update the relevant changelog content under `Unreleased`
3. Commit the release prep
4. Tag the commit:

```bash
git tag v0.1.0-rc.1
```

5. Push the tag:

```bash
git push origin v0.1.0-rc.1
```

CI builds the release archive, checksum, and `release-manifest.json`, then creates a GitHub prerelease.

### Promote RC to stable

1. Ensure the repo is on `main` and clean
2. Update `Cargo.toml` from `0.1.0-rc.N` to `0.1.0`
3. Add a `## [0.1.0]` section to `CHANGELOG.md`
4. Run:

```bash
python3 scripts/release_preflight.py
```

5. Commit the promotion
6. Tag the stable release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

CI then publishes a normal GitHub release.

## Automated vs manual steps

### Manual
- choose the version
- update `Cargo.toml`
- update `CHANGELOG.md`
- run release preflight before stable promotion
- create and push the tag

### Automated
- build the release binary in CI
- archive the artifact
- generate checksums
- generate `release-manifest.json`
- publish the GitHub Release

## Release artifacts

Each release currently emits:

- `auspex-<version>-aarch64-apple-darwin.tar.gz`
- `auspex-<version>-aarch64-apple-darwin.tar.gz.sha256`
- `release-manifest.json`

The manifest is the structured source of truth for downstream automation.
