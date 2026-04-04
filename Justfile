# Auspex — first-party host shell for Omegon and Styrene
# Run `just` with no args to see available recipes.

set shell := ["bash", "-cu"]

default:
    @just --list --unsorted

# ─── Development ────────────────────────────────────────────

# Run the desktop app in development mode.
run:
    cargo run

# Type-check quickly.
check:
    cargo check

# Run all tests.
test:
    cargo test

# Format code.
fmt:
    cargo fmt

# Verify formatting without changing files.
fmt-check:
    cargo fmt --check

# Full local validation (format + typecheck + lint + test).
validate:
    cargo fmt --check
    cargo check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Back-compat alias.
lint: validate

# ─── Build & distribution ───────────────────────────────────

# Build a release binary — fast, no bundle.
build:
    cargo build --release

# Bundle a distributable desktop app (.app + .dmg on macOS).
# Output lands in dist/
bundle:
    dx bundle --platform desktop --release

# Validate, bundle, and open in one shot.
dist: validate bundle
    open dist/Auspex.app

# Open the last-built .app without rebuilding.
open:
    open dist/Auspex.app

# Remove build artefacts and bundle output.
clean:
    cargo clean
    rm -rf dist

# ─── Release guardrails ─────────────────────────────────────

# Show current package version.
version:
    @grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/'

# Cut a release candidate tag from main after strict validation.
# Policy: RCs are cut from main with a clean working tree.
rc version="":
    #!/usr/bin/env bash
    set -euo pipefail

    BRANCH=$(git branch --show-current)
    if [ -z "$BRANCH" ]; then
        echo "✗ Detached HEAD. Check out main before cutting an RC."
        exit 1
    fi
    if [ "$BRANCH" != "main" ]; then
        echo "✗ RC cuts must run from main. Current branch: $BRANCH"
        exit 1
    fi

    DIRTY=$(git status --porcelain)
    if [ -n "$DIRTY" ]; then
        echo "✗ Working tree is dirty. Commit or stash first."
        echo "$DIRTY"
        exit 1
    fi

    CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    INPUT_VERSION="{{version}}"

    if [ -n "$INPUT_VERSION" ]; then
        NEW_VERSION="$INPUT_VERSION"
    elif echo "$CURRENT" | grep -q '\-rc\.'; then
        BASE=$(echo "$CURRENT" | sed 's/-rc\.[0-9]*//')
        RC_NUM=$(echo "$CURRENT" | sed 's/.*-rc\.//')
        NEW_RC=$((RC_NUM + 1))
        NEW_VERSION="${BASE}-rc.${NEW_RC}"
    else
        IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"
        NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))-rc.1"
    fi

    if ! echo "$NEW_VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$'; then
        echo "✗ RC version must match X.Y.Z-rc.N. Got: $NEW_VERSION"
        exit 1
    fi

    if git rev-parse -q --verify "refs/tags/v${NEW_VERSION}" >/dev/null; then
        echo "✗ Tag v${NEW_VERSION} already exists."
        exit 1
    fi

    echo "New version: $NEW_VERSION"
    sed -i '' "s/^version = \"${CURRENT}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

    cargo fmt --check
    cargo check
    cargo clippy --all-targets -- -D warnings
    cargo test

    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): ${NEW_VERSION}"
    git tag "v${NEW_VERSION}"

    echo "✓ Cut RC v${NEW_VERSION}"
    echo "  build:  cargo build --release"
    echo "  push:   git push origin main --tags"

# Promote current RC to a stable release.
release:
    #!/usr/bin/env bash
    set -euo pipefail

    BRANCH=$(git branch --show-current)
    if [ -z "$BRANCH" ]; then
        echo "✗ Detached HEAD. Check out main before cutting a release."
        exit 1
    fi
    if [ "$BRANCH" != "main" ]; then
        echo "✗ Releases must run from main. Current branch: $BRANCH"
        exit 1
    fi

    DIRTY=$(git status --porcelain)
    if [ -n "$DIRTY" ]; then
        echo "✗ Working tree is dirty. Commit or stash first."
        echo "$DIRTY"
        exit 1
    fi

    CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    if ! echo "$CURRENT" | grep -q '\-rc\.'; then
        echo "✗ Stable release expects an RC version. Current: $CURRENT"
        exit 1
    fi

    python3 scripts/release_preflight.py

    NEW_VERSION=$(echo "$CURRENT" | sed 's/-rc\.[0-9]*//')
    if git rev-parse -q --verify "refs/tags/v${NEW_VERSION}" >/dev/null; then
        echo "✗ Tag v${NEW_VERSION} already exists."
        exit 1
    fi

    echo "Stable version: $NEW_VERSION"
    sed -i '' "s/^version = \"${CURRENT}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

    cargo fmt --check
    cargo check
    cargo clippy --all-targets -- -D warnings
    cargo test

    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): ${NEW_VERSION}"
    git tag "v${NEW_VERSION}"

    echo "✓ Cut release v${NEW_VERSION}"
    echo "  build:  cargo build --release"
    echo "  push:   git push origin main --tags"

# Open the next development cycle after a stable release.
next:
    #!/usr/bin/env bash
    set -euo pipefail

    DIRTY=$(git status --porcelain)
    if [ -n "$DIRTY" ]; then
        echo "✗ Working tree is dirty. Commit or stash first."
        echo "$DIRTY"
        exit 1
    fi

    CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    if echo "$CURRENT" | grep -q '\-rc\.'; then
        echo "✗ next expects a stable version, got RC: $CURRENT"
        exit 1
    fi

    IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"
    NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))-dev"
    echo "Next development version: $NEW_VERSION"
    sed -i '' "s/^version = \"${CURRENT}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

    cargo fmt

    git add -u
    git commit -m "chore(release): begin ${NEW_VERSION}"

    echo "✓ Advanced to ${NEW_VERSION}"
