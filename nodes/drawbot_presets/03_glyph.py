from helpers import *

GRID = False

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
    grid(WIDTH * 0.05)
