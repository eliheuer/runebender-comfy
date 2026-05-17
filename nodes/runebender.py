"""Runebender — full-screen glyph editor node.

The actual editor is a Vue widget in `web/` backed by a Vello+Kurbo
WASM module in `rust-core/`. This Python class is the ComfyUI graph
endpoint: it passes a FONT workspace reference through the graph while
keeping the live editor state available as a side-channel preview.
"""

from __future__ import annotations

from aiohttp import web

from server import PromptServer

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
                "font": ("FONT",),
            },
            "hidden": {
                "unique_id": "UNIQUE_ID",
            },
        }

    RETURN_TYPES = ("FONT", "STRING")
    RETURN_NAMES = ("edited_font", "glyph_svg")
    FUNCTION = "run"

    def run(self, font: str, unique_id: str):
        node_id = str(unique_id)
        state = RUNEBENDER_STATE.get(node_id, {})
        return (
            state.get("font") or font,
            state.get("glyph_data") or "",
        )
