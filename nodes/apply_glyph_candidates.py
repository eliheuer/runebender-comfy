"""Apply reviewed glyph candidates back into a target FONT workspace."""

from __future__ import annotations

import json
import plistlib
import shutil
from pathlib import Path
import xml.etree.ElementTree as ET

from .glyph_candidate_builder import MARK_COLORS, master_pair, open_ufo, require_font_dependencies
from .mark_colors import mark_color_matches
from .workspace import (
    _read_manifest,
    _write_manifest,
    invalidate_workspace_path,
    resolve_slot,
    source_info_for_slot,
    source_path,
)


class ApplyGlyphCandidates:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Copy selected glyphs from a reviewed candidate FONT back into a target FONT."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "candidate_font": ("FONT",),
                "target_font": ("FONT",),
            },
            "optional": {
                "glyphs": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "mark:red",
                        "tooltip": "Use mark:red for red glyphs from the candidate report, 'report' for the full candidate report list, or comma-separated names.",
                    },
                ),
                "clear_mark_color": (
                    "BOOLEAN",
                    {
                        "default": False,
                        "tooltip": "Leave off to preserve your review colors after applying.",
                    },
                ),
                "write_linked_source": (
                    "BOOLEAN",
                    {
                        "default": True,
                        "tooltip": "When target_font is linked, also write to the original disk source.",
                    },
                ),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("target_font", "report")
    FUNCTION = "run"

    def run(
        self,
        candidate_font: str,
        target_font: str,
        glyphs: str = "report",
        clear_mark_color: bool = False,
        write_linked_source: bool = True,
    ):
        report = apply_glyph_candidates(
            candidate_font,
            target_font,
            glyphs_spec=glyphs,
            clear_mark_color=clear_mark_color,
            write_linked_source=write_linked_source,
        )
        return (target_font, json.dumps(report, indent=2))


def apply_glyph_candidates(
    candidate_slot: str,
    target_slot: str,
    *,
    glyphs_spec: str,
    clear_mark_color: bool,
    write_linked_source: bool,
) -> dict:
    require_font_dependencies()
    candidate_source = source_path(candidate_slot)
    target_source = source_path(target_slot)
    glyph_names = resolve_glyph_names(candidate_slot, glyphs_spec)
    if not glyph_names:
        raise ValueError("Apply Glyph Candidates needs at least one glyph name or a non-empty candidate report")

    destinations = [{"kind": "workspace", "source": target_source}]
    source_info = source_info_for_slot(target_slot)
    if write_linked_source and source_info.linked and source_info.origin_source is not None:
        origin_source = source_info.origin_source
        if origin_source.resolve() != target_source.resolve():
            destinations.append({"kind": "linked_source", "source": origin_source})

    destination_reports = []
    for destination in destinations:
        destination_reports.extend(
            apply_to_designspace_pair(
                candidate_source,
                destination["source"],
                glyph_names,
                clear_mark_color=clear_mark_color,
                destination_kind=destination["kind"],
            )
        )

    invalidate_target_slot(target_slot, target_source)
    report = {
        "candidate_slot": candidate_slot,
        "target_slot": target_slot,
        "candidate_source": str(candidate_source),
        "target_source": str(target_source),
        "write_linked_source": write_linked_source,
        "clear_mark_color": clear_mark_color,
        "glyphs": glyph_names,
        "results": destination_reports,
    }
    report_path = resolve_slot(target_slot) / "applied-glyph-candidates-report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    report["report_path"] = str(report_path)
    return report


def resolve_glyph_names(candidate_slot: str, glyphs_spec: str) -> list[str]:
    raw = (glyphs_spec or "mark:red").strip()
    if raw.startswith(("mark:", "marked:")):
        color_name = raw.split(":", 1)[1].strip().lower()
        return marked_candidate_glyphs(candidate_slot, color_name)
    if raw.lower() in {"report", "candidate-report", "glyph-candidate-report"}:
        report_path = resolve_slot(candidate_slot) / "glyph-candidate-report.json"
        if not report_path.exists():
            raise FileNotFoundError(f"Candidate report not found: {report_path}")
        report = json.loads(report_path.read_text(encoding="utf-8"))
        return [str(glyph) for glyph in report.get("glyphs", []) if str(glyph).strip()]
    return [part.strip() for part in raw.replace("\n", ",").split(",") if part.strip()]


