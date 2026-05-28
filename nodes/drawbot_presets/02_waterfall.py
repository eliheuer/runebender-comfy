from helpers import *

GRID = False

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

sample = input_text or "The quick brown fox jumps over the lazy dog"
margin = WIDTH * 0.04
y = HEIGHT - margin
fill(0.92)
for size in [144, 120, 96, 72, 60, 48, 36, 28, 24, 18]:
    if y < margin:
        break
    font(font_path, size)
    text(sample, (margin, y))
    y -= size * 1.35

if GRID:
    grid(WIDTH * 0.04)
