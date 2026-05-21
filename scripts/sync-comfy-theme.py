#!/usr/bin/env python3
"""Sync the Runebender Dark palette into a ComfyUI settings file.

ComfyUI only reads custom color palettes from
`user/default/comfy.settings.json` under the `Comfy.CustomColorPalettes`
key — there's no drop-a-file-in-a-folder mechanism. This script copies
`themes/runebender-dark.json` (the version-controlled source of truth)
into that settings file so "Runebender Dark" shows up in ComfyUI's
theme menu.

Designed to run from run-mac.sh just before ComfyUI starts, so the
palette is always in sync with the repo on launch. Idempotent: it only
updates the runebender-dark entry, leaves every other setting and
palette untouched, and never changes the active palette.

Best-effort by design: any error prints a warning and exits 0 so a
theme-sync hiccup can never block ComfyUI from starting.

Usage:
    sync-comfy-theme.py /path/to/comfy.settings.json
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

THEME_PATH = Path(__file__).resolve().parent.parent / "themes" / "runebender-dark.json"
PALETTES_KEY = "Comfy.CustomColorPalettes"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: sync-comfy-theme.py <comfy.settings.json>", file=sys.stderr)
        return 0

    settings_path = Path(sys.argv[1])

    try:
        theme = json.loads(THEME_PATH.read_text(encoding="utf-8"))
    except Exception as exc:
        print(f"[sync-comfy-theme] skip: cannot read {THEME_PATH}: {exc}", file=sys.stderr)
        return 0

    theme_id = str(theme.get("id") or "").strip()
    if not theme_id:
        print(f"[sync-comfy-theme] skip: theme has no id ({THEME_PATH})", file=sys.stderr)
        return 0

    if settings_path.exists():
        try:
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
        except Exception as exc:
            print(
                f"[sync-comfy-theme] skip: cannot parse {settings_path}: {exc}",
                file=sys.stderr,
            )
            return 0
    else:
        settings = {}

    if not isinstance(settings, dict):
        print(f"[sync-comfy-theme] skip: {settings_path} is not a JSON object", file=sys.stderr)
        return 0

    palettes = settings.get(PALETTES_KEY)
    if not isinstance(palettes, dict):
        palettes = {}

    if palettes.get(theme_id) == theme:
        print(f"[sync-comfy-theme] '{theme_id}' already up to date")
        return 0

    palettes[theme_id] = theme
    settings[PALETTES_KEY] = palettes

    try:
        settings_path.parent.mkdir(parents=True, exist_ok=True)
        settings_path.write_text(
            json.dumps(settings, indent=4, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    except Exception as exc:
        print(f"[sync-comfy-theme] skip: cannot write {settings_path}: {exc}", file=sys.stderr)
        return 0

    print(f"[sync-comfy-theme] synced '{theme_id}' into {settings_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