def marked_candidate_glyphs(candidate_slot: str, color_name: str) -> list[str]:
    if color_name not in MARK_COLORS:
        known = ", ".join(sorted(MARK_COLORS))
        raise ValueError(f"Unknown mark color {color_name!r}; expected one of: {known}")
    candidate_source = source_path(candidate_slot)
    glyphs: set[str] = set()
    for source in master_pair(candidate_source):
        font = open_ufo(source.path)
        for glyph in font:
            raw = glyph.lib.get("public.markColor")
            if raw and mark_color_matches(raw, color_name):
                glyphs.add(glyph.name)
    report_names = candidate_report_candidate_glyphs(candidate_slot)
    if report_names:
        glyphs &= set(report_names)
    return sorted(glyphs)


def candidate_report_glyphs(candidate_slot: str) -> list[str]:
    report_path = resolve_slot(candidate_slot) / "glyph-candidate-report.json"
    if not report_path.exists():
        return []
    report = json.loads(report_path.read_text(encoding="utf-8"))
    return [str(glyph) for glyph in report.get("glyphs", []) if str(glyph).strip()]


def candidate_report_candidate_glyphs(candidate_slot: str) -> list[str]:
    report_path = resolve_slot(candidate_slot) / "glyph-candidate-report.json"
    if not report_path.exists():
        return []
    report = json.loads(report_path.read_text(encoding="utf-8"))
    glyphs = {
        str(result.get("glyph", "")).strip()
        for result in report.get("results", [])
        if result.get("status") == "candidate"
    }
    return sorted(glyph for glyph in glyphs if glyph)


def apply_to_designspace_pair(
    candidate_source: Path,
    target_source: Path,
    glyph_names: list[str],
    *,
    clear_mark_color: bool,
    destination_kind: str,
) -> list[dict[str, str]]:
    candidate_regular, candidate_bold = master_pair(candidate_source)
    target_regular, target_bold = master_pair(target_source)
    pairs = [
        (candidate_regular, target_regular),
        (candidate_bold, target_bold),
    ]
    reports: list[dict[str, str]] = []
    for candidate_master, target_master in pairs:
        for glyph_name in glyph_names:
            status = copy_glyph_file(
                candidate_master.path,
                target_master.path,
                glyph_name,
                clear_mark_color=clear_mark_color,
            )
            if status != "applied":
                reports.append({
                    "destination": destination_kind,
                    "master": target_master.style,
                    "glyph": glyph_name,
                    "status": status,
                })
                continue
            reports.append({
                "destination": destination_kind,
                "master": target_master.style,
                "glyph": glyph_name,
                "status": "applied",
            })
    update_linked_snapshot_if_needed(target_source)
    return reports


def copy_glyph_file(
    candidate_ufo: Path,
    target_ufo: Path,
    glyph_name: str,
    *,
    clear_mark_color: bool,
) -> str:
    """Copy one existing .glif file without rewriting the surrounding UFO."""
    candidate_file = glyph_file_for_name(candidate_ufo, glyph_name)
    if candidate_file is None or not candidate_file.exists():
        return "missing-candidate"
    target_file = glyph_file_for_name(target_ufo, glyph_name)
    if target_file is None:
        return "missing-target"
    if clear_mark_color:
        data = candidate_file.read_bytes()
        data = remove_mark_color_from_glif(data)
        target_file.write_bytes(data)
    else:
        shutil.copyfile(candidate_file, target_file)
    return "applied"


def glyph_file_for_name(ufo_path: Path, glyph_name: str) -> Path | None:
    contents_path = ufo_path / "glyphs" / "contents.plist"
    if not contents_path.exists():
        return None
    with contents_path.open("rb") as handle:
        contents = plistlib.load(handle)
    file_name = contents.get(glyph_name)
    if not file_name:
        return None
    return ufo_path / "glyphs" / file_name


def remove_mark_color_from_glif(data: bytes) -> bytes:
    root = ET.fromstring(data)
    lib = root.find("lib")
    if lib is None:
        return data
    plist = lib.find("dict")
    if plist is None:
        return data
    children = list(plist)
    for index, child in enumerate(children):
        if child.tag == "key" and child.text == "public.markColor":
            plist.remove(child)
            if index + 1 < len(children):
                plist.remove(children[index + 1])
            return ET.tostring(root, encoding="utf-8", xml_declaration=True)
    return data


def invalidate_target_slot(target_slot: str, target_source: Path) -> None:
    try:
        rel = target_source.resolve().relative_to(resolve_slot(target_slot).resolve())
    except ValueError:
        return
    invalidate_workspace_path(str(Path(target_slot) / rel))


def update_linked_snapshot_if_needed(target_source: Path) -> None:
    slot_dir = target_source.parent
    while slot_dir != slot_dir.parent:
        manifest = _read_manifest(slot_dir)
        if manifest.get("origin_mode") == "linked":
            manifest.pop("origin_snapshot_mtime_ns", None)
            _write_manifest(slot_dir, manifest)
            return
        slot_dir = slot_dir.parent
