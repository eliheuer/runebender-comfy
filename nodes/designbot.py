"""DesignBot — DrawBot/Processing-style 2D graphics node.

Wraps the `designbot` Rust crate (https://github.com/eliheuer/designbot).
Takes a script string, executes it via the designbot runtime, returns
the rendered raster as an `IMAGE` tensor.
"""

from __future__ import annotations


class DesignBot:
    CATEGORY = "type / graphics"

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
        # TODO: invoke designbot (CLI or PyO3 bindings) with `script`,
        # collect the rendered PNG, decode to a torch tensor of shape
        # [1, H, W, 3] in [0, 1].
        raise NotImplementedError("DesignBot runtime wiring not yet implemented")
