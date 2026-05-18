"""runebender-comfy-nodes — Rust-powered type-design nodes for ComfyUI.

Nodes:
  Runebender   — workspace producer + full-screen glyph editor
                 (Vello + Kurbo via WASM); UFO/designspace by default,
                 Glyphs/glyphspackage as alternates
  CompileFont  — compile a workspace when a backend exists
  FontPreview  — simple specimen renderer for a FONT reference
  FontSpecimen — scriptable specimen renderer with IMAGE + MASK outputs
  ForkFont     — deep-copy a workspace under a new name
  DesignBot    — DrawBot/Processing-style 2D graphics scripting
"""

from __future__ import annotations

# Importing nodes.font registers its aiohttp routes (import_font,
# list_workspaces, preview_workspace_slot) even though Font itself is no
# longer registered as a graph node.
from .nodes import font as _font  # noqa: F401
from .nodes.compile_font import CompileFont
from .nodes.font_preview import FontPreview
from .nodes.font_specimen import FontSpecimen
from .nodes.fork_font import ForkFont
from .nodes.runebender import Runebender
from .nodes.designbot import DesignBot

NODE_CLASS_MAPPINGS = {
    "CompileFont": CompileFont,
    "FontPreview": FontPreview,
    "FontSpecimen": FontSpecimen,
    "ForkFont": ForkFont,
    "Runebender": Runebender,
    "DesignBot": DesignBot,
}

NODE_DISPLAY_NAME_MAPPINGS = {
    "CompileFont": "Compile Font",
    "FontPreview": "Font Preview",
    "FontSpecimen": "Specimen",
    "ForkFont": "Fork Font",
    "Runebender": "Runebender",
    "DesignBot": "DesignBot",
}

WEB_DIRECTORY = "./web/dist"

__all__ = ["NODE_CLASS_MAPPINGS", "NODE_DISPLAY_NAME_MAPPINGS", "WEB_DIRECTORY"]
