"""Compile a FONT workspace reference.

This node is intentionally narrow: it turns a workspace into a
compiled font artifact when the source kind can be compiled directly
with the current backend wiring. The current Google Fonts-oriented path
is glyphspackage -> fontc.
"""

from __future__ import annotations

from .workspace import compile_slot


class CompileFont:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
            },
            "optional": {
                "output_path": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "",
                    },
                ),
                "force": ("BOOLEAN", {"default": False}),
            },
        }

    RETURN_TYPES = ("FONT",)
    RETURN_NAMES = ("font",)
    FUNCTION = "run"

    def run(self, font: str, output_path: str = "", force: bool = False):
        compiled = compile_slot(font, output_path=output_path.strip() or None, force=force)
        return (font,)
