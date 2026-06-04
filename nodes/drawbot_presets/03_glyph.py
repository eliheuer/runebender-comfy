from helpers import *

GRID = False
GRID_MARGIN = WIDTH * 0.125
GRID_UNIT_SIZE = WIDTH * 0.0625
GRID_COLOR = (0.4, 0.4, 0.4)

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

font(font_path, HEIGHT * 0.75)
fontVariations(wght=400)
fill(0.92)
text(
    input_text or "A",
    (WIDTH / 2, HEIGHT * 0.15),
    align="center",
)

if GRID:
    grid(margin=GRID_MARGIN, unit_size=GRID_UNIT_SIZE, color=GRID_COLOR)
