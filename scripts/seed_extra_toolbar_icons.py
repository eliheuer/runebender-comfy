#!/usr/bin/env python3
"""Seed additional editable UI icons into assets/runebender-icons.ufo.

The main edit-mode toolbar already uses this UFO. This script extends
that same source of truth to the remaining editor chrome icons. Some
glyphs are copied from Virtua Grotesk's private-use icon glyphs; the
rest are seeded from the existing simple SVG stroke drawings so they
can be redesigned in Runebender later.

Run with an interpreter that has fontTools + ufoLib2, e.g.:
    /Users/eli/Work/comfy/repos/ComfyUI/.venv/bin/python \
        scripts/seed_extra_toolbar_icons.py
"""

from __future__ import annotations

from pathlib import Path

import ufoLib2
from fontTools.pens.transformPen import TransformPen
from fontTools.svgLib.path import parse_path

UFO_TOOLS_KEY = "com.runebender.iconRenderMode"
SCREEN_TO_UFO = (1, 0, 0, -1, 0, 0)


def rect_path(x: float, y: float, w: float, h: float) -> str:
    return f"M{x} {y}H{x + w}V{y + h}H{x}Z"


def ellipse_path(cx: float, cy: float, rx: float, ry: float) -> str:
    k = 0.5522847498307936
    return (
        f"M{cx + rx} {cy}"
        f"C{cx + rx} {cy + ry * k} {cx + rx * k} {cy + ry} {cx} {cy + ry}"
        f"C{cx - rx * k} {cy + ry} {cx - rx} {cy + ry * k} {cx - rx} {cy}"
        f"C{cx - rx} {cy - ry * k} {cx - rx * k} {cy - ry} {cx} {cy - ry}"
        f"C{cx + rx * k} {cy - ry} {cx + rx} {cy - ry * k} {cx + rx} {cy}Z"
    )


STROKE_ICONS: dict[str, str] = {
    "save": " ".join(
        [
            "M5 3H17L19 5V21H5Z",
            "M8 3V9H16V3",
            "M8 21V14H16V21",
            "M10 6H14",
        ]
    ),
    "save-as": " ".join(
        [
            "M4 6H10L12 8H20V18H4Z",
            "M12 12V17",
            "M9.5 14.5L12 17L14.5 14.5",
        ]
    ),
    "close": "M6 6L18 18 M18 6L6 18",
    "flip-h": "M12 4V20 M5 7L10 12L5 17Z M19 7L14 12L19 17Z",
    "flip-v": "M4 12H20 M7 5L12 10L17 5Z M7 19L12 14L17 19Z",
    "rot-cw": "M7 7A7 7 0 1 0 17 7 M17 7V12H12",
    "rot-ccw": "M17 7A7 7 0 1 1 7 7 M7 7V12H12",
    "duplicate": " ".join(
        [
            rect_path(8, 8, 10, 10),
            "M6 16H4V4H16V6",
        ]
    ),
    "duplicate-repeat": " ".join(
        [
            rect_path(9, 9, 9, 9),
            "M6 15H4V4H15V6",
            "M17 4L20 7L17 10",
        ]
    ),
    "union": f"{ellipse_path(10, 12, 5, 5)} {ellipse_path(14, 12, 5, 5)}",
    "subtract": f"{ellipse_path(10, 12, 5, 5)} M14 7A5 5 0 0 1 14 17",
    "intersect": "M12 8A5 5 0 0 1 12 16A5 5 0 0 1 12 8Z",
    "exclude": f"{ellipse_path(9, 12, 4, 4)} {ellipse_path(15, 12, 4, 4)} M9 8L15 16 M15 8L9 16",
    "shape-rectangle": rect_path(5, 6, 14, 12),
    "shape-ellipse": ellipse_path(12, 12, 7, 5.5),
    "text-ltr": "M5 6H15 M12 3L15 6L12 9 M5 13H18 M5 18H14",
    "text-rtl": "M9 6H19 M12 3L9 6L12 9 M6 13H19 M10 18H19",
}

VIRTUA_COPIES = {
    "glyph-grid": "E004",
}


def replace_glyph(font: ufoLib2.Font, name: str):
    if name in font:
        del font[name]
    return font.newGlyph(name)


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    icon_ufo = repo_root / "assets" / "runebender-icons.ufo"
    virtua_ufo = repo_root / "web" / "assets" / "test-fonts" / "VirtuaGrotesk-Regular.ufo"

    font = ufoLib2.Font.open(icon_ufo)
    virtua = ufoLib2.Font.open(virtua_ufo)

    for target_name, source_name in VIRTUA_COPIES.items():
        glyph = replace_glyph(font, target_name)
        glyph.copyDataFromGlyph(virtua[source_name])
        glyph.unicodes = []
        glyph.lib[UFO_TOOLS_KEY] = "fill"

    for name, path_data in STROKE_ICONS.items():
        glyph = replace_glyph(font, name)
        parse_path(path_data, TransformPen(glyph.getPen(), SCREEN_TO_UFO))
        glyph.width = 24
        glyph.lib[UFO_TOOLS_KEY] = "stroke"

    font.save(icon_ufo, overwrite=True)
    print(f"seeded {len(VIRTUA_COPIES) + len(STROKE_ICONS)} icons into {icon_ufo}")


if __name__ == "__main__":
    main()
