from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import release_preflight


class ReleasePreflightTests(unittest.TestCase):
    def test_stable_version_from_rc_strips_suffix(self) -> None:
        self.assertEqual(release_preflight.stable_version_from_rc("0.1.0-rc.1"), "0.1.0")

    def test_stable_version_from_rc_rejects_non_rc(self) -> None:
        with self.assertRaises(release_preflight.PreflightError):
            release_preflight.stable_version_from_rc("0.1.0")

    def test_collect_failures_detects_missing_changelog_section(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0-rc.1"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## [Unreleased]\n')
            (root / ".github" / "workflows").mkdir(parents=True)
            (root / ".github" / "workflows" / "release.yml").write_text("name: release\n")
            (root / "scripts").mkdir()
            (root / "scripts" / "release_manifest.py").write_text("# test\n")
            (root / "scripts" / "release_preflight.py").write_text("# test\n")
            (root / "docs").mkdir()
            (root / "docs" / "release-candidate-system.md").write_text("# doc\n")

            with patch.object(release_preflight, "git_stdout", side_effect=["main", ""]):
                failures = release_preflight.collect_failures(root)

        self.assertIn("CHANGELOG.md is missing section [0.1.0]", failures)

    def test_collect_failures_reports_dirty_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0-rc.1"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## [0.1.0]\n')
            (root / ".github" / "workflows").mkdir(parents=True)
            (root / ".github" / "workflows" / "release.yml").write_text("name: release\n")
            (root / "scripts").mkdir()
            (root / "scripts" / "release_manifest.py").write_text("# test\n")
            (root / "scripts" / "release_preflight.py").write_text("# test\n")
            (root / "docs").mkdir()
            (root / "docs" / "release-candidate-system.md").write_text("# doc\n")

            with patch.object(release_preflight, "git_stdout", side_effect=["main", " M Cargo.toml"]):
                failures = release_preflight.collect_failures(root)

        self.assertIn("working tree is not clean", failures)

    def test_collect_failures_passes_for_coherent_repo(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0-rc.1"\n')
            (root / "CHANGELOG.md").write_text('# Changelog\n\n## [0.1.0]\n')
            (root / ".github" / "workflows").mkdir(parents=True)
            (root / ".github" / "workflows" / "release.yml").write_text("name: release\n")
            (root / "scripts").mkdir()
            (root / "scripts" / "release_manifest.py").write_text("# test\n")
            (root / "scripts" / "release_preflight.py").write_text("# test\n")
            (root / "docs").mkdir()
            (root / "docs" / "release-candidate-system.md").write_text("# doc\n")

            with patch.object(release_preflight, "git_stdout", side_effect=["main", ""]):
                failures = release_preflight.collect_failures(root)

        self.assertEqual(failures, [])


if __name__ == "__main__":
    unittest.main()
