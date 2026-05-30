"""Load and normalize a FONT workspace reference.

This node creates the graph-level reference for a type-design
workspace. Downstream nodes should treat the returned string as a
stable workspace reference, not as raw glyph bytes. UFO/designspace is
the default source path; Glyphs is supported as an alternate source
format.
"""

from __future__ import annotations

import asyncio
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path

from aiohttp import web

from server import PromptServer

from .font_preview import render_preview_png, render_workspace_preview_png
from .workspace import (
    clear_workspace_slots,
    compile_slot,
    create_slot_from_path,
    list_workspace_choices,
    locate_source_root,
    resolve_slot,
    source_display_label,
    slot_from_name,
)


def _preview_log(message: str, /, **fields) -> None:
    """Single structured-log point for the preview route. Lets us trace
    requests end-to-end in the ComfyUI terminal when the browser shows
    'Loading preview...' but never resolves."""
    if fields:
        parts = [f"{key}={value!r}" for key, value in fields.items()]
        print(f"[runebender preview] {message} {' '.join(parts)}", flush=True)
    else:
        print(f"[runebender preview] {message}", flush=True)

SOURCE_KIND_OPTIONS = ("auto", "ufo/designspace", "glyphs", "glyphspackage")
DEMO_SOURCE_PATH = (
    Path(__file__).resolve().parent.parent
    / "samples"
    / "virtua-grotesk"
    / "VirtuaGrotesk.designspace"
)

routes = PromptServer.instance.routes


@routes.post("/runebender/import_font")
async def import_font(request):
    data = await request.post()
    workspace_name = str(data.get("workspace_name", "")).strip()
    source_kind = str(data.get("source_kind", "auto")).strip().lower()
    upload_fields = data.getall("file") if hasattr(data, "getall") else []
    if not upload_fields:
        raise web.HTTPBadRequest(reason="file required")

    with tempfile.TemporaryDirectory(prefix="runebender-import-") as tmp:
        staging_root = Path(tmp)
        for field in upload_fields:
            filename = str(getattr(field, "filename", "") or getattr(field, "name", "")).strip()
            if not filename:
                continue
            upload_path = Path(filename)
            if upload_path.is_absolute() or ".." in upload_path.parts:
                raise web.HTTPBadRequest(reason=f"invalid upload path: {filename!r}")
            target = staging_root / upload_path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(field.file.read())

        source_root = locate_source_root(staging_root)
        if source_root is None:
            raise web.HTTPBadRequest(reason="no importable font source found")

        slot = create_slot_from_path(
            str(source_root),
            workspace_name or None,
            source_kind=None if source_kind in {"", "auto"} else source_kind,
        )
        loop = asyncio.get_running_loop()
        try:
            await loop.run_in_executor(None, lambda: compile_slot(slot))
        except Exception as exc:
            print(f"[runebender] eager compile failed for slot {slot!r}: {exc}", flush=True)
        return web.json_response(
            {
                "success": True,
                "slot": slot,
                "source_root": str(source_root.relative_to(staging_root.resolve())),
            }
        )


@routes.post("/runebender/link_source")
async def link_source(request):
    data = await request.post()
    source_path = str(data.get("source_path", "")).strip()
    workspace_name = str(data.get("workspace_name", "")).strip()
    source_kind = str(data.get("source_kind", "auto")).strip().lower()
    if not source_path:
        raise web.HTTPBadRequest(reason="source_path required")

    candidate = Path(source_path).expanduser()
    if candidate.exists() and candidate.is_dir():
        located = locate_source_root(candidate)
        if located is not None:
            candidate = located

    slot = create_slot_from_path(
        str(candidate),
        workspace_name or None,
        source_kind=None if source_kind in {"", "auto"} else source_kind,
        linked=True,
    )
    loop = asyncio.get_running_loop()
    try:
        await loop.run_in_executor(None, lambda: compile_slot(slot))
    except Exception as exc:
        print(f"[runebender] eager compile failed for slot {slot!r}: {exc}", flush=True)
    slot_info = slot_from_name(slot)
    return web.json_response(
        {
            "success": True,
            "slot": slot,
            "label": source_display_label(slot),
            "origin_source": str(candidate),
            "source_root": str(candidate),
            "source_kind": slot_info.source_kind if slot_info else "ufo/designspace",
        }
    )


