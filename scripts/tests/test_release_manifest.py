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

    def test_manifest_is_json_serializable(self) -> None:
        manifest = build_manifest("v0.1.0", "styrene-lab/auspex", "deadbeef", [])
        encoded = json.dumps(manifest)
        self.assertIn('"channel": "stable"', encoded)


if __name__ == "__main__":
    unittest.main()
