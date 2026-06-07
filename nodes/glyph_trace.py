"""Graph nodes for background-image glyph tracing providers."""

from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import re
import struct
import time
import xml.etree.ElementTree as ET
from urllib import error as urllib_error
from urllib import parse as urllib_parse
from urllib import request as urllib_request
import zlib

from .glyph_candidate_builder import detach_candidate_slot
from .mark_colors import MARK_COLORS, mark_color_matches
from .runebender import (
    TraceImageTransform,
    _format_number,
    _glyph_file_for_name,
    _select_trace_source,
    load_glyph_trace_request,
    trace_background_with_img2bez,
    write_glyph_trace_request,
)
from .workspace import fork_slot, make_fork_name, resolve_slot, source_path


class BuildGlyphTraceRequest:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Store a server-visible background image trace request for provider nodes."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "glyph": ("STRING", {"default": ""}),
                "master": ("STRING", {"default": "Regular"}),
                "image_path": ("STRING", {"default": ""}),
                "image_width": ("FLOAT", {"default": 1024, "min": 1}),
                "image_height": ("FLOAT", {"default": 1024, "min": 1}),
                "design_x": ("FLOAT", {"default": 0}),
                "design_y": ("FLOAT", {"default": 0}),
                "design_scale_x": ("FLOAT", {"default": 1}),
                "design_scale_y": ("FLOAT", {"default": 1}),
                "advance_width": ("FLOAT", {"default": 600, "min": 1}),
                "units_per_em": ("FLOAT", {"default": 1000, "min": 1}),
                "ascender": ("FLOAT", {"default": 800}),
                "descender": ("FLOAT", {"default": -200}),
            },
        }

    RETURN_TYPES = ("GLYPH_TRACE_REQUEST", "STRING")
    RETURN_NAMES = ("trace_request", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        glyph: str,
        master: str,
        image_path: str,
        image_width: float,
        image_height: float,
        design_x: float,
        design_y: float,
        design_scale_x: float,
        design_scale_y: float,
        advance_width: float,
        units_per_em: float,
        ascender: float,
        descender: float,
    ):
        path = Path(image_path).expanduser()
        if not path.exists() or not path.is_file():
            raise FileNotFoundError(f"Trace image not found: {path}")
        transform = TraceImageTransform(
            pixel_width=image_width,
            pixel_height=image_height,
            design_x=design_x,
            design_y=design_y,
            design_scale_x=design_scale_x,
            design_scale_y=design_scale_y,
        )
        artifact = write_glyph_trace_request(
            slot=font,
            glyph=glyph,
            master=master,
            image_bytes=path.read_bytes(),
            image_suffix=path.suffix,
            transform=transform,
            advance_width=advance_width,
            units_per_em=units_per_em,
            ascender=ascender,
            descender=descender,
        )
        report = {
            "success": True,
            "trace_request": str(artifact.request_path),
            "image_path": str(artifact.image_path),
            "request_id": artifact.request_id,
            "glyph": glyph,
            "master": master,
        }
        return (str(artifact.request_path), json.dumps(report, indent=2))


