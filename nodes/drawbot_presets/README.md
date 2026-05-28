# DrawBot Presets

Each `.py` file in this directory appears as a preset in the Runebender
`DrawBot` node. Scripts run with these variables already defined:

| Variable | Description |
| --- | --- |
| `font_path` | Absolute path to the compiled font for the incoming `FONT` wire. |
| `WIDTH` | Output canvas width in pixels. |
| `HEIGHT` | Output canvas height in pixels. |
| `input_text` | Text from the node's `input_text` field. |

All public functions from `drawbot_skia.drawbot` are available as globals.
Use the `eliheuer/drawbot-skia` fork for this project.
