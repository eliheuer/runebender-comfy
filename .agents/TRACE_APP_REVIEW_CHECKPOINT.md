# Trace App Review Checkpoint

Checkpoint marker:

```text
rb-bundle-2026-06-09-review-clean
```

Use this checkpoint for manual Runebender Comfy review before starting another
`img2bez` autoresearch pass. The goal is to judge the real editor output with
the traced outline over the retained background image.

## Quality Bar

This project is optimizing for type-design-friendly UFO output, not generic
raster-to-vector output. A trace only counts as progress when the point
structure looks like it belongs in the reference UFOs used by autoresearch:

- Simple outlines with the fewest points a type designer would reasonably keep.
- On-curve points at meaningful extrema, smooth turns, and intentional corners.
- Smooth continuity through curves, without random bumps or stair-step clusters.
- Correct line-versus-curve decisions: stems, shelves, and right-angle shapes
  stay straight; smooth arches stay smooth.
- Handles follow the shape of the raster. Horizontal or vertical handles are
  only acceptable when the source curve actually wants them.
- Major visual divergence from the retained source image is a hard failure,
  even if the filled glyph preview is recognizable.

Aggregate scores, IoU, contour counts, and point-count ratios are supporting
signals. They do not override the editor overlay. If the traced UFO would need
obvious cleanup before a type designer could use it, the tracing pipeline is
moving in the wrong direction.

## What Changed In This Checkpoint

- Runebender keeps the background image after `Trace Image`, so outline errors
  can be inspected against the source raster.
- The retained background image is aligned to the tracing coordinate space, so
  point placement and divergence are easier to debug in the editor.
- Runebender bundles the current local `img2bez` path dependency from the
  sibling checkout.
- Successful WASM traces log img2bez diagnostic counters, including extrema
  fixes, divergence splits, tangent overrides, tangent near-misses,
  over-segment cleanup, final outline divergences, and final outline repairs.
- If img2bez reports more final outline divergences than repairs, Runebender
  still applies the trace for visual inspection but sets the status to say how
  many outline divergences need review and logs an explicit warning.
- Runebender now resolves and logs an explicit trace threshold before tracing.
  High-contrast black-on-white sources use `max(Otsu, 192)` so antialiased
  glyph edges are included; lower-contrast/gray sources keep the Otsu value.
  The same threshold is used for tracing and background-image alignment.
- The local `img2bez` tangent policy allows a narrow visible diagonal-handle
  rescue path at curve sections adjacent to straight line sections, plus a
  capped local smooth-section rescue for short sections where an H/V snap
  visibly diverges from the raw outline. The local rescue is capped at short
  spans so older `S`/ampersand regressions do not leak back in.
- `img2bez` has diagnostics for final outline divergence, extrema/deviation
  fixes, tangent override behavior, and over-segmented stair-step cleanup.
- `img2bez` can now repair final outline divergences in bounded cases:
  clearly rectilinear misses fall back to a line path, while severe curved
  misses compare a structured span refit against a smoother closed-curve
  source fallback and choose a repair that balances source deviation and point
  count. Smooth fallback candidates are rejected if they underfit the failed
  contour by using fewer on-curve points than the path being repaired.
- The exact `aRE` stress trace still accepts several tight local diagonal
  tangent candidates in the `a`/`R` zones while retaining the point-count shape
  guard.
- The local `img2bez` final-repair selector now gives silhouette error more
  weight once a contour has already failed final divergence validation. This
  moved the exact `aRE` app-review stress score out of the earlier low `0.63x`
  range while keeping `E` stable.
- The final-repair point-count cap is now additive (`current on-curves + 16`)
  instead of allowing a full doubling. This keeps the useful `aRE` app repair
  while preventing the focused `S` gate from accepting a 40-on-curve repair
  where the reference has 20.
- The stress gate writes a composite reference GLIF for multi-glyph traces and
  parses `img2bez` reference metrics. Generated low-resolution `aen` stress
  must clear `eval_score >= 0.70`, and exact app-style sources can be checked
  against per-glyph structural slices.
- `img2bez` includes a stress gate for low-resolution multi-glyph traces such
  as the hard `aen`/`n` case, plus diagnostics for exact app-style sources.
- `img2bez` whole-contour final repair now treats a repair as "good enough"
  once it is below `30%` of the final divergence threshold and then prefers the
  simpler point structure.
- `img2bez` now has a final source-backed line repair: after divergence repair,
  a fitted cubic is converted back to a line when its endpoints map to a raw
  traced source span that is clearly collinear. This targets the app-review
  failures where a true stem or straight join was being drawn as a bowed curve.
- `img2bez` now has a scoped near-linear cubic cleanup. It only runs on
  contours that already failed final divergence validation, and converts a
  cubic to a line only when both handles are already close to the chord. This
  preserved the focused ampersand gate while cleaning up app-style `a/R`
  contours that were carrying unnecessary off-curve handles.
- The exact `aRE` app-review image now scores `0.661` with
  `raster_iou=72.9%`, `final_outline_divergences=3`, and
  `final_outline_repairs=3`. Per-glyph structure for this checkpoint is:
  `a` = `13l+12c` vs reference `15l+12c`, `R` = `23l+6c` vs reference
  `20l+6c`, `E` = `13l+0c` vs reference `24l+0c`. The important change is
  that `a` and `R` now match the reference curve/off-curve counts on this
  app-style source, while still needing manual review for exact line placement.
- The trace stress gate now reports per-glyph line and curve segment counts in
  addition to contours, on-curves, off-curves, and ratios. This makes `E`
  visible as a rectilinear control case and keeps `a/R` failures from hiding
  inside a combined composite score.

## Manual Review Focus

Test with the current ampersand image first, then at least one `n`-heavy image.
For each test, keep the source image visible and inspect these items:

- Do on-curve extrema land at the visually obvious top, bottom, left, and right
  turns of smooth curves?
- Do smooth curves stay smooth without random bumps or small stair-step point
  clusters?
- Are non-horizontal/non-vertical handles preserved where the raster curve is
  clearly diagonal or transitional?
- Are true straight stems and right-angle corners kept straight, especially in
  `n` and other simple stem glyphs?
- Does the traced outline visibly diverge from the raster in any large region?
- Are there obvious extra on-curve points that a type designer would delete
  immediately?

## Known Open Issues

- Some `n` and multi-glyph app-style traces can still over-segment or place
  points poorly on arches. The stress gate catches the point-explosion class,
  but it does not yet score every visual mismatch.
- Some corner joins can still choose horizontal or vertical handles where a
  visible diagonal transition would better match the raster.
- The exact `aRE` app-review trace is improving, but `a` is still short by two
  line segments compared with the reference structure. The next tracing pass
  should inspect whether those remaining sections should be true lines or
  better-shaped curves in the live editor.
- The current `aRE` app review shows `E` is not the blocker. `E` is now a
  useful rectilinear control case; the next pass should treat `a` and `R` as
  the primary app-debug glyphs and compare them against the retained background
  image, not just the isolated reference UFO structural gate.
- The final divergence guard now repairs clear rectilinear misses and bounded
  curved misses. The curved repair is still a safety net, not a finished
  type-designer smoothing pass; manually inspect whether repaired `a`/`R`
  contours are smooth enough or merely closer to the raster.

## Suggested Next Decision

After manual review, decide whether the next `img2bez` pass should focus on:

1. `n` arch over-segmentation and low-resolution stress behavior.
2. Diagonal tangent preservation at corner joins.
3. Smoother final divergence repair for visibly bad curved spans.
