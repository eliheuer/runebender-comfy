"""FONT specimen preview node.

Consumes a FONT workspace reference and renders a simple raster
specimen. This is intentionally lightweight: it validates the workspace
contract and gives us a usable rendering node before a full DrawBot
pipeline lands.
"""

from __future__ import annotations

from io import BytesIO
import os

from .workspace import compiled_path


def _image_stack():
    try:
        import numpy as np
        from PIL import Image, ImageDraw, ImageFont
        import torch
    except ImportError as exc:
        raise ImportError(
            "FontPreview requires numpy, Pillow, and torch; install them in the ComfyUI Python environment"
        ) from exc
    return np, Image, ImageDraw, ImageFont, torch


def _pil_to_tensor(img):
    np, _image, _image_draw, _image_font, torch = _image_stack()
    arr = np.array(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy(arr).unsqueeze(0)


def render_specimen_image(font_path, text: str, width: int, height: int):
    _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
    img = Image.new("RGBA", (width, height), (16, 16, 16, 255))
    draw = ImageDraw.Draw(img)
    if font_path and os.path.exists(font_path):
        size = max(12, int(height * 0.64))
        face = ImageFont.truetype(str(font_path), size=size)
        bbox = draw.textbbox((0, 0), text, font=face)
        text_w = (bbox[2] - bbox[0]) if bbox else 0
        text_h = (bbox[3] - bbox[1]) if bbox else 0
        x = max(0, (width - text_w) // 2)
        y = max(0, (height - text_h) // 2)
        draw.text((x, y), text, fill=(220, 220, 220, 255), font=face)
    else:
        face = ImageFont.load_default()
        message = "No compiled font yet"
        bbox = draw.textbbox((0, 0), message, font=face)
        text_w = (bbox[2] - bbox[0]) if bbox else 0
        text_h = (bbox[3] - bbox[1]) if bbox else 0
        x = max(0, (width - text_w) // 2)
        y = max(0, (height - text_h) // 2)
        draw.text((x, y), message, fill=(180, 180, 180, 255), font=face)
    return img


def render_preview_png(font_path, text: str, width: int, height: int) -> bytes:
    img = render_specimen_image(font_path, text, width, height)
    bio = BytesIO()
    img.convert("RGB").save(bio, format="PNG")
    return bio.getvalue()


class FontPreview:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "text": ("STRING", {"multiline": False, "default": "Aa"}),
                "width": ("INT", {"default": 1024, "min": 64, "max": 4096}),
                "height": ("INT", {"default": 512, "min": 64, "max": 4096}),
            },
        }

    RETURN_TYPES = ("IMAGE",)
    RETURN_NAMES = ("image",)
    FUNCTION = "run"

    def run(self, font: str, text: str, width: int, height: int):
        try:
            font_path = compiled_path(font)
        except Exception:
            font_path = None
        img = render_specimen_image(font_path, text, width, height)
        return (_pil_to_tensor(img),)
