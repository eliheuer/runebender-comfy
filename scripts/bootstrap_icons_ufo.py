#!/usr/bin/env python3
"""One-time bootstrap: build assets/runebender-icons.ufo from the icon
outlines that were previously hardcoded in the toolbar (themselves
transcribed from runebender-xilem's VirtuaGrotesk icon glyphs).

After this runs once, the UFO is the source of truth — design/edit the
icons in Runebender itself, then run `scripts/build_toolbar_icons.py`
to regenerate the toolbar. This script refuses to overwrite an existing
UFO so it can never clobber later edits.

The toolbar consumes SVG (Y-down); UFOs are Y-up. We store the icons
Y-up here (flip dy=800 so they sit baseline→ascender for comfortable
editing) and build_toolbar_icons.py flips back when generating SVG.

Run with an interpreter that has fontTools + ufoLib2, e.g. the ComfyUI
venv:
    /Users/eli/Work/comfy/repos/ComfyUI/.venv/bin/python \
        scripts/bootstrap_icons_ufo.py
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import ufoLib2
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.transformPen import TransformPen
from fontTools.svgLib.path import parse_path

UPM = 1000
ASCENDER = 800
DESCENDER = -200
# Flip screen-space (Y-down) icon paths up into UFO space (Y-up), placing
# them baseline→ascender. y_ufo = 800 - y_screen.
SCREEN_TO_UFO = (1, 0, 0, -1, 0, ASCENDER)

# Glyph name -> screen-space SVG path. These are the outlines the toolbar
# shipped with; this script seeds the UFO with them exactly once.
ICONS: dict[str, str] = {
    "select": "M328,768 C314,768 308,762 300,734 L246,542 C240,522 236,514 224,514 C210,514 202,520 192,530 L138,584 C122,600 108,608 90,608 C72,608 66,598 66,574 L64,50 C64,18 78,0 96,0 C120,0 142,16 176,50 L506,368 C526,386 540,404 540,422 C540,440 528,450 502,450 L388,450 C368,450 360,458 360,470 C360,484 370,496 378,510 L450,634 C460,650 478,674 478,688 C478,706 462,714 444,722 L366,760 C352,766 344,768 328,768 Z",
    "pen": "M200,768 L432,768 C452,768 456,764 456,744 L456,678 C456,658 452,654 432,654 L200,654 C180,654 176,658 176,678 L176,744 C176,764 180,768 200,768 Z M200,602 L432,602 C454,602 460,604 480,576 L548,484 C556,472 564,462 564,452 L564,416 C564,410 560,396 556,384 L440,32 C430,0 416,0 400,0 L364,0 C348,0 342,8 342,32 L342,336 C342,358 346,362 352,366 C374,378 392,400 392,434 C392,478 360,510 316,510 C272,510 240,478 240,434 C240,400 258,378 280,366 C286,362 290,358 290,336 L290,32 C290,8 284,0 268,0 L232,0 C216,0 202,0 192,32 L76,384 C72,396 68,410 68,416 L68,452 C68,462 76,472 84,484 L152,576 C172,602 180,602 200,602 Z",
    "hyperpen": "M320,428 C272,428 190,390 190,320 C190,240 234,180 344,180 C464,180 552,264 552,408 C552,490 470,600 314,600 C102,600 0,476 0,322 C0,132 130,0 366,0 C564,0 752,116 752,390 C752,590 648,784 320,784 C192,784 40,756 40,696 C40,662 62,646 96,646 C144,646 172,686 320,686 C542,686 650,560 650,386 C650,244 524,92 366,92 C172,92 102,210 102,322 C102,436 196,508 320,508 C436,508 454,444 454,376 C454,310 404,268 336,268 C296,268 280,290 280,320 C280,360 362,346 362,386 C362,410 350,428 320,428 Z",
    "knife": "M746,406 C772,380 770,338 746,314 C720,288 680,288 654,314 C638,330 632,388 616,404 L572,448 C564,456 556,452 552,446 L230,58 C224,50 220,40 214,34 C208,28 198,24 190,24 L28,6 C16,4 10,8 8,10 C4,14 2,18 4,30 L20,186 C20,194 28,212 34,218 C40,224 48,228 54,232 L444,554 C450,558 454,566 446,574 L388,632 C372,648 316,652 298,670 C274,694 272,736 298,762 C322,786 366,786 390,762 C408,744 412,688 428,672 L492,608 C500,600 506,600 514,608 C518,612 532,630 578,676 C598,696 630,720 632,722 C640,730 636,752 644,760 C668,784 710,760 734,736 C760,710 782,670 758,646 C750,638 728,642 720,634 C718,632 694,600 674,580 C644,550 610,520 606,516 C598,508 598,502 606,494 L656,444 C674,426 730,422 746,406 Z",
    "measure": "M520,764 L680,604 C696,588 696,574 680,558 L136,16 C120,0 106,0 90,16 L-70,176 C-86,192 -86,206 -70,222 L474,764 C490,780 504,780 520,764 Z M22,238 C10,250 6,250 -6,238 L-12,232 C-24,220 -24,216 -12,204 L46,146 C58,134 62,134 74,146 L80,152 C92,164 92,168 80,180 Z M100,318 C88,330 84,330 72,318 L66,312 C54,300 54,296 66,284 L164,186 C176,174 180,174 192,186 L198,192 C210,204 210,208 198,220 Z M180,396 C168,408 164,408 152,396 L146,390 C134,378 134,374 146,362 L204,304 C216,292 220,292 232,304 L238,310 C250,322 250,326 238,338 Z M260,476 C248,488 242,488 230,476 L226,470 C214,458 214,454 226,442 L322,344 C334,332 338,332 350,344 L356,350 C368,362 368,366 356,378 Z M338,554 C326,566 322,566 310,554 L304,550 C292,538 292,532 304,520 L362,462 C374,450 378,450 390,462 L396,468 C408,480 408,484 396,496 Z M418,634 C406,646 402,646 390,634 L384,628 C372,616 372,612 384,600 L482,502 C494,490 498,490 510,502 L516,508 C528,520 528,524 516,536 Z M496,714 C484,726 480,726 468,714 L462,708 C450,696 450,692 462,680 L520,622 C532,610 536,610 548,622 L554,626 C566,638 566,644 554,656 Z",
    "shapes": "M460,222 L538,222 L538,32 C538,12 526,0 506,0 L32,0 C12,0 0,12 0,32 L0,506 C0,526 12,538 32,538 L220,538 L220,460 L102,460 C86,460 78,452 78,436 L78,102 C78,86 86,78 102,78 L436,78 C452,78 460,86 460,102 Z M486,784 C648,784 782,652 782,488 C782,324 648,192 486,192 C322,192 192,324 192,488 C192,652 322,784 486,784 Z M486,704 C368,704 270,608 270,488 C270,368 368,272 486,272 C606,272 702,368 702,488 C702,608 606,704 486,704 Z",
    "preview": "M256,798 L240,798 C232,788 232,774 232,774 L232,726 C232,714 226,704 208,686 C128,606 90,466 90,272 C90,202 114,168 138,168 C152,168 158,178 158,192 C158,208 154,224 154,264 C154,290 168,356 182,384 C186,392 194,394 200,394 C206,394 212,392 212,384 C212,372 200,332 200,296 C200,194 230,56 266,56 C302,56 298,80 298,92 C298,110 286,136 286,222 C286,292 290,318 292,326 C294,334 302,340 308,340 C314,340 322,334 322,326 C322,174 370,66 396,30 C412,8 428,0 450,0 C462,0 470,12 470,30 C470,54 416,118 416,272 C416,298 416,318 418,324 C420,330 424,332 428,332 C432,332 440,328 442,322 C470,194 518,122 552,90 C566,76 578,72 592,72 C606,72 610,82 610,98 C610,118 522,268 522,406 C522,464 558,490 582,490 C612,490 638,442 660,402 C686,356 708,336 734,336 C748,336 756,344 756,362 C756,402 668,668 518,734 C500,742 490,752 490,764 L490,774 C490,790 484,798 470,798 L256,798 Z",
    "text": "M56,0 L712,0 C734,0 744,8 744,32 L744,232 C744,248 728,264 712,264 L696,264 C678,264 668,248 662,232 C642,178 624,128 572,128 L532,128 C484,128 468,140 468,192 L468,516 C468,614 488,672 616,672 C640,672 648,680 648,704 L648,736 C648,760 640,768 616,768 L152,768 C128,768 120,760 120,736 L120,704 C120,680 128,672 152,672 C280,672 300,614 300,516 L300,192 C300,140 284,128 236,128 L196,128 C144,128 126,178 106,232 C100,248 90,264 72,264 L56,264 C40,264 24,248 24,232 L24,32 C24,8 34,0 56,0 Z",
}

# Order glyphs appear in the toolbar (drives the UFO glyph order).
ORDER = ["select", "pen", "hyperpen", "knife", "measure", "shapes", "preview", "text"]


def load_icon_codepoints(repo_root: Path) -> dict[str, int]:
    path = repo_root / "assets" / "runebender-icons.codepoints.json"
    return {
        name: int(str(value).removeprefix("U+"), 16)
        for name, value in json.loads(path.read_text(encoding="utf-8")).items()
    }


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    ufo_path = repo_root / "assets" / "runebender-icons.ufo"
    icon_codepoints = load_icon_codepoints(repo_root)
    if ufo_path.exists():
        raise SystemExit(
            f"refusing to overwrite existing {ufo_path}\n"
            "The UFO is the source of truth — edit it in Runebender, then run "
            "scripts/build_toolbar_icons.py. Delete it manually to re-bootstrap."
        )

    font = ufoLib2.Font()
    font.info.familyName = "Runebender Icons"
    font.info.styleName = "Regular"
    font.info.unitsPerEm = UPM
    font.info.ascender = ASCENDER
    font.info.descender = DESCENDER
    font.info.capHeight = 700
    font.info.xHeight = 500

    for name in ORDER:
        d = ICONS[name]
        glyph = font.newGlyph(name)
        glyph.unicodes = [icon_codepoints[name]]
        parse_path(d, TransformPen(glyph.getPen(), SCREEN_TO_UFO))
        # Advance width = right edge of the (flipped) outline, so the
        # editor shows a sensible metrics box around the icon.
        bounds_pen = BoundsPen(font)
        glyph.draw(bounds_pen)
        if bounds_pen.bounds is not None:
            glyph.width = int(math.ceil(bounds_pen.bounds[2]))

    ufo_path.parent.mkdir(parents=True, exist_ok=True)
    font.save(ufo_path)
    print(f"wrote {ufo_path} with {len(font)} icon glyphs: {', '.join(ORDER)}")


if __name__ == "__main__":
    main()
