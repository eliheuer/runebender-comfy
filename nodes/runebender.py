"""Runebender — workspace producer + full-screen glyph editor node.

This is the merged producer/editor: it owns the workspace selection
widgets (so it can stand alone as a FONT producer) and also accepts an
optional incoming FONT input for users who want to drive it from
another producer in the graph. The editor itself is a Vue widget in
`web/` backed by a Vello+Kurbo WASM module in `rust-core/`.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
from dataclasses import dataclass
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from aiohttp import web

from server import PromptServer

from .font import DEMO_SOURCE_PATH, SOURCE_KIND_OPTIONS, resolve_font_source
from .glyph_candidate_builder import read_sources
from .workspace import (
    FONTS_DIR,
    compile_slot,
    create_slot_from_path,
    export_slot_to_directory,
    export_slot_text_files,
    invalidate_workspace_path,
    refresh_linked_slot_from_source_if_newer,
    resolve_slot,
    source_path,
    source_info_for_slot,
    write_workspace_text_file_with_result,
)

IMG2BEZ_DEFAULT_LSB = 64.0


@dataclass(frozen=True)
class TraceBackgroundResult:
    glyph: str
    glif: str
    source_ufo: Path
    command: list[str]
    trace_tool: str = "img2bez"


@dataclass(frozen=True)
class LocalTraceTool:
    name: str
    command: list[str]
    cwd: Path | None = None


@dataclass(frozen=True)
class GlyphTraceRequestArtifact:
    request_id: str
    request_path: Path
    image_path: Path
    payload: dict


@dataclass(frozen=True)
class TraceImageTransform:
    pixel_width: float
    pixel_height: float
    design_x: float
    design_y: float
    design_scale_x: float
    design_scale_y: float

    @property
    def design_width(self) -> float:
        return self.pixel_width * self.design_scale_x

    @property
    def design_height(self) -> float:
        return self.pixel_height * self.design_scale_y

    def pixel_to_design(self, x: float, y: float) -> tuple[float, float]:
        return (
            self.design_x + x * self.design_scale_x,
            self.design_y + (self.pixel_height - y) * self.design_scale_y,
        )

    def design_to_pixel(self, x: float, y: float) -> tuple[float, float]:
        return (
            (x - self.design_x) / self.design_scale_x,
            self.pixel_height - ((y - self.design_y) / self.design_scale_y),
        )

    def trace_target_height(self) -> float:
        return max(1.0, abs(self.design_height))

    def snapped_origin(self, grid: int) -> tuple[float, float]:
        snap = max(grid, 1)
        return (
            round(self.design_x / snap) * snap,
            round(self.design_y / snap) * snap,
        )


def _safe_trace_path_part(value: str, fallback: str) -> str:
    part = re.sub(r"[^A-Za-z0-9_.-]+", "-", value.strip()).strip("-._")
    return part or fallback


def write_glyph_trace_request(
    *,
    slot: str,
    glyph: str,
    master: str,
    image_bytes: bytes,
    image_suffix: str,
    transform: TraceImageTransform,
    advance_width: float,
    units_per_em: float,
    ascender: float,
    descender: float,
) -> GlyphTraceRequestArtifact:
    if not slot.strip():
        raise ValueError("slot required")
    if not glyph.strip():
        raise ValueError("glyph required")
    if not master.strip():
        raise ValueError("master required")
    if not image_bytes:
        raise ValueError("image required")
    if transform.pixel_width <= 0 or transform.pixel_height <= 0:
        raise ValueError("image dimensions must be positive")
    if abs(transform.design_scale_x) < 0.000001 or abs(transform.design_scale_y) < 0.000001:
        raise ValueError("image design scale must be nonzero")
    if advance_width <= 0:
        raise ValueError("advance width must be positive")
    if units_per_em <= 0:
        raise ValueError("units per em must be positive")

    slot_dir = resolve_slot(slot)
    image_hash = hashlib.sha256(image_bytes).hexdigest()
    suffix = image_suffix.lower()
    if suffix not in {".png", ".jpg", ".jpeg", ".bmp", ".webp", ".svg"}:
        suffix = ".png"

    request_dir = (
        slot_dir
        / "trace-requests"
        / _safe_trace_path_part(master, "master")
        / _safe_trace_path_part(glyph, "glyph")
    )
    request_dir.mkdir(parents=True, exist_ok=True)
    image_path = request_dir / f"background{suffix}"
    request_path = request_dir / "request.json"
    image_path.write_bytes(image_bytes)

    payload = {
        "version": 1,
        "slot": slot,
        "glyph": glyph,
        "master": master,
        "requestId": image_hash[:16],
        "image": {
            "path": str(image_path.relative_to(slot_dir)),
            "width": transform.pixel_width,
            "height": transform.pixel_height,
            "sha256": image_hash,
        },
        "transform": {
            "designX": transform.design_x,
            "designY": transform.design_y,
            "designScaleX": transform.design_scale_x,
            "designScaleY": transform.design_scale_y,
        },
        "metrics": {
            "advanceWidth": advance_width,
            "unitsPerEm": units_per_em,
            "ascender": ascender,
            "descender": descender,
        },
    }
    request_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return GlyphTraceRequestArtifact(
        request_id=payload["requestId"],
        request_path=request_path,
        image_path=image_path,
        payload=payload,
    )


def load_glyph_trace_request(request_path: Path) -> GlyphTraceRequestArtifact:
    payload = json.loads(request_path.read_text(encoding="utf-8"))
    if payload.get("version") != 1:
        raise ValueError("unsupported trace request version")
    slot = str(payload.get("slot", "")).strip()
    if not slot:
        raise ValueError("slot required")
    slot_dir = resolve_slot(slot)
    image_payload = payload.get("image") or {}
    image_rel = str(image_payload.get("path", "")).strip()
    if not image_rel:
        raise ValueError("image path required")
    image_path = slot_dir / image_rel
    if not image_path.exists():
        raise FileNotFoundError(f"trace request image not found: {image_path}")
    request_id = str(payload.get("requestId") or image_payload.get("sha256", ""))[:16]
    if not request_id:
        raise ValueError("requestId required")
    return GlyphTraceRequestArtifact(
        request_id=request_id,
        request_path=request_path,
        image_path=image_path,
        payload=payload,
    )


def _format_number(value: float | int) -> str:
    number = float(value)
    if number.is_integer():
        return str(int(number))
    return f"{number:.6f}".rstrip("0").rstrip(".")


def _parse_float(value: object, default: float) -> float:
    try:
        parsed = float(str(value))
    except (TypeError, ValueError):
        return default
    return parsed if parsed == parsed else default


def _parse_int(value: object, default: int) -> int:
    try:
        return int(float(str(value)))
    except (TypeError, ValueError):
        return default


def _parse_bool(value: object, default: bool = False) -> bool:
    if value is None:
        return default
    text = str(value).strip().lower()
    if not text:
        return default
    return text in {"1", "true", "yes", "on"}


def _resolve_configured_trace_tool(env_name: str) -> LocalTraceTool | None:
    configured = os.environ.get(env_name, "").strip()
    if configured:
        configured_path = Path(configured).expanduser()
        if configured_path.exists():
            return LocalTraceTool(env_name, [str(configured_path)])
        found = shutil.which(configured)
        if found:
            return LocalTraceTool(env_name, [found])
    return None


def _resolve_trace_tool() -> LocalTraceTool:
    configured = _resolve_configured_trace_tool("RUNEBENDER_TRACE_TOOL")
    if configured is not None:
        return configured
    configured = _resolve_configured_trace_tool("RUNEBENDER_IMG2BEZ")
    if configured is not None:
        return configured

    sibling = Path.home() / "GH" / "repos" / "img2bez"
    release_bin = sibling / "target" / "release" / "img2bez"
    if release_bin.exists():
        return LocalTraceTool("img2bez", [str(release_bin)], cwd=sibling)
    manifest = sibling / "Cargo.toml"
    if manifest.exists():
        return LocalTraceTool(
            "img2bez",
            ["cargo", "run", "--quiet", "--manifest-path", str(manifest), "--"],
            cwd=sibling,
        )
    explicit = shutil.which(str(Path.home() / ".cargo" / "bin" / "img2bez"))
    env_path = shutil.which("img2bez")
    if env_path:
        return LocalTraceTool("img2bez", [env_path])
    if explicit:
        return LocalTraceTool("img2bez", [explicit])
    raise FileNotFoundError(
        "No local trace tool was found. Set RUNEBENDER_TRACE_TOOL to an "
        "img2bez-compatible Rust tracer, set RUNEBENDER_IMG2BEZ, build "
        "~/GH/repos/img2bez, or put img2bez on PATH."
    )


def _resolve_img2bez_command() -> list[str]:
    return _resolve_trace_tool().command


def _select_trace_source(font_source: Path, master_name: str) -> Path:
    sources = read_sources(font_source)
    if not sources:
        raise ValueError(f"No UFO sources found in {font_source}")
    wanted = master_name.strip().lower()
    if wanted:
        for source in sources:
            if source.style.strip().lower() == wanted:
                return source.path
        for source in sources:
            if source.path.stem.strip().lower() == wanted:
                return source.path
    return sources[0].path


def _glyph_file_for_name(ufo_path: Path, glyph_name: str) -> Path | None:
    import plistlib

    contents_path = ufo_path / "glyphs" / "contents.plist"
    if not contents_path.exists():
        return None
    with contents_path.open("rb") as handle:
        contents = plistlib.load(handle)
    file_name = contents.get(glyph_name)
    if not file_name:
        return None
    return ufo_path / "glyphs" / file_name


def _translate_glif_x(data: bytes, dx: float) -> bytes:
    if abs(dx) < 0.000001:
        return data
    root = ET.fromstring(data)
    for point in root.iter("point"):
        raw = point.get("x")
        if raw is None:
            continue
        try:
            point.set("x", _format_number(float(raw) + dx))
        except ValueError:
            continue
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def trace_background_with_img2bez(
    *,
    slot: str,
    master_name: str,
    glyph_name: str,
    image_bytes: bytes,
    image_suffix: str,
    unicode_hex: str,
    width: float,
    target_height: float,
    y_offset: float,
    x_offset: float,
    grid: int = 2,
    accuracy: float = 2.0,
    smooth: int = 0,
    alphamax: float = 0.8,
    global_fit: bool = False,
    invert: bool = False,
    threshold: int | None = None,
) -> TraceBackgroundResult:
    if not slot:
        raise ValueError("slot required")
    if not glyph_name:
        raise ValueError("glyph required")
    if not image_bytes:
        raise ValueError("image required")

    font_source = source_path(slot)
    source_ufo = _select_trace_source(font_source, master_name)
    if not source_ufo.exists() or source_ufo.suffix.lower() != ".ufo":
        raise ValueError(f"Trace source is not a UFO: {source_ufo}")

    suffix = image_suffix.lower()
    if suffix not in {".png", ".jpg", ".jpeg", ".bmp"}:
        suffix = ".png"

    trace_tool = _resolve_trace_tool()
    with tempfile.TemporaryDirectory(prefix="runebender-img2bez-") as tmp:
        tmp_dir = Path(tmp)
        temp_ufo = tmp_dir / source_ufo.name
        shutil.copytree(source_ufo, temp_ufo)
        image_path = tmp_dir / f"{glyph_name}{suffix}"
        image_path.write_bytes(image_bytes)

        args = [
            "--input",
            str(image_path),
            "--output",
            str(temp_ufo),
            "--name",
            glyph_name,
            "--width",
            _format_number(width),
            "--target-height",
            _format_number(target_height),
            "--y-offset",
            _format_number(y_offset),
            "--grid",
            str(grid),
            "--accuracy",
            _format_number(accuracy),
            "--smooth",
            str(smooth),
            "--alphamax",
            _format_number(alphamax),
        ]
        if unicode_hex:
            args.extend(["--unicode", unicode_hex.upper()])
        if invert:
            args.append("--invert")
        if global_fit:
            args.append("--global-fit")
        if threshold is not None:
            args.extend(["--threshold", str(threshold)])

        command = trace_tool.command + args
        print(
            f"[runebender] trace subprocess start tool={trace_tool.name!r} "
            f"cwd={str(trace_tool.cwd) if trace_tool.cwd is not None else None!r} "
            f"command={' '.join(command)}",
            flush=True,
        )
        completed = subprocess.run(
            command,
            cwd=str(trace_tool.cwd) if trace_tool.cwd is not None else None,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        print(
            f"[runebender] trace subprocess done returncode={completed.returncode} "
            f"stdout_bytes={len(completed.stdout)} stderr_bytes={len(completed.stderr)}",
            flush=True,
        )
        if completed.returncode != 0:
            stderr = completed.stderr.strip() or completed.stdout.strip()
            raise RuntimeError(f"{trace_tool.name} failed: {stderr}")

        glif_path = _glyph_file_for_name(temp_ufo, glyph_name)
        if glif_path is None or not glif_path.exists():
            raise FileNotFoundError(f"{trace_tool.name} did not write {glyph_name}.glif")
        glif = _translate_glif_x(
            glif_path.read_bytes(),
            round(x_offset / max(grid, 1)) * max(grid, 1) - IMG2BEZ_DEFAULT_LSB,
        ).decode("utf-8")
        print(
            f"[runebender] trace glif ready glyph={glyph_name!r} bytes={len(glif.encode('utf-8'))}",
            flush=True,
        )
        return TraceBackgroundResult(
            glyph=glyph_name,
            glif=glif,
            source_ufo=source_ufo,
            command=command,
            trace_tool=trace_tool.name,
        )


def _ensure_demo_workspace_fresh() -> None:
    """Rebuild the cached 'demo' workspace if the source has changed.

    Lets us swap DEMO_SOURCE_PATH (e.g. to a richer demo font) without
    requiring users to manually delete the cached workspace/fonts/demo
    directory. Compares the mtime of the bundled source against the
    cached slot's directory mtime.
    """
    if not DEMO_SOURCE_PATH.exists():
        return
    slot_dir = FONTS_DIR / "demo"
    try:
        source_mtime = DEMO_SOURCE_PATH.stat().st_mtime
    except OSError:
        return
    if slot_dir.exists():
        try:
            slot_mtime = slot_dir.stat().st_mtime
        except OSError:
            slot_mtime = 0.0
        if slot_mtime >= source_mtime:
            return
    try:
        create_slot_from_path(str(DEMO_SOURCE_PATH), "demo", source_kind=None)
    except Exception as exc:
        # Non-fatal: workspace may still be usable in its previous state.
        # Log and move on so the editor at least opens.
        print(f"[runebender] failed to refresh demo workspace: {exc}")
        return
    # Compile to TTF immediately so the drawbot-skia specimen preview
    # has something to render against on first request. Best-effort —
    # if fontc isn't installed or compilation fails, previews fall
    # back to direct-skia from UFO.
    try:
        compile_slot("demo")
    except Exception as exc:
        print(f"[runebender] eager demo compile failed: {exc}", flush=True)


RUNEBENDER_STATE: dict[str, dict[str, str]] = {}

routes = PromptServer.instance.routes


@routes.post("/runebender/log")
async def forward_browser_log(request):
    """Mirror browser-side runebender console messages into ComfyUI's
    terminal output. Saves the user from constantly opening DevTools
    when they're tracing edit-time loading, preview rendering, etc.
    Only [runebender...]-prefixed messages reach this route (the JS
    side filters before posting), so this won't drown the terminal in
    unrelated browser noise.
    """
    data = await request.post()
    level = str(data.get("level", "info")).lower()
    message = str(data.get("message", ""))
    if not message:
        return web.json_response({"ok": True})
    tag = f"browser {level}"
    print(f"[{tag}] {message}", flush=True)
    return web.json_response({"ok": True})


@routes.post("/runebender/set_state")
async def set_state(request):
    data = await request.post()
    node_id = str(data.get("node_id", ""))
    font = str(data.get("font", ""))
    glyph_data = str(data.get("glyph_data", ""))
    if node_id:
        RUNEBENDER_STATE[node_id] = {
            "font": font,
            "glyph_data": glyph_data,
        }
    return web.json_response({"success": True})


@routes.get("/runebender/workspace/{slot}")
async def get_workspace_slot(request):
    slot = str(request.match_info.get("slot", ""))
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    if slot == "demo":
        _ensure_demo_workspace_fresh()
    refreshed_from_source = refresh_linked_slot_from_source_if_newer(slot)
    files = export_slot_text_files(slot)
    source_info = source_info_for_slot(slot)
    return web.json_response({
        "slot": slot,
        "files": files,
        "linked_source": source_info.linked,
        "origin_root": str(source_info.origin_root) if source_info.origin_root is not None else "",
        "origin_source": str(source_info.origin_source) if source_info.origin_source is not None else "",
        "refreshed_from_source": refreshed_from_source,
    })


@routes.post("/runebender/workspace/write")
async def write_workspace_file(request):
    data = await request.post()
    path = str(data.get("path", "")).strip()
    text = str(data.get("text", ""))
    if not path:
        raise web.HTTPBadRequest(reason="path required")
    result = write_workspace_text_file_with_result(path, text)
    return web.json_response({
        "success": True,
        "path": path,
        "workspace_path": str(result.workspace_path),
        "source_path": str(result.source_path) if result.source_path is not None else "",
        "saved_to_source": result.source_path is not None,
    })


@routes.post("/runebender/workspace/save_as")
async def save_workspace_as(request):
    data = await request.post()
    slot = str(data.get("slot", "")).strip()
    destination = str(data.get("destination", "")).strip()
    relink = str(data.get("relink", "")).strip().lower() in {"1", "true", "yes", "on"}
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    if not destination:
        raise web.HTTPBadRequest(reason="destination required")
    result = export_slot_to_directory(slot, destination, relink=relink)
    return web.json_response({
        "success": True,
        "slot": slot,
        "destination": str(result.destination),
        "copied_paths": [str(path) for path in result.copied_paths],
        "linked_source": result.linked,
        "origin_root": str(result.origin_root) if result.origin_root is not None else "",
        "origin_source": str(result.origin_source) if result.origin_source is not None else "",
    })


@routes.post("/runebender/workspace/invalidate")
async def invalidate_workspace_file(request):
    data = await request.post()
    path = str(data.get("path", "")).strip()
    if not path:
        raise web.HTTPBadRequest(reason="path required")
    invalidate_workspace_path(path)
    return web.json_response({"success": True, "path": path})


@routes.post("/runebender/workspace/trace_background")
async def trace_background_image(request):
    data = await request.post()
    slot = str(data.get("slot", "")).strip()
    master = str(data.get("master", "")).strip()
    glyph = str(data.get("glyph", "")).strip()
    image_field = data.get("image")
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    if not glyph:
        raise web.HTTPBadRequest(reason="glyph required")
    if image_field is None or not hasattr(image_field, "file"):
        raise web.HTTPBadRequest(reason="image required")

    image_file = image_field.file
    image_file.seek(0)
    image_bytes = image_file.read()
    filename = str(getattr(image_field, "filename", "") or "")
    print(
        f"[runebender] trace_background route hit slot={slot!r} master={master!r} "
        f"glyph={glyph!r} image={filename!r} bytes={len(image_bytes)}",
        flush=True,
    )
    suffix = Path(filename).suffix
    threshold_raw = str(data.get("threshold", "")).strip()
    threshold = _parse_int(threshold_raw, -1) if threshold_raw else -1
    grid = max(0, _parse_int(data.get("grid"), 2))
    target_height = _parse_float(data.get("target_height"), 1000.0)
    x_offset = _parse_float(data.get("x_offset"), IMG2BEZ_DEFAULT_LSB)
    y_offset = _parse_float(data.get("y_offset"), 0.0)
    if str(data.get("image_width", "")).strip() and str(data.get("image_height", "")).strip():
        transform = TraceImageTransform(
            pixel_width=_parse_float(data.get("image_width"), 1.0),
            pixel_height=_parse_float(data.get("image_height"), 1.0),
            design_x=_parse_float(data.get("design_x"), x_offset),
            design_y=_parse_float(data.get("design_y"), y_offset),
            design_scale_x=_parse_float(data.get("design_scale_x"), 1.0),
            design_scale_y=_parse_float(data.get("design_scale_y"), 1.0),
        )
        target_height = transform.trace_target_height()
        x_offset, y_offset = transform.snapped_origin(grid)
    try:
        result = await asyncio.to_thread(
            trace_background_with_img2bez,
            slot=slot,
            master_name=master,
            glyph_name=glyph,
            image_bytes=image_bytes,
            image_suffix=suffix,
            unicode_hex=str(data.get("unicode", "")).strip(),
            width=_parse_float(data.get("width"), 600.0),
            target_height=target_height,
            y_offset=y_offset,
            x_offset=x_offset,
            grid=grid,
            accuracy=_parse_float(data.get("accuracy"), 2.0),
            smooth=max(0, _parse_int(data.get("smooth"), 0)),
            alphamax=_parse_float(data.get("alphamax"), 0.8),
            global_fit=_parse_bool(data.get("globalFit"), False),
            invert=_parse_bool(data.get("invert"), False),
            threshold=threshold if 0 <= threshold <= 255 else None,
        )
    except Exception as exc:
        print(f"[runebender] trace_background failed: {exc}", flush=True)
        return web.json_response({"success": False, "error": str(exc)}, status=400)
    print(
        f"[runebender] trace_background success glyph={result.glyph!r} "
        f"glif_bytes={len(result.glif.encode('utf-8'))}",
        flush=True,
    )
    return web.json_response({
        "success": True,
        "glyph": result.glyph,
        "glif": result.glif,
        "source_ufo": str(result.source_ufo),
        "command": result.command,
    })


@routes.post("/runebender/workspace/trace_background_candidate")
async def trace_background_candidate(request):
    data = await request.post()
    slot = str(data.get("slot", "")).strip()
    master = str(data.get("master", "")).strip()
    glyph = str(data.get("glyph", "")).strip()
    image_field = data.get("image")
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    if not glyph:
        raise web.HTTPBadRequest(reason="glyph required")
    if image_field is None or not hasattr(image_field, "file"):
        raise web.HTTPBadRequest(reason="image required")

    image_file = image_field.file
    image_file.seek(0)
    image_bytes = image_file.read()
    filename = str(getattr(image_field, "filename", "") or "")
    grid = max(0, _parse_int(data.get("grid"), 2))
    width = _parse_float(data.get("width"), 600.0)
    transform = TraceImageTransform(
        pixel_width=_parse_float(data.get("image_width"), 1.0),
        pixel_height=_parse_float(data.get("image_height"), 1.0),
        design_x=_parse_float(data.get("design_x"), _parse_float(data.get("x_offset"), 0.0)),
        design_y=_parse_float(data.get("design_y"), _parse_float(data.get("y_offset"), 0.0)),
        design_scale_x=_parse_float(data.get("design_scale_x"), 1.0),
        design_scale_y=_parse_float(data.get("design_scale_y"), 1.0),
    )
    try:
        artifact = await asyncio.to_thread(
            write_glyph_trace_request,
            slot=slot,
            glyph=glyph,
            master=master or "Regular",
            image_bytes=image_bytes,
            image_suffix=Path(filename).suffix,
            transform=transform,
            advance_width=width,
            units_per_em=_parse_float(data.get("units_per_em"), 1000.0),
            ascender=_parse_float(data.get("ascender"), 800.0),
            descender=_parse_float(data.get("descender"), -200.0),
        )
        threshold_raw = str(data.get("threshold", "")).strip()
        threshold = _parse_int(threshold_raw, -1) if threshold_raw else -1
        from .glyph_trace import trace_request_to_candidate

        report = await asyncio.to_thread(
            trace_request_to_candidate,
            slot,
            str(artifact.request_path),
            candidate_name=str(data.get("candidate_name", "")).strip(),
            grid=grid,
            accuracy=_parse_float(data.get("accuracy"), 2.0),
            smooth=max(0, _parse_int(data.get("smooth"), 0)),
            alphamax=_parse_float(data.get("alphamax"), 0.8),
            global_fit=_parse_bool(data.get("globalFit"), True),
            invert=_parse_bool(data.get("invert"), False),
            threshold=threshold if 0 <= threshold <= 255 else None,
            provider="placed-background-img2bez",
        )
    except Exception as exc:
        return web.json_response({"success": False, "error": str(exc)}, status=400)

    return web.json_response({
        "success": True,
        "candidate_slot": report["candidate_slot"],
        "trace_request": str(artifact.request_path),
        "request_id": artifact.request_id,
        "glyph": glyph,
        "master": master,
        "report": report,
    })


class Runebender:
    CATEGORY = "Runebender / Editor"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "source_path": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "",
                        "placeholder": "/path/to/font.designspace",
                        "tooltip": "Use 'demo' for the bundled sample font, or enter a .designspace/.ufo path to open a disk source for editing and save-back. Ignored when a FONT wire is connected.",
                    },
                ),
            },
            "optional": {
                "source_kind": (
                    SOURCE_KIND_OPTIONS,
                    {
                        "default": "auto",
                        "tooltip": "Auto-detect from the file extension unless you need to override it.",
                        "advanced": True,
                    },
                ),
                "workspace_name": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "",
                        "tooltip": "Leave blank to auto-name the workspace from the source file.",
                        "advanced": True,
                    },
                ),
                "font": ("FONT",),
            },
            "hidden": {
                "unique_id": "UNIQUE_ID",
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("font", "glyph_svg")
    FUNCTION = "run"

    def run(
        self,
        source_path: str,
        source_kind: str = "auto",
        workspace_name: str = "",
        font: str | None = None,
        unique_id: str | None = None,
    ):
        node_id = str(unique_id) if unique_id is not None else ""
        state = RUNEBENDER_STATE.get(node_id, {})

        if state.get("font"):
            resolved = state["font"]
        elif font:
            resolved = font
        else:
            resolved = resolve_font_source(source_path, source_kind, workspace_name)

        return (
            resolved,
            state.get("glyph_data") or "",
        )
