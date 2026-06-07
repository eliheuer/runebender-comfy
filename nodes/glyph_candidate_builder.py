"""Build donor-derived glyph candidates inside a Runebender FONT workspace.

The node is intentionally conservative: it forks the incoming workspace and
only edits that fork. The original FONT wire is never modified.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import xml.etree.ElementTree as ET

from .workspace import (
    COMPILED_EXTS,
    _read_manifest,
    _write_manifest,
    fork_slot,
    make_fork_name,
    resolve_slot,
    source_path,
)
from .mark_colors import MARK_COLORS, mark_color_matches, rgba_matches


DEFAULT_DONOR = "/Users/eli/GH/repos/rubik/sources/designspace/Rubik.designspace"

DONOR_NAME_OVERRIDES = {
    "farsiYeh-ar": "yeh-farsi",
    "farsiYeh-ar.fina": "yeh-farsi.fina",
    "farsiYeh-ar.init": "yeh-farsi.init",
    "farsiYeh-ar.medi": "yeh-farsi.medi",
    "zeroFarsi-ar": "zero-persian",
    "oneFarsi-ar": "one-persian",
    "twoFarsi-ar": "two-persian",
    "threeFarsi-ar": "three-persian",
    "fourFarsi-ar": "four-persian",
    "fiveFarsi-ar": "five-persian",
    "sixFarsi-ar": "six-persian",
    "sevenFarsi-ar": "seven-persian",
    "eightFarsi-ar": "eight-persian",
    "nineFarsi-ar": "nine-persian",
    "threedotsdowncenter-ar": "threedotsdownbelow-ar",
}


@dataclass(frozen=True)
class Source:
    path: Path
    style: str
    weight: float


@dataclass
class GlyphResult:
    glyph: str
    donor_glyph: str
    master: str
    status: str
    x_scale: float = 0.0
    y_scale: float = 0.0
    target_width: int = 0
    donor_width: int = 0


class GlyphCandidateBuilder:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Fork a FONT workspace and replace marked glyphs with donor-derived candidates."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "donor_path": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": DEFAULT_DONOR,
                        "tooltip": "OFL donor .designspace or .ufo source, e.g. Rubik.",
                    },
                ),
            },
            "optional": {
                "glyphs": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "mark:red",
                        "tooltip": "Use mark:red, or a comma-separated glyph list.",
                    },
                ),
                "arabic_only": ("BOOLEAN", {"default": True}),
                "x_scale_mode": (("target-advance", "upm", "same-as-y"), {"default": "target-advance"}),
                "candidate_name": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "",
                        "tooltip": "Optional workspace name for the generated candidate.",
                    },
                ),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("candidate_font", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        donor_path: str = DEFAULT_DONOR,
        glyphs: str = "mark:red",
        arabic_only: bool = True,
        x_scale_mode: str = "target-advance",
        candidate_name: str = "",
    ):
        report = build_candidate_workspace(
            font,
            donor_path=donor_path,
            glyphs_spec=glyphs,
            arabic_only=arabic_only,
            x_scale_mode=x_scale_mode,
            candidate_name=candidate_name,
        )
        return (report["candidate_slot"], json.dumps(report, indent=2))


def build_candidate_workspace(
    font_slot: str,
    *,
    donor_path: str,
    glyphs_spec: str,
    arabic_only: bool,
    x_scale_mode: str,
    candidate_name: str,
) -> dict:
    require_font_dependencies()
    candidate_slot = (candidate_name or "").strip() or make_fork_name(font_slot)
    candidate_slot = fork_slot(font_slot, candidate_slot)
    detach_candidate_slot(candidate_slot)

    target_source = source_path(candidate_slot)
    donor_source = Path(donor_path).expanduser().resolve()
    if not donor_source.exists():
        raise FileNotFoundError(f"Donor source not found: {donor_source}")

    target_regular, target_bold = master_pair(target_source)
    donor_regular, donor_bold = master_pair(donor_source)
    target_regular_font = open_ufo(target_regular.path)
    target_bold_font = open_ufo(target_bold.path)
    donor_regular_font = open_ufo(donor_regular.path)
    donor_bold_font = open_ufo(donor_bold.path)

    glyph_names = parse_glyphs(glyphs_spec, target_source)
    if arabic_only:
        glyph_names = arabic_glyph_filter(glyph_names)
    if not glyph_names:
        raise ValueError(
            f"Glyph Candidate Builder found no glyphs for {glyphs_spec!r} "
            f"in source slot {font_slot!r}. Save the marked source, connect the "
            "Runebender node that contains the red marks, or pass an explicit "
            "comma-separated glyph list."
        )

    results: list[GlyphResult] = []
    for glyph_name in glyph_names:
        donor_name = DONOR_NAME_OVERRIDES.get(glyph_name, glyph_name)
        results.append(copy_transformed_glyph(target_regular_font, donor_regular_font, glyph_name, donor_name, x_scale_mode))
        results.append(copy_transformed_glyph(target_bold_font, donor_bold_font, glyph_name, donor_name, x_scale_mode))

    target_regular_font.save(target_regular.path, overwrite=True)
    target_bold_font.save(target_bold.path, overwrite=True)

    report = {
        "candidate_slot": candidate_slot,
        "source_slot": font_slot,
        "target_source": str(target_source),
        "donor_source": str(donor_source),
        "glyphs": glyph_names,
        "x_scale_mode": x_scale_mode,
        "results": [result.__dict__ for result in results],
    }
    report_path = resolve_slot(candidate_slot) / "glyph-candidate-report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    report["report_path"] = str(report_path)
    return report


def detach_candidate_slot(slot: str) -> None:
    slot_dir = resolve_slot(slot)
    manifest = _read_manifest(slot_dir)
    source_kind = manifest.get("source_kind", "")
    for key in ("compiled_path", "package_path"):
        rel_path = manifest.get(key, "")
        if not rel_path:
            continue
        candidate = (slot_dir / rel_path).resolve()
        if slot_dir.resolve() not in candidate.parents:
            continue
        if candidate.is_dir():
            import shutil

            shutil.rmtree(candidate)
        elif candidate.is_file():
            candidate.unlink()
    if source_kind != "glyphspackage":
        for candidate in slot_dir.glob("*.glyphspackage"):
            if candidate.is_dir():
                import shutil

                shutil.rmtree(candidate)
    for candidate in slot_dir.iterdir():
        if candidate.is_file() and candidate.suffix.lower() in COMPILED_EXTS:
            candidate.unlink()
    for key in (
        "origin_mode",
        "origin_kind",
        "origin_root",
        "origin_source",
        "origin_snapshot_mtime_ns",
        "compiled_path",
        "compile_backend",
        "package_path",
    ):
        manifest.pop(key, None)
    _write_manifest(slot_dir, manifest)


def require_font_dependencies() -> None:
    try:
        import ufoLib2  # noqa: F401
        import fontTools  # noqa: F401
    except Exception as exc:
        raise ImportError("Glyph Candidate Builder requires ufoLib2 and fontTools in the ComfyUI Python environment") from exc


def open_ufo(path: Path):
    import ufoLib2

    return ufoLib2.Font.open(path)


def read_sources(path: Path) -> list[Source]:
    path = path.expanduser().resolve()
    if path.suffix.lower() == ".ufo":
        return [Source(path=path, style=path.stem, weight=0)]
    tree = ET.parse(path)
    sources: list[Source] = []
    for source in tree.findall(".//source"):
        if source.get("layer"):
            continue
        filename = source.get("filename")
        if not filename:
            continue
        weight = 0.0
        dim = source.find("./location/dimension[@name='Weight']")
        if dim is not None and dim.get("xvalue"):
            weight = float(dim.get("xvalue", "0"))
        sources.append(
            Source(
                path=(path.parent / filename).resolve(),
                style=source.get("stylename") or Path(filename).stem,
                weight=weight,
            )
        )
    if not sources:
        raise ValueError(f"No UFO sources found in {path}")
    return sources


def master_pair(path: Path) -> tuple[Source, Source]:
    sources = sorted(read_sources(path), key=lambda item: item.weight)
    if len(sources) == 1:
        return sources[0], sources[0]
    return sources[0], sources[-1]


def parse_glyphs(raw: str, target: Path) -> list[str]:
    raw = (raw or "mark:red").strip()
    if raw.startswith(("mark:", "marked:")):
        color_name = raw.split(":", 1)[1].strip().lower()
        return marked_glyphs(target, color_name)
    return [part.strip() for part in raw.replace("\n", ",").split(",") if part.strip()]


def marked_glyphs(target: Path, color_name: str) -> list[str]:
    if color_name not in MARK_COLORS:
        known = ", ".join(sorted(MARK_COLORS))
        raise ValueError(f"Unknown mark color {color_name!r}; expected one of: {known}")
    glyphs: set[str] = set()
    for source in read_sources(target):
        font = open_ufo(source.path)
        for glyph in font:
            raw = glyph.lib.get("public.markColor")
            if raw and mark_color_matches(raw, color_name):
                glyphs.add(glyph.name)
    return sorted(glyphs)


def arabic_glyph_filter(glyphs: list[str]) -> list[str]:
    return [
        glyph
        for glyph in glyphs
        if "-ar" in glyph or "Farsi-ar" in glyph or glyph in {"dottedCircle"}
    ]


def font_metric(font, name: str, fallback: float) -> float:
    return float(getattr(font.info, name, None) or fallback)


def round_even(value: float) -> int:
    return int(round(value / 2.0) * 2)


def copy_transformed_glyph(target_font, donor_font, glyph_name: str, donor_name: str, x_scale_mode: str) -> GlyphResult:
    donor_name = resolve_donor_name(donor_font, glyph_name, donor_name)
    if donor_name not in donor_font:
        return GlyphResult(glyph_name, donor_name, target_font.info.styleName or "target", "missing-donor")

    from fontTools.pens.recordingPen import DecomposingRecordingPen
    from fontTools.pens.transformPen import TransformPen

    donor_glyph = donor_font[donor_name]
    if glyph_name not in target_font:
        target_font.newGlyph(glyph_name)
    target_glyph = target_font[glyph_name]
    previous_width = int(target_glyph.width or 0)
    previous_unicodes = list(target_glyph.unicodes or [])
    donor_width = int(donor_glyph.width or 0)

    target_upm = font_metric(target_font, "unitsPerEm", 1000)
    donor_upm = font_metric(donor_font, "unitsPerEm", 1000)
    target_x_height = font_metric(target_font, "xHeight", target_upm * 0.5)
    donor_x_height = font_metric(donor_font, "xHeight", donor_upm * 0.5)
    y_scale = target_x_height / donor_x_height if donor_x_height else target_upm / donor_upm

    if x_scale_mode == "same-as-y":
        x_scale = y_scale
    elif x_scale_mode == "target-advance" and previous_width and donor_width:
        x_scale = previous_width / donor_width
    else:
        x_scale = target_upm / donor_upm

    target_glyph.clear()
    recording_pen = DecomposingRecordingPen(donor_font)
    donor_glyph.draw(recording_pen)
    recording_pen.replay(TransformPen(target_glyph.getPen(), (x_scale, 0, 0, y_scale, 0, 0)))
    round_glyph_coordinates(target_glyph)
    for anchor in donor_glyph.anchors:
        target_glyph.appendAnchor(
            {
                "name": anchor.name,
                "x": round_even(anchor.x * x_scale),
                "y": round_even(anchor.y * y_scale),
                "color": anchor.color,
            }
        )
    target_glyph.width = previous_width or round_even(donor_width * x_scale)
    target_glyph.unicodes = previous_unicodes or list(donor_glyph.unicodes or [])
    target_glyph.lib["com.runebenderComfy.donorCandidate"] = {
        "donorGlyph": donor_name,
        "xScale": round(x_scale, 5),
        "yScale": round(y_scale, 5),
    }

    return GlyphResult(
        glyph=glyph_name,
        donor_glyph=donor_name,
        master=target_font.info.styleName or "target",
        status="candidate",
        x_scale=x_scale,
        y_scale=y_scale,
        target_width=int(target_glyph.width or 0),
        donor_width=donor_width,
    )


def resolve_donor_name(donor_font, glyph_name: str, donor_name: str) -> str:
    if donor_name in donor_font:
        return donor_name
    if glyph_name in donor_font:
        return glyph_name
    if glyph_name.endswith("Farsi-ar"):
        candidate = glyph_name.removesuffix("Farsi-ar").lower() + "-persian"
        if candidate in donor_font:
            return candidate
    return donor_name


def round_glyph_coordinates(glyph) -> None:
    from fontTools.misc.transform import Transform

    for contour in glyph.contours:
        for point in contour.points:
            point.x = round_even(point.x)
            point.y = round_even(point.y)
    for component in glyph.components:
        xx, xy, yx, yy, dx, dy = tuple(component.transformation)
        component.transformation = Transform(xx, xy, yx, yy, round_even(dx), round_even(dy))
