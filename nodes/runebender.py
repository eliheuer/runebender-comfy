"""Runebender — full-screen glyph editor node.

The actual editor is a Vue widget in `web/` backed by a Vello+Kurbo
WASM module in `rust-core/`. This Python class is the ComfyUI graph
endpoint: it surfaces the editor's serialized output (UFO/SVG/contour
JSON) to downstream nodes.
"""

from __future__ import annotations


class Runebender:
    CATEGORY = "type / glyph"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                # Serialized editor state, written by the Vue widget.
                "glyph_data": ("STRING", {"multiline": True, "default": ""}),
            },
        }

    RETURN_TYPES = ("STRING",)
    RETURN_NAMES = ("svg",)
    FUNCTION = "run"

    def run(self, glyph_data: str):
        # TODO: parse glyph_data (UFO/SVG/contour JSON from the Vue widget)
        # and return SVG markup.
        return (glyph_data,)
