"""Font specimen rendering node.

This is the DrawBot-style companion to FontPreview: it consumes a FONT
workspace reference and returns both IMAGE and MASK outputs, with a
small scriptable drawing surface for specimen generation.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .workspace import compiled_path


def _image_stack():
    try:
        import numpy as np
        from PIL import Image, ImageDraw, ImageFont
        import torch
    except ImportError as exc:
        raise ImportError(
            "FontSpecimen requires numpy, Pillow, and torch; install them in the ComfyUI Python environment"
        ) from exc
    return np, Image, ImageDraw, ImageFont, torch


def _image_to_tensor(img):
    np, _Image, _ImageDraw, _ImageFont, torch = _image_stack()
    arr = np.array(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy(arr).unsqueeze(0)


def _mask_to_tensor(img):
    np, _Image, _ImageDraw, _ImageFont, torch = _image_stack()
    alpha = np.array(img.getchannel("A"), dtype=np.float32) / 255.0
    return torch.from_numpy(alpha).unsqueeze(0)


def _resolve_text(preset: str, input_text: str) -> str:
    text = input_text.strip()
    if preset == "glyph":
        return text or "A"
    if preset == "pangram":
        return text or "The quick brown fox jumps over the lazy dog."
    if preset == "waterfall":
        return text or "Aa"
    return text or "Aa"


def _render_text_layer(font_path: str | None, text: str, width: int, height: int, size: int | None = None):
    _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    if font_path and Path(font_path).exists():
        face = ImageFont.truetype(str(font_path), size=size or max(12, int(height * 0.64)))
        bbox = draw.textbbox((0, 0), text, font=face)
        text_w = (bbox[2] - bbox[0]) if bbox else 0
        text_h = (bbox[3] - bbox[1]) if bbox else 0
        x = max(0, (width - text_w) // 2)
        y = max(0, (height - text_h) // 2)
        draw.text((x, y), text, fill=(255, 255, 255, 255), font=face)
    return img


@dataclass
class _ScriptCanvas:
    width: int
    height: int
    font_path: str | None
    image: Any
    draw: Any
    fill_color: tuple[int, int, int, int] = (255, 255, 255, 255)
    font_size: int = 120

    def new_page(self, width: int | None = None, height: int | None = None) -> None:
        _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
        if width is not None:
            self.width = int(width)
        if height is not None:
            self.height = int(height)
        self.image = Image.new("RGBA", (self.width, self.height), (0, 0, 0, 0))
        self.draw = ImageDraw.Draw(self.image)

    def fill(self, *args: Any) -> None:
        if len(args) == 1 and isinstance(args[0], (int, float)):
            level = int(max(0, min(255, round(float(args[0]) * 255))))
            self.fill_color = (level, level, level, 255)
            return
        values = [int(max(0, min(255, round(float(v) * 255 if float(v) <= 1 else float(v))))) for v in args[:4]]
        while len(values) < 4:
            values.append(255)
        self.fill_color = tuple(values[:4])  # type: ignore[assignment]

    def rect(self, x: float, y: float, w: float, h: float) -> None:
        self.draw.rectangle([x, y, x + w, y + h], fill=self.fill_color)

    def font(self, font_path: str | None, size: float) -> None:
        self.font_path = font_path
        self.font_size = max(1, int(size))

    def _face(self) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
        _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
        if self.font_path and Path(self.font_path).exists():
            return ImageFont.truetype(str(self.font_path), size=self.font_size)
        return ImageFont.load_default()

    def text(self, text: str, position: tuple[float, float], align: str = "left") -> None:
        face = self._face()
        self.draw.text(position, text, fill=self.fill_color, font=face, anchor={"center": "mm", "right": "rm"}.get(align))

    def text_box(self, text: str, box: tuple[float, float, float, float], align: str = "left") -> None:
        face = self._face()
        x, y, w, h = box
        self.draw.multiline_text((x, y), text, fill=self.fill_color, font=face, spacing=6, align=align)


def _run_custom_script(script: str, font_path: str | None, width: int, height: int, input_text: str):
    _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
    canvas = _ScriptCanvas(
        width=width,
        height=height,
        font_path=font_path,
        image=Image.new("RGBA", (width, height), (0, 0, 0, 0)),
        draw=ImageDraw.Draw(Image.new("RGBA", (width, height), (0, 0, 0, 0))),
    )
    canvas.new_page(width, height)
    env: dict[str, Any] = {
        "WIDTH": width,
        "HEIGHT": height,
        "font_path": font_path,
        "input_text": input_text,
        "ctx": canvas,
        "newPage": canvas.new_page,
        "fill": canvas.fill,
        "rect": canvas.rect,
        "font": canvas.font,
        "text": canvas.text,
        "textBox": canvas.text_box,
        "__builtins__": {
            "range": range,
            "len": len,
            "min": min,
            "max": max,
            "abs": abs,
            "round": round,
            "int": int,
            "float": float,
            "str": str,
            "tuple": tuple,
        },
    }
    exec(script, env, env)
    return canvas.image


class FontSpecimen:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "preset": (
                    ("specimen", "waterfall", "glyph", "pangram", "custom"),
                    {"default": "specimen"},
                ),
                "input_text": ("STRING", {"multiline": True, "default": "Aa"}),
                "width": ("INT", {"default": 1024, "min": 64, "max": 4096}),
                "height": ("INT", {"default": 512, "min": 64, "max": 4096}),
            },
            "optional": {
                "custom_script": (
                    "STRING",
                    {
                        "multiline": True,
                        "default": "",
                        "tooltip": "DrawBot-style Python using newPage, fill, rect, font, text, textBox.",
                    },
                ),
            },
        }

    RETURN_TYPES = ("IMAGE", "MASK")
    RETURN_NAMES = ("image", "mask")
    FUNCTION = "run"

    def run(self, font: str, preset: str, input_text: str, width: int, height: int, custom_script: str = ""):
        _np, Image, ImageDraw, ImageFont, _torch = _image_stack()
        try:
            font_path = compiled_path(font)
        except Exception:
            font_path = None

        if preset == "custom" and custom_script.strip():
            img = _run_custom_script(custom_script, str(font_path) if font_path else None, width, height, input_text)
        elif preset == "waterfall":
            base = _resolve_text(preset, input_text)
            layer = Image.new("RGBA", (width, height), (0, 0, 0, 0))
            draw = ImageDraw.Draw(layer)
            y = 16
            for size in (96, 72, 48, 36, 24, 18):
                if not font_path or not Path(font_path).exists():
                    break
                face = ImageFont.truetype(str(font_path), size=size)
                draw.text((16, y), base, fill=(220, 220, 220, 255), font=face)
                y += size + 6
            img = layer
        else:
            img = _render_text_layer(font_path, _resolve_text(preset, input_text), width, height)

        preview = Image.alpha_composite(
            Image.new("RGBA", (width, height), (16, 16, 16, 255)),
            img,
        )
        return (_image_to_tensor(preview), _mask_to_tensor(img))
