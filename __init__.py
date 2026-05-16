"""runebender-comfy-nodes — Rust-powered type-design nodes for ComfyUI.

Nodes:
  Font         — workspace creator / importer for FONT wires
                 (UFO/designspace default, Glyphs alternate)
  CompileFont  — compile a workspace when a backend exists
  FontPreview  — simple specimen renderer for a FONT reference
  FontSpecimen — scriptable specimen renderer with IMAGE + MASK outputs
  Runebender   — full-screen glyph editor (Vello + Kurbo via WASM)
  DesignBot    — DrawBot/Processing-style 2D graphics scripting
"""

from __future__ import annotations

from .nodes.font import Font
from .nodes.compile_font import CompileFont
from .nodes.font_preview import FontPreview
from .nodes.font_specimen import FontSpecimen
from .nodes.fork_font import ForkFont
from .nodes.runebender import Runebender
from .nodes.designbot import DesignBot

NODE_CLASS_MAPPINGS = {
    "Font": Font,
    "CompileFont": CompileFont,
    "FontPreview": FontPreview,
    "FontSpecimen": FontSpecimen,
    "ForkFont": ForkFont,
    "Runebender": Runebender,
    "DesignBot": DesignBot,
}

NODE_DISPLAY_NAME_MAPPINGS = {
    "Font": "Load Font",
    "CompileFont": "Compile Font",
    "FontPreview": "Font Preview",
    "FontSpecimen": "Specimen",
    "ForkFont": "Fork Font",
    "Runebender": "Runebender",
    "DesignBot": "DesignBot",
}

WEB_DIRECTORY = "./web/dist"

__all__ = ["NODE_CLASS_MAPPINGS", "NODE_DISPLAY_NAME_MAPPINGS", "WEB_DIRECTORY"]
