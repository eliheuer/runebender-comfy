# Glyphs Source Support TODO

Goal: allow users to open `.glyphs` files in Runebender Comfy without changing
the core source-of-truth policy. UFO/designspace should remain the native
Runebender editing format.

## Policy

- `.designspace` and `.ufo` are linked editable sources. Save-back can mirror
  supported edits to disk.
- `.glyphs` should be supported as an import format first.
- Do not save back into the original `.glyphs` file until we explicitly design
  and test round-trip support. Glyphs files contain app-specific metadata that
  is easy to damage.

## First Implementation

1. Let the open-source flow accept `.glyphs` files.
2. Convert the selected `.glyphs` file with `glyphsLib` into a managed
   UFO/designspace workspace slot.
3. Mark the slot as a managed copy, not a linked source.
4. Show UI copy that makes the contract clear:
   `Glyphs files are imported into a Runebender UFO/designspace workspace.
   Saves will not modify the original .glyphs file.`
5. Keep `.designspace` / `.ufo` behavior unchanged.

## Follow-Up Work

- Add an explicit `Export UFO/designspace` path for imported Glyphs sources.
- Decide whether an `Export Glyphs` path is worth supporting.
- If true `.glyphs` save-back is ever added, build it as a separate feature
  with fixture-based round-trip tests before exposing it in the UI.

## Tests

- Opening `.glyphs` creates a managed workspace copy and does not set
  `origin_mode: linked`.
- Opening `.designspace` and `.ufo` still creates linked slots.
- The browser error message for unsupported linked save-back never falls back
  to a generic `500 Internal Server Error`.
