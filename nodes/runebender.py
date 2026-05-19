"""Runebender — workspace producer + full-screen glyph editor node.

This is the merged producer/editor: it owns the workspace selection
widgets (so it can stand alone as a FONT producer) and also accepts an
optional incoming FONT input for users who want to drive it from
another producer in the graph. The editor itself is a Vue widget in
`web/` backed by a Vello+Kurbo WASM module in `rust-core/`.
"""

from __future__ import annotations

from aiohttp import web

from server import PromptServer

from .font import DEMO_SOURCE_PATH, SOURCE_KIND_OPTIONS, resolve_font_source
from .workspace import (
    FONTS_DIR,
    create_slot_from_path,
    export_slot_to_directory,
    export_slot_text_files,
    invalidate_workspace_path,
    source_info_for_slot,
    write_workspace_text_file_with_result,
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


RUNEBENDER_STATE: dict[str, dict[str, str]] = {}

routes = PromptServer.instance.routes


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
    files = export_slot_text_files(slot)
    source_info = source_info_for_slot(slot)
    return web.json_response({
        "slot": slot,
        "files": files,
        "linked_source": source_info.linked,
        "origin_root": str(source_info.origin_root) if source_info.origin_root is not None else "",
        "origin_source": str(source_info.origin_source) if source_info.origin_source is not None else "",
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
                        "default": "demo",
                        "tooltip": "Use 'demo' for the bundled sample font, or enter an absolute path to a .designspace, .ufo, .glyphs, or .glyphspackage source. Ignored when a FONT wire is connected.",
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