class TraceToCandidate:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Trace a GLYPH_TRACE_REQUEST with local tracing and write a candidate FONT."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "trace_request": ("GLYPH_TRACE_REQUEST",),
            },
            "optional": {
                "candidate_name": ("STRING", {"default": ""}),
                "grid": ("INT", {"default": 2, "min": 0}),
                "accuracy": ("FLOAT", {"default": 2}),
                "smooth": ("INT", {"default": 0, "min": 0}),
                "alphamax": ("FLOAT", {"default": 0.8}),
                "global_fit": ("BOOLEAN", {"default": True}),
                "invert": ("BOOLEAN", {"default": False}),
                "threshold": ("INT", {"default": -1, "min": -1, "max": 255}),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("candidate_font", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        trace_request: str,
        candidate_name: str = "",
        grid: int = 2,
        accuracy: float = 2.0,
        smooth: int = 0,
        alphamax: float = 0.8,
        global_fit: bool = True,
        invert: bool = False,
        threshold: int = -1,
    ):
        report = trace_request_to_candidate(
            font,
            trace_request,
            candidate_name=candidate_name,
            grid=grid,
            accuracy=accuracy,
            smooth=smooth,
            alphamax=alphamax,
            global_fit=global_fit,
            invert=invert,
            threshold=threshold if 0 <= threshold <= 255 else None,
        )
        return (report["candidate_slot"], json.dumps(report, indent=2))


class TraceWithQuiverAI:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Import a QuiverAI Image-to-SVG result into the trace candidate flow."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "trace_request": ("GLYPH_TRACE_REQUEST",),
                "svg_path": ("STRING", {"default": ""}),
            },
            "optional": {
                "candidate_name": ("STRING", {"default": ""}),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("candidate_font", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        trace_request: str,
        svg_path: str,
        candidate_name: str = "",
    ):
        report = trace_quiver_svg_to_candidate(
            font,
            trace_request,
            svg_path=svg_path,
            candidate_name=candidate_name,
        )
        return (report["candidate_slot"], json.dumps(report, indent=2))


class TraceWithComfyCloudQuiverAI:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Run a Comfy Cloud QuiverAI workflow, import its SVG, and write a candidate FONT."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "trace_request": ("GLYPH_TRACE_REQUEST",),
                "workflow_api_json": ("STRING", {"default": "", "multiline": True}),
                "image_node_id": ("STRING", {"default": "1"}),
                "image_input_name": ("STRING", {"default": "image"}),
            },
            "optional": {
                "candidate_name": ("STRING", {"default": ""}),
                "svg_output_node_id": ("STRING", {"default": ""}),
                "api_key": ("STRING", {"default": ""}),
                "base_url": ("STRING", {"default": "https://cloud.comfy.org"}),
                "timeout_seconds": ("INT", {"default": 300, "min": 1}),
                "poll_interval_seconds": ("FLOAT", {"default": 5, "min": 0.1}),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("candidate_font", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        trace_request: str,
        workflow_api_json: str,
        image_node_id: str,
        image_input_name: str,
        candidate_name: str = "",
        svg_output_node_id: str = "",
        api_key: str = "",
        base_url: str = "https://cloud.comfy.org",
        timeout_seconds: int = 300,
        poll_interval_seconds: float = 5.0,
    ):
        svg_path, cloud_report = run_comfy_cloud_quiver_svg(
            trace_request=trace_request,
            workflow_api_json=workflow_api_json,
            image_node_id=image_node_id,
            image_input_name=image_input_name,
            svg_output_node_id=svg_output_node_id,
            api_key=api_key,
            base_url=base_url,
            timeout_seconds=timeout_seconds,
            poll_interval_seconds=poll_interval_seconds,
        )
        report = trace_quiver_svg_to_candidate(
            font,
            trace_request,
            svg_path=svg_path,
            candidate_name=candidate_name,
            provider="comfy-cloud-quiverai",
            provider_report=cloud_report,
        )
        return (report["candidate_slot"], json.dumps(report, indent=2))


class TraceLocalMaskToCandidate:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Trace a locally generated mask image into a candidate FONT."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "trace_request": ("GLYPH_TRACE_REQUEST",),
                "mask_path": ("STRING", {"default": ""}),
            },
            "optional": {
                "candidate_name": ("STRING", {"default": ""}),
                "grid": ("INT", {"default": 2, "min": 0}),
                "accuracy": ("FLOAT", {"default": 2}),
                "smooth": ("INT", {"default": 0, "min": 0}),
                "alphamax": ("FLOAT", {"default": 0.8}),
                "global_fit": ("BOOLEAN", {"default": True}),
                "invert": ("BOOLEAN", {"default": False}),
                "threshold": ("INT", {"default": -1, "min": -1, "max": 255}),
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("candidate_font", "report")
    FUNCTION = "run"

    def run(
        self,
        font: str,
        trace_request: str,
        mask_path: str,
        candidate_name: str = "",
        grid: int = 2,
        accuracy: float = 2.0,
        smooth: int = 0,
        alphamax: float = 0.8,
        global_fit: bool = True,
        invert: bool = False,
        threshold: int = -1,
    ):
        report = trace_request_to_candidate(
            font,
            trace_request,
            candidate_name=candidate_name,
            grid=grid,
            accuracy=accuracy,
            smooth=smooth,
            alphamax=alphamax,
            global_fit=global_fit,
            invert=invert,
            threshold=threshold if 0 <= threshold <= 255 else None,
            override_image_path=mask_path,
            provider="local-model-mask-img2bez",
        )
        return (report["candidate_slot"], json.dumps(report, indent=2))


class ScoreCandidate:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = "Score a candidate glyph for review before promotion."

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "candidate_font": ("FONT",),
                "glyph": ("STRING", {"default": ""}),
                "master": ("STRING", {"default": "Regular"}),
            },
            "optional": {
                "trace_request": ("GLYPH_TRACE_REQUEST",),
                "speckle_area": ("FLOAT", {"default": 16, "min": 0}),
            },
        }

    RETURN_TYPES = ("STRING",)
    RETURN_NAMES = ("report",)
    FUNCTION = "run"

    def run(
        self,
        candidate_font: str,
        glyph: str,
        master: str,
        trace_request: str = "",
        speckle_area: float = 16,
    ):
        return (
            json.dumps(
                score_candidate_glyph(
                    candidate_font,
                    glyph=glyph,
                    master=master,
                    trace_request=trace_request or None,
                    speckle_area=speckle_area,
                ),
                indent=2,
            ),
        )


def trace_request_to_candidate(
    font: str,
    trace_request: str,
    *,
    candidate_name: str = "",
    grid: int = 2,
    accuracy: float = 2.0,
    smooth: int = 0,
    alphamax: float = 0.8,
    global_fit: bool = True,
    invert: bool = False,
    threshold: int | None = None,
    override_image_path: str | None = None,
    provider: str = "img2bez",
) -> dict:
    artifact = load_glyph_trace_request(Path(trace_request))
    payload = artifact.payload
    glyph = str(payload["glyph"])
    master = str(payload["master"])
    metrics = payload["metrics"]
    transform = payload["transform"]

    candidate_slot = (candidate_name or "").strip() or make_fork_name(font)
    candidate_slot = fork_slot(font, candidate_slot)
    detach_candidate_slot(candidate_slot)

    trace_image_path = artifact.image_path
    if override_image_path:
        override = Path(override_image_path).expanduser()
        if not override.exists() or not override.is_file():
            raise FileNotFoundError(f"Trace mask not found: {override}")
        trace_image_path = override

    result = trace_background_with_img2bez(
        slot=candidate_slot,
        master_name=master,
        glyph_name=glyph,
        image_bytes=trace_image_path.read_bytes(),
        image_suffix=trace_image_path.suffix,
        unicode_hex="",
        width=float(metrics["advanceWidth"]),
        target_height=max(1.0, abs(float(payload["image"]["height"]) * float(transform["designScaleY"]))),
        y_offset=float(transform["designY"]),
        x_offset=float(transform["designX"]),
        grid=max(0, int(grid)),
        accuracy=accuracy,
        smooth=max(0, int(smooth)),
        alphamax=alphamax,
        global_fit=global_fit,
        invert=invert,
        threshold=threshold,
    )

    candidate_source = source_path(candidate_slot)
    candidate_ufo = _select_trace_source(candidate_source, master)
    glif_path = write_candidate_glif(candidate_ufo, glyph, result.glif, mark_color="orange")
    report = {
        "success": True,
        "provider": provider,
        "candidate_slot": candidate_slot,
        "trace_request": str(artifact.request_path),
        "request_id": artifact.request_id,
        "trace_image_path": str(trace_image_path),
        "glyph": glyph,
        "master": master,
        "glif_path": str(glif_path),
        "source_ufo": str(result.source_ufo),
        "command": result.command,
        "trace_tool": result.trace_tool,
        "mark_color": "orange",
        "settings": {
            "grid": max(0, int(grid)),
            "accuracy": accuracy,
            "smooth": max(0, int(smooth)),
            "alphamax": alphamax,
            "global_fit": global_fit,
            "invert": invert,
            "threshold": threshold,
        },
    }
    report["score"] = score_candidate_glyph(
        candidate_slot,
        glyph=glyph,
        master=master,
        trace_request=str(artifact.request_path),
    )
    report_path = resolve_slot(candidate_slot) / "glyph-trace-report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    report["report_path"] = str(report_path)
    return report


