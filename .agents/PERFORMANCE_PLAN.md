# Performance Plan — 60fps editor target

Audit date: 2026-06-10. Goal: the editor canvas should feel like a
60fps game — pan, zoom, drag, nudge all under 16ms/frame, with
snappy grid and startup. Reference: `runebender-xilem` (native) never
had these problems; this doc explains why and what to fix, in order.

Re-audit sources: `rust-core/src/renderer.rs`, `wasm_api.rs`,
`editor.rs`, `text.rs`, `web/src/Runebender.vue`, `vite.config.ts`,
and the xilem sibling repo. Line numbers are as of audit date —
re-grep before trusting them.

## TL;DR — why the port feels slower than xilem

1. **You are almost certainly running an unoptimized WASM build.**
   `pnpm wasm` is a `--dev` build (no optimization, debug info —
   `web/wasm/runebender_comfy_core_bg.wasm` is 12MB). The native
   xilem build you compare against is `cargo run --release`. Dev-vs-
   release for CPU-heavy Rust (Vello scene encoding, path building)
   is commonly a 5–20× gap. **This one item may explain most of the
   perceived difference.**
2. Even the release profile optimizes for **size, not speed**
   (`opt-level = "z"` in `rust-core/Cargo.toml`).
3. The ComfyUI dist bundle **inlines the wasm as base64** (Vite lib
   mode) — 17MB of JS, no streaming compilation, and it embeds
   whichever wasm was last built (currently the dev one).
4. Architecture is mostly right (on-demand rAF with dirty flags,
   coarse WASM calls, AutoVsync + frame latency 1). The remaining
   real costs are per-frame geometry rebuilds during pan/zoom,
   full-state clones the xilem version avoided via `Arc`, and an
   unvirtualized glyph grid the xilem version virtualized.

**Process rule: measure before and after every item.** Add the Tier 5
instrumentation first, then work top-down. Don't fix what the
profiler says is already cheap.

---

## Tier 0 — Build configuration (do these first; biggest wins, lowest risk)

- [x] **Profile with a release WASM build before anything else.**
      `pnpm wasm:release`, restart `pnpm dev`, hard-reload, and
      re-evaluate the perf complaints. Every later item should be
      measured against release builds only.
      *Done 2026-06-10: release wasm (12MB dev → 2.4MB) rebuilt into
      dist (17MB → 3.7MB); user confirmed noticeably better
      performance in ComfyUI via run-mac.sh.*
- [x] **Make the dev loop default to release wasm.** Keep a
      `wasm:debug` script for when you actually need debug info;
      `pnpm wasm` should produce the fast build so a stale dev
      binary can't silently ship or skew perception again.
      (`web/package.json:12-13`)
      *Done 2026-06-10: `pnpm wasm` now builds release; `wasm:debug`
      added; AGENTS.md build docs updated.*
- [x] **Switch the release profile to speed.** In
      `rust-core/Cargo.toml`: `opt-level = "z"` → `opt-level = 3`
      (try `"s"` if binary size regresses unacceptably), add
      `codegen-units = 1`. Keep `lto = true`. Vello's scene encoding
      and kurbo flattening are exactly the kind of hot CPU loops
      "z" hurts.
      *Done 2026-06-11: wasm 2.4MB → 2.7MB, dist 3.7MB → 4.2MB,
      well under 8MB cap. User confirmed good performance.*
- [ ] **Tune wasm-opt for speed.** wasm-pack runs `wasm-opt -O` by
      default; pin it explicitly in `rust-core/Cargo.toml`:
      `[package.metadata.wasm-pack.profile.release] wasm-opt =
      ["-O3"]` (try `-O4`).
- [ ] **Stop base64-inlining the wasm in the dist bundle.** Vite lib
      mode inlines all assets — `web/dist/runebender-comfy.js` is
      17MB because the 12MB wasm rides inside as base64. This kills
      `WebAssembly.instantiateStreaming` (compile-during-download),
      doubles memory at startup, and bloats ComfyUI page load. Emit
      the `.wasm` as a sibling asset in `dist/` and load by URL
      (copy step + `init(new URL(...))`, or a wasm plugin).
      (`web/vite.config.ts:15-25`)
- [x] **CI/test guard: dist must contain a release wasm.** The dist
      built today embeds the dev binary. Add a check (e.g. in
      `tests/test_web_bundle.py`) on wasm size or a build fingerprint
      so a dev build can't be committed into `dist/` again.
      *Done 2026-06-10: `test_built_bundle_embeds_release_wasm` caps
      the dist bundle at 8MB (release ≈ 3.7MB, debug embed ≈ 17MB).*

