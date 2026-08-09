#!/usr/bin/env python3
"""Generate a machine-readable release manifest for Auspex."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def parse_checksums(path: Path) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line:
            continue
        parts = line.split(maxsplit=1)
        if len(parts) != 2:
            raise ValueError(f"invalid checksum line: {raw_line!r}")
        sha256, file_name = parts
        entries.append(
            {
                "name": file_name.lstrip(" *"),
                "sha256": sha256,
            }
        )
    return entries


def build_manifest(
    tag: str,
    repo: str,
    commit: str,
    assets: list[dict[str, str]],
    omegon_runtime: dict[str, object] | None = None,
) -> dict[str, object]:
    version = tag[1:] if tag.startswith("v") else tag
    manifest: dict[str, object] = {
        "tag": tag,
        "version": version,
        "channel": "prerelease" if "-rc." in version else "stable",
        "commit": commit,
        "repository": repo,
        "assets": assets,
    }
    if omegon_runtime is not None:
        manifest["omegon_runtime"] = omegon_runtime
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    generate = subparsers.add_parser("generate")
    generate.add_argument("--tag", required=True)
    generate.add_argument("--checksums", type=Path, required=True)
    generate.add_argument("--output", type=Path, required=True)
    generate.add_argument("--repo", required=True)
    generate.add_argument("--commit", required=True)
    generate.add_argument("--omegon-repository")
    generate.add_argument("--omegon-branch")
    generate.add_argument("--omegon-revision")
    generate.add_argument("--omegon-version")
    generate.add_argument("--omegon-binary")
    generate.add_argument("--omegon-sha256")

    args = parser.parse_args(argv)

    if args.command == "generate":
        assets = parse_checksums(args.checksums)
        omegon_values = {
            "repository": args.omegon_repository,
            "branch": args.omegon_branch,
            "revision": args.omegon_revision,
            "version": args.omegon_version,
            "binary": args.omegon_binary,
            "sha256": args.omegon_sha256,
            "cargo_features": [],
            "default_features": False,
        }
        supplied = [value is not None for key, value in omegon_values.items() if key not in {"cargo_features", "default_features"}]
        if any(supplied) and not all(supplied):
            parser.error("all Omegon runtime provenance arguments must be supplied together")
        omegon_runtime = omegon_values if all(supplied) else None
        manifest = build_manifest(args.tag, args.repo, args.commit, assets, omegon_runtime)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(manifest, indent=2) + "\n")
        return 0

    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