def score_candidate_glyph(
    candidate_font: str,
    *,
    glyph: str,
    master: str,
    trace_request: str | None = None,
    speckle_area: float = 16,
) -> dict:
    if not glyph.strip():
        raise ValueError("glyph required")
    candidate_source = source_path(candidate_font)
    candidate_ufo = _select_trace_source(candidate_source, master)
    glif_path = _glyph_file_for_name(candidate_ufo, glyph)
    if glif_path is None or not glif_path.exists():
        raise FileNotFoundError(f"Candidate glyph not found: {glyph}")
    root = ET.fromstring(glif_path.read_bytes())
    advance = float(root.find("advance").get("width", "0")) if root.find("advance") is not None else 0.0
    contours = root.findall("./outline/contour")
    all_points: list[tuple[float, float]] = []
    contour_reports = []
    speckles = 0
    for contour in contours:
        points = []
        oncurves = []
        for point in contour.findall("point"):
            x = float(point.get("x", "0"))
            y = float(point.get("y", "0"))
            points.append((x, y))
            if point.get("type"):
                oncurves.append((x, y))
        all_points.extend(points)
        bbox = _points_bbox(points)
        area = _bbox_area(bbox)
        if area <= speckle_area:
            speckles += 1
        contour_reports.append(
            {
                "points": len(points),
                "oncurves": len(oncurves),
                "bbox": bbox,
                "bboxArea": area,
                "winding": _winding(oncurves),
            }
        )

    bbox = _points_bbox(all_points)
    left = bbox["xMin"] if bbox else None
    right = advance - bbox["xMax"] if bbox and advance else None
    candidate_stem_width = _stem_width_proxy(contour_reports, min_area=speckle_area)
    reference_stems = _green_reference_stems(candidate_ufo, exclude_glyph=glyph)
    report = {
        "success": True,
        "candidate_font": candidate_font,
        "glyph": glyph,
        "master": master,
        "glif_path": str(glif_path),
        "advanceWidth": advance,
        "bbox": bbox,
        "contours": len(contours),
        "points": len(all_points),
        "speckles": speckles,
        "sidebearings": {
            "left": left,
            "right": right,
        },
        "stemWidth": candidate_stem_width,
        "contourReports": contour_reports,
    }
    if reference_stems and candidate_stem_width is not None:
        average = sum(item["stemWidth"] for item in reference_stems) / len(reference_stems)
        report["stemComparison"] = {
            "referenceMark": "green",
            "referenceCount": len(reference_stems),
            "candidateStemWidth": candidate_stem_width,
            "referenceAverageStemWidth": average,
            "delta": candidate_stem_width - average,
            "references": reference_stems,
        }
    if trace_request:
        artifact = load_glyph_trace_request(Path(trace_request))
        transform_payload = artifact.payload["transform"]
        image_payload = artifact.payload["image"]
        transform = TraceImageTransform(
            pixel_width=float(image_payload["width"]),
            pixel_height=float(image_payload["height"]),
            design_x=float(transform_payload["designX"]),
            design_y=float(transform_payload["designY"]),
            design_scale_x=float(transform_payload["designScaleX"]),
            design_scale_y=float(transform_payload["designScaleY"]),
        )
        expected = {
            "xMin": transform.design_x,
            "yMin": transform.design_y,
            "xMax": transform.design_x + transform.pixel_width * transform.design_scale_x,
            "yMax": transform.design_y + transform.pixel_height * transform.design_scale_y,
        }
        report["backgroundComparison"] = {
            "expectedBBox": expected,
            "bboxDelta": _bbox_delta(bbox, expected),
        }
        foreground = _foreground_bbox_from_trace_image(artifact.image_path, transform)
        if foreground is not None:
            foreground_expected = foreground["designBBox"]
            report["foregroundComparison"] = {
                "foregroundBBoxPixels": foreground["pixelBBox"],
                "expectedBBox": foreground_expected,
                "bboxDelta": _bbox_delta(bbox, foreground_expected),
                "areaRatio": _area_ratio(bbox, foreground_expected),
            }
        overlay = _raster_overlay_diff_from_trace_image(
            artifact.image_path,
            transform,
            contours,
        )
        if overlay is not None:
            report["rasterOverlayDiff"] = overlay
    return report


def _points_bbox(points: list[tuple[float, float]]) -> dict | None:
    if not points:
        return None
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return {
        "xMin": min(xs),
        "yMin": min(ys),
        "xMax": max(xs),
        "yMax": max(ys),
        "width": max(xs) - min(xs),
        "height": max(ys) - min(ys),
    }


def _bbox_area(bbox: dict | None) -> float:
    if not bbox:
        return 0.0
    return float(bbox["width"]) * float(bbox["height"])


def _winding(points: list[tuple[float, float]]) -> str:
    if len(points) < 3:
        return "unknown"
    area = 0.0
    for index, (x1, y1) in enumerate(points):
        x2, y2 = points[(index + 1) % len(points)]
        area += (x2 - x1) * (y2 + y1)
    if area > 0:
        return "clockwise"
    if area < 0:
        return "counterclockwise"
    return "unknown"


def _bbox_delta(actual: dict | None, expected: dict) -> dict | None:
    if not actual:
        return None
    return {
        "xMin": actual["xMin"] - expected["xMin"],
        "yMin": actual["yMin"] - expected["yMin"],
        "xMax": actual["xMax"] - expected["xMax"],
        "yMax": actual["yMax"] - expected["yMax"],
    }


def _area_ratio(actual: dict | None, expected: dict) -> float | None:
    expected_area = _bbox_area(expected)
    if not actual or expected_area <= 0:
        return None
    return _bbox_area(actual) / expected_area


