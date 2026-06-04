from helpers import *

GRID = False
GRID_MARGIN = WIDTH * 0.05
GRID_UNIT_SIZE = GRID_MARGIN / 2
GRID_COLOR = (0.4, 0.4, 0.4)

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

lines = [
    "ABCDEFGHIJKLM",
    "NOPQRSTUVWXYZ",
    "abcdefghijklm",
    "nopqrstuvwxyz",
    "0123456789",
]
padding = HEIGHT * 0.05
size = (HEIGHT - padding * 2) / (len(lines) * 1.25)
font(font_path, size)
fill(0.92)
for i, line in enumerate(lines):
    text(line, (WIDTH / 2, HEIGHT - padding - size * (i + 1)), align="center")

if GRID:
    grid(margin=GRID_MARGIN, unit_size=GRID_UNIT_SIZE, color=GRID_COLOR)
