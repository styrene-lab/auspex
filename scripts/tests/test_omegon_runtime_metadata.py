from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.omegon_runtime_metadata import github_output, load_runtime_metadata


VALID_MANIFEST = """
[package]
name = "auspex"

[package.metadata.omegon]
minimum_version = "0.29.0-dev"
maximum_tested_version = "0.29.0-dev"
control_plane_schema = 2
source_repository = "https://github.com/styrene-lab/omegon.git"
source_branch = "design/kernel-plugin-decomposition"
source_revision = "227f73502c9c7218ef76ffbb2a020980568c7103"
package = "omegon"
features = []
packaged_binary = "runtime/omegon-headless"
"""


class OmegonRuntimeMetadataTests(unittest.TestCase):
    def write_manifest(self, content: str) -> Path:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "Cargo.toml"
        path.write_text(content)
        return path

    def test_loads_pinned_source_metadata(self) -> None:
        metadata = load_runtime_metadata(self.write_manifest(VALID_MANIFEST))

        self.assertEqual(metadata["source_branch"], "design/kernel-plugin-decomposition")
        self.assertEqual(metadata["features"], [])
        self.assertEqual(metadata["packaged_binary"], "runtime/omegon-headless")

    def test_rejects_unapproved_branch(self) -> None:
        path = self.write_manifest(
            VALID_MANIFEST.replace("design/kernel-plugin-decomposition", "main")
        )

        with self.assertRaisesRegex(ValueError, "approved release or design branch"):
            load_runtime_metadata(path)

    def test_rejects_abbreviated_revision(self) -> None:
        path = self.write_manifest(
            VALID_MANIFEST.replace(
                "227f73502c9c7218ef76ffbb2a020980568c7103", "227f7350"
            )
        )

        with self.assertRaisesRegex(ValueError, "40-character Git SHA"):
            load_runtime_metadata(path)

    def test_emits_github_output(self) -> None:
        metadata = load_runtime_metadata(self.write_manifest(VALID_MANIFEST))
        output = github_output(metadata)

        self.assertIn("omegon_revision=227f73502c9c7218ef76ffbb2a020980568c7103", output)
        self.assertIn("omegon_features=\n", output)


if __name__ == "__main__":
    unittest.main()
