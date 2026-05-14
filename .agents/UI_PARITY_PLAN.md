# UI Parity Plan: runebender-comfy → runebender-xilem

Goal: make `runebender-comfy` look and behave like `runebender-xilem`,
component for component, so a user moving between the two sees the
same tool. Phase 1 (palette + point colors) shipped in the last
commit. Everything below is still to do.

## Reference

Both screenshots in this plan come from 2026-05-13:

- **comfy** today: single-pane grid view with a help overlay,
  toolbar in the top-right, status pill bottom-left. Cells are 96px,
  glyph name only.
- **xilem** today: window-sized "bento" layout with a top file-info
  bar, master switcher, left category sidebar, large glyph cells
  with name + Unicode codepoint, right glyph-info sidebar, bottom
  mark-color panel.

xilem source map (everything we need to mirror):

```
src/views/glyph_grid/mod.rs       — bento layout assembly
src/views/glyph_grid/glyph_cell.rs — per-cell rendering + click
src/components/
  category_panel.rs               — left sidebar
  glyph_info_panel.rs             — right sidebar (top)
  glyph_anatomy_panel.rs          — right sidebar (bottom)
  mark_color_panel.rs             — bottom-left color swatches
  master_toolbar.rs               — master switcher
  system_toolbar.rs               — save/system buttons
  grid_scroll_handler.rs          — keyboard + wheel routing
src/theme.rs                       — every color + size constant
```

xilem's bento layout uses `BENTO_GAP = 6.0` everywhere. Adopt the
same gap in comfy CSS.

## Architecture: where things should live

Right now `Runebender.vue` is one ~700-line file that handles grid +
editor + drag-drop + keyboard + file I/O. To get to xilem's
component-per-panel feel we need to decompose. Two reasonable shapes:

1. **One file per panel** — `TopBar.vue`, `CategorySidebar.vue`,
   `GlyphCell.vue`, `GlyphGrid.vue`, `GlyphInfoSidebar.vue`,
   `MarkColorBar.vue`, etc., orchestrated by `Runebender.vue` as a
   thin layout root.
2. **Sections inside `Runebender.vue`** with `<!-- ===== -->`
   dividers, no extra component files. Less ceremony, easier to
   refactor across panels.

Recommendation: **shape (1)**, because it maps 1:1 to xilem's
`components/*.rs` files. Each xilem component gets a Vue sibling
with the same name. Makes future "look at xilem's `glyph_info_panel.rs`
and copy what changed" trivially obvious.

State management: lift the shared state (loaded UFO, current glyph,
category filter, selection, master, …) into a Pinia store
(`useEditorStore`) or just a `provide`/`inject` object. Each panel
becomes pure presentation.

## Phase order

Ordered by visible impact + lift. Each phase is independently
ship-able.

### Phase 1 — palette + point colors  ✅ done

- renderer.rs palette mirrors `theme.rs` (BG, fills, points,
  marquee, metric guides, stroke widths).
- Point shapes/colors by `PointType` (smooth blue circle / corner
  green square / off-curve purple circle / selected yellow+orange).
- Vue CSS switched from warm-cream to grayscale + green accents.

### Phase 2 — grid cell redesign

xilem cells (left screenshot): tall tile, glyph centered, **glyph
name + Unicode codepoint stacked underneath**, larger overall, more
breathing room. Selected cell has filled green-tinted background
(`#146414`) + green outline (`#66EE88`).

Concrete changes:

- Component: `GlyphCell.vue` (extract from `Runebender.vue`).
- Layout: 144×170 cells (vs current 96×110), 6px gap.
- Render glyph at the top in `GRID_GLYPH_COLOR` (`#a0a0a0`).
- Underneath: glyph name (`#909090`) + Unicode hex (`#707070`) on
  two lines. Pull Unicode from the .glif's `<unicode hex="..."/>`
  (extend `parseGlyphName` in `Runebender.vue` to also extract
  codepoints).
- Hover: outline brightens to `#66EE88` (xilem behavior).
- Selected: `#146414` fill + `#66EE88` outline.

xilem reference: `glyph_cell.rs` line 1-200.

### Phase 3 — category sidebar (left)

The list: **All / Letter / Number / Punctuation / Symbol / Mark /
Separator / Other**. xilem derives category from Unicode general
category property (`unicode-general-category` crate).

Concrete changes:

- Component: `CategorySidebar.vue` (left, 200px wide).
- Same `unicode-general-category` mapping. Either:
  - **Rust-side**: add the crate to `runebender-comfy-core`, expose
    `glyph_category(codepoint) -> category` via wasm-bindgen.
  - **JS-side**: use `Intl.Segmenter` + a hand-coded category map.
  Rust-side keeps logic shared with xilem. **Recommended.**
- State: `useEditorStore` gets `categoryFilter: Category` + filtered
  `visibleGlyphs` computed prop. Grid renders only `visibleGlyphs`.
- xilem reference: `components/category_panel.rs`.

### Phase 4 — top file-info bar

Top row of xilem's layout: full file path + "Not saved" indicator on
the left (stretches), master switcher in the middle-right, save
button on the far right.

Concrete changes:

