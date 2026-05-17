"""DesignBot — DrawBot/Processing-style 2D graphics node.

Wraps the `designbot` Rust crate (https://github.com/eliheuer/designbot).
Takes a script string, executes it via the designbot runtime, returns
the rendered raster as an `IMAGE` tensor.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def _image_stack():
    try:
        import numpy as np
        from PIL import Image
        import torch
    except ImportError as exc:
        raise ImportError(
            "DesignBot requires numpy, Pillow, and torch; install them in the ComfyUI Python environment"
        ) from exc
    return np, Image, torch


def _png_to_tensor(path: Path):
    np, Image, torch = _image_stack()
    with Image.open(path) as img:
        arr = np.array(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy(arr).unsqueeze(0)


def _designbot_bin() -> str:
    configured = os.environ.get("DESIGNBOT_BIN")
    if configured:
        return configured

    found = shutil.which("designbot")
    if found:
        return found

    raise RuntimeError(
        "DesignBot CLI not found. Install it with `cargo install --git "
        "https://github.com/eliheuer/designbot designbot-cli`, or set DESIGNBOT_BIN."
    )


def _escape_rust_string(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _script_for_render(script: str, width: int, height: int, output_path: Path) -> str:
    if "fn main" in script or "render_to_png" in script:
        return script

    output = _escape_rust_string(str(output_path))
    return f"""
use designbot::prelude::*;

fn main() {{
    let mut ctx = Canvas::new({float(width):.1f}, {float(height):.1f});
    let mut renderer = Renderer::new({int(width)}, {int(height)});

{script}

    renderer.render_to_png(&ctx, "{output}").unwrap();
}}
"""


class DesignBot:
    CATEGORY = "Runebender / Graphics"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "script": ("STRING", {
                    "multiline": True,
                    "default": "// designbot script\n",
                }),
                "width": ("INT", {"default": 1024, "min": 1, "max": 8192}),
                "height": ("INT", {"default": 1024, "min": 1, "max": 8192}),
            },
        }

    RETURN_TYPES = ("IMAGE",)
    FUNCTION = "run"

    def run(self, script: str, width: int, height: int):
        with tempfile.TemporaryDirectory(prefix="runebender-designbot-") as tmp:
            tmp_dir = Path(tmp)
            output_path = tmp_dir / "designbot.png"
            script_path = tmp_dir / "designbot.rs"
            script_path.write_text(
                _script_for_render(script, width, height, output_path),
                encoding="utf-8",
            )

            subprocess.run(
                [
                    _designbot_bin(),
                    "--render",
                    str(script_path),
                    "--output",
                    str(output_path),
                ],
                check=True,
                cwd=tmp_dir,
            )

            if not output_path.exists():
                raise RuntimeError(f"DesignBot did not create an output image at {output_path}")

            return (_png_to_tensor(output_path),)
