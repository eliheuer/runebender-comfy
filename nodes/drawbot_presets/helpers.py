"""Shared helpers for Runebender DrawBot specimen presets."""


def remap(value, inputMin, inputMax, outputMin, outputMax):
    inputSpan = inputMax - inputMin
    outputSpan = outputMax - outputMin
    valueScaled = float(value - inputMin) / float(inputSpan)
    return outputMin + (valueScaled * outputSpan)


def grid(margin, step=None, color=(0.4, 0.4, 0.4), weight=1):
    import drawbot_skia.drawbot as _db

    if step is None:
        step = margin / 2

    w = _db.width()
    h = _db.height()
    with _db.savedState():
        _db.stroke(*color)
        _db.strokeWidth(weight)
        _db.fill(None)
        _db.rect(margin, margin, w - margin * 2, h - margin * 2)

        x = margin
        while x <= w - margin:
            _db.line((x, margin), (x, h - margin))
            x += step

        y = margin
        while y <= h - margin:
            _db.line((margin, y), (w - margin, y))
            y += step