## Tier 1 — Per-frame Rust costs (pan/zoom/drag hot path)

- [ ] **Edit-controls cache misses on every pan/zoom frame.**
      `EditControlsCacheKey` includes the full view matrix
      (`view_coeffs`) — any pan or zoom changes the key, so all
      point/handle geometry for every contour is rebuilt every frame
      while navigating (`renderer.rs:295-316`, used by
      `draw_edit_controls` at `renderer.rs:783`). Fix: key on
      `(path_signature, selection_signature, zoom, point_scale)`
      only, build geometry relative to a stored reference transform,
      and apply the residual translation as the scene-level `Affine`
      at draw time. Pan then becomes ~free; zoom still rebuilds
      (point sizes are zoom-dependent — acceptable).
- [ ] **Outline signatures rehashed every frame.**
      `path_outline_signature(path)` hashes every point of every
      contour per frame even when nothing changed
      (`renderer.rs:1064-1124`). The editor already tracks
      `edit_revision` — store `(edit_revision, per-path signature)`
      alongside the cached `Rc<BezPath>` and skip rehashing when the
      revision is unchanged. Longer-term: per-path dirty flags /
      revision counters on `Path` itself instead of hashing.
- [ ] **Text layout recomputed per frame and per hit-test.**
      `state.text_buffer.layout(...)` is a full O(sorts) recompute
      with per-item kerning lookups, called in `draw_state`
      (`renderer.rs:688`), in `hit_test` (`text.rs:393-403`), and in
      several snapshot getters. Memoize the layout in `TextBuffer`
      behind a dirty flag (invalidate on mutation / line-height
      change). This is the main cost while the text tool is active.
- [ ] **Restrict Vello AA support to what's used.**
      `AaSupport::all()` compiles MSAA8/MSAA16 pipeline variants that
      are never used (render always passes `AaConfig::Area`) —
      startup shader-compilation time and GPU memory for nothing.
      Use `AaSupport::area_only()`. (`renderer.rs:526`,
      `renderer.rs:2046`)
- [ ] **Selection signature scans per path per frame.**
      `path_selection_signature` linearly scans the selection per
      contour per frame (`renderer.rs:312`). Cheap-ish, but free to
      fix alongside the cache-key rework: bump a selection revision
      counter on mutation and key on that.

## Tier 2 — JS↔WASM boundary and event path

- [ ] **Coalesce pointermove → one WASM call per frame.** Handlers
      call into WASM per event (`Runebender.vue:3877-3929`); mice and
      tablets deliver 120–1000Hz. rAF already coalesces *rendering*,
      but `pointerMove*` / `pointerMoveSelectionState()` run per
      event. Store the latest pointer state in the handler and do the
      WASM call inside the existing `requestRender` rAF callback
      (use `getCoalescedEvents()` if intermediate points ever matter
      for knife/pen). This also makes drag cost independent of input
      device Hz — the same trick xilem used (it throttled session
      rebuilds to every 3rd frame; canvas repaint stayed per-frame).
- [ ] **Restore Arc-style cheap snapshots for undo/drag.** xilem's
      `EditSession` held `paths: Arc<Vec<Path>>` + `glyph:
      Arc<Glyph>`, so undo snapshots and drag-begin clones were O(1).
      The port deep-clones the whole `EditorState` on pointer-down
      and on drag-threshold crossing (`wasm_api.rs:1963`, `:2005`),
      and per undo step (`wasm_api.rs:2231-2246`). Not per-frame, but
      it's per-gesture latency and unbounded undo memory
      (50 undos × full state). Wrap `paths` (and other heavy fields)
      in `Arc`/`Rc` with copy-on-write at the mutation sites.
      The nudge path already does snapshot-reuse correctly
      (`wasm_api.rs:2658-2690`) — extend that idea.
- [ ] **Replace hot JSON getters with typed arrays.**
      `textBufferState()` / `textBufferSnapshot()` serde-serialize
      JSON per call (`wasm_api.rs:1689-1711`), parsed at
      `Runebender.vue:3284` on text-tool actions.
      `textLayoutState()` already returns `Vec<f64>` — do the same
      (or cache the JSON string behind the text dirty flag) for the
      snapshot. Same pattern for `anchorContextAt` /
      `selectedAnchorInfo` (`wasm_api.rs:2089-2102`).
- [ ] **`glifCompatibility` double JSON round-trip.**
      `JSON.stringify(otherMasters)` → WASM → JSON string → parse
      (`Runebender.vue:1842`). Runs on glyph change, not per frame —
      fine for now, but it's on the glyph-switch latency path. Cache
      per-master parsed state on the Rust side keyed by master name +
      revision so switching glyphs doesn't re-send all masters.