def _foreground_bbox_from_trace_image(
    image_path: Path,
    transform: TraceImageTransform,
) -> dict | None:
    try:
        width, height, rows, channels = _read_png_rows(image_path.read_bytes())
    except (OSError, ValueError, zlib.error, struct.error):
        return None
    if width <= 0 or height <= 0:
        return None

    x_min = width
    y_min = height
    x_max = -1
    y_max = -1
    for y, row in enumerate(rows):
        for x in range(width):
            offset = x * channels
            sample = row[offset : offset + channels]
            if _is_foreground_sample(sample, channels):
                x_min = min(x_min, x)
                y_min = min(y_min, y)
                x_max = max(x_max, x)
                y_max = max(y_max, y)
    if x_max < x_min or y_max < y_min:
        return None

    pixel_bbox = {
        "xMin": x_min,
        "yMin": y_min,
        "xMax": x_max + 1,
        "yMax": y_max + 1,
        "width": x_max + 1 - x_min,
        "height": y_max + 1 - y_min,
    }
    lower_left = transform.pixel_to_design(pixel_bbox["xMin"], pixel_bbox["yMax"])
    upper_right = transform.pixel_to_design(pixel_bbox["xMax"], pixel_bbox["yMin"])
    return {
        "pixelBBox": pixel_bbox,
        "designBBox": _bbox_from_corners(lower_left, upper_right),
    }


def _raster_overlay_diff_from_trace_image(
    image_path: Path,
    transform: TraceImageTransform,
    contours: list[ET.Element],
    *,
    max_samples: int = 262144,
) -> dict | None:
    try:
        width, height, rows, channels = _read_png_rows(image_path.read_bytes())
    except (OSError, ValueError, zlib.error, struct.error):
        return None
    polygons = _glif_contour_polygons(contours)
    if not polygons:
        return None
    total_pixels = width * height
    sample_step = 1
    if total_pixels > max_samples:
        sample_step = max(1, int((total_pixels / max_samples) ** 0.5))

    true_positive = 0
    false_positive = 0
    false_negative = 0
    true_negative = 0
    foreground_samples = 0
    candidate_samples = 0
    for y in range(0, height, sample_step):
        row = rows[y]
        for x in range(0, width, sample_step):
            offset = x * channels
            foreground = _is_foreground_sample(row[offset : offset + channels], channels)
            design = transform.pixel_to_design(x + 0.5, y + 0.5)
            candidate = _point_inside_even_odd_polygons(design, polygons)
            if foreground:
                foreground_samples += 1
            if candidate:
                candidate_samples += 1
            if foreground and candidate:
                true_positive += 1
            elif foreground:
                false_negative += 1
            elif candidate:
                false_positive += 1
            else:
                true_negative += 1

    union = true_positive + false_positive + false_negative
    iou = 1.0 if union == 0 else true_positive / union
    precision_denominator = true_positive + false_positive
    recall_denominator = true_positive + false_negative
    return {
        "fillRule": "evenOddApproximation",
        "sampleStep": sample_step,
        "samples": true_positive + false_positive + false_negative + true_negative,
        "foregroundSamples": foreground_samples,
        "candidateSamples": candidate_samples,
        "truePositive": true_positive,
        "falsePositive": false_positive,
        "falseNegative": false_negative,
        "trueNegative": true_negative,
        "intersectionOverUnion": iou,
        "precision": None if precision_denominator == 0 else true_positive / precision_denominator,
        "recall": None if recall_denominator == 0 else true_positive / recall_denominator,
    }


def _glif_contour_polygons(contours: list[ET.Element]) -> list[list[tuple[float, float]]]:
    polygons = []
    for contour in contours:
        points = []
        for point in contour.findall("point"):
            try:
                points.append((float(point.get("x", "0")), float(point.get("y", "0"))))
            except ValueError:
                continue
        if len(points) >= 3:
            polygons.append(points)
    return polygons


def _point_inside_even_odd_polygons(
    point: tuple[float, float],
    polygons: list[list[tuple[float, float]]],
) -> bool:
    inside = False
    for polygon in polygons:
        if _point_inside_polygon(point, polygon):
            inside = not inside
    return inside


def _point_inside_polygon(point: tuple[float, float], polygon: list[tuple[float, float]]) -> bool:
    x, y = point
    inside = False
    previous_x, previous_y = polygon[-1]
    for current_x, current_y in polygon:
        crosses = (current_y > y) != (previous_y > y)
        if crosses:
            slope_x = (previous_x - current_x) * (y - current_y) / (previous_y - current_y) + current_x
            if x < slope_x:
                inside = not inside
        previous_x, previous_y = current_x, current_y
    return inside


def _read_png_rows(data: bytes) -> tuple[int, int, list[bytes], int]:
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("not a PNG")
    pos = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        payload = data[pos + 8 : pos + 8 + length]
        pos += length + 12
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _compression, _filter, interlace = struct.unpack(
                ">IIBBBBB",
                payload,
            )
        elif kind == b"IDAT":
            idat.extend(payload)
        elif kind == b"IEND":
            break
    if width is None or height is None or bit_depth != 8 or interlace != 0:
        raise ValueError("unsupported PNG")
    channels_by_type = {0: 1, 2: 3, 4: 2, 6: 4}
    channels = channels_by_type.get(color_type)
    if channels is None:
        raise ValueError("unsupported PNG color type")
    stride = width * channels
    raw = zlib.decompress(bytes(idat))
    rows: list[bytes] = []
    prev = bytes(stride)
    source = 0
    for _y in range(height):
        filter_type = raw[source]
        source += 1
        row = bytearray(raw[source : source + stride])
        source += stride
        _unfilter_png_row(row, prev, filter_type, channels)
        rows.append(bytes(row))
        prev = rows[-1]
    return width, height, rows, channels


