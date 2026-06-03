#!/usr/bin/env python3
"""Assign stable PUA codepoints to assets/runebender-icons.ufo."""

from __future__ import annotations

import argparse
import json
import plistlib
import re
from pathlib import Path

PUA_START = 0xE000
PUA_END = 0xF8FF


def load_codepoints(path: Path) -> dict[str, int]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    codepoints: dict[str, int] = {}
    seen: dict[int, str] = {}
    for glyph_name, value in raw.items():
        codepoint = int(str(value).removeprefix("U+"), 16)
        if not PUA_START <= codepoint <= PUA_END:
            raise SystemExit(
                f"{glyph_name} uses U+{codepoint:04X}, outside private-use range "
                f"U+{PUA_START:04X}-U+{PUA_END:04X}"
            )
        if codepoint in seen:
            raise SystemExit(
                f"{glyph_name} and {seen[codepoint]} both use U+{codepoint:04X}"
            )
        seen[codepoint] = glyph_name
        codepoints[glyph_name] = codepoint
    return codepoints


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--allow-missing",
        action="store_true",
        help="skip manifest glyphs that are not present in the UFO",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    ufo_path = repo_root / "assets" / "runebender-icons.ufo"
    glyphs_path = ufo_path / "glyphs"
    contents_path = glyphs_path / "contents.plist"
    lib_path = ufo_path / "lib.plist"
    manifest_path = repo_root / "assets" / "runebender-icons.codepoints.json"

    codepoints = load_codepoints(manifest_path)
    contents = plistlib.loads(contents_path.read_bytes())

    missing = [name for name in codepoints if name not in contents]
    if missing and not args.allow_missing:
        raise SystemExit(
            "Icon codepoint manifest references glyphs missing from the UFO: "
            + ", ".join(missing)
        )

    assigned = 0
    for glyph_name, codepoint in codepoints.items():
        glif_name = contents.get(glyph_name)
        if glif_name is None:
            continue
        glif_path = glyphs_path / glif_name
        text = glif_path.read_text(encoding="utf-8")
        without_unicode = re.sub(r"\n[ \t]*<unicode hex=\"[0-9A-Fa-f]+\"/>", "", text)
        unicode_line = f'<unicode hex="{codepoint:04X}"/>'

        match = re.search(r"(?m)^([ \t]*<advance\b[^\n]*/>)", without_unicode)
        if match:
            insert_at = match.end()
            indent = re.match(r"[ \t]*", match.group(1)).group(0)
            updated = (
                without_unicode[:insert_at]
                + "\n"
                + indent
                + unicode_line
                + without_unicode[insert_at:]
            )
        else:
            match = re.search(r"(?m)^([ \t]*<glyph\b[^\n]*>)", without_unicode)
            if match is None:
                raise SystemExit(f"Could not find <glyph> in {glif_path}")
            insert_at = match.end()
            indent = re.match(r"[ \t]*", match.group(1)).group(0)
            updated = (
                without_unicode[:insert_at]
                + "\n"
                + indent
                + "\t"
                + unicode_line
                + without_unicode[insert_at:]
            )

        if updated != text:
            glif_path.write_text(updated, encoding="utf-8")
        assigned += 1

    lib = plistlib.loads(lib_path.read_bytes()) if lib_path.exists() else {}
    lib["public.glyphOrder"] = [
        glyph_name for glyph_name in codepoints if glyph_name in contents
    ]
    lib_path.write_bytes(plistlib.dumps(lib, sort_keys=False))

    print(f"assigned {assigned} PUA codepoints in {ufo_path}")


if __name__ == "__main__":
    main()
