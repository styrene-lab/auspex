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
    generate.add_argument("--omegon-runtime", type=Path)

    args = parser.parse_args(argv)

    if args.command == "generate":
        assets = parse_checksums(args.checksums)
        omegon_runtime = None
        if args.omegon_runtime is not None:
            omegon_runtime = json.loads(args.omegon_runtime.read_text())
            required = {
                "repository",
                "branch",
                "revision",
                "version",
                "binary",
                "sha256",
                "cargo_features",
                "default_features",
                "control_plane_schema",
            }
            if not isinstance(omegon_runtime, dict) or set(omegon_runtime) != required:
                parser.error("Omegon runtime provenance has an invalid schema")
            if omegon_runtime["default_features"] is not False:
                parser.error("packaged Omegon runtime must disable default features")
        manifest = build_manifest(args.tag, args.repo, args.commit, assets, omegon_runtime)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(manifest, indent=2) + "\n")
        return 0

    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
