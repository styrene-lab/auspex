from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.release_manifest import build_manifest, parse_checksums


class ReleaseManifestTests(unittest.TestCase):
    def test_parse_checksums_supports_sha256sum_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "checksums.sha256"
            path.write_text(
                "abc123  auspex-0.1.0-rc.1-aarch64-apple-darwin.tar.gz\n"
                "def456 *release-manifest.json\n"
            )

            assets = parse_checksums(path)

        self.assertEqual(
            assets,
            [
                {
                    "name": "auspex-0.1.0-rc.1-aarch64-apple-darwin.tar.gz",
                    "sha256": "abc123",
                },
                {"name": "release-manifest.json", "sha256": "def456"},
            ],
        )

    def test_build_manifest_marks_rc_as_prerelease(self) -> None:
        manifest = build_manifest(
            "v0.1.0-rc.1",
            "styrene-lab/auspex",
            "deadbeef",
            [{"name": "artifact.tar.gz", "sha256": "abc123"}],
        )

        self.assertEqual(manifest["version"], "0.1.0-rc.1")
        self.assertEqual(manifest["channel"], "prerelease")
        self.assertEqual(manifest["repository"], "styrene-lab/auspex")

    def test_manifest_includes_pinned_headless_omegon_runtime(self) -> None:
        runtime = {
            "repository": "https://github.com/styrene-lab/omegon.git",
            "branch": "release/0.29",
            "revision": "6c7e39f66fcc4d3576d1351175edc168fed5d982",
            "version": "0.29.0-dev",
            "binary": "runtime/omegon-headless",
            "sha256": "feedface",
            "cargo_features": [],
            "default_features": False,
            "control_plane_schema": 2,
        }
        manifest = build_manifest(
            "v0.2.0-rc.1", "styrene-lab/auspex", "deadbeef", [], runtime
        )

        self.assertEqual(manifest["omegon_runtime"], runtime)
        self.assertFalse(manifest["omegon_runtime"]["default_features"])

    def test_generate_reads_runtime_provenance_file(self) -> None:
        runtime = {
            "repository": "https://github.com/styrene-lab/omegon.git",
            "branch": "release/0.29",
            "revision": "6c7e39f66fcc4d3576d1351175edc168fed5d982",
            "version": "0.29.0-dev",
            "binary": "runtime/omegon-headless",
            "sha256": "feedface",
            "cargo_features": [],
            "default_features": False,
            "control_plane_schema": 2,
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            checksums = root / "checksums.sha256"
            checksums.write_text("abc123  artifact.tar.gz\n")
            provenance = root / "omegon-runtime.json"
            provenance.write_text(json.dumps(runtime))
            output = root / "release-manifest.json"
            from scripts.release_manifest import main

            result = main(
                [
                    "generate",
                    "--tag", "v0.2.0-rc.1",
                    "--checksums", str(checksums),
                    "--output", str(output),
                    "--repo", "styrene-lab/auspex",
                    "--commit", "deadbeef",
                    "--omegon-runtime", str(provenance),
                ]
            )
            manifest = json.loads(output.read_text())

        self.assertEqual(result, 0)
        self.assertEqual(manifest["omegon_runtime"], runtime)

    def test_manifest_is_json_serializable(self) -> None:
        manifest = build_manifest("v0.1.0", "styrene-lab/auspex", "deadbeef", [])
        encoded = json.dumps(manifest)
        self.assertIn('"channel": "stable"', encoded)


if __name__ == "__main__":
    unittest.main()