@routes.post("/runebender/import_source_path")
async def import_source_path(request):
    data = await request.post()
    source_path = str(data.get("source_path", "")).strip()
    workspace_name = str(data.get("workspace_name", "")).strip()
    source_kind = str(data.get("source_kind", "auto")).strip().lower()
    if not source_path:
        raise web.HTTPBadRequest(reason="source_path required")

    candidate = Path(source_path).expanduser()
    if candidate.exists() and candidate.is_dir():
        located = locate_source_root(candidate)
        if located is not None:
            candidate = located

    slot = create_slot_from_path(
        str(candidate),
        workspace_name or None,
        source_kind=None if source_kind in {"", "auto"} else source_kind,
        linked=False,
    )
    loop = asyncio.get_running_loop()
    try:
        await loop.run_in_executor(None, lambda: compile_slot(slot))
    except Exception as exc:
        print(f"[runebender] eager compile failed for slot {slot!r}: {exc}", flush=True)
    slot_info = slot_from_name(slot)
    return web.json_response(
        {
            "success": True,
            "slot": slot,
            "source_root": str(candidate),
            "source_kind": slot_info.source_kind if slot_info else "ufo/designspace",
            "linked_source": False,
        }
    )


@routes.post("/runebender/choose_source")
async def choose_source(request):
    data = await request.post()
    mode = str(data.get("mode", "source")).strip().lower()
    if mode not in {"source", "folder"}:
        raise web.HTTPBadRequest(reason="mode must be 'source' or 'folder'")
    if sys.platform != "darwin":
        raise web.HTTPBadRequest(reason="native source picker is currently available only on macOS")

    if mode == "source":
        script = 'POSIX path of (choose file with prompt "Choose a font source")'
    else:
        script = 'POSIX path of (choose folder with prompt "Choose a destination folder")'
    result = subprocess.run(
        ["osascript", "-e", script],
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        if "User canceled" in detail or "(-128)" in detail:
            return web.json_response({"success": False, "cancelled": True, "path": ""})
        raise web.HTTPBadRequest(reason=detail or "source picker failed")

    return web.json_response({"success": True, "path": result.stdout.strip()})


@routes.get("/runebender/workspaces")
async def list_workspaces(request):
    choices = [{"slot": "demo", "label": "demo", "origin_source": ""}]
    choices.extend(list_workspace_choices())
    return web.json_response({
        "slots": [choice["slot"] for choice in choices],
        "choices": choices,
    })


@routes.post("/runebender/workspaces/clear")
async def clear_workspaces(request):
    cleared = clear_workspace_slots()
    choices = [{"slot": "demo", "label": "demo", "origin_source": ""}]
    return web.json_response({
        "success": True,
        "deleted": cleared,
        "slots": [choice["slot"] for choice in choices],
        "choices": choices,
    })


@routes.get("/runebender/workspace/{slot}/preview")
async def preview_workspace_slot(request):
    slot = str(request.match_info.get("slot", "")).strip()
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    text = str(request.query.get("text", "Aa"))
    width = int(request.query.get("width", "320"))
    height = int(request.query.get("height", "180"))
    request_started = time.monotonic()
    _preview_log("request", slot=slot, text=text, width=width, height=height)

    try:
        if slot == "demo" or slot == "ufo/designspace":
            slot = create_slot_from_path(str(DEMO_SOURCE_PATH), "demo", source_kind=None)
            _preview_log("resolved demo alias", slot=slot)
        if Path(slot).exists():
            try:
                maybe_slot = create_slot_from_path(slot, None, source_kind=None)
                slot = maybe_slot
                _preview_log("imported from path", slot=slot)
            except Exception as exc:
                _preview_log("path import failed (continuing)", error=str(exc))

        slot_info = slot_from_name(slot)
        if slot_info is None:
            _preview_log("slot not found, returning placeholder", slot=slot)
            png = render_preview_png(None, text, width, height)
            return web.Response(body=png, content_type="image/png")

        loop = asyncio.get_running_loop()

        # Eager compile: if the slot has source files but no TTF yet,
        # build one via fontc so drawbot-skia can render. Skipped when
        # the TTF already exists (compile_slot is itself idempotent
        # when not forced).
        if slot_info.compiled_path is None and slot_info.source_path is not None:
            _preview_log("compiling for drawbot-skia preview", slot=slot)
            try:
                await loop.run_in_executor(None, lambda: compile_slot(slot))
                slot_info = slot_from_name(slot)
            except Exception as exc:
                _preview_log("compile failed (will fall back to direct skia)", slot=slot, error=str(exc))
        elif slot_info.source_path is None and slot_info.compiled_path is None:
            _preview_log("slot has no source or compiled artifact; attempting compile", slot=slot)
            try:
                await loop.run_in_executor(None, lambda: compile_slot(slot))
                slot_info = slot_from_name(slot)
            except Exception as exc:
                _preview_log("compile failed", error=str(exc))

        slot_dir = resolve_slot(slot)
        source_path = str(slot_info.source_path) if slot_info and slot_info.source_path else None
        compiled_path = (
            str(slot_info.compiled_path) if slot_info and slot_info.compiled_path else None
        )
        _preview_log(
            "resolved",
            slot=slot,
            slot_dir=str(slot_dir),
            source=source_path,
            compiled=compiled_path,
        )

        render_started = time.monotonic()
        try:
            if slot_info and slot_info.compiled_path:
                _preview_log("render path=drawbot+ttf", slot=slot, font=compiled_path)
                ttf = slot_info.compiled_path
                png = await loop.run_in_executor(
                    None,
                    lambda: render_workspace_preview_png(
                        slot_dir, text, width, height, ttf_path=ttf
                    ),
                )
            elif slot_info and slot_info.source_path:
                _preview_log("render path=direct_skia_from_ufo", slot=slot)
                png = await loop.run_in_executor(
                    None,
                    lambda: render_workspace_preview_png(slot_dir, text, width, height),
                )
            else:
                _preview_log("render path=workspace_source_fallback", slot=slot)
                png = await loop.run_in_executor(
                    None,
                    lambda: render_workspace_preview_png(slot_dir, text, width, height),
                )
        except Exception as exc:
            _preview_log(
                "render failed; returning placeholder",
                slot=slot,
                error=str(exc),
                traceback=traceback.format_exc(),
            )
            png = render_preview_png(None, text, width, height)

        render_ms = int((time.monotonic() - render_started) * 1000)
        total_ms = int((time.monotonic() - request_started) * 1000)
        _preview_log(
            "response",
            slot=slot,
            bytes=len(png) if isinstance(png, (bytes, bytearray)) else None,
            render_ms=render_ms,
            total_ms=total_ms,
        )
        return web.Response(body=png, content_type="image/png")
    except Exception as exc:
        _preview_log(
            "preview route crashed",
            slot=slot,
            error=str(exc),
            traceback=traceback.format_exc(),
        )
        # Send a real 500 so the frontend fetch resolves rather than
        # hanging until the JS-side abort timeout fires.
        raise web.HTTPInternalServerError(reason=f"preview render failed: {exc}")


class Font:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "source_path": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "demo",
                        "tooltip": "Use 'demo' for the bundled sample font, or enter an absolute path to a .designspace, .ufo, .glyphs, or .glyphspackage source.",
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
            },
        }

    RETURN_TYPES = ("FONT",)
    RETURN_NAMES = ("font",)
    FUNCTION = "run"

    def run(self, source_path: str, source_kind: str = "auto", workspace_name: str = ""):
        return (resolve_font_source(source_path, source_kind, workspace_name),)


