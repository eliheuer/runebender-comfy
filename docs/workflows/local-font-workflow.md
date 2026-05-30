# Local Font Workflow

This is the smallest practical ComfyUI graph for type-design work in
`runebender-comfy`.

In the ComfyUI add-node menu, look under `Runebender`.

## Goal

Keep the font as a `FONT` wire, edit it in Runebender, then branch,
compile, and preview it through downstream nodes.

## Recommended graph

```text
Runebender -> Compile Font -> Font Preview
          +-> Fork Font -> (alternate branch)
```

## Node roles

`Runebender`
: Loads a workspace from a source font path, imports a local
  UFO/designspace bundle into the workspace, or links a disk source for
  save-back. It opens the editor widget for the active `FONT`
  workspace, passes the reference through unchanged, and exposes the
  current glyph SVG as a secondary string output for side-channel
  inspection.

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

`Glyph Candidate Builder`
: Copies a workspace into a new candidate `FONT` and replaces only the
  selected glyphs with normalized donor outlines. Use `glyphs:
  mark:red` and `arabic_only: true` to turn red-labeled Arabic
  placeholders into a visual candidate branch without touching the
  original source.

`Apply Glyph Candidates`
: Copies reviewed glyphs from a candidate `FONT` back into a target
  `FONT`. Keep `clear_mark_color: false` while reviewing so red labels
  stay under your control. When the target is a linked source, the node
  writes through to the original disk source by default.

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
4. Load `example_workflows/runebender-linked-source-smoke.json` for a
   one-node smoke graph, or add a `Runebender` node manually.
5. Load a font with `Runebender`, edit it, then send the same `FONT`
   wire into `Compile Font` and `Font Preview`. For source files you
   want to edit on disk, either click `Open Font Source...` or paste an
   absolute/relative `.designspace` or `.ufo` path into `source_path`
   and click `Edit`; Runebender will link the source before opening the
   editor.

For a bundled smoke test, use
`samples/virtua-grotesk/VirtuaGrotesk.designspace` as the source path.
The bundled demo is the two-master (Regular/Bold) Virtua Grotesk font;
the `demo` workspace alias resolves to it automatically.

## Red-Marked Arabic Candidate Workflow

Use this when you want to replace only the glyphs you marked visually in
Runebender:

```text
Runebender -> Glyph Candidate Builder -> Runebender
            -> Apply Glyph Candidates -> Runebender
```

1. Load the real source in the first `Runebender` node.
2. Mark the Arabic placeholders that should be replaced as red.
3. Save.
4. Run `Glyph Candidate Builder` with:
   - `donor_path`: `/Users/eli/GH/repos/rubik/sources/designspace/Rubik.designspace`
   - `glyphs`: `mark:red`
   - `arabic_only`: `true`
5. Open the output `FONT` in the second `Runebender` node.
6. When a candidate is worth keeping, run `Apply Glyph Candidates` with:
   - `candidate_font`: the candidate builder output
   - `target_font`: the original linked `Runebender` source
   - `glyphs`: `mark:red` to apply only red glyphs from the candidate
     report, `report`, or an explicit accepted glyph list
   - `clear_mark_color`: `false`

The candidate builder forks the workspace, so the original source is not
overwritten. `Apply Glyph Candidates` is the deliberate promotion step
that copies only the selected reviewed drawings back to the real source.

## Notes

- UFO/designspace is the default editable path.
- `Import Folder...` copies a source into the local workspace cache.
- `Open Font Source...` creates a workspace cache that mirrors saved
  UFO/designspace edits back to the original disk source.
- Linked sources are checked when the editor opens. If the original
  disk source is newer than the cached workspace, Runebender refreshes
  the cache from disk before loading the glyphs.
- If `Open Font Source...` reports `404` or `405`, the browser has a
  newer bundle than the running Python backend. Fully stop and restart
  ComfyUI, then hard-refresh the browser.
- The grid sidebar includes a raw Designspace XML panel for
  designspace-backed workspaces. Its edits are dirty-tracked and saved
  by the same Save button as glyph, groups, and kerning edits.
- Glyphs imports normalize into UFO/designspace when `glyphsLib` is
  installed.
- `fontc` is optional but required for the compile/preview branch.
- The workspace reference is the stable graph value; treat the filesystem
  layout as an implementation detail.
