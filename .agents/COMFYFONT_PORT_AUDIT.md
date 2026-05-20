# ComfyFont Port Audit

Status: active
Last updated: 2026-05-19

Goal: compare `runebender-comfy` against the working `comfyfont`
implementation and identify the features we have not copied over yet,
especially the DrawBot and DesignBot-adjacent workflows.

Reference source:

- Local: `/Users/eli/Work/comfy/repos/comfyfont`
- Upstream: `eliheuer/comfyfont`

## Summary

The biggest gap is not the Runebender editor shell anymore. It is the
surrounding workflow surface that made comfyfont usable inside ComfyUI:

1. A compact source/editor node with a useful specimen preview.
2. A Python DrawBot render node with presets.
3. A CodeMirror script editor embedded in the node.
4. A full-screen DrawBot visual editor that renders the script, traces draw
   calls, lets users drag literal shapes, and writes the patched Python source
   back into the node.
5. A font backend/session layer that exposes glyph data, saves glyph changes
   to disk, broadcasts external changes, and serves specimen outlines without
   requiring a compiled font.

Runebender-comfy now has parts of item 1 and early parts of item 5. Items 2-4
are not ported. `nodes/designbot.py` is currently a Rust CLI wrapper, not a
comfyfont-equivalent DrawBot workflow.

## Feature Matrix

| Area | comfyfont implementation | runebender-comfy state | Status | Priority |
| --- | --- | --- | --- | --- |
| Single graph node source/editor UX | `nodes/comfyfont.py`, `js/load-node-widget.js` | `nodes/runebender.py`, `web/src/extension.ts` | Mostly present | P0 |
| Node specimen preview before compile | `js/load-node-widget.js` draws vector outlines through `FontController.getSpecimenAtLocation()` | `web/src/extension.ts` custom preview widget plus `nodes/font.py` preview route; source fallback recently added in `nodes/font_preview.py` | Partial | P0 |
| Import/link font source from disk | `/comfyfont/import`, folder search path registration, designspace UFO copy | `/runebender/link_source`, `/runebender/import_source_path`, `/runebender/choose_source`, workspace manifests | Present but young | P0 |
| Save edited source back to disk | `core/server.py::editFinal()` calls writable backend `putGlyph()` | `/runebender/workspace/write` mirrors text files for linked designspace/UFO sources; Runebender editor save path still needs real glyph edit integration | Partial | P0 |
| WebSocket font session | `/comfyfont/ws`, `core/server.py`, `core/remote.py`, `js/remote.js` | No equivalent WebSocket RPC; Runebender loads workspace snapshots over HTTP | Missing | P1 |
| JS font controller/glyph cache | `js/font-controller.js`, `js/packed-path.js` | Runebender Vue/WASM owns model state; no ComfyUI-side reusable controller | Different architecture | P1 |
| Glyph editor | `js/glyph-editor-tab.js`, canvas editor, change recorder | `web/src/Runebender.vue` + WASM editor | Present, but save-back and UX need verification | P0 |
| Mark colors | `core/server.py::getMarkColors/putMarkColor`, grid UI | Runebender has color panels/UI concepts, but persistence needs verification | Partial | P1 |
| DrawBot image node | `nodes/drawbot.py` uses `drawbot_skia.drawbot`, presets, font input, script override | `nodes/font_specimen.py` has a small PIL script surface; `nodes/designbot.py` wraps Rust CLI | Missing equivalent | P0 for DrawBot/DesignBot work |
| DrawBot presets | `nodes/drawbot_presets/*.py`, `/comfyfont/drawbot_preset` | No preset loader/routes for DesignBot | Missing | P0 |
| Inline code editor | `js/drawbot-editor.js` loads local CodeMirror 5 assets | No CodeMirror integration for DesignBot | Missing | P0 |
| DrawBot visual editor | `js/drawbot-visual-editor.js`, `/comfyfont/drawbot_editor/preview`, `/patch` | No visual editor for DesignBot | Missing | P1 |
| Draw call trace/patch backend | `core/drawbot_trace.py` uses `libcst` to instrument, execute, patch Python | No trace/patch layer | Missing | P1 |
| AI font nodes | `nodes/ai_nodes.py` stubs for kerning, spacing, glyph synth | No equivalent registered nodes | Missing, but not urgent | P2 |
| Fork font | `nodes/fork.py` | `nodes/fork_font.py` | Present | P2 |
| Compile helpers | `core/compile.py`, `nodes/render.py::_resolve_font` | `nodes/workspace.py::compile_slot`, `nodes/compile_font.py` | Present but different backend | P1 |

## What ComfyFont Did That We Should Copy First

### DrawBot render node

Comfyfont's `ComfyFontDrawBot` was a normal ComfyUI image-producing node:

- Inputs: `font`, `preset`, `canvas_width`, `canvas_height`.
- Optional inputs: `input_text`, `script_override`.
- Output: `IMAGE`.
- Backend: `drawbot_skia.drawbot`.
- Font resolution: compile or resolve UFO/designspace via `nodes/render.py::_resolve_font`.
- Presets: Python files under `nodes/drawbot_presets/`.

Runebender-comfy should port this as a Python DrawBot node first, even if the
long-term DesignBot direction is Rust. That gives users the working comfyfont
behavior now, while DesignBot can become a replacement or sibling workflow
later.

### CodeMirror script editor

Comfyfont's `js/drawbot-editor.js` made the multiline script widget usable:

- Local CodeMirror 5 vendor assets, no CDN.
- Python mode.
- Overlay follows the underlying ComfyUI textarea with `getBoundingClientRect()`.
- Preset dropdown loads source into `script_override`.
- Script edits sync back to the hidden Comfy widget value.

Without this, a script node is technically present but not pleasant to use.

