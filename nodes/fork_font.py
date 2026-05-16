"""Fork a FONT workspace reference.

This is the graph primitive for parallel font experimentation.
"""

from __future__ import annotations

from .workspace import fork_slot, make_fork_name


class ForkFont:
    CATEGORY = "Runebender / Font"

    @classmethod
    def INPUT_TYPES(cls):
        return {
            "required": {
                "font": ("FONT",),
                "fork_name": (
                    "STRING",
                    {
                        "multiline": False,
                        "default": "",
                    },
                ),
            },
        }

    RETURN_TYPES = ("FONT",)
    RETURN_NAMES = ("font",)
    FUNCTION = "run"

    def run(self, font: str, fork_name: str):
        fork_name = fork_name.strip() or make_fork_name(font)
        return (fork_slot(font, fork_name),)
