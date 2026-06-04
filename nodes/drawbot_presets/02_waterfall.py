from helpers import *

GRID = False
GRID_MARGIN = WIDTH * 0.04
GRID_UNIT_SIZE = GRID_MARGIN / 2
GRID_COLOR = (0.4, 0.4, 0.4)

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

sample = input_text or "The quick brown fox jumps over the lazy dog"
margin = GRID_MARGIN
y = HEIGHT - margin
fill(0.92)
for size in [144, 120, 96, 72, 60, 48, 36, 28, 24, 18]:
    if y < margin:
        break
    font(font_path, size)
    text(sample, (margin, y))
    y -= size * 1.35

if GRID:
    grid(margin=GRID_MARGIN, unit_size=GRID_UNIT_SIZE, color=GRID_COLOR)
