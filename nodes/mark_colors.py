"""Shared UFO mark-color palette for Runebender Python nodes.

Values mirror ``web/src/themeTokens.ts`` so Comfy-side workflows and the
editor agree on which visual marks mean red, green, etc.
"""

from __future__ import annotations

from typing import Iterable


Rgba = tuple[float, float, float, float]


MARK_COLORS: dict[str, Rgba] = {
    "red": (1.0, 0.29, 0.24, 1.0),
    "orange": (1.0, 0.6, 0.06, 1.0),
    "yellow": (1.0, 0.86, 0.2, 1.0),
    "green": (0.09, 0.72, 0.44, 1.0),
    "blue": (0.27, 0.44, 1.0, 1.0),
    "purple": (0.55, 0.42, 1.0, 1.0),
    "pink": (0.91, 0.42, 0.72, 1.0),
}

# Older candidate-node work used approximate display colors instead of
# xilem/editor UFO values. Keep these accepted when selecting marked
# glyphs so existing workspaces do not silently drop references.
LEGACY_MARK_COLOR_ALIASES: dict[str, tuple[Rgba, ...]] = {
    "red": ((1.0, 0.3, 0.3, 1.0),),
    "orange": ((1.0, 0.6, 0.2, 1.0),),
    "yellow": ((1.0, 0.9, 0.2, 1.0),),
    "green": ((0.3, 0.7, 0.3, 1.0),),
    "blue": ((0.1, 0.3, 0.8, 1.0),),
    "purple": ((0.6, 0.3, 0.9, 1.0),),
    "pink": ((0.9, 0.3, 0.7, 1.0),),
}


def parse_rgba(raw: str) -> Rgba | None:
    try:
        values = tuple(float(part.strip()) for part in str(raw).split(","))
    except ValueError:
        return None
    if len(values) != 4:
        return None
    return values  # type: ignore[return-value]


def rgba_matches(raw: str, expected: Rgba, tolerance: float = 0.08) -> bool:
    values = parse_rgba(raw)
    if values is None:
        return False
    return all(abs(value - target) <= tolerance for value, target in zip(values, expected, strict=True))


def accepted_mark_rgba(color_name: str) -> Iterable[Rgba]:
    color_name = color_name.strip().lower()
    expected = MARK_COLORS[color_name]
    yield expected
    yield from LEGACY_MARK_COLOR_ALIASES.get(color_name, ())


def mark_color_matches(raw: str, color_name: str, tolerance: float = 0.08) -> bool:
    color_name = color_name.strip().lower()
    if color_name not in MARK_COLORS:
        known = ", ".join(sorted(MARK_COLORS))
        raise ValueError(f"Unknown mark color {color_name!r}; expected one of: {known}")
    return any(rgba_matches(raw, expected, tolerance=tolerance) for expected in accepted_mark_rgba(color_name))
