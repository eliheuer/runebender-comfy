"""FONT specimen preview node.

Consumes a FONT workspace reference and renders a simple raster
specimen. This is intentionally lightweight: it validates the workspace
contract and gives us a usable rendering node before a full DrawBot
pipeline lands.
"""

from __future__ import annotations

import tempfile
import threading
from io import BytesIO
import os
from pathlib import Path
import plistlib
import xml.etree.ElementTree as ET

from .workspace import compiled_path

# drawbot-skia uses module-level global state for the active canvas.
# Serialize render calls so two concurrent preview requests can't
# corrupt each other. The aiohttp default thread-pool executor is
# single-threaded so this is usually a no-op, but if the caller ever
# bumps the executor's worker count we're still safe.
_DRAWBOT_LOCK = threading.Lock()


def _image_stack():
    try:
        import numpy as np
        from PIL import Image, ImageDraw, ImageFont
        import torch
    except ImportError as exc:
        raise ImportError(
            "FontPreview requires numpy, Pillow, and torch; install them in the ComfyUI Python environment"
        ) from exc
    return np, Image, ImageDraw, ImageFont, torch


def _pil_to_tensor(img):
    np, _image, _image_draw, _image_font, torch = _image_stack()
    arr = np.array(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy(arr).unsqueeze(0)


def render_specimen_image(font_path, text: str, width: int, height: int):
    _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
    img = Image.new("RGBA", (width, height), (16, 16, 16, 255))
    draw = ImageDraw.Draw(img)
    if font_path and os.path.exists(font_path):
        size = max(12, int(height * 0.64))
        face = ImageFont.truetype(str(font_path), size=size)
        bbox = draw.textbbox((0, 0), text, font=face)
        text_w = (bbox[2] - bbox[0]) if bbox else 0
        text_h = (bbox[3] - bbox[1]) if bbox else 0
        x = max(0, (width - text_w) // 2)
        y = max(0, (height - text_h) // 2)
        draw.text((x, y), text, fill=(220, 220, 220, 255), font=face)
    else:
        face = ImageFont.load_default()
        message = "No compiled font yet"
        bbox = draw.textbbox((0, 0), message, font=face)
        text_w = (bbox[2] - bbox[0]) if bbox else 0
        text_h = (bbox[3] - bbox[1]) if bbox else 0
        x = max(0, (width - text_w) // 2)
        y = max(0, (height - text_h) // 2)
        draw.text((x, y), message, fill=(180, 180, 180, 255), font=face)
    return img


def render_preview_png(font_path, text: str, width: int, height: int) -> bytes:
    img = render_specimen_image(font_path, text, width, height)
    bio = BytesIO()
    img.convert("RGB").save(bio, format="PNG")
    return bio.getvalue()


def render_workspace_preview_png(
    slot_dir: Path,
    text: str,
    width: int,
    height: int,
    ttf_path: Path | None = None,
) -> bytes:
    """Render a specimen for a workspace.

    Preference order (each falls through to the next on failure):
      1. drawbot-skia + compiled TTF, if a TTF path is provided.
         Matches the rendering pipeline used by the DrawBot script nodes
         so previews and pipeline outputs come from the same engine.
      2. Direct skia-python + raw UFO. Works without a compiled TTF.
      3. PIL polygon fallback for environments without skia.
    """
    if ttf_path is not None and Path(ttf_path).exists():
        try:
            return _render_workspace_preview_png_drawbot(Path(ttf_path), text, width, height)
        except Exception:
            pass
    try:
        return _render_workspace_preview_png_skia(slot_dir, text, width, height)
    except Exception:
        return _render_workspace_preview_png_polygon(slot_dir, text, width, height)


def _render_workspace_preview_png_drawbot(
    ttf_path: Path,
    text: str,
    width: int,
    height: int,
) -> bytes:
    """Render the specimen via drawbot-skia from a compiled TTF.

    Single-string input is auto-wrapped: tries 1..8 lines and picks the
    line count that yields the largest font size while still fitting
    both the canvas width and height.
    """
    import drawbot_skia.drawbot as db

    glyphs = text.replace("\n", "")
    if not glyphs:
        return render_preview_png(None, text, width, height)

    margin = min(width, height) * 0.04
    avail_w = max(1.0, width - margin * 2)
    avail_h = max(1.0, height - margin * 2)
    LINE_SPACING = 1.1

    with _DRAWBOT_LOCK:
        db.newDrawing()
        try:
            db.newPage(width, height)
            db.fill(16 / 255)
            db.rect(0, 0, width, height)

            db.font(str(ttf_path))

            REF_SIZE = 100.0
            db.fontSize(REF_SIZE)
            total = len(glyphs)

            # Measure each glyph's width once at REF_SIZE. We use these
            # to pack glyphs into n lines such that every line ends up
            # roughly the same pixel width — without this width-balanced
            # packing, fixed N-chars-per-line means a single wide letter
            # (W/M/m) pins max_w and the other lines have huge slack on
            # the sides.
            char_widths = [db.textSize(c)[0] for c in glyphs]
            total_char_width = sum(char_widths)

            def balanced_lines(n_lines: int) -> list[str]:
                if n_lines <= 1 or n_lines >= total:
                    return [glyphs]
                target = total_char_width / n_lines
                lines_out: list[list[str]] = [[]]
                cur_w = 0.0
                for i, ch in enumerate(glyphs):
                    w = char_widths[i]
                    remaining_chars = total - i
                    remaining_lines = n_lines - len(lines_out)
                    # Start a new line when the current line has reached
                    # the target width AND we still need to open more
                    # lines AND we have enough chars left to fill them
                    # (at least one per remaining line).
                    if (
                        cur_w >= target
                        and remaining_lines > 0
                        and remaining_chars > remaining_lines
                    ):
                        lines_out.append([])
                        cur_w = 0.0
                    lines_out[-1].append(ch)
                    cur_w += w
                return ["".join(line) for line in lines_out]

            def scale_for(lines: list[str]) -> float:
                if not lines:
                    return 0.0
                # Measure the actual rendered width (includes kerning) for
                # the final pick rather than the additive estimate.
                max_w = max((db.textSize(line)[0] for line in lines), default=1.0)
                ref_line_h = REF_SIZE * LINE_SPACING
                return min(
                    avail_w / max(1.0, max_w),
                    avail_h / max(1.0, len(lines) * ref_line_h),
                )

            best_lines = balanced_lines(1)
            best_scale = scale_for(best_lines)
            for n in range(2, 9):
                if n > total:
                    break
                cand = balanced_lines(n)
                s = scale_for(cand)
                if s > best_scale:
                    best_scale = s
                    best_lines = cand

            target_size = REF_SIZE * best_scale
            db.fontSize(target_size)
            line_h = target_size * LINE_SPACING
            n_lines = len(best_lines)
            block_h = n_lines * line_h
            # In drawbot-skia y goes up from the bottom; db.text(s, (x, y))
            # draws at the baseline. Approximate the ascender as 0.8 * size
            # so the visible block of N lines is centered vertically.
            ascender_approx = target_size * 0.8
            first_baseline_y = (height + block_h) / 2 - ascender_approx

            db.fill(220 / 255)
            for i, line in enumerate(best_lines):
                line_w, _ = db.textSize(line)
                x = (width - line_w) / 2
                y = first_baseline_y - i * line_h
                db.text(line, (x, y))

            with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
                png_path = Path(f.name)
            try:
                db.saveImage(str(png_path))
                return png_path.read_bytes()
            finally:
                png_path.unlink(missing_ok=True)
        finally:
            db.endDrawing()


def _render_workspace_preview_png_skia(slot_dir: Path, text: str, width: int, height: int) -> bytes:
    import skia
    import ufoLib2
    from fontTools.pens.recordingPen import DecomposingRecordingPen

    ufo_dir = _pick_preview_ufo(slot_dir)
    if ufo_dir is None:
        return render_preview_png(None, text, width, height)

    font = ufoLib2.Font.open(ufo_dir)
    glyphs_by_char = _ufo_glyphs_by_char(font)
    has_explicit_newlines = "\n" in text
    entries: list[object | None] = []
    for char in text:
        if char == "\n":
            entries.append(None)
            continue
        glyph = glyphs_by_char.get(char)
        if glyph is not None:
            entries.append(glyph)

    if not entries:
        return render_preview_png(None, text, width, height)

    units_per_em, ascender, descender = _font_metrics(font)
    line_height = max(1, ascender - descender)
    margin = min(width, height) * 0.06
    avail_w = max(1.0, width - margin * 2)
    avail_h = max(1.0, height - margin * 2)
    if has_explicit_newlines:
        # Respect the caller's explicit line breaks.
        lines = _preview_lines(entries)
    else:
        # Auto-wrap the glyph run into the line count that produces the
        # largest per-glyph scale given the canvas aspect ratio. The
        # user gets the biggest glyphs that still fit all of them.
        flat_glyphs = [g for g in entries if g is not None]
        lines = _auto_wrap_glyphs(flat_glyphs, line_height, avail_w, avail_h)
    max_line_width = max((_line_width(line) for line in lines), default=1)
    total_height = len(lines) * line_height
    scale = min(avail_w / max(1, max_line_width), avail_h / max(1, total_height))
    scale = max(0.01, scale)

    surface = skia.Surface(width, height)
    canvas = surface.getCanvas()
    canvas.clear(skia.Color(16, 16, 16, 255))

    paint = skia.Paint(
        AntiAlias=True,
        Color=skia.Color(220, 220, 220, 255),
        Style=skia.Paint.kFill_Style,
    )

    block_height = len(lines) * line_height * scale
    first_baseline = (height - block_height) / 2 + ascender * scale
    for line_index, line in enumerate(lines):
        line_width = _line_width(line)
        pen_x = (width - line_width * scale) / 2
        baseline = first_baseline + line_index * line_height * scale
        for glyph in line:
            path = _glyph_to_skia_path(font, glyph, DecomposingRecordingPen, skia)
            if not path.isEmpty():
                canvas.save()
                canvas.translate(pen_x, baseline)
                canvas.scale(scale, -scale)
                canvas.drawPath(path, paint)
                canvas.restore()
            pen_x += float(glyph.width or units_per_em * 0.6) * scale

    data = surface.makeImageSnapshot().encodeToData(skia.kPNG, 100)
    if data is None:
        return render_preview_png(None, text, width, height)
    return bytes(data)


def _render_workspace_preview_png_polygon(slot_dir: Path, text: str, width: int, height: int) -> bytes:
    """Last-resort source preview for environments without skia/ufoLib2."""
    _np, Image, ImageDraw, _image_font, _torch = _image_stack()
    img = Image.new("RGBA", (width, height), (16, 16, 16, 255))
    draw = ImageDraw.Draw(img)

    ufo_dir = _pick_preview_ufo(slot_dir)
    if ufo_dir is None:
        return render_preview_png(None, text, width, height)

    glyphs = _load_ufo_glyphs(ufo_dir)
    entries = []
    for char in text:
        glyph = _glyph_for_char(glyphs, char)
        if glyph is not None:
            entries.append(glyph)

    if not entries:
        return render_preview_png(None, text, width, height)

    units_per_em, ascender, descender = _read_ufo_metrics(ufo_dir)
    line_height = max(1, ascender - descender)
    text_width = sum(max(1, glyph["advance"]) for glyph in entries)
    margin = min(width, height) * 0.12
    scale = min((width - margin * 2) / max(1, text_width), (height - margin * 2) / line_height)
    scale = max(0.01, scale)
    baseline = (height + (ascender + descender) * scale) / 2
    pen_x = (width - text_width * scale) / 2

    for glyph in entries:
        for polygon in glyph["polygons"]:
            if len(polygon) < 2:
                continue
            points = [
                (pen_x + x * scale, baseline - y * scale)
                for x, y in polygon
            ]
            draw.polygon(points, fill=(220, 220, 220, 255))
        pen_x += glyph["advance"] * scale

    bio = BytesIO()
    img.convert("RGB").save(bio, format="PNG")
    return bio.getvalue()


def _ufo_glyphs_by_char(font) -> dict[str, object]:
    glyphs = {}
    for glyph in font:
        for codepoint in getattr(glyph, "unicodes", []) or []:
            try:
                glyphs[chr(int(codepoint))] = glyph
            except (TypeError, ValueError):
                continue
    return glyphs


def _font_metrics(font) -> tuple[int, int, int]:
    info = font.info
    units_per_em = int(getattr(info, "unitsPerEm", None) or 1000)
    ascender = int(getattr(info, "ascender", None) or units_per_em * 0.8)
    descender = int(getattr(info, "descender", None) or -units_per_em * 0.2)
    return units_per_em, ascender, descender


def _preview_lines(entries: list[object | None]) -> list[list[object]]:
    lines: list[list[object]] = [[]]
    for entry in entries:
        if entry is None:
            if lines[-1]:
                lines.append([])
            continue
        lines[-1].append(entry)
    return [line for line in lines if line]


def _line_width(line: list[object]) -> float:
    return sum(float(getattr(glyph, "width", None) or 600) for glyph in line)


def _auto_wrap_glyphs(
    glyphs: list[object],
    line_height: float,
    avail_w: float,
    avail_h: float,
) -> list[list[object]]:
    """Wrap a flat glyph run into the line count that gives the biggest
    per-glyph scale on the available canvas.

    Tries 1..8 lines and picks whichever maximizes the limiting scale
    (min of width-fit and height-fit). This lets a long single-string
    specimen flow across multiple lines so the glyphs fill the box
    instead of being pinned by the widest line.
    """
    if not glyphs:
        return []
    advances = [float(getattr(g, "width", None) or 600) for g in glyphs]
    total = len(glyphs)

    def lines_for(n_lines: int) -> list[list[object]]:
        per_line = max(1, (total + n_lines - 1) // n_lines)
        out: list[list[object]] = []
        for i in range(0, total, per_line):
            out.append(glyphs[i : i + per_line])
        return out

    def scale_for(lines: list[list[object]]) -> float:
        if not lines:
            return 0.0
        offsets = [0]
        for line in lines:
            offsets.append(offsets[-1] + len(line))
        widths = [
            sum(advances[offsets[i] : offsets[i + 1]]) for i in range(len(lines))
        ]
        max_w = max(widths) if widths else 1.0
        total_h = len(lines) * line_height
        return min(avail_w / max(1.0, max_w), avail_h / max(1.0, total_h))

    best_lines = [glyphs]
    best_scale = scale_for(best_lines)
    for n in range(2, 9):
        if n > total:
            break
        candidate = lines_for(n)
        candidate_scale = scale_for(candidate)
        if candidate_scale > best_scale:
            best_scale = candidate_scale
            best_lines = candidate
    return best_lines


def _glyph_to_skia_path(font, glyph, pen_cls, skia):
    pen = pen_cls(font)
    glyph.draw(pen)

    path = skia.Path()
    path.setFillType(skia.PathFillType.kWinding)
    for op, args in pen.value:
        if op == "moveTo":
            (x, y), = args
            path.moveTo(float(x), float(y))
        elif op == "lineTo":
            (x, y), = args
            path.lineTo(float(x), float(y))
        elif op == "curveTo":
            if len(args) != 3:
                continue
            (x1, y1), (x2, y2), (x3, y3) = args
            path.cubicTo(float(x1), float(y1), float(x2), float(y2), float(x3), float(y3))
        elif op == "qCurveTo":
            _append_qcurve(path, args)
        elif op == "closePath":
            path.close()
    return path


def _append_qcurve(path, points) -> None:
    if not points:
        return
    offcurves = list(points[:-1])
    oncurve = points[-1]
    if oncurve is None:
        return
    if len(offcurves) == 1:
        (cx, cy), (x, y) = offcurves[0], oncurve
        path.quadTo(float(cx), float(cy), float(x), float(y))
        return
    current_controls = offcurves
    for index, control in enumerate(current_controls):
        if index < len(current_controls) - 1:
            next_control = current_controls[index + 1]
            implied = ((control[0] + next_control[0]) / 2, (control[1] + next_control[1]) / 2)
            target = implied
        else:
            target = oncurve
        path.quadTo(float(control[0]), float(control[1]), float(target[0]), float(target[1]))


def _pick_preview_ufo(slot_dir: Path) -> Path | None:
    ufo_dirs = sorted(
        path
        for path in slot_dir.rglob("*.ufo")
        if path.is_dir() and not any(parent.suffix.lower() == ".glyphspackage" for parent in path.parents)
    )
    if not ufo_dirs:
        return None
    for path in ufo_dirs:
        if "regular" in path.name.lower():
            return path
    return ufo_dirs[0]


def _read_ufo_metrics(ufo_dir: Path) -> tuple[int, int, int]:
    fontinfo = ufo_dir / "fontinfo.plist"
    if fontinfo.exists():
        try:
            data = plistlib.loads(fontinfo.read_bytes())
            units_per_em = int(data.get("unitsPerEm") or 1000)
            ascender = int(data.get("ascender") or units_per_em * 0.8)
            descender = int(data.get("descender") or -units_per_em * 0.2)
            return units_per_em, ascender, descender
        except Exception:
            pass
    return 1000, 800, -200


def _load_ufo_glyphs(ufo_dir: Path) -> dict[str, dict]:
    glyphs_dir = ufo_dir / "glyphs"
    contents_path = glyphs_dir / "contents.plist"
    if not contents_path.exists():
        return {}
    try:
        contents = plistlib.loads(contents_path.read_bytes())
    except Exception:
        return {}

    glyphs = {}
    for name, filename in contents.items():
        glif_path = glyphs_dir / str(filename)
        if not glif_path.exists():
            continue
        glyph = _parse_glif(glif_path)
        if glyph is not None:
            glyphs[str(name)] = glyph
    return glyphs


def _glyph_for_char(glyphs: dict[str, dict], char: str) -> dict | None:
    if char in glyphs:
        return glyphs[char]
    codepoint = f"{ord(char):04X}"
    for glyph in glyphs.values():
        if codepoint in glyph["unicodes"]:
            return glyph
    return None


def _parse_glif(path: Path) -> dict | None:
    try:
        root = ET.fromstring(path.read_text(encoding="utf-8"))
    except Exception:
        return None
    advance_node = root.find("advance")
    try:
        advance = int(float(advance_node.get("width", "600"))) if advance_node is not None else 600
    except ValueError:
        advance = 600
    unicodes = {node.get("hex", "").upper() for node in root.findall("unicode") if node.get("hex")}
    polygons = []
    outline = root.find("outline")
    if outline is not None:
        for contour in outline.findall("contour"):
            polygon = _contour_to_polygon(contour)
            if polygon:
                polygons.append(polygon)
    return {"advance": advance, "unicodes": unicodes, "polygons": polygons}


def _contour_to_polygon(contour: ET.Element) -> list[tuple[float, float]]:
    raw_points = []
    for point in contour.findall("point"):
        try:
            x = float(point.get("x", "0"))
            y = float(point.get("y", "0"))
        except ValueError:
            continue
        raw_points.append((x, y, point.get("type")))
    if not raw_points:
        return []

    start_index = next((i for i, point in enumerate(raw_points) if point[2] in {"move", "line", "curve"}), 0)
    ordered = raw_points[start_index:] + raw_points[:start_index]
    current = (ordered[0][0], ordered[0][1])
    polygon = [current]
    offcurves: list[tuple[float, float]] = []

    for x, y, point_type in ordered[1:] + [ordered[0]]:
        point = (x, y)
        if point_type is None:
            offcurves.append(point)
            continue
        if point_type == "curve" and len(offcurves) >= 2:
            polygon.extend(_cubic_points(current, offcurves[-2], offcurves[-1], point))
        elif point_type == "qcurve" and offcurves:
            polygon.extend(_quadratic_points(current, offcurves[-1], point))
        else:
            polygon.append(point)
        current = point
        offcurves = []

    return polygon


def _cubic_points(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
    steps: int = 10,
) -> list[tuple[float, float]]:
    points = []
    for index in range(1, steps + 1):
        t = index / steps
        mt = 1 - t
        x = mt ** 3 * p0[0] + 3 * mt ** 2 * t * p1[0] + 3 * mt * t ** 2 * p2[0] + t ** 3 * p3[0]
        y = mt ** 3 * p0[1] + 3 * mt ** 2 * t * p1[1] + 3 * mt * t ** 2 * p2[1] + t ** 3 * p3[1]
        points.append((x, y))
    return points


def _quadratic_points(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    steps: int = 8,
) -> list[tuple[float, float]]:
    points = []
    for index in range(1, steps + 1):
        t = index / steps
        mt = 1 - t
        x = mt ** 2 * p0[0] + 2 * mt * t * p1[0] + t ** 2 * p2[0]
        y = mt ** 2 * p0[1] + 2 * mt * t * p1[1] + t ** 2 * p2[1]
        points.append((x, y))
    return points


class FontPreview:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "text": ("STRING", {"multiline": False, "default": "Aa"}),
                "width": ("INT", {"default": 1024, "min": 64, "max": 4096}),
                "height": ("INT", {"default": 512, "min": 64, "max": 4096}),
            },
        }

    RETURN_TYPES = ("IMAGE",)
    RETURN_NAMES = ("image",)
    FUNCTION = "run"

    def run(self, font: str, text: str, width: int, height: int):
        try:
            font_path = compiled_path(font)
        except Exception:
            font_path = None
        img = render_specimen_image(font_path, text, width, height)
        return (_pil_to_tensor(img),)
