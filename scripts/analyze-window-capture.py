#!/usr/bin/env python3
"""Score a native Auspex PNG capture for composition regressions.

The analyzer deliberately uses only the standard library so it can run in CI and
inside the native visual-iteration loop without an image-processing dependency.
"""

from __future__ import annotations

import argparse
import json
import struct
import zlib
from pathlib import Path

Pixel = tuple[int, int, int]


def png_pixels(path: Path) -> tuple[int, int, list[Pixel]]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("capture must be a PNG")
    offset, compressed = 8, bytearray()
    width = height = color_type = bit_depth = 0
    while offset < len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        kind = data[offset + 4 : offset + 8]
        body = data[offset + 8 : offset + 8 + length]
        offset += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", body[:10])
        elif kind == b"IDAT":
            compressed.extend(body)
        elif kind == b"IEND":
            break
    channels = {2: 3, 6: 4}.get(color_type)
    if bit_depth != 8 or channels is None:
        raise ValueError(f"unsupported PNG bit depth/color type: {bit_depth}/{color_type}")
    raw, stride = zlib.decompress(compressed), width * channels
    rows: list[bytearray] = []
    cursor = 0
    for _ in range(height):
        filter_type = raw[cursor]
        source = raw[cursor + 1 : cursor + 1 + stride]
        cursor += stride + 1
        previous = rows[-1] if rows else bytearray(stride)
        row = bytearray(stride)
        for index, value in enumerate(source):
            left = row[index - channels] if index >= channels else 0
            above = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 0:
                predictor = 0
            elif filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = above
            elif filter_type == 3:
                predictor = (left + above) // 2
            elif filter_type == 4:
                estimate = left + above - upper_left
                distances = abs(estimate - left), abs(estimate - above), abs(estimate - upper_left)
                predictor = (left, above, upper_left)[distances.index(min(distances))]
            else:
                raise ValueError(f"unsupported PNG filter: {filter_type}")
            row[index] = (value + predictor) & 255
        rows.append(row)
    pixels = [tuple(row[index : index + 3]) for row in rows for index in range(0, stride, channels)]
    return width, height, pixels


def luminance(pixel: Pixel) -> float:
    red, green, blue = pixel
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def _longest_false_run(values: list[bool]) -> int:
    longest = current = 0
    for value in values:
        current = 0 if value else current + 1
        longest = max(longest, current)
    return longest


def analyze_pixels(width: int, height: int, pixels: list[Pixel]) -> dict[str, object]:
    if width <= 0 or height <= 0 or len(pixels) != width * height:
        raise ValueError("pixel dimensions do not match capture")

    # Ignore only the native titlebar. Analyze the application viewport itself.
    top = min(60, height // 20)
    viewport_height = height - top
    content = pixels[width * top :]
    active_mask = [luminance(pixel) >= 28 for pixel in content]
    dark_void = sum(luminance(pixel) < 18 for pixel in content) / len(content)
    active_ratio = sum(active_mask) / len(active_mask)
    cyan_ratio = sum(
        blue > red * 1.25 and green > red * 1.25 and blue > 55
        for red, green, blue in content
    ) / len(content)

    row_step = max(1, viewport_height // 120)
    row_activity: list[float] = []
    for y in range(top, height, row_step):
        row = pixels[y * width : (y + 1) * width]
        row_activity.append(sum(luminance(pixel) >= 28 for pixel in row) / width)
    occupied = [value > 0.035 for value in row_activity]
    occupied_indices = [index for index, value in enumerate(occupied) if value]
    occupied_ratio = sum(occupied) / len(occupied)
    vertical_span = (
        (occupied_indices[-1] - occupied_indices[0] + 1) / len(occupied)
        if occupied_indices else 0.0
    )
    largest_void = _longest_false_run(occupied) / len(occupied)

    # Horizontal balance catches narrow islands and accidental edge anchoring.
    column_step = max(1, width // 120)
    column_activity: list[float] = []
    for x in range(0, width, column_step):
        active = 0
        samples = 0
        for y in range(top, height, max(1, viewport_height // 160)):
            active += luminance(pixels[y * width + min(x, width - 1)]) >= 28
            samples += 1
        column_activity.append(active / samples)
    occupied_columns = [index for index, value in enumerate(column_activity) if value > 0.025]
    horizontal_span = (
        (occupied_columns[-1] - occupied_columns[0] + 1) / len(column_activity)
        if occupied_columns else 0.0
    )
    left_energy = sum(column_activity[: len(column_activity) // 2])
    right_energy = sum(column_activity[(len(column_activity) + 1) // 2 :])
    balance = min(left_energy, right_energy) / max(left_energy, right_energy, 1e-9)

    gates = {
        "content_spans_55_percent": vertical_span >= 0.55,
        "largest_void_below_45_percent": largest_void < 0.45,
        "horizontal_span_above_60_percent": horizontal_span >= 0.60,
        "horizontal_balance_above_55_percent": balance >= 0.55,
        "dark_void_below_90_percent": dark_void < 0.90,
        "accent_below_8_percent": cyan_ratio < 0.08,
    }
    return {
        "dimensions": [width, height],
        "dark_void_ratio": round(dark_void, 4),
        "active_pixel_ratio": round(active_ratio, 4),
        "accent_pixel_ratio": round(cyan_ratio, 4),
        "occupied_row_ratio": round(occupied_ratio, 4),
        "vertical_content_span": round(vertical_span, 4),
        "largest_vertical_void": round(largest_void, 4),
        "horizontal_content_span": round(horizontal_span, 4),
        "horizontal_balance": round(balance, 4),
        "gates": gates,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    width, height, pixels = png_pixels(args.capture)
    report = {"capture": str(args.capture), **analyze_pixels(width, height, pixels)}
    print(json.dumps(report, indent=2) if args.json else "\n".join(f"{key}: {value}" for key, value in report.items()))
    raise SystemExit(0 if all(report["gates"].values()) else 1)


if __name__ == "__main__":
    main()
