# Local Font Workflow

This is the smallest practical ComfyUI graph for type-design work in
`runebender-comfy`.

In the ComfyUI add-node menu, look under `Runebender`.

## Goal

Keep the font as a `FONT` wire, edit it in Runebender, then branch,
compile, and preview it through downstream nodes.

## Recommended graph

```text
Load Font -> Runebender -> Compile Font -> Font Preview
          |
          +-> Fork Font -> (alternate branch)
```

## Node roles

`Load Font`
: Loads a workspace from a source font path or imports a local
  UFO/designspace bundle into the workspace with the built-in import
  button. It also shows an inline specimen preview and a dropdown of
  existing workspace slots. `ufo/designspace` is the default path;
  Glyphs and glyphspackage are supported as alternate source kinds
  when needed.

`Runebender`
: Opens the editor widget for the active `FONT` workspace. It passes
  the reference through unchanged and exposes the current glyph SVG as
  a secondary string output for side-channel inspection.

`Compile Font`
: Rebuilds a compiled artifact for the workspace when `fontc` is
  available. This is the Google Fonts-oriented path that turns editable
  source into a renderable font file. The node returns the same `FONT`
  reference after the side effect completes.

`Font Preview`
: Renders a specimen from the compiled side of the same `FONT`
  workspace.

`Fork Font`
: Copies a workspace so you can keep exploring variants without
  overwriting the original source.

`Specimen`
: Renders a DrawBot-style specimen from a `FONT` wire and returns both
  `IMAGE` and `MASK` outputs. Use this when you want a scripted
  specimen or a text mask derived from the current font.

## Setup

1. Install the package into ComfyUI's `custom_nodes/`.
2. Build the web bundle and WASM core:
   ```bash
   cd web
   pnpm install
   pnpm wasm
   pnpm build
   ```
3. Restart ComfyUI.
4. Load a font with `Load Font`, edit it in `Runebender`, then send
   the same `FONT` wire into `Compile Font` and `Font Preview`.

For a bundled smoke test, use
`samples/demo-font/Demo.designspace` as the source path.

## Notes

- UFO/designspace is the default editable path.
- Glyphs imports normalize into UFO/designspace when `glyphsLib` is
  installed.
- `fontc` is optional but required for the compile/preview branch.
- The workspace reference is the stable graph value; treat the filesystem
  layout as an implementation detail.
