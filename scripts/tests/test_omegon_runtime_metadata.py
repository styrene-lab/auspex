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
source_branch = "release/0.29"
source_revision = "547b46097d8a04a0bd49da0f96e0c3a002d5b834"
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

    def test_loads_pinned_release_metadata(self) -> None:
        metadata = load_runtime_metadata(self.write_manifest(VALID_MANIFEST))

        self.assertEqual(metadata["source_branch"], "release/0.29")
        self.assertEqual(metadata["features"], [])
        self.assertEqual(metadata["packaged_binary"], "runtime/omegon-headless")

    def test_rejects_non_release_branch(self) -> None:
        path = self.write_manifest(VALID_MANIFEST.replace("release/0.29", "main"))

        with self.assertRaisesRegex(ValueError, "release branch"):
            load_runtime_metadata(path)

    def test_rejects_abbreviated_revision(self) -> None:
        path = self.write_manifest(
            VALID_MANIFEST.replace(
                "547b46097d8a04a0bd49da0f96e0c3a002d5b834", "547b4609"
            )
        )

        with self.assertRaisesRegex(ValueError, "40-character Git SHA"):
            load_runtime_metadata(path)

    def test_emits_github_output(self) -> None:
        metadata = load_runtime_metadata(self.write_manifest(VALID_MANIFEST))
        output = github_output(metadata)

        self.assertIn("omegon_revision=547b46097d8a04a0bd49da0f96e0c3a002d5b834", output)
        self.assertIn("omegon_features=\n", output)


if __name__ == "__main__":
    unittest.main()
