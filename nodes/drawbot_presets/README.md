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

## Grid helper

Preset scripts can call `grid(...)` from `helpers.py`.

```python
grid(
    margin=128,
    unit_size=64,
    color=(0.4, 0.4, 0.4),
    weight=1,
)
```

`margin` sets the inset from each page edge. `unit_size` sets the grid
spacing inside that inset box, so a `1024 x 1024` image with `margin=128`
and `unit_size=64` produces 12 divisions between the margins.

To fit a fixed number of divisions instead of a fixed unit size, use
`divisions`:

```python
grid(margin=128, divisions=12, color=(0.2, 0.7, 0.5))
```

`unit_size` and `divisions` may also be 2-item tuples for separate x/y
spacing, for example `unit_size=(64, 32)` or `divisions=(12, 8)`.
