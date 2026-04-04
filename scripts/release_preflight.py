#!/usr/bin/env python3
"""Release preflight checks for Auspex."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

VERSION_RE = re.compile(r'^version = "([^"]+)"', re.MULTILINE)
CHANGELOG_SECTION_RE = r"^## \[{version}\](?=\s|$)"


class PreflightError(Exception):
    pass


def read_workspace_version(repo_root: Path) -> str:
    cargo_toml = (repo_root / "Cargo.toml").read_text()
    match = VERSION_RE.search(cargo_toml)
    if not match:
        raise PreflightError("Could not read package version from Cargo.toml")
    return match.group(1)


def stable_version_from_rc(version: str) -> str:
    if "-rc." not in version:
        raise PreflightError(f"Package version {version} is not an RC version")
    return version.split("-rc.", 1)[0]


def changelog_has_version(repo_root: Path, version: str) -> bool:
    changelog = (repo_root / "CHANGELOG.md").read_text()
    return (
        re.search(
            CHANGELOG_SECTION_RE.format(version=re.escape(version)),
            changelog,
            flags=re.MULTILINE,
        )
        is not None
    )


def release_files_present(repo_root: Path) -> bool:
    required = [
        repo_root / ".github" / "workflows" / "release.yml",
        repo_root / "scripts" / "release_manifest.py",
        repo_root / "scripts" / "release_preflight.py",
        repo_root / "docs" / "release-candidate-system.md",
    ]
    return all(path.exists() for path in required)


def git_stdout(repo_root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def collect_failures(repo_root: Path) -> list[str]:
    failures: list[str] = []

    branch = git_stdout(repo_root, "branch", "--show-current")
    if branch != "main":
        failures.append(f"must be on main (currently: {branch or 'detached'})")

    dirty = git_stdout(repo_root, "status", "--porcelain")
    if dirty:
        failures.append("working tree is not clean")

    try:
        current_version = read_workspace_version(repo_root)
        stable_version = stable_version_from_rc(current_version)
    except PreflightError as err:
        failures.append(str(err))
        return failures

    if not changelog_has_version(repo_root, stable_version):
        failures.append(f"CHANGELOG.md is missing section [{stable_version}]")

    if not release_files_present(repo_root):
        failures.append("release workflow/docs/scripts are not fully present")

    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)

    repo_root = args.repo_root.resolve()
    failures = collect_failures(repo_root)
    if failures:
        print("✗ Release preflight failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    stable = stable_version_from_rc(read_workspace_version(repo_root))
    print(f"✓ Release preflight passed — repo is releasable as {stable}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
