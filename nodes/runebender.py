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

from .font import SOURCE_KIND_OPTIONS, resolve_font_source
from .workspace import export_slot_text_files, invalidate_workspace_path, write_workspace_text_file


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
    files = export_slot_text_files(slot)
    return web.json_response({"slot": slot, "files": files})


@routes.post("/runebender/workspace/write")
async def write_workspace_file(request):
    data = await request.post()
    path = str(data.get("path", "")).strip()
    text = str(data.get("text", ""))
    if not path:
        raise web.HTTPBadRequest(reason="path required")
    write_workspace_text_file(path, text)
    return web.json_response({"success": True, "path": path})


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