def _unfilter_png_row(row: bytearray, prev: bytes, filter_type: int, bpp: int) -> None:
    if filter_type == 0:
        return
    for index in range(len(row)):
        left = row[index - bpp] if index >= bpp else 0
        up = prev[index]
        up_left = prev[index - bpp] if index >= bpp else 0
        if filter_type == 1:
            row[index] = (row[index] + left) & 0xFF
        elif filter_type == 2:
            row[index] = (row[index] + up) & 0xFF
        elif filter_type == 3:
            row[index] = (row[index] + ((left + up) // 2)) & 0xFF
        elif filter_type == 4:
            row[index] = (row[index] + _png_paeth(left, up, up_left)) & 0xFF
        else:
            raise ValueError("unsupported PNG filter")


def _png_paeth(left: int, up: int, up_left: int) -> int:
    estimate = left + up - up_left
    left_delta = abs(estimate - left)
    up_delta = abs(estimate - up)
    up_left_delta = abs(estimate - up_left)
    if left_delta <= up_delta and left_delta <= up_left_delta:
        return left
    if up_delta <= up_left_delta:
        return up
    return up_left


def _is_foreground_sample(sample: bytes, channels: int) -> bool:
    if channels == 1:
        return sample[0] < 250
    if channels == 2:
        return sample[1] > 0 and sample[0] < 250
    if channels == 3:
        return sum(sample[:3]) / 3 < 250
    if channels == 4:
        return sample[3] > 0 and sum(sample[:3]) / 3 < 250
    return False


def _bbox_from_corners(
    a: tuple[float, float],
    b: tuple[float, float],
) -> dict:
    x_min = min(a[0], b[0])
    y_min = min(a[1], b[1])
    x_max = max(a[0], b[0])
    y_max = max(a[1], b[1])
    return {
        "xMin": x_min,
        "yMin": y_min,
        "xMax": x_max,
        "yMax": y_max,
        "width": x_max - x_min,
        "height": y_max - y_min,
    }


def _stem_width_proxy(contour_reports: list[dict], *, min_area: float = 16) -> float | None:
    widths = [
        float(report["bbox"]["width"])
        for report in contour_reports
        if report.get("bbox")
        and float(report["bbox"]["width"]) > 0
        and float(report["bbox"]["height"]) > 0
        and _bbox_area(report["bbox"]) > min_area
    ]
    return min(widths) if widths else None


def _green_reference_stems(ufo_path: Path, *, exclude_glyph: str) -> list[dict]:
    contents_path = ufo_path / "glyphs" / "contents.plist"
    if not contents_path.exists():
        return []
    with contents_path.open("rb") as handle:
        contents = plistlib.load(handle)
    references = []
    for glyph_name, file_name in sorted(contents.items()):
        if glyph_name == exclude_glyph:
            continue
        path = ufo_path / "glyphs" / file_name
        if not path.exists():
            continue
        root = ET.fromstring(path.read_bytes())
        mark = _glif_mark_color(root)
        if not mark:
            continue
        try:
            if not mark_color_matches(mark, "green"):
                continue
        except ValueError:
            continue
        contour_reports = []
        for contour in root.findall("./outline/contour"):
            points = [
                (float(point.get("x", "0")), float(point.get("y", "0")))
                for point in contour.findall("point")
            ]
            contour_reports.append({"bbox": _points_bbox(points)})
        stem_width = _stem_width_proxy(contour_reports)
        if stem_width is not None:
            references.append({"glyph": glyph_name, "stemWidth": stem_width})
    return references


def _glif_mark_color(root: ET.Element) -> str | None:
    children = list(root.findall("./lib/dict/*"))
    for index, child in enumerate(children):
        if child.tag == "key" and child.text == "public.markColor" and index + 1 < len(children):
            return children[index + 1].text or ""
    return None


def run_comfy_cloud_quiver_svg(
    *,
    trace_request: str,
    workflow_api_json: str,
    image_node_id: str,
    image_input_name: str,
    svg_output_node_id: str = "",
    api_key: str = "",
    base_url: str = "https://cloud.comfy.org",
    timeout_seconds: int = 300,
    poll_interval_seconds: float = 5.0,
) -> tuple[str, dict]:
    artifact = load_glyph_trace_request(Path(trace_request))
    key = (api_key or os.environ.get("COMFY_CLOUD_API_KEY", "")).strip()
    if not key:
        raise ValueError("Comfy Cloud API key required; set COMFY_CLOUD_API_KEY or pass api_key")
    workflow = _load_workflow_api_json(workflow_api_json)
    if not image_node_id.strip():
        raise ValueError("image_node_id required")
    if not image_input_name.strip():
        raise ValueError("image_input_name required")
    node = workflow.get(image_node_id)
    if not isinstance(node, dict):
        raise ValueError(f"workflow node not found: {image_node_id}")
    inputs = node.setdefault("inputs", {})
    if not isinstance(inputs, dict):
        raise ValueError(f"workflow node has invalid inputs: {image_node_id}")

    base = base_url.rstrip("/") or "https://cloud.comfy.org"
    upload = _comfy_cloud_upload_image(
        base_url=base,
        api_key=key,
        image_path=artifact.image_path,
    )
    inputs[image_input_name] = upload["filename"]
    submit = _comfy_cloud_json_request(
        "POST",
        f"{base}/api/prompt",
        api_key=key,
        payload={
            "prompt": workflow,
            "extra_data": {
                "api_key_comfy_org": key,
                "runebender_trace_request": artifact.request_id,
            },
        },
    )
    prompt_id = str(submit.get("prompt_id", "")).strip()
    if not prompt_id:
        raise RuntimeError(f"Comfy Cloud did not return prompt_id: {submit}")
    history = _wait_for_comfy_cloud_history(
        base_url=base,
        api_key=key,
        prompt_id=prompt_id,
        timeout_seconds=max(1, int(timeout_seconds)),
        poll_interval_seconds=max(0.1, float(poll_interval_seconds)),
    )
    svg = _extract_svg_from_cloud_history(
        base_url=base,
        api_key=key,
        history=history,
        prompt_id=prompt_id,
        svg_output_node_id=svg_output_node_id.strip(),
    )
    output_path = artifact.request_path.parent / "comfy-cloud-quiver.svg"
    output_path.write_text(svg, encoding="utf-8")
    report = {
        "prompt_id": prompt_id,
        "upload": upload,
        "svg_path": str(output_path),
        "svg_output_node_id": svg_output_node_id.strip(),
        "base_url": base,
    }
    return str(output_path), report


def _load_workflow_api_json(value: str) -> dict:
    raw = value.strip()
    if not raw:
        raise ValueError("workflow_api_json required")
    if not raw.startswith("{"):
        path = Path(raw).expanduser()
        if path.exists() and path.is_file():
            raw = path.read_text(encoding="utf-8")
    parsed = json.loads(raw)
    if not isinstance(parsed, dict):
        raise ValueError("workflow_api_json must be a Comfy API workflow object")
    return parsed


def _comfy_cloud_upload_image(*, base_url: str, api_key: str, image_path: Path) -> dict:
    boundary = "runebender-comfy-boundary"
    body = _multipart_form_data(
        boundary,
        fields={"type": "input", "overwrite": "true"},
        files={
            "image": {
                "filename": image_path.name,
                "content_type": _image_content_type(image_path),
                "data": image_path.read_bytes(),
            },
        },
    )
    response = _comfy_cloud_request(
        "POST",
        f"{base_url}/api/upload/image",
        api_key=api_key,
        body=body,
        content_type=f"multipart/form-data; boundary={boundary}",
    )
    upload = json.loads(response.decode("utf-8"))
    if not isinstance(upload, dict):
        raise RuntimeError(f"unexpected upload response: {upload}")
    filename = str(upload.get("name") or upload.get("filename") or image_path.name)
    return {
        "filename": filename,
        "subfolder": str(upload.get("subfolder", "")),
        "type": str(upload.get("type", "input")),
        "raw": upload,
    }


def _comfy_cloud_json_request(method: str, url: str, *, api_key: str, payload: dict | None = None) -> dict:
    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    response = _comfy_cloud_request(
        method,
        url,
        api_key=api_key,
        body=body,
        content_type="application/json" if body is not None else None,
    )
    parsed = json.loads(response.decode("utf-8"))
    if not isinstance(parsed, dict):
        raise RuntimeError(f"unexpected JSON response: {parsed}")
    return parsed


def _comfy_cloud_request(
    method: str,
    url: str,
    *,
    api_key: str,
    body: bytes | None = None,
    content_type: str | None = None,
    timeout: int = 60,
) -> bytes:
    headers = {"X-API-Key": api_key}
    if content_type:
        headers["Content-Type"] = content_type
    request = urllib_request.Request(url, data=body, method=method, headers=headers)
    try:
        with urllib_request.urlopen(request, timeout=timeout) as response:
            return response.read()
    except urllib_error.HTTPError as exc:
        details = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"Comfy Cloud HTTP {exc.code}: {details}") from exc


def _wait_for_comfy_cloud_history(
    *,
    base_url: str,
    api_key: str,
    prompt_id: str,
    timeout_seconds: int,
    poll_interval_seconds: float,
) -> dict:
    deadline = time.monotonic() + timeout_seconds
    last_status: dict | None = None
    while time.monotonic() < deadline:
        last_status = _comfy_cloud_json_request(
            "GET",
            f"{base_url}/api/job/{urllib_parse.quote(prompt_id)}/status",
            api_key=api_key,
        )
        status = str(last_status.get("status", "")).lower()
        if status == "completed":
            return _comfy_cloud_json_request(
                "GET",
                f"{base_url}/api/history_v2/{urllib_parse.quote(prompt_id)}",
                api_key=api_key,
            )
        if status in {"failed", "cancelled"}:
            raise RuntimeError(f"Comfy Cloud job {status}: {last_status}")
        time.sleep(poll_interval_seconds)
    raise TimeoutError(f"Comfy Cloud job timed out: {prompt_id}; last status {last_status}")


def _extract_svg_from_cloud_history(
    *,
    base_url: str,
    api_key: str,
    history: dict,
    prompt_id: str,
    svg_output_node_id: str = "",
) -> str:
    entry = history.get(prompt_id, history)
    outputs = entry.get("outputs", {}) if isinstance(entry, dict) else {}
    if not isinstance(outputs, dict):
        raise RuntimeError(f"Comfy Cloud history has no outputs: {history}")
    node_items = (
        [(svg_output_node_id, outputs.get(svg_output_node_id))]
        if svg_output_node_id
        else list(outputs.items())
    )
    for _node_id, output in node_items:
        for candidate in _iter_output_candidates(output):
            if isinstance(candidate, str) and "<svg" in candidate.lower():
                return candidate
            if isinstance(candidate, dict):
                filename = str(candidate.get("filename") or candidate.get("name") or "")
                if not filename.lower().endswith(".svg"):
                    continue
                return _comfy_cloud_view_file(
                    base_url=base_url,
                    api_key=api_key,
                    filename=filename,
                    subfolder=str(candidate.get("subfolder", "")),
                    file_type=str(candidate.get("type", "output")),
                )
    raise RuntimeError("Comfy Cloud history did not contain an SVG output")


def _iter_output_candidates(output: object):
    if output is None:
        return
    if isinstance(output, str):
        yield output
        return
    if isinstance(output, list):
        for item in output:
            yield from _iter_output_candidates(item)
        return
    if isinstance(output, dict):
        yield output
        for value in output.values():
            yield from _iter_output_candidates(value)


def _comfy_cloud_view_file(
    *,
    base_url: str,
    api_key: str,
    filename: str,
    subfolder: str,
    file_type: str,
) -> str:
    query = urllib_parse.urlencode({
        "filename": filename,
        "subfolder": subfolder,
        "type": file_type,
    })
    response = _comfy_cloud_request(
        "GET",
        f"{base_url}/api/view?{query}",
        api_key=api_key,
    )
    return response.decode("utf-8")


def _multipart_form_data(boundary: str, *, fields: dict[str, str], files: dict[str, dict]) -> bytes:
    chunks: list[bytes] = []
    for name, value in fields.items():
        chunks.extend([
            f"--{boundary}\r\n".encode("utf-8"),
            f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode("utf-8"),
            str(value).encode("utf-8"),
            b"\r\n",
        ])
    for name, info in files.items():
        chunks.extend([
            f"--{boundary}\r\n".encode("utf-8"),
            (
                f'Content-Disposition: form-data; name="{name}"; '
                f'filename="{info["filename"]}"\r\n'
            ).encode("utf-8"),
            f'Content-Type: {info["content_type"]}\r\n\r\n'.encode("utf-8"),
            info["data"],
            b"\r\n",
        ])
    chunks.append(f"--{boundary}--\r\n".encode("utf-8"))
    return b"".join(chunks)


def _image_content_type(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix in {".jpg", ".jpeg"}:
        return "image/jpeg"
    if suffix == ".webp":
        return "image/webp"
    if suffix == ".bmp":
        return "image/bmp"
    return "image/png"


def trace_quiver_svg_to_candidate(
    font: str,
    trace_request: str,
    *,
    svg_path: str,
    candidate_name: str = "",
    provider: str = "quiver-ai-manual-svg",
    provider_report: dict | None = None,
) -> dict:
    artifact = load_glyph_trace_request(Path(trace_request))
    payload = artifact.payload
    glyph = str(payload["glyph"])
    master = str(payload["master"])
    metrics = payload["metrics"]
    transform_payload = payload["transform"]
    image_payload = payload["image"]
    transform = TraceImageTransform(
        pixel_width=float(image_payload["width"]),
        pixel_height=float(image_payload["height"]),
        design_x=float(transform_payload["designX"]),
        design_y=float(transform_payload["designY"]),
        design_scale_x=float(transform_payload["designScaleX"]),
        design_scale_y=float(transform_payload["designScaleY"]),
    )
    svg_file = Path(svg_path).expanduser()
    if not svg_file.exists() or not svg_file.is_file():
        raise FileNotFoundError(f"Quiver SVG not found: {svg_file}")

    candidate_slot = (candidate_name or "").strip() or make_fork_name(font)
    candidate_slot = fork_slot(font, candidate_slot)
    detach_candidate_slot(candidate_slot)
    candidate_source = source_path(candidate_slot)
    candidate_ufo = _select_trace_source(candidate_source, master)
    glif = svg_to_glif(
        svg_file.read_text(encoding="utf-8"),
        glyph_name=glyph,
        transform=transform,
        width=float(metrics["advanceWidth"]),
        unicode_hex="",
        mark_color="orange",
    )
    glif_path = write_candidate_glif(candidate_ufo, glyph, glif, mark_color="orange")
    report = {
        "success": True,
        "provider": provider,
        "candidate_slot": candidate_slot,
        "trace_request": str(artifact.request_path),
        "request_id": artifact.request_id,
        "glyph": glyph,
        "master": master,
        "svg_path": str(svg_file),
        "glif_path": str(glif_path),
        "mark_color": "orange",
    }
    if provider_report is not None:
        report["provider_report"] = provider_report
    report["score"] = score_candidate_glyph(
        candidate_slot,
        glyph=glyph,
        master=master,
        trace_request=str(artifact.request_path),
    )
    report_path = resolve_slot(candidate_slot) / "glyph-trace-report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    report["report_path"] = str(report_path)
    return report


def write_candidate_glif(ufo_path: Path, glyph_name: str, glif: str, *, mark_color: str = "orange") -> Path:
    glyphs_dir = ufo_path / "glyphs"
    glyphs_dir.mkdir(parents=True, exist_ok=True)
    glif_path = _glyph_file_for_name(ufo_path, glyph_name)
    if glif_path is None:
        glif_path = glyphs_dir / f"{_safe_glyph_file_stem(glyph_name)}.glif"
        contents_path = glyphs_dir / "contents.plist"
        contents = {}
        if contents_path.exists():
            with contents_path.open("rb") as handle:
                contents = plistlib.load(handle)
        contents[glyph_name] = glif_path.name
        with contents_path.open("wb") as handle:
            plistlib.dump(contents, handle)
    glif_path.write_bytes(_glif_with_mark_color(glif, mark_color).encode("utf-8"))
    return glif_path


def svg_to_glif(
    svg: str,
    *,
    glyph_name: str,
    transform: TraceImageTransform,
    width: float,
    unicode_hex: str = "",
    mark_color: str = "orange",
) -> str:
    root = ET.fromstring(svg.encode("utf-8"))
    _reject_unsupported_svg(root)
    min_x, min_y, view_width, view_height = _svg_view_box(root, transform)
    contours: list[list[tuple[float, float, str | None]]] = []
    for path in _svg_paths(root):
        d = (path.get("d") or "").strip()
        if not d:
            continue
        contours.extend(_parse_svg_path(d, transform, (min_x, min_y, view_width, view_height)))
    if not contours:
        raise ValueError("SVG did not contain any path contours")

    glyph = ET.Element("glyph", {"name": glyph_name, "format": "2"})
    if unicode_hex:
        ET.SubElement(glyph, "unicode", {"hex": unicode_hex.upper()})
    ET.SubElement(glyph, "advance", {"width": _format_number(width)})
    outline = ET.SubElement(glyph, "outline")
    for contour_points in contours:
        contour = ET.SubElement(outline, "contour")
        for x, y, point_type in contour_points:
            attrs = {"x": _format_number(x), "y": _format_number(y)}
            if point_type:
                attrs["type"] = point_type
            ET.SubElement(contour, "point", attrs)
    return _glif_with_mark_color(
        ET.tostring(glyph, encoding="utf-8", xml_declaration=True).decode("utf-8"),
        mark_color,
    )


def _reject_unsupported_svg(root: ET.Element) -> None:
    forbidden_tags = {
        "lineargradient",
        "radialgradient",
        "mask",
        "filter",
        "image",
        "text",
        "use",
        "clipPath".lower(),
    }
    allowed_tags = {"svg", "g", "path", "title", "desc", "metadata", "defs"}
    for node in root.iter():
        tag = _local_tag(node.tag)
        if tag in forbidden_tags:
            raise ValueError(f"Unsupported SVG element: {tag}")
        if tag not in allowed_tags:
            raise ValueError(f"Unsupported SVG element: {tag}")
        if node.get("transform"):
            raise ValueError("Unsupported SVG transform")
        style = (node.get("style") or "").lower()
        if any(token in style for token in ("stroke", "mask", "filter", "gradient")):
            raise ValueError("Unsupported SVG style")
        stroke = (node.get("stroke") or "").strip().lower()
        if stroke and stroke != "none":
            raise ValueError("Unsupported SVG stroke")
        if node.get("mask") or node.get("filter"):
            raise ValueError("Unsupported SVG mask/filter")
    for path in _svg_paths(root):
        fill = (path.get("fill") or "").strip().lower()
        if fill == "none":
            raise ValueError("SVG path fill cannot be none")


def _local_tag(tag: str) -> str:
    return tag.rsplit("}", 1)[-1].lower()


def _svg_paths(root: ET.Element) -> list[ET.Element]:
    return [node for node in root.iter() if _local_tag(node.tag) == "path"]


def _svg_view_box(root: ET.Element, transform: TraceImageTransform) -> tuple[float, float, float, float]:
    raw = (root.get("viewBox") or root.get("viewbox") or "").strip()
    if raw:
        values = [float(part) for part in re.split(r"[,\s]+", raw) if part]
        if len(values) != 4 or values[2] == 0 or values[3] == 0:
            raise ValueError("Invalid SVG viewBox")
        return values[0], values[1], values[2], values[3]
    return 0.0, 0.0, transform.pixel_width, transform.pixel_height


def _parse_svg_path(
    d: str,
    transform: TraceImageTransform,
    view_box: tuple[float, float, float, float],
) -> list[list[tuple[float, float, str | None]]]:
    tokens = re.findall(r"[MmLlHhVvCcZz]|[-+]?(?:\d*\.\d+|\d+)(?:[eE][-+]?\d+)?", d)
    contours: list[list[tuple[float, float, str | None]]] = []
    current: list[tuple[float, float, str | None]] = []
    command = ""
    index = 0
    current_x = 0.0
    current_y = 0.0
    start_x = 0.0
    start_y = 0.0

    def read_number() -> float:
        nonlocal index
        if index >= len(tokens) or re.match(r"^[A-Za-z]$", tokens[index]):
            raise ValueError("Malformed SVG path data")
        value = float(tokens[index])
        index += 1
        return value

    def append_point(x: float, y: float, point_type: str | None) -> None:
        current.append((*_svg_point_to_design(x, y, transform, view_box), point_type))

    while index < len(tokens):
        token = tokens[index]
        if re.match(r"^[A-Za-z]$", token):
            command = token
            index += 1
        if not command:
            raise ValueError("SVG path data must start with a command")

        relative = command.islower()
        op = command.upper()
        if op == "M":
            x = read_number()
            y = read_number()
            if relative:
                x += current_x
                y += current_y
            if current:
                contours.append(current)
                current = []
            append_point(x, y, "line")
            current_x, current_y = x, y
            start_x, start_y = x, y
            command = "l" if relative else "L"
        elif op == "L":
            x = read_number()
            y = read_number()
            if relative:
                x += current_x
                y += current_y
            append_point(x, y, "line")
            current_x, current_y = x, y
        elif op == "H":
            x = read_number()
            if relative:
                x += current_x
            append_point(x, current_y, "line")
            current_x = x
        elif op == "V":
            y = read_number()
            if relative:
                y += current_y
            append_point(current_x, y, "line")
            current_y = y
        elif op == "C":
            x1 = read_number()
            y1 = read_number()
            x2 = read_number()
            y2 = read_number()
            x = read_number()
            y = read_number()
            if relative:
                x1 += current_x
                y1 += current_y
                x2 += current_x
                y2 += current_y
                x += current_x
                y += current_y
            append_point(x1, y1, None)
            append_point(x2, y2, None)
            append_point(x, y, "curve")
            current_x, current_y = x, y
        elif op == "Z":
            if current:
                contours.append(current)
                current = []
            current_x, current_y = start_x, start_y
        else:
            raise ValueError(f"Unsupported SVG path command: {command}")
    if current:
        contours.append(current)
    return contours


def _svg_point_to_design(
    x: float,
    y: float,
    transform: TraceImageTransform,
    view_box: tuple[float, float, float, float],
) -> tuple[float, float]:
    min_x, min_y, view_width, view_height = view_box
    pixel_x = (x - min_x) * (transform.pixel_width / view_width)
    pixel_y = (y - min_y) * (transform.pixel_height / view_height)
    return transform.pixel_to_design(pixel_x, pixel_y)


def _safe_glyph_file_stem(glyph_name: str) -> str:
    stem = re.sub(r"[^A-Za-z0-9_.-]+", "_", glyph_name).strip("._")
    return stem or "glyph"


def _glif_with_mark_color(glif: str, mark_color: str) -> str:
    if mark_color not in MARK_COLORS:
        known = ", ".join(sorted(MARK_COLORS))
        raise ValueError(f"Unknown mark color {mark_color!r}; expected one of: {known}")
    root = ET.fromstring(glif.encode("utf-8"))
    lib = root.find("lib")
    if lib is None:
        lib = ET.SubElement(root, "lib")
    dict_node = lib.find("dict")
    if dict_node is None:
        dict_node = ET.SubElement(lib, "dict")

    children = list(dict_node)
    index = 0
    while index < len(children):
        child = children[index]
        if child.tag == "key" and child.text == "public.markColor":
            dict_node.remove(child)
            if index + 1 < len(children):
                dict_node.remove(children[index + 1])
            children = list(dict_node)
            continue
        index += 1

    key = ET.SubElement(dict_node, "key")
    key.text = "public.markColor"
    value = ET.SubElement(dict_node, "string")
    value.text = ",".join(str(component) for component in MARK_COLORS[mark_color])
    return ET.tostring(root, encoding="utf-8", xml_declaration=True).decode("utf-8")