## Tier 3 — Vue / DOM (grid mode and panels)

- [ ] **Virtualize the glyph grid.** The grid `v-for`s every glyph
      as a live component (`Runebender.vue:7323-7336`) — thousands of
      DOM nodes each containing an inline SVG; any parent re-render
      (selection change, svg map identity change) diffs all of them.
      xilem only built cells for the visible slice
      (`glyph_grid/mod.rs:281-325` in the sibling repo). Implement a
      windowed/virtual scroller (or adopt one). Cheap stopgap first:
      `content-visibility: auto; contain-intrinsic-size: <cell>` on
      `.glyph-cell` lets the browser skip offscreen layout/paint.
- [ ] **Don't pass `selected` as a per-cell prop diff over all
      cells.** Selection changes currently re-render the whole grid
      list. Provide the `selectedGlyphs` set via `provide/inject` or
      per-cell `computed` so only affected cells update.
- [ ] **`categoryCounts` is O(N×M) over all glyph names with nested
      `.filter()` per language group** (`Runebender.vue:620-669`).
      Recomputes on any `glyphNames` change. Compute counts in one
      pass per glyph (classify once, increment buckets), and only
      when the sidebar is visible.
- [ ] **Lazy-import heavy static data.** `gfSidebarData.generated.ts`
      (5.7k lines) is in the main bundle/parse path — dynamic-import
      it when the sidebar first opens.

## Tier 4 — Startup and load

- [ ] **Streaming wasm + smaller bundle** (covered in Tier 0 — the
      single biggest startup item).
- [ ] **Defer per-glyph SVG generation.** Font load runs
      metadata+SVG extraction for every glyph up front
      (`glifMapToSvgs` returns one big JSON of all SVG strings,
      `wasm_api.rs:475`). Generate SVGs in visible-window batches
      (or in chunks on idle) so a 2000-glyph UFO opens instantly.
- [ ] **Pipeline cache.** `wgpu::Features::PIPELINE_CACHE` is already
      requested when available (`renderer.rs:482`) but
      `RendererOptions.pipeline_cache: None` — wire it up if shader
      compile time shows up in startup traces (browsers also cache
      WebGPU pipelines themselves; measure first).

## Tier 5 — Instrumentation (do alongside Tier 0)

- [ ] **Frame-time overlay (debug-only).** `performance.mark/measure`
      around `editor.render()` and around the whole rAF callback;
      on-screen rolling p50/p95/worst frame ms + fps. A 60fps goal
      is unfalsifiable without this.
- [ ] **Rust-side timers.** Feature-gated `web_sys::Performance`
      timings for scene build vs `render_to_texture` vs present, so
      CPU encode and GPU submit can be told apart.
- [ ] **A repeatable stress scenario.** Script/checklist: load a
      large UFO, open a dense glyph (many contours), drag-select +
      drag 200 points, pan/zoom continuously, text mode with a long
      buffer. Record overlay numbers before/after each checklist
      item lands.

## Already right — don't "fix" these

- On-demand rAF render with dirty-flag coalescing
  (`Runebender.vue:1292-1341`); no continuous loop, no polling
  timers. Same model as Masonry's request_render.
- `PresentMode::AutoVsync` + `desired_maximum_frame_latency: 1` —
  correct latency/smoothness trade for an editor
  (`renderer.rs:512-513`).
- Intermediate texture + blit is **required** (Vello can't write the
  surface from compute) — not overhead to remove
  (`renderer.rs:2051-2065`).
- DPR-correct canvas sizing with ResizeObserver, resize only on
  actual dimension change (`Runebender.vue:1137-1155`, `:3588`).
- Nudge fast-path: snapshot reuse + `render_changed_paths` partial
  rebuild (`wasm_api.rs:1604-1621`, `:2658`).
- Single combined `BezPath` fill for counters (correctness +
  one fill call), batched metric-line strokes by color.
- `pointerMoveSelectionState` returning `Float64Array`, and
  `textLayoutState` as `Vec<f64>` — the right boundary pattern;
  Tier 2 just extends it.

## Expected outcome

Tier 0 alone should close most of the gap to xilem (it removes a
debug-build and size-opt handicap the native version never had).
Tiers 1–2 get pan/zoom/drag flat at 16ms on big glyphs. Tier 3 fixes
grid-mode jank on large fonts. Tier 4 is startup polish for the
ComfyUI embedding.
