from helpers import *

GRID = False
GRID_MARGIN = WIDTH * 0.125
GRID_UNIT_SIZE = WIDTH * 0.0625
GRID_COLOR = (0.4, 0.4, 0.4)

# Available variables:
#   font_path   - absolute path to the compiled font
#   WIDTH       - canvas width in pixels
#   HEIGHT      - canvas height in pixels
#   input_text  - value from the node's input_text field
#
# DrawBot functions are available as globals:
#   newPage(), fill(), rect(), font(), text(), textBox(), oval(), line(), image()

newPage(WIDTH, HEIGHT)

fill(0.06)
rect(0, 0, WIDTH, HEIGHT)

font(font_path, 160)
fill(0.92)
text(input_text or "Aa", (WIDTH / 2, HEIGHT / 2 - 60), align="center")

if GRID:
    grid(margin=GRID_MARGIN, unit_size=GRID_UNIT_SIZE, color=GRID_COLOR)
