"""DrawBot-powered font specimen rendering node.

Consumes a Runebender ``FONT`` workspace reference and renders an image
with drawbot-skia. Preset scripts live in ``nodes/drawbot_presets`` and
custom scripts can be edited directly in the ComfyUI node.
"""

from __future__ import annotations

import re
import sys
import tempfile
import threading
from pathlib import Path

from aiohttp import web
from server import PromptServer

from .workspace import compile_slot, compiled_path

_DRAWBOT_LOCK = threading.Lock()
_PRESETS_DIR = Path(__file__).resolve().parent / "drawbot_presets"


def _image_stack():
    try:
        import numpy as np
        from PIL import Image
        import torch
    except ImportError as exc:
        raise ImportError(
            "FontSpecimen requires numpy, Pillow, and torch; install them in the ComfyUI Python environment"
        ) from exc
    return np, Image, torch


def _pil_image_module():
    try:
        from PIL import Image
    except ImportError as exc:
        raise ImportError(
            "FontSpecimen requires Pillow; install it in the ComfyUI Python environment"
        ) from exc
    return Image


def _image_to_tensor(img):
    np, _Image, torch = _image_stack()
    arr = np.array(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy(arr).unsqueeze(0)


def _mask_to_tensor(img):
    np, _Image, torch = _image_stack()
    alpha = np.array(img.convert("RGBA").getchannel("A"), dtype=np.float32) / 255.0
    return torch.from_numpy(alpha).unsqueeze(0)


def _name_from_filename(filename: str) -> str:
    stem = Path(filename).stem
    stem = re.sub(r"^\d+_", "", stem)
    return stem.replace("_", " ").title()


def load_presets() -> dict[str, str]:
    """Return ``{display_name: source}`` for bundled DrawBot presets."""
    presets: dict[str, str] = {}
    if not _PRESETS_DIR.is_dir():
        return presets
    for path in sorted(_PRESETS_DIR.glob("*.py")):
        if path.name == "helpers.py":
            continue
        try:
            presets[_name_from_filename(path.name)] = path.read_text(encoding="utf-8")
        except OSError:
            continue
    return presets


routes = PromptServer.instance.routes


@routes.get("/runebender/drawbot_preset")
async def drawbot_preset(request):
    name = request.query.get("name", "")
    if not name:
        raise web.HTTPBadRequest(reason="name required")
    presets = load_presets()
    if name not in presets:
        raise web.HTTPNotFound()
    return web.Response(text=presets[name], content_type="text/plain")


def _resolve_drawbot_font_path(font: str) -> Path:
    candidate = Path(str(font)).expanduser()
    if candidate.exists() and candidate.is_file():
        return candidate

    try:
        return compiled_path(font)
    except Exception:
        pass

    try:
        compile_slot(font)
        return compiled_path(font)
    except Exception as exc:
        raise RuntimeError(
            "FontSpecimen needs a compiled font file for DrawBot rendering. "
            "Run Compile Font first, or use a Runebender source that can compile with fontc."
        ) from exc


def _script_for_preset(preset: str, script_override: str) -> str:
    override = script_override.strip() if script_override else ""
    if override:
        return override

    presets = load_presets()
    if preset not in presets:
        lower = {name.lower(): name for name in presets}
        preset = lower.get(str(preset).lower(), preset)
    if preset not in presets:
        raise ValueError(f"Preset {preset!r} not found. Available presets: {list(presets)}")
    return presets[preset]


def _render_drawbot(script: str, font_path: Path, width: int, height: int, input_text: str):
    try:
        import drawbot_skia.drawbot as db
    except ImportError as exc:
        raise RuntimeError(
            "drawbot-skia is not installed. Install the Runebender fork with: "
            "pip install 'drawbot-skia @ git+https://github.com/eliheuer/drawbot-skia.git'"
        ) from exc

    Image = _pil_image_module()
    namespace: dict[str, object] = {
        "font_path": str(font_path),
        "WIDTH": int(width),
        "HEIGHT": int(height),
        "input_text": input_text,
    }
    for name in dir(db):
        if not name.startswith("_"):
            namespace[name] = getattr(db, name)

    preset_dir = str(_PRESETS_DIR)
    if preset_dir not in sys.path:
        sys.path.insert(0, preset_dir)

    with _DRAWBOT_LOCK:
        db.newDrawing()
        tmp = tempfile.mktemp(suffix=".png")
        try:
            exec(script, namespace)  # noqa: S102 - user-authored local specimen scripts
            db.saveImage(tmp)
            path = Path(tmp)
            if not path.exists():
                path = Path(tmp.replace(".png", "_1.png"))
            if not path.exists():
                raise RuntimeError("DrawBot did not produce a PNG image")
            return Image.open(path).convert("RGBA")
        except Exception as exc:
            raise RuntimeError(f"DrawBot script error: {exc}") from exc
        finally:
            try:
                db.endDrawing()
            finally:
                for candidate in (Path(tmp), Path(tmp.replace(".png", "_1.png"))):
                    if candidate.exists():
                        candidate.unlink()


class FontSpecimen:
    CATEGORY = "Runebender / Font"
    DESCRIPTION = (
        "Render editable DrawBot scripts from a Runebender FONT wire. "
        "Custom scripts execute locally in the ComfyUI Python process; run only trusted scripts."
    )

    @classmethod
    def INPUT_TYPES(cls):
        presets = load_presets()
        preset_names = list(presets) or ["(no presets found)"]
        return {
            "required": {
                "font": ("FONT", {"forceInput": True}),
                "preset": (preset_names, {"default": preset_names[0]}),
                "width": ("INT", {"default": 2048, "min": 64, "max": 8192, "step": 8}),
                "height": ("INT", {"default": 2048, "min": 64, "max": 8192, "step": 8}),
            },
            "optional": {
                "custom_script": (
                    "STRING",
                    {
                        "multiline": True,
                        "default": "",
                        "tooltip": (
                            "Trusted DrawBot Python only; custom scripts execute locally in the ComfyUI Python process. "
                            "Available: font_path, WIDTH, HEIGHT, input_text, and drawbot-skia globals."
                        ),
                    },
                ),
            },
        }

    RETURN_TYPES = ("IMAGE", "MASK")
    RETURN_NAMES = ("image", "mask")
    FUNCTION = "run"

    def run(self, font: str, preset: str, width: int, height: int, custom_script: str = ""):
        font_path = _resolve_drawbot_font_path(font)
        script = _script_for_preset(preset, custom_script)
        # input_text widget removed; presets still reference `input_text` and
        # fall back to their own defaults (e.g. `input_text or "A"`), so pass
        # an empty string to keep that contract working.
        img = _render_drawbot(script, font_path, width, height, "")
        return (_image_to_tensor(img), _mask_to_tensor(img))