- Component: `TopBar.vue` (row, full width).
- Display: UFO/designspace path + dirty indicator (`#FFDD33` yellow,
  matches xilem `MARK_YELLOW`).
- For now, "Not saved" can be hardcoded text since we have no
  save yet. Mark as TODO.
- xilem reference: `views/glyph_grid/mod.rs` `file_info_panel`,
  `components/system_toolbar.rs`.

### Phase 5 — glyph info sidebar (right)

xilem shows the SELECTED glyph's details: Master, Glyph Name, Width,
Kerning Groups (Left/Right), Unicode, Contours count.

Concrete changes:

- Component: `GlyphInfoSidebar.vue` (right, 240px wide).
- Plumbing: when the user clicks a cell (or arrow-keys onto one) the
  store updates `selectedGlyph`. Sidebar reads from it.
- "Contours 0" updates live if the user is in the editor.
- Below it, xilem has `glyph_anatomy_panel` (an empty box in the
  screenshot — probably reserved for the live preview of the
  in-editing glyph). Stub it as an empty card for now.
- xilem reference: `components/glyph_info_panel.rs`,
  `components/glyph_anatomy_panel.rs`.

### Phase 6 — mark-color panel (bottom-left)

Seven color swatches + an X (clear) for tagging glyphs by workflow
state. xilem uses `MARK_RED / MARK_ORANGE / MARK_YELLOW /
MARK_GREEN / MARK_BLUE / MARK_PURPLE / MARK_PINK` plus a clear
button. UFO stores the mark color as `public.markColor` per glyph.

Concrete changes:

- Component: `MarkColorBar.vue` (bottom-left, under the category
  sidebar).
- norad has `Glyph.lib` for arbitrary UFO data including
  `public.markColor`. Already in the data model.
- Clicking a swatch sets the mark color on the current glyph; clear
  removes it. Grid cell background tints by the mark color.
- xilem reference: `components/mark_color_panel.rs`.

### Phase 7 — designspace / master support

xilem opens `.designspace` files directly and shows a master switcher
(`n`, `n`, `n` buttons with the active one outlined green). Comfy
currently refuses `.designspace`.

This is the biggest single feature. Pulls in:

- Parse `.designspace` XML (norad has `Designspace::load_data`).
- Load each referenced `.ufo`.
- Master-switcher UI in TopBar.
- Per-master glyph data + current-master state.

Defer until Phases 2–6 are done.

### Phase 8 — remove dev help overlay + welcome screen

The "Runebender dev preview" text overlay only exists in `web/index.html`
for the standalone dev page. xilem doesn't have it; remove (or move
into a `?` popup keyed off `F1`) once the editor's affordances are
self-evident from the UI.

xilem also has a `views/welcome.rs` for first-launch state. Comfy's
drop-hint is the equivalent — refine to match xilem's welcome
visually if we ever bring xilem's welcome screen forward.

## Theme architecture (revisit after Phase 2)

When 2–6 stabilize, do the previously-agreed theme work:

1. Define `runebender-core/themes/runebender.json` — every named
   color from xilem's `theme.rs` plus sizes. Schema mirrors
   ComfyUI's palette JSON for adapter-compatibility.
2. Both editors load this at startup:
   - xilem: `theme.rs` becomes a thin "load JSON, populate consts"
     wrapper.
   - comfy: load on init, push CSS custom properties for chrome,
     push values into the wasm core for Vello.
3. ComfyUI adapter: detects ComfyUI's `comfy_base.bg-color` etc. and
   remaps onto runebender's semantic tokens.

Why wait: doing the file/loader machinery before the UI is built
means designing the schema with incomplete information about what
tokens we actually need.

## Open decisions

- **Component vs section file shape** — committing to one-Vue-file
  -per-xilem-component (rec'd) requires reorganizing `Runebender.vue`.
  ~1 hour of refactor cost before any new UI lands.
- **Unicode category source** — Rust crate (`unicode-general-category`)
  shared with xilem, or JS lookup table. Rust-side is the right
  answer long-term but adds wasm bytes.
- **Pinia vs `provide`/`inject` for shared state** — Pinia adds a
  dep, `provide` is dep-free. For a single store, `provide` is
  enough; Pinia pays off only once we have 3+ stores.
- **Mark-color tinting in the grid** — cell background tints by mark
  color, OR just a corner dot? xilem appears to tint the entire cell
  faintly; check `glyph_cell.rs` before implementing.

## Sequencing recommendation

Phases in order, batched into commits like before. After **Phase 2**
the editor will already look noticeably more xilem-like (bigger
cells, name + codepoint). After **Phase 4** it'll feel structurally
xilem-shaped. Phases 5–6 are polish that pushes parity to ~90%.
Phase 7 (designspace) is its own commitment and can wait.

Estimated effort, very rough:

| Phase | Effort |
|---|---|
| 2 — cells | 1–2 hours |
| 3 — categories | 2–3 hours (incl. Rust category fn) |
| 4 — top bar | 1 hour |
| 5 — info sidebar | 2 hours |
| 6 — mark colors | 1–2 hours |
| 7 — designspace | half-day to a day |
| 8 — overlay cleanup | 15 minutes |

Total to ~90% parity (without designspace): a focused day.
