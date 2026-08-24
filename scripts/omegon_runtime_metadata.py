#!/usr/bin/env python3
"""Read and validate the pinned Omegon runtime metadata from Cargo.toml."""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from pathlib import Path

REQUIRED_FIELDS = (
    "minimum_version",
    "maximum_tested_version",
    "control_plane_schema",
    "source_repository",
    "source_branch",
    "source_revision",
    "package",
    "features",
    "packaged_binary",
)


def load_runtime_metadata(path: Path) -> dict[str, object]:
    document = tomllib.loads(path.read_text())
    try:
        metadata = document["package"]["metadata"]["omegon"]
    except (KeyError, TypeError) as error:
        raise ValueError("Cargo.toml is missing [package.metadata.omegon]") from error
    if not isinstance(metadata, dict):
        raise ValueError("[package.metadata.omegon] must be a table")

    missing = [field for field in REQUIRED_FIELDS if field not in metadata]
    if missing:
        raise ValueError(f"Omegon metadata is missing: {', '.join(missing)}")

    revision = metadata["source_revision"]
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("source_revision must be a full lowercase 40-character Git SHA")
    branch = metadata["source_branch"]
    if not isinstance(branch, str) or not branch.startswith(("release/", "design/")):
        raise ValueError("source_branch must name an approved release or design branch")
    features = metadata["features"]
    if not isinstance(features, list) or not all(isinstance(item, str) for item in features):
        raise ValueError("features must be a list of Cargo feature names")
    packaged_binary = metadata["packaged_binary"]
    if not isinstance(packaged_binary, str) or not packaged_binary.startswith("runtime/"):
        raise ValueError("packaged_binary must be under runtime/")

    return dict(metadata)


def github_output(metadata: dict[str, object]) -> str:
    fields = {
        "omegon_repository": metadata["source_repository"],
        "omegon_branch": metadata["source_branch"],
        "omegon_revision": metadata["source_revision"],
        "omegon_version": metadata["maximum_tested_version"],
        "omegon_package": metadata["package"],
        "omegon_features": ",".join(metadata["features"]),
        "omegon_binary": metadata["packaged_binary"],
    }
    return "\n".join(f"{key}={value}" for key, value in fields.items()) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--format", choices=("json", "github-output"), default="json")
    args = parser.parse_args(argv)

    metadata = load_runtime_metadata(args.manifest)
    if args.format == "github-output":
        print(github_output(metadata), end="")
    else:
        print(json.dumps(metadata, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
