"""Load and normalize a FONT workspace reference.

This node creates the graph-level reference for a type-design
workspace. Downstream nodes should treat the returned string as a
stable workspace reference, not as raw glyph bytes. UFO/designspace is
the default source path; Glyphs is supported as an alternate source
format.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

from aiohttp import web

from server import PromptServer

from .font_preview import render_preview_png, render_workspace_preview_png
from .workspace import compile_slot, create_slot_from_path, list_slots, locate_source_root, resolve_slot, slot_from_name

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
        try:
            compile_slot(slot)
        except Exception:
            pass
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
    try:
        compile_slot(slot)
    except Exception:
        pass
    slot_info = slot_from_name(slot)
    return web.json_response(
        {
            "success": True,
            "slot": slot,
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
    try:
        compile_slot(slot)
    except Exception:
        pass
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
    mode = str(data.get("mode", "file")).strip().lower()
    if mode not in {"file", "folder"}:
        raise web.HTTPBadRequest(reason="mode must be 'file' or 'folder'")
    if sys.platform != "darwin":
        raise web.HTTPBadRequest(reason="native source picker is currently available only on macOS")

    if mode == "folder":
        script = 'POSIX path of (choose folder with prompt "Choose a folder containing a font source")'
    else:
        script = 'POSIX path of (choose file with prompt "Choose a .designspace, .glyphs, or font source file")'
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
    return web.json_response({"slots": ["demo", *list_slots()]})


@routes.get("/runebender/workspace/{slot}/preview")
async def preview_workspace_slot(request):
    slot = str(request.match_info.get("slot", "")).strip()
    if not slot:
        raise web.HTTPBadRequest(reason="slot required")
    text = str(request.query.get("text", "Aa"))
    width = int(request.query.get("width", "320"))
    height = int(request.query.get("height", "180"))
    if slot == "demo" or slot == "ufo/designspace":
        slot = create_slot_from_path(str(DEMO_SOURCE_PATH), "demo", source_kind=None)
    if Path(slot).exists():
        try:
            maybe_slot = create_slot_from_path(slot, None, source_kind=None)
            slot = maybe_slot
        except Exception:
            pass
    slot_info = slot_from_name(slot)
    if slot_info is not None and slot_info.source_path is None and slot_info.compiled_path is None:
        try:
            compile_slot(slot)
        except Exception:
            pass
    try:
        slot_info = slot_from_name(slot)
        if slot_info and slot_info.source_path:
            png = render_workspace_preview_png(resolve_slot(slot), text, width, height)
        elif slot_info and slot_info.compiled_path:
            png = render_preview_png(slot_info.compiled_path, text, width, height)
        else:
            png = render_workspace_preview_png(resolve_slot(slot), text, width, height)
    except Exception:
        png = render_preview_png(None, text, width, height)
    return web.Response(body=png, content_type="image/png")


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


def resolve_font_source(source_path: str, source_kind: str = "auto", workspace_name: str = "") -> str:
    """Resolve a user-entered source_path into a workspace slot.

    Centralized so both the (legacy) Font node and the merged Runebender
    node share the same path-to-slot rules.
    """
    source_path = (source_path or "").strip()
    if not source_path:
        source_path = "demo"

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

    source_kind = (source_kind or "auto").strip().lower()
    if source_kind in {"", "auto"}:
        source_kind = None
    workspace_name = (workspace_name or "").strip()
    return create_slot_from_path(source_path, workspace_name or None, source_kind=source_kind)
