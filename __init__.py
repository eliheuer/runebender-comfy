"""runebender-comfy-nodes — Rust-powered type-design nodes for ComfyUI.

Nodes:
  Runebender   — full-screen glyph editor (Vello + Kurbo via WASM)
  DesignBot    — DrawBot/Processing-style 2D graphics scripting
"""

from __future__ import annotations

from .nodes.runebender import Runebender
from .nodes.designbot import DesignBot

NODE_CLASS_MAPPINGS = {
    "Runebender": Runebender,
    "DesignBot": DesignBot,
}

NODE_DISPLAY_NAME_MAPPINGS = {
    "Runebender": "Runebender (glyph editor)",
    "DesignBot": "DesignBot (script → image)",
}

WEB_DIRECTORY = "./web/dist"

__all__ = ["NODE_CLASS_MAPPINGS", "NODE_DISPLAY_NAME_MAPPINGS", "WEB_DIRECTORY"]