def resolve_font_source(
    source_path: str,
    source_kind: str = "auto",
    workspace_name: str = "",
    link_source_paths: bool = True,
) -> str:
    """Resolve a user-entered source_path into a workspace slot.

    Centralized so both the (legacy) Font node and the merged Runebender
    node share the same path-to-slot rules.
    """
    source_path = (source_path or "").strip()
    if not source_path:
        source_path = "demo"

    original_source_path = source_path
    is_demo_alias = source_path.lower() in {"demo", "ufo/designspace"}
    if source_path.lower() == "demo" or source_path.lower() == "ufo/designspace":
        source_path = str(DEMO_SOURCE_PATH)
    elif source_path.startswith("workspace://"):
        source_path = source_path.removeprefix("workspace://").strip()

    if source_path and slot_from_name(source_path) is not None and not Path(source_path).expanduser().exists():
        return source_path

    candidate = Path(source_path).expanduser()
    if candidate.exists() and candidate.is_dir():
        located = locate_source_root(candidate)
        if located is not None:
            source_path = str(located)
            candidate = Path(source_path).expanduser()

    source_kind = (source_kind or "auto").strip().lower()
    if source_kind in {"", "auto"}:
        source_kind = None
    workspace_name = (workspace_name or "").strip()
    should_link = (
        link_source_paths
        and not is_demo_alias
        and not str(original_source_path).startswith("workspace://")
        and candidate.exists()
        and candidate.suffix.lower() in {".designspace", ".ufo"}
        and (source_kind is None or source_kind == "ufo/designspace")
    )
    slot = create_slot_from_path(
        source_path,
        workspace_name or None,
        source_kind=source_kind,
        linked=should_link,
    )
    # Eagerly produce the TTF via fontc so the drawbot-skia preview
    # and any downstream FONT consumer can find it immediately.
    # Best-effort: a compile failure leaves the workspace usable (the
    # editor opens fine on the raw UFO; the preview falls back to
    # direct-skia rendering from the UFO).
    try:
        compile_slot(slot)
    except Exception as exc:
        print(f"[runebender] eager compile failed for slot {slot!r}: {exc}", flush=True)
    return slot
