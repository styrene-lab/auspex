# release — Delta Spec

## ADDED Requirements

### Requirement: Auspex maintains an operator-facing changelog
Auspex MUST maintain a repository-level `CHANGELOG.md` using Keep a Changelog structure.

The changelog MUST include an `Unreleased` section and versioned release sections keyed by the release version.

#### Scenario: Stable release requires a changelog entry
Given Auspex is preparing release `0.1.0`
When release preflight runs
Then it MUST fail if `CHANGELOG.md` does not contain a `## [0.1.0]` section

### Requirement: Auspex release workflow is tag-driven
Auspex MUST build release artifacts from semver tags of the form `v*`.

RC tags such as `v0.1.0-rc.1` MUST be treated as prereleases.
Stable tags such as `v0.1.0` MUST be treated as normal releases.

#### Scenario: RC tag becomes a prerelease
Given the repository has tag `v0.1.0-rc.1`
When the release workflow runs
Then it MUST publish a prerelease GitHub Release for that tag
And the artifact names MUST include the tagged version

### Requirement: Release preflight enforces repository coherence
Auspex MUST provide a release preflight script that validates the repository state before a stable release is cut.

The preflight MUST check at least:
- current branch is `main`
- working tree is clean
- current Cargo package version is an RC version when promoting to stable
- `CHANGELOG.md` contains the target stable version section
- release workflow and manifest script are present

#### Scenario: Stable release blocked by missing changelog section
Given `Cargo.toml` version is `0.1.0-rc.1`
And `CHANGELOG.md` lacks a `## [0.1.0]` section
When release preflight runs
Then it MUST fail with a clear message

### Requirement: Release artifacts include a machine-readable manifest
Auspex MUST generate a `release-manifest.json` during release assembly.

The manifest MUST describe at least:
- release tag
- version
- commit
- generated assets
- checksums

#### Scenario: Release manifest summarizes built artifacts
Given release archives and checksum data exist for tag `v0.1.0-rc.1`
When the manifest generator runs
Then it MUST emit JSON containing the tag, version, commit, and asset/checksum entries

### Requirement: Auspex documents RC progression toward 0.1.0
Auspex MUST document how RC versions progress to stable releases.

The documentation MUST distinguish prerelease tags from stable tags and describe the expected workflow.

#### Scenario: Operator needs to cut a release candidate
Given an operator wants to publish the first RC toward `0.1.0`
When they read the release-candidate documentation
Then they MUST be able to determine the expected version progression
And they MUST be able to tell which release steps are manual versus automated
