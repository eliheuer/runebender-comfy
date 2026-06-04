from helpers import *

GRID = False
GRID_COLOR = (0.4, 0.4, 0.4)

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

margin = WIDTH * 0.06
font(font_path, HEIGHT * 0.075)
fill(0.92)
textBox(
    input_text or "The quick brown fox jumps over the lazy dog.",
    (margin, margin, WIDTH - margin * 2, HEIGHT - margin * 2),
)

if GRID:
    grid(margin=margin, unit_size=margin / 2, color=GRID_COLOR)
