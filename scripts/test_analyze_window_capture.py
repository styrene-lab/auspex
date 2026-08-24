#!/usr/bin/env python3
"""Regression tests for the native capture composition analyzer."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("analyze-window-capture.py")
SPEC = importlib.util.spec_from_file_location("capture_analyzer", MODULE_PATH)
assert SPEC and SPEC.loader
ANALYZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYZER)


class AnalyzePixelsTests(unittest.TestCase):
    def test_rejects_content_stranded_at_top(self) -> None:
        width, height = 100, 100
        pixels = [(5, 8, 12)] * (width * height)
        for y in range(5, 20):
            for x in range(15, 85):
                pixels[y * width + x] = (95, 140, 160)

        report = ANALYZER.analyze_pixels(width, height, pixels)

        self.assertFalse(report["gates"]["content_spans_55_percent"])
        self.assertFalse(report["gates"]["largest_void_below_45_percent"])

    def test_accepts_balanced_full_height_workspace(self) -> None:
        width, height = 100, 100
        pixels = [(5, 8, 12)] * (width * height)
        for y in range(8, 94, 9):
            for row in range(y, min(y + 3, height)):
                for x in range(12, 88):
                    pixels[row * width + x] = (75, 82, 88)

        report = ANALYZER.analyze_pixels(width, height, pixels)

        self.assertTrue(all(report["gates"].values()), report)
        self.assertGreaterEqual(report["vertical_content_span"], 0.55)


if __name__ == "__main__":
    unittest.main()