### Visual editor

Comfyfont's `js/drawbot-visual-editor.js` was more than a preview:

- Full-screen overlay.
- Server-rendered PNG, so preview matches node execution.
- SVG overlay in DrawBot coordinates.
- Trace list for recorded draw calls.
- Rect/oval drag support for literal `x, y` args.
- Drag commits to `/comfyfont/drawbot_editor/patch`.
- Patched source syncs back to the Comfy widget and CodeMirror editor.

The backend is `core/drawbot_trace.py`:

- Uses `libcst` to instrument known DrawBot calls.
- Executes the instrumented script with real `drawbot_skia`.
- Records evaluated args and CTM.
- Rewrites literal positional args format-preservingly.

This is the part we have most obviously not ported.

## Architecture Recommendation

Do not make the Rust `DesignBot` node carry all of this immediately. Port the
working comfyfont DrawBot workflow first as `DrawBot` or `FontDrawBot`, then
decide whether `DesignBot` replaces it later.

Suggested node split:

- `Runebender`: load/link/edit/save font sources.
- `Compile Font`: compile the current source workspace.
- `Font Preview`: quick image output from a FONT.
- `DrawBot`: Python DrawBot-Skia script/preset node, copied from comfyfont.
- `DesignBot`: Rust-native graphics scripting node, later parity target.

This keeps the user-facing workflow simple and avoids blocking on Rust
DesignBot feature parity before restoring known-good ComfyUI behavior.

## Actionable Checklist

### Phase 0: Keep Current Source UX Stable

- [x] Simplify Runebender node controls to `Open Source...`, `source`, `Edit`.
- [x] Restore a visible graph-node specimen preview.
- [x] Render preview from source UFOs when no compiled font exists.
- [x] Use a real Skia/UFO outline renderer for source previews instead of the
  temporary polygon fallback, so counters and antialiasing work.
- [ ] Verify preview in live ComfyUI after a full restart and hard refresh.
- [ ] Verify linked source write-back with a small copy of `virtua-grotesk`.
- [ ] Verify edited glyph data flows from Runebender Vue/WASM to `/runebender/workspace/write`.

Specimen preview acceptance criteria:

- [ ] The Runebender node shows a readable specimen immediately after loading a
  designspace/UFO source, before running `Compile Font`.
- [ ] The specimen updates when the `source` chooser changes.
- [ ] The specimen is large enough to read in the compact node, not a tiny
  status message.
- [ ] If preview rendering fails, the node shows a useful fallback state and
  the console/backend logs identify the reason.

Implementation note: comfyfont renders node specimens by fetching real glyph
outlines, constructing browser `Path2D`, and filling them with the nonzero
winding rule in `js/load-node-widget.js`. Runebender-comfy's backend preview
now follows the same outline-first principle by loading UFOs with `ufoLib2`
and rasterizing paths with `skia-python`; the older polygon path is only a
last-resort fallback for environments missing those packages.

### Phase 1: Port Working DrawBot Node

- [ ] Add `nodes/drawbot.py` based on comfyfont `nodes/drawbot.py`.
- [ ] Add `nodes/drawbot_presets/` with specimen, waterfall, glyph, pangram, custom.
- [ ] Add a shared render helper equivalent to comfyfont `nodes/render.py::_resolve_font`.
- [ ] Register the node as `DrawBot` or `Font DrawBot` in `__init__.py`.
- [ ] Add tests for preset loading and rendering a tiny script when `drawbot-skia` is installed.
- [ ] Keep `DesignBot` registered separately so Rust-native work can continue.

### Phase 2: Port Inline Script Editor

- [ ] Vendor or reuse local CodeMirror assets.
- [ ] Add a Runebender/DrawBot frontend extension equivalent to `js/drawbot-editor.js`.
- [ ] Hide the raw multiline widget when CodeMirror is active.
- [ ] Load selected preset source into `script_override`.
- [ ] Sync CodeMirror edits back into the Comfy widget value.

### Phase 3: Port DrawBot Visual Editor

- [ ] Port `core/drawbot_trace.py` and add `libcst` dependency handling.
- [ ] Add `/runebender/drawbot_preset`.
- [ ] Add `/runebender/drawbot_editor/preview`.
- [ ] Add `/runebender/drawbot_editor/patch`.
- [ ] Port `js/drawbot-visual-editor.js` under `web/src` or plain extension JS.
- [ ] Confirm preview image is byte-equivalent to the node's graph execution.
- [ ] Confirm dragging a literal rect/oval patches source and updates CodeMirror.

### Phase 4: Font Session / Save-Back Parity

- [ ] Decide whether Runebender needs a comfyfont-style WebSocket RPC layer or whether HTTP workspace snapshots are enough.
- [ ] Map Vue/WASM edits to concrete `.glif`/`.plist` writes.
- [ ] Save point movement, contour edits, widths, mark colors, and designspace metadata to the workspace.
- [ ] For linked sources, mirror supported text-file changes back to the original disk source.
- [ ] Add a clear dirty/clean state that only turns dirty after real edits.

### Phase 5: DesignBot Direction

- [ ] Define which comfyfont DrawBot behaviors DesignBot must match.
- [ ] Add DesignBot presets once the Rust language/runtime is stable enough.
- [ ] Decide whether DesignBot gets its own visual editor or reuses the DrawBot trace/editor concepts.
- [ ] Only retire or hide Python DrawBot after DesignBot reaches practical feature parity.

## Next Implementation Slice

Recommended next slice: Phase 1.

Start by porting the Python DrawBot node and presets. This gives us a working
font-to-image script workflow quickly and gives the future visual editor a real
node to attach to. It also makes the DesignBot work concrete: we can compare
Rust DesignBot against a known-good DrawBot baseline instead of guessing.
