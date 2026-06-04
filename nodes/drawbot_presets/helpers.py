"""Shared helpers for Runebender DrawBot specimen presets."""


def remap(value, inputMin, inputMax, outputMin, outputMax):
    inputSpan = inputMax - inputMin
    outputSpan = outputMax - outputMin
    valueScaled = float(value - inputMin) / float(inputSpan)
    return outputMin + (valueScaled * outputSpan)


def _pair(value, name):
    if isinstance(value, (list, tuple)):
        if len(value) != 2:
            raise ValueError(f"{name} must be a number or a 2-item tuple")
        return value[0], value[1]
    return value, value


def _positions_by_unit(start, end, unit):
    if unit <= 0:
        raise ValueError("grid unit_size must be greater than 0")

    positions = [start]
    value = start + unit
    epsilon = max(abs(unit), 1) * 0.000001
    while value < end - epsilon:
        positions.append(value)
        value += unit
    positions.append(end)
    return positions


def _positions_by_divisions(start, end, divisions):
    divisions = int(divisions)
    if divisions <= 0:
        raise ValueError("grid divisions must be greater than 0")

    unit = (end - start) / divisions
    return [start + unit * i for i in range(divisions + 1)]


def _set_stroke_color(db, color):
    if isinstance(color, (list, tuple)):
        db.stroke(*color)
    else:
        db.stroke(color)


def grid(
    margin,
    unit_size=None,
    color=(0.4, 0.4, 0.4),
    weight=1,
    divisions=None,
    step=None,
):
    import drawbot_skia.drawbot as _db

    w = _db.width()
    h = _db.height()
    left = margin
    right = w - margin
    bottom = margin
    top = h - margin
    if right <= left or top <= bottom:
        raise ValueError("grid margin is too large for the current page")

    if step is not None:
        if unit_size is not None:
            raise ValueError("grid accepts either unit_size or step, not both")
        unit_size = step

    if divisions is not None:
        x_divisions, y_divisions = _pair(divisions, "divisions")
        x_positions = _positions_by_divisions(left, right, x_divisions)
        y_positions = _positions_by_divisions(bottom, top, y_divisions)
    else:
        if unit_size is None:
            unit_size = margin / 2
        x_unit, y_unit = _pair(unit_size, "unit_size")
        x_positions = _positions_by_unit(left, right, x_unit)
        y_positions = _positions_by_unit(bottom, top, y_unit)

    with _db.savedState():
        _set_stroke_color(_db, color)
        _db.strokeWidth(weight)
        _db.fill(None)
        _db.rect(left, bottom, right - left, top - bottom)

        for x in x_positions:
            _db.line((x, bottom), (x, top))

        for y in y_positions:
            _db.line((left, y), (right, y))
