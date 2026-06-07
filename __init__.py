"""runebender-comfy-nodes — Rust-powered type-design nodes for ComfyUI.

Nodes:
  Runebender   — workspace producer + full-screen glyph editor
                 (Vello + Kurbo via WASM); UFO/designspace by default,
                 Glyphs/glyphspackage as alternates
  CompileFont  — compile a workspace when a backend exists
  FontPreview  — simple specimen renderer for a FONT reference
  GlyphCandidateBuilder — fork a workspace and replace marked glyphs
                 with donor-derived candidates
  BuildGlyphTraceRequest — persist a background trace request artifact
  TraceToCandidate — trace a request into a candidate FONT
  TraceWithQuiverAI — import a QuiverAI SVG into a candidate FONT
  TraceWithComfyCloudQuiverAI — run Comfy Cloud QuiverAI into a candidate
  TraceLocalMaskToCandidate — trace a local model mask into a candidate
  ScoreCandidate — report candidate outline review metrics
  ApplyGlyphCandidates — copy reviewed candidate glyphs back into a
                 target workspace/source
  DrawBot Skia — scriptable DrawBot renderer with IMAGE + MASK outputs
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
from .nodes.apply_glyph_candidates import ApplyGlyphCandidates
from .nodes.glyph_candidate_builder import GlyphCandidateBuilder
from .nodes.glyph_trace import (
    BuildGlyphTraceRequest,
    ScoreCandidate,
    TraceWithComfyCloudQuiverAI,
    TraceLocalMaskToCandidate,
    TraceToCandidate,
    TraceWithQuiverAI,
)
from .nodes.runebender import Runebender
from .nodes.designbot import DesignBot


class LegacyComfyFontDrawBot(FontSpecimen):
    """Compatibility shim for workflows saved with the old comfyfont DrawBot."""

    DEPRECATED = True


NODE_CLASS_MAPPINGS = {
    "CompileFont": CompileFont,
    "FontPreview": FontPreview,
    "FontSpecimen": FontSpecimen,
    "ComfyFontDrawBot": LegacyComfyFontDrawBot,
    "ForkFont": ForkFont,
    "ApplyGlyphCandidates": ApplyGlyphCandidates,
    "GlyphCandidateBuilder": GlyphCandidateBuilder,
    "BuildGlyphTraceRequest": BuildGlyphTraceRequest,
    "TraceToCandidate": TraceToCandidate,
    "TraceWithQuiverAI": TraceWithQuiverAI,
    "TraceWithComfyCloudQuiverAI": TraceWithComfyCloudQuiverAI,
    "TraceLocalMaskToCandidate": TraceLocalMaskToCandidate,
    "ScoreCandidate": ScoreCandidate,
    "Runebender": Runebender,
    "DesignBot": DesignBot,
}

NODE_DISPLAY_NAME_MAPPINGS = {
    "CompileFont": "Compile Font",
    "FontPreview": "Font Preview",
    "FontSpecimen": "DrawBot Skia",
    "ComfyFontDrawBot": "DrawBot Skia (legacy)",
    "ForkFont": "Fork Font",
    "ApplyGlyphCandidates": "Apply Glyph Candidates",
    "GlyphCandidateBuilder": "Glyph Candidate Builder",
    "BuildGlyphTraceRequest": "Build Glyph Trace Request",
    "TraceToCandidate": "Trace To Candidate",
    "TraceWithQuiverAI": "Trace With QuiverAI",
    "TraceWithComfyCloudQuiverAI": "Trace With Comfy Cloud QuiverAI",
    "TraceLocalMaskToCandidate": "Trace Local Mask To Candidate",
    "ScoreCandidate": "Score Candidate",
    "Runebender": "Runebender",
    "DesignBot": "DesignBot",
}

WEB_DIRECTORY = "./web/dist"

__all__ = ["NODE_CLASS_MAPPINGS", "NODE_DISPLAY_NAME_MAPPINGS", "WEB_DIRECTORY"]
