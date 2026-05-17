# UI Parity and Next Tasks

Goal: make `runebender-comfy` look and behave like
`runebender-xilem`, component for component, so a user moving between
the two sees the same tool. This file is the active task list for
the ComfyUI port.

Updated from the current codebase on 2026-05-14. Keep this file
current instead of creating parallel task lists.

## Current Baseline

- UI parity Phases 1-6 are mostly implemented: bento layout, category
  sidebar, top bar, glyph info sidebar, mark color panel, and
  xilem-ish cells exist.
- Designspace/master loading exists in `web/src/Runebender.vue`; the
  top bar can switch masters, and active glyphs show multi-master
  interpolation compatibility diagnostics.
- Edit mode has a toolbar. Select, Pen, HyperPen, Knife, Measure,
  Preview, Shapes, and the Text buffer/session shell are functionally
  wired.
- Standalone Vite remains the main dev loop. ComfyUI integration now
  has a live editor-state bridge and a `FONT` wire contract, but
  broader workspace management and node polish still need work.
- Save/export is wired for dirty glyphs, groups, and kerning across
  loaded masters. Dropped UFO handles and Comfy workspace slots write
  back when available, with browser export fallbacks when no writable
  route exists.

## Reference Map

xilem source map for parity work:

```text
src/views/glyph_grid/mod.rs        - bento layout assembly
src/views/glyph_grid/glyph_cell.rs - per-cell rendering + click
src/components/
  category_panel.rs                - left sidebar
  glyph_info_panel.rs              - right sidebar top
  glyph_anatomy_panel.rs           - right sidebar bottom
  mark_color_panel.rs              - bottom-left color swatches
  master_toolbar.rs                - master switcher
  system_toolbar.rs                - save/system buttons
  grid_scroll_handler.rs           - keyboard + wheel routing
  coordinate_panel.rs              - point coordinate display
  transform_panel.rs               - selection transform controls
src/theme.rs                       - colors and size constants
```

Comfy source map:

```text
web/src/Runebender.vue             - layout root and current state owner
web/src/components/*.vue           - Vue peers for xilem panels
rust-core/src/wasm_api.rs          - JS/Vue API surface
rust-core/src/editor.rs            - editable glyph state
rust-core/src/renderer.rs          - Vello/WebGPU rendering
nodes/*.py                         - ComfyUI Python nodes
```

For broader routing, see `.agents/REPO_MAP.md`.

## Completed / Mostly Completed Parity Phases

### Phase 1 - palette + point colors

- Renderer palette mirrors xilem's grayscale + green-accent theme.
- Point shapes/colors dispatch on `PointType`.
- The editor canvas draws xilem's zoom-dependent design grid behind
  metrics and outlines.
- Vue CSS uses the same broad palette.

### Phase 2 - grid cell redesign

- `GlyphCell.vue` exists.
- Cells show glyph preview, glyph name, Unicode, hover state, selected
  state, and mark-color tinting.
- Grid cells now use xilem's 16px two-line label styling and apply
  mark colors to the cell outline, glyph, and labels instead of
  tinting the background.
- Grid packing now follows xilem's span-aware bento rules: the grid
  caps at eight columns, long names or wide glyphs span multiple
  columns based on `fontinfo.plist` units per em, and row-ending cells
  expand to absorb leftover columns.

### Phase 3 - category sidebar

- `CategorySidebar.vue` exists.
- Categories use `runebender-core::GlyphCategory` via wasm helper
  `glyphCategoryForCodepoint`.
- Grid filtering is wired through Vue computed state.
- Grid arrow-key navigation, Enter-to-open, and Cmd/Ctrl+S/C/V
  routing now mirror xilem's grid scroll handler.
- Grid shift-click range selection now mirrors xilem's multi-select
  behavior, with grid mark-color and paste actions applying to the
  selected glyph set.
- Loading a font or switching/filtering masters now keeps the grid
  selection valid and selects the first visible glyph when needed.
- Switching category filters now resets the grid scroll position to
  the top, matching xilem's `grid_scroll_row = 0` behavior.
- Category sidebar typography now matches xilem's 16px header/row
  text and neutral header color, with the selected-row highlight
  carrying the green accent.
- Category sidebar selected rows now use xilem's outline-only
  highlight with 4px horizontal inset and 10px header top spacing.

### Phase 4 - top file-info bar

- `TopBar.vue` exists.
- Font/designspace label, unsaved indicator, multi-master switcher,
  and icon-only system toolbar are present.
- The master switcher uses rendered `n` preview SVGs and is hidden for
  single-master fonts, matching xilem's master-toolbar behavior.
- Save is enabled when there are dirty glyph, groups, or kerning
  changes.
- The dirty indicator only appears after an in-memory mutation.
- The top file-info tile now follows xilem's stacked 16px file/status
  row treatment instead of the earlier compact single-line browser
  variant.
- The top file-info tile now always shows xilem's save-status row for
  loaded fonts: yellow `Not saved` until a clean save, then green
  `Saved HH:MM`.
- The system save toolbar is now enabled whenever a font is loaded,
  matching xilem's always-available workspace save action instead of
  only enabling for dirty browser state.

### Phase 5 - glyph info sidebar

- `GlyphInfoSidebar.vue` exists.
- Master, glyph name, Unicode, width, contours, and kerning group
  rows are present. Unicode display preserves all codepoints on a
  glyph, matching xilem's comma-separated glyph-info panel behavior.
- Width and contours now populate for selected glyphs without opening
  them.
- Kerning groups now populate from `groups.plist`.
- The right glyph-info column uses xilem's 220px panel width.
- Glyph info typography and empty-state text now match xilem's 16px
  rows, `(single UFO)` master label, and `No Selection` glyph fields.
- The anatomy panel is now an untitled x-ray canvas with xilem's
  uniform 16px internal padding, instead of a labeled browser card.

### Phase 6 - mark-color panel

- `MarkColorPanel.vue` and `markColors.ts` exist.
- Grid cells tint from `public.markColor`.
- Mark changes update in-memory `MasterData`.
- Mark changes serialize back into `.glif` data and participate in the
  multi-master save route.
- Mark color panel typography, swatch size, gap, and 66px panel
  height now match xilem's `mark_color_panel.rs` constants.
- Clear/default mark color no longer draws a selected ring, matching
  xilem's mark panel behavior.
- Mark color panel header/swatch offsets and active/hover rings now
  match xilem's constants, including the white selected swatch ring.

### Phase 7 - designspace / master support

- `.designspace` drops are parsed in the browser.
- Referenced UFO masters are resolved from dropped files.
- The top bar switches active masters.
- Designspace source filenames now resolve through normalized relative
  paths, including `./`, `..`, and Windows-style separators.
- Designspace master lookup now also tolerates common drag/drop path
  quirks by resolving case-insensitive UFO roots and unique UFO
  basenames when the exact normalized relative path is unavailable.
- Remaining gap: decide whether designspace parsing should eventually
  move to Rust.

### Phase 8 - edit mode and welcome polish

- `EditModeToolbar.vue` and `WelcomePanel.vue` exist.
- Welcome now mirrors xilem's first-run structure more closely: a
  compact upper-left welcome panel sits over an interactive demo `R`
  glyph rendered with xilem's demo viewport constants in the editor
  canvas when no font is loaded.
- The welcome panel now has an `Open UFO...` action wired to a browser
  directory picker and the existing UFO loader, matching xilem's
  first-run open affordance instead of requiring drag-and-drop only.
- The welcome picker now prefers the File System Access directory API
  so opened UFO/designspace folders retain writable file handles for
  save/write-back, with the older directory input kept as fallback.
- The top file-info bar is hidden while no font is loaded, so the
  welcome screen occupies the editor surface like xilem.
- Select tool is functional.
- Pen supports click-to-add corner points, live next-segment preview,
  close-by-clicking the start point, and drag-to-create smooth points
  with mirrored handles.
- HyperPen supports click-to-add smooth hyperbezier points, live
  next-segment preview, and close-by-clicking the start point.
- Preview pans the viewport.
- Preview renders a clean xilem-style filled glyph/text view without
  edit grid, metrics, handles, points, or transient tool overlays.
- Shapes has a rectangle/ellipse sub-toolbar, live drag preview, and
  drag-to-create outline behavior.
- Measure supports drag-to-measure with distance/angle feedback,
  intersection dots, per-segment length labels, and Shift axis
  constraint.
- Knife supports drag-preview with outline intersection markers and
  mouse-release contour splitting for cubic outlines.
- Text is an active tool with the xilem-style LTR/RTL direction
  sub-toolbar, typed-character insertion into a bottom text buffer
  strip, cursor movement/deletion, line breaks, kerning-aware preview
  spacing, basic Arabic positional shaping, and double-click sort
  activation. Preview-strip sorts can be selected and move the text
  cursor.
- Tool keyboard switching mirrors xilem for V/P/H/K, and holding
  Space temporarily switches to Preview before returning to the
  previous tool.
- Editor toolbars now float over the canvas at the top-left like
  xilem instead of consuming a persistent left layout column.
- Editor mode now hides the grid top bar and right glyph-info sidebar,
  matching xilem's full-surface editor layout with floating overlays.
- The editor has xilem's top-right master/workspace toolbar row:
  multi-master designspaces can switch masters in editor mode, and the
  glyph-grid button returns from an open glyph to the grid.
- Toolbar components now use xilem's shared toolbar sizing: 48px
  buttons, 8px panel padding, 6px item gaps, and 64px toolbar panels.
- The edit-mode toolbar now uses filled glyph-like tool icons instead
  of the earlier stroked browser placeholders, matching xilem's icon
  treatment more closely while preserving the shared tool ordering and
  identifiers.
- Coordinate panel exists as a bottom-right editor overlay with an
  interactive reference picker and editable X/Y fields.
- Coordinate panel reference dots now stay selectable even without an
  active selection, and W/H stay blank for single-point selections,
  matching xilem's coordinate display rules.
- Coordinate panel picker dots now sit on the 3x3 grid intersections
  with center guide lines, matching xilem's quadrant picker geometry.
- Coordinate panel hit targets now cover the full 3x3 picker zones,
  matching xilem's grid-based quadrant hit testing.
- Transform panel exists as a right-side editor overlay for transform
  actions.
- Transform panel now matches xilem's dedicated compact action grid;
  selection bounds are owned by the coordinate panel instead of being
  duplicated in the transform panel.
- Transform panel tooltips now use xilem's labels for Duplicate
  Repeat and Exclude/XOR.
- Flip horizontal / vertical are wired through wasm with undo support.
- Rotate 90 CW / CCW are wired through wasm with undo support.
- Duplicate is wired through wasm with undo support.
- Duplicate Repeat is wired through wasm with undo support.
- Remaining gap: Text still needs broader script shaping coverage
  beyond the current xilem-aligned Arabic joining subset. Knife
  splitting now handles cubic, quadratic, and hyperbezier outlines,
  with cut hyperbeziers converted to explicit cubic contours.

## Active Task Queue

### Done 2026-05-15 - Persist Text manual kerning edits

Rust now exposes the live Text kerning model after manual kerning
drags. Vue syncs edited kerning back into `MasterData.kerning`, tracks
kerning dirty state separately from glyph dirty state, serializes a
UFO-compatible `kerning.plist`, and writes it through the workspace
save route or an existing dropped-file handle when available.

Follow-up completed 2026-05-15: when a dirty non-empty kerning model
has no workspace path or writable file handle, save now offers a
browser picker/download for `kerning.plist`.

### Done 2026-05-15 - Make coordinate W/H fields scale selections

The coordinate panel's W and H fields now apply transforms instead of
being read-only. Rust scales selected points around the active
coordinate reference point, wasm exposes `resizeSelectionReference()`,
and Vue routes W/H commits through the same dirty-state, `.glif`
serialization, preview refresh, and Comfy sync path as X/Y moves.

Follow-up completed 2026-05-15: point and component W/H scaling now
have focused Rust coverage, including centered width scaling for
selected points and centered height scaling for selected components.

Follow-up: compare the interaction against xilem if/when xilem's
display-only W/H fields become editable too.

### Done 2026-05-15 - Add selected-point delete and type toggle shortcuts

Editor-mode Backspace/Delete now removes selected points through Rust
with undo support and `.glif` reserialization. Cubic off-curve handle
deletion also removes the paired handle so the segment collapses to a
line instead of leaving a malformed one. Pressing `T` while points are
selected toggles selected on-curve points between corner and smooth;
with no selection it still switches to the Text tool.

Follow-up completed 2026-05-15: selected outline copy/paste and
Pen/HyperPen insert-point-on-segment behavior are wired, including
line/cubic/quadratic segment insertion.

### Done 2026-05-15 - Add xilem-style zoom keyboard shortcuts

Editor mode now handles Cmd/Ctrl `+` and `-` for zoom in/out and
Cmd/Ctrl `0` for fit-to-canvas, matching xilem's viewport keyboard
shortcuts. Rust exposes the current viewport zoom over wasm so Vue can
apply multiplicative zoom steps without guessing.

Follow-up completed 2026-05-15: keep keyboard zoom behavior aligned
with xilem. Keyboard zoom changes the zoom scalar in place, while
wheel zoom preserves the cursor design point.

### Done 2026-05-15 - Add option-click line-to-curve conversion

Select tool now mirrors xilem's Option/Alt-click segment behavior for
cubic outlines. Alt-clicking a line segment inserts two selected
off-curve handles at one-third and two-thirds along the line, turning
the segment into an editable cubic curve. Alt-drag still pans when no
line segment is hit.

Follow-up completed 2026-05-15: Pen/HyperPen can insert points on
line, cubic, and quadratic segments, with line insertion snapping to
the 2-unit design grid.

### Done 2026-05-15 - Add Pen insert-point-on-segment snapping

Pen now mirrors xilem's curve snapping insertion path for existing
outlines when no pen contour is active. Clicking near a line, cubic,
or quadratic segment inserts an on-curve point at the nearest segment
parameter. Cubic and quadratic segments are subdivided in-place with
the correct off-curve handles preserved around the inserted point, and
the new on-curve point becomes selected.

Follow-up completed 2026-05-15: Pen and HyperPen share the snapped
segment insertion path and draw hover insertion-ring previews.

### Done 2026-05-15 - Add selected outline copy/paste shortcuts

Editor mode now supports Cmd/Ctrl+C and Cmd/Ctrl+V for selected
outline data. Copy stores selected contours, or selected on-curve
points plus adjacent off-curve handles for partial contour selections,
in a session-local wasm clipboard. Paste appends fresh-ID paths at the
same coordinates, closes pasted paths to match xilem behavior, selects
the pasted points, and flows through undo, dirty-state, `.glif`
serialization, preview refresh, and Comfy sync.

Follow-up completed 2026-05-15: copy/paste now shows transient
in-editor clipboard feedback, including copied selected-point counts
and pasted-outline confirmation. External clipboard exposure remains a
separate product decision because the current wasm clipboard is scoped
to the editor session.

Follow-up completed 2026-05-15: editor copy/paste now shows a
transient canvas badge confirming selected-point copy and outline
paste actions, so partial-selection clipboard operations are visible
without changing the session-local clipboard contract.

### Done 2026-05-15 - Reuse segment insertion from HyperPen

HyperPen now mirrors xilem's snapped-segment click behavior when it is
idle. Clicking near an existing line, cubic, or quadratic segment uses
the same Rust segment insertion path as Pen instead of starting a new
hyper path. Active hyper path drawing and close behavior are unchanged.

### Done 2026-05-15 - Add Pen and HyperPen segment snap previews

Pen and HyperPen now show xilem-style hover feedback when idle over an
existing segment. The shared Rust preview carries a snap target, both
tools move the preview dot to the evaluated segment point, and the
renderer draws an insertion ring so users can see where a clicked
point will land before mutating the outline.

Follow-up completed 2026-05-15: line-segment insertion now snaps the
new on-curve point and idle preview dot to xilem's 2-unit design grid.

Follow-up completed 2026-05-15: Select now shows the xilem-style
Option/Alt-hover segment highlight before line-to-curve conversion.

Follow-up completed 2026-05-15: Pen drag-start now respects xilem's
guard rails for close targets and snapped existing segments. Dragging
from the start-point close zone no longer appends a smooth point
instead of preserving the pending close action.

Follow-up completed 2026-05-15: Pen and HyperPen close-zone hit
testing now uses xilem's 20 design-unit threshold instead of shrinking
or growing with viewport zoom.

### Done 2026-05-15 - Add xilem editor transform shortcuts

Editor mode now handles the next set of xilem keyboard actions:
bare `R` reverses contours, Cmd/Ctrl+Shift+H converts selected
HyperBezier contours to explicit cubic outlines, Shift+H/V flips the
selection, Cmd/Ctrl+Shift+R/L rotates, Cmd/Ctrl+D duplicates,
Cmd/Ctrl+Shift+D duplicates with repeat-transform, and
Cmd/Ctrl+Shift+O removes overlap through the existing union path.
Reverse-contour and HyperBezier conversion are exposed from Rust over
wasm and participate in undo, dirty-state tracking, `.glif`
serialization, preview refresh, and Comfy state sync.
The pass also wires Cmd/Ctrl+S to the existing save route and
Ctrl+Space to xilem's persistent Preview-tool switch.

Follow-up completed 2026-05-15: Comfy now has the image shortcuts
that match its browser-local background image model: Cmd/Ctrl+I opens
image import, Cmd/Ctrl+L toggles lock, arrows nudge an unlocked
selected image, and Delete removes it. Trace/refit shortcuts remain
blocked on a browser/local vectorization backend.

Follow-up completed 2026-05-15: Cmd/Ctrl+T, Cmd/Ctrl+Shift+T, and
Cmd/Ctrl+Shift+Y now get intercepted in editor mode and report the
current browser-editor trace/refit limitation instead of silently doing
nothing. Cmd/Ctrl+Y redo is limited to the unshifted shortcut so it
does not shadow the Quiver trace shortcut.

Follow-up completed 2026-05-15: the browser background-image selection
overlay now uses xilem's blue selection border and handle stroke
instead of the orange point-selection color.

Follow-up completed 2026-05-15: locked background images still receive
pointer events for the context menu, so right-clicking a locked image
can show the xilem-style Unlock Image action while drag/resize remains
blocked.

Follow-up completed 2026-05-15: locked background images are now
transparent to normal canvas pointer input, matching xilem's tool
pass-through behavior. The browser editor preserves the Unlock Image
context menu by hit-testing locked image bounds from the canvas
context-menu route instead of relying on the locked DOM overlay to
receive pointer events.

Follow-up completed 2026-05-15: locked background image context menus
now follow xilem's priority order. Right-clicking a contour under a
locked image opens the contour actions first, while right-clicking
empty image area falls back to Unlock Image.

Follow-up completed 2026-05-15: contour context menus now also fall
back to the first selected on-curve point when the right-click misses
a point, matching xilem's selected-point context behavior for Set
Start Point, Reverse Contour, and contour reordering.

Follow-up completed 2026-05-15: contour reorder context-menu labels
now include the source and destination contour indices, matching
xilem's `Move Contour Up (i -> j)` / `Move Contour Down (i -> j)`
affordance.

Follow-up completed 2026-05-15: background images now pass pointer
events through while Preview/pan mode is active, matching xilem's
`is_preview_mode()` guard so image overlays do not block viewport
panning.

Follow-up completed 2026-05-15: changing tools now immediately
recomputes the browser background-image frame, so the Preview-mode
pointer pass-through state updates as soon as the toolbar/keyboard
switch happens instead of waiting for another viewport refresh.

### Done 2026-05-15 - Add editor panel visibility toggle

Editor mode now mirrors xilem's Tab shortcut for hiding auxiliary
panels while leaving the tool palette available. Pressing Tab outside
Text mode toggles the coordinate and transform overlays, giving the
canvas more room for outline work without changing the active tool.

### Done 2026-05-15 - Add bottom active-glyph editor panels

The editor canvas now has xilem-style bottom panels outside Text mode:
a compact active glyph preview on the lower left and a centered
active glyph metrics panel showing glyph name, Unicode, width, and
kerning groups. These panels use the same live Vue/Rust metadata path
as the sidebars, so outline edits that update width/contour metadata
also refresh the bottom editor chrome.

Follow-up completed 2026-05-15: the active-glyph panel now edits
width, sidebearings, glyph name, Unicode, kerning groups, and Text
neighbor kerning pairs.

Follow-up completed 2026-05-15: the lower-left glyph preview and
centered active-glyph metrics panel now use xilem's 140px panel height
instead of the earlier taller browser-specific placeholder height.

Follow-up completed 2026-05-15: the lower-left glyph preview width
now follows xilem's aspect-ratio fitting rule, clamping between 60px
and 200px based on the active glyph SVG viewBox instead of staying at
a fixed width for every glyph.

### Done 2026-05-15 - Make active glyph width editable

The centered active-glyph panel now has an editable Width field. Rust
owns the advance-width mutation, exposes it through wasm as
`setAdvanceWidth()`, records undo snapshots, and serializes the new
width into the active glyph `.glif`. Vue routes width commits through
the same dirty-state, metadata refresh, Text inventory refresh, render,
and Comfy state-sync path as other glyph mutations.

Follow-up completed 2026-05-15: sidebearing, glyph-name, Unicode,
kerning-group, and Text neighbor kerning pair edits are now wired.

Follow-up completed 2026-05-15: the active-glyph panel now matches
xilem's three-row input structure without extra explanatory label
rows under the sidebearing/kerning fields.

Follow-up completed 2026-05-15: the active-glyph panel now also uses
xilem's exact 488px panel width and fixed row tracks: 346/110, 4 x
110, and 149/150/149 with 8px gaps.

### Done 2026-05-15 - Make active glyph kerning groups editable

The centered active-glyph panel now edits left/right kerning group
membership, matching xilem's active glyph panel direction. Vue updates
the active master's `groups.plist` model, rebuilds per-glyph group
metadata, pushes the new groups into Rust Text kerning layout, marks
groups dirty, and writes `groups.plist` through the workspace route or
an existing dropped-file handle on save.

Follow-up completed 2026-05-15: when dirty non-empty groups data has
no workspace path or writable file handle, save now offers a browser
picker/download for `groups.plist`.

### Done 2026-05-15 - Add active glyph Text kerning fields

The centered active-glyph panel now includes xilem-style left/right
Kern fields around the LSB/RSB fields. In Text mode the panel moves
above the bottom text buffer, reads the active sort's previous/current
and current/next glyph pairs, resolves UFO direct/group kerning with
the same precedence as Rust Text layout, and writes edits back into
the active master's `kerning.plist` model. Empty, `-`, and zero values
remove the direct pair; non-zero values mark kerning dirty, resync the
Rust Text kerning model, reshape the preview strip, and update the
Comfy state bridge.

Follow-up completed 2026-05-15: Text sort activation now loads the
active sort glyph into the outline editor without refitting the
canvas, so the active-glyph panel and its Kern fields track the same
glyph that is selected in the Text buffer. The panel hides in Text
mode when no glyph sort is active.

Follow-up completed 2026-05-15: Text reshaping now reloads the active
sort glyph after the Rust buffer snapshot is refreshed, so Arabic
positional substitutions and other reshape-driven glyph-name changes
keep the outline editor and active-glyph panel aligned with the
visible Text sort.

Follow-up completed 2026-05-15: double-clicking a component while the
Text tool is active now appends that component's base glyph into the
Text buffer, matching xilem's component-to-sort editing path.

Follow-up completed 2026-05-15: Text layout line spacing now uses the
active font metric box height (`ascender - descender`) in both the
canvas renderer and Vue preview strip, matching xilem's Text session
layout instead of falling back to UPM/1000.

Follow-up completed 2026-05-15: the editable glyph layer now follows
the active Text sort's Rust layout origin. Metrics, filled outlines,
component previews, handles, points, Select hit-testing, marquee
selection, contour context actions, component double-clicks, and point
double-click toggles use the same active-sort coordinate frame instead
of leaving the active sort's editable outline at the glyph origin.

Follow-up completed 2026-05-15: renderer dispatch now mirrors xilem's
single-glyph versus Text-buffer paint split. When a Text session is
active, Comfy renders the Text buffer as the owning glyph layer instead
of painting the standalone active glyph underneath it; the active sort
still uses live editable paths/components so in-progress outline edits
remain visible immediately.

Follow-up completed 2026-05-15: Text-buffer rendering now receives the
active Text-mode flag from the Rust tool state. Outside Text mode,
Comfy hides the Text insertion cursor and draws xilem-style full
metrics for the active editable sort while keeping inactive sorts on
minimal metric markers.

Follow-up completed 2026-05-15: active sort point/handle controls are
now hidden while the Text tool is active. Switching back to an outline
editing tool restores editable controls for the active sort, matching
xilem's Text-buffer versus single-sort editing split.

Follow-up completed 2026-05-15: the wasm Text API now treats
codepoint `0` as the browser-side "no Unicode" sentinel for glyph-name
sort insertion and sort metadata updates. Glyph-by-name sorts,
including component-to-sort insertion, no longer carry an accidental
U+0000 character through Text snapshots or shaping.

Follow-up completed 2026-05-15: typed-character insertion now goes
through Rust Text session state instead of Vue doing the Unicode-to-
glyph lookup. The core uses the active glyph inventory to create the
sort, applies RTL Arabic shaping immediately, returns a miss when the
font has no glyph for the character, and Vue only refreshes the
snapshot/editor selection afterward. Focused Rust tests cover inventory
lookup and RTL neighbor reshaping.

Follow-up completed 2026-05-15: multiline Text buffers now support
vertical ArrowUp/ArrowDown cursor movement through Rust. The cursor
preserves its visual X position as it moves between text lines and
clamps at the first/last line, with Vue routing the keys through the
same snapshot refresh path as horizontal movement.

Follow-up completed 2026-05-15: Text-mode Home/End are now Rust-owned
line-boundary moves instead of whole-buffer jumps. This makes the
multiline Text buffer behave like an editor session once line breaks
are present while preserving snapshot-driven Vue rendering.

Follow-up completed 2026-05-15: Text-mode printable key repeats now
insert repeated sorts instead of being filtered out by Vue. This
matches xilem's Text key handling, where repeated character keydown
events continue through the normal insertion path.

Follow-up completed 2026-05-15: edit toolbar titles now expose the
remaining xilem keyboard affordances for HyperPen (`H`), Knife (`K`),
and temporary Preview (`Space`) instead of leaving those buttons
without shortcut hints.

Follow-up completed 2026-05-15: Space no longer triggers temporary
Preview while the Text tool is active, matching xilem's Text-mode
guard. Space is now handled only by Text insertion/missing-glyph
feedback in Text mode instead of accidentally switching tools when the
font has no space glyph.

Follow-up completed 2026-05-15: the matching Space keyup path now
also ignores Text mode, so browser Text handling owns the whole Space
key lifecycle just like xilem's `handle_spacebar` guard.

### Done 2026-05-15 - Resolve components in grid preview SVGs

The browser UFO loader now builds grid preview SVGs through a wasm
`glifToSvgWithComponents()` helper. Vue passes a glyph-name to `.glif`
XML map for the active master, and Rust/norad recursively resolves
component references with their affine transforms before computing the
SVG viewBox. Composite-only glyphs now preview in the grid/text strip
instead of appearing blank when their own `.glif` has no contours.

Follow-up completed 2026-05-15: component resolution is now used in
the anatomy panel and live editor canvas, and live components support
hit-testing, selection, dragging, transform actions, deletion,
duplication, keyboard nudging, and double-click-to-edit navigation.

Follow-up completed 2026-05-15: the anatomy panel now uses
`glifAnatomySvgWithComponents()` with the same active-master glyph XML
map, so composite-only glyphs show their resolved component outline in
the x-ray preview instead of rendering blank. Point and handle markers
still reflect the selected glyph's own contours.

Follow-up completed 2026-05-15: opening a glyph now calls
`setGlyphGlifWithComponents()` so the live editor canvas also renders
resolved component outlines as preview-only component shapes. Fit to
canvas includes those component outlines, while contour editing and
`.glif` save behavior continue to preserve the source component
references.

Follow-up completed 2026-05-15: the live editor now keeps individual
top-level component preview records with stable session IDs. Select
can hit-test component fills, selected components draw in the selected
component color, dragging a selected component updates its transform,
and `currentGlyphGlif()` writes moved component transforms back into
the saved `.glif`.

Follow-up completed 2026-05-15: double-clicking a component in Select
mode now opens that component's base glyph in the editor, giving Comfy
the xilem component edit-navigation path.

Follow-up completed 2026-05-15: selected components now consume the
same arrow-key nudge path as selected points. Base, Shift, and
Cmd/Ctrl nudge amounts match xilem, and moved component transforms
flow through dirty-state tracking and `.glif` serialization.

Follow-up completed 2026-05-15: component selection now feeds the
coordinate/transform overlays. `selectionBounds()` reports the selected
component's transformed bounds, and X/Y coordinate edits move the
component through the same transform serialization path as dragging
and keyboard nudging.

Follow-up completed 2026-05-15: because Comfy's coordinate panel has
editable W/H fields, those fields now scale selected components around
the active coordinate reference point instead of no-oping when a
component is selected.

Follow-up completed 2026-05-15: selected components now also flow
through the existing flip horizontal, flip vertical, rotate clockwise,
rotate counter-clockwise, duplicate, duplicate-repeat, and delete
actions. Deleted components are removed from the serialized `.glif`,
new duplicate component references are written into the serialized
`.glif`, and unresolved undeleted components remain preserved.

Follow-up completed 2026-05-15: when dirty non-empty groups data has
no workspace path or writable file handle, save now offers a browser
picker/download for `groups.plist`.

### Done 2026-05-15 - Add first background image import support

Editor mode now handles xilem's Cmd/Ctrl+I image-import shortcut in
the browser. A selected PNG/JPEG is fit into design space behind the
active glyph view, follows viewport zoom/pan through a wasm
`designToScreen()` helper, and can be locked/unlocked with Cmd/Ctrl+L.
This is session-only like xilem's background images.

### Done 2026-05-15 - Add background image dragging

Unlocked imported background images can now be dragged in design-space
units, matching xilem's basic image positioning behavior. Vue uses a
wasm `screenToDesign()` helper for pointer deltas, while locked images
ignore pointer events so normal outline tools work through them.

### Done 2026-05-15 - Add background image corner resize handles

Unlocked imported background images now draw four xilem-style corner
handles. Dragging a corner resizes the image proportionally around the
opposite corner in design-space units, preserving aspect ratio and
keeping locked images transparent to editor tool input.

### Done 2026-05-15 - Add background image side resize handles

Background images now include xilem's side handles as square resize
controls. Top/bottom handles change height only, left/right handles
change width only, and corner handles continue to preserve aspect
ratio. The image overlay now stores independent design-space X/Y
scales to support both resize modes.

### Done 2026-05-15 - Add background image selection and delete

Imported background images now have xilem-style independent selection.
Importing or clicking an unlocked image selects it, canvas clicks
deselect it, resize handles only appear for selected unlocked images,
and Backspace/Delete removes the selected background image before
falling through to outline point deletion.

### Done 2026-05-15 - Add background image arrow-key nudging

Selected, unlocked background images now consume the editor arrow keys
before outline movement, matching xilem's one-design-unit nudge
behavior. ArrowLeft/ArrowRight move in X, and ArrowUp/ArrowDown follow
design-space Y where up is positive.

### Done 2026-05-15 - Add background image context menu

Right-clicking an imported background image now opens a xilem-style
one-item context menu with Lock Image or Unlock Image based on the
current image state. The menu dismisses on outside pointer down or
Escape, and the selected-image border/opacity now matches xilem's
tracing-image presentation more closely.

### Done 2026-05-15 - Add contour context actions

Right-clicking an on-curve point in Select mode now opens a compact
xilem-style contour menu. Closed contours can set the clicked point as
the start point, any hit contour can be reversed, and multi-contour
glyphs can move the hit contour up or down in contour order. These
actions flow through wasm undo, `.glif` serialization, dirty state,
preview refresh, and Comfy state sync.

Follow-up completed 2026-05-15: the renderer now draws xilem-style
start-point arrows beside the first on-curve point of closed contours,
so context-menu start-point changes are visible on canvas immediately.

Follow-up completed 2026-05-15: selected smooth, corner, off-curve,
and start-arrow nodes now use xilem's slightly larger selected sizes
instead of only changing color.

Follow-up completed 2026-05-15: HyperBezier on-curve points now use
xilem's dedicated cyan/teal Hyper point colors and selected sizes
instead of rendering as ordinary smooth/corner points.

Follow-up completed 2026-05-15: double-clicking an on-curve point in
Select mode now selects just that point and toggles smooth/corner,
matching xilem's point double-click edit path.

Follow-up completed 2026-05-15: editor undo/redo now reports whether
history changed and Vue resyncs `.glif` bytes, dirty state, contour
metadata, compatibility diagnostics, and Comfy state after history
changes instead of only repainting the canvas.

### Done 2026-05-15 - Add interpolation compatibility diagnostics

The Rust core now ports xilem's multi-master interpolation checker as
`editing::compat`, with wasm exposing a JSON-friendly
`glifCompatibility` helper for browser-held designspace data. The Vue
editor compares the active glyph against the same glyph in other
masters after loads, edits, and master switches, then displays a red
summary badge and point markers for point-level mismatches.

### Done 2026-05-15 - Add glyph-grid keyboard navigation and paste

Grid view now handles xilem-style Arrow navigation, Enter-to-open, and
Cmd/Ctrl+S/C/V. Copy stores the selected glyph's source bytes in a
session-local clipboard, while paste uses a wasm
`glifWithOutlinesFrom()` helper to copy contours, components, and
advance width into the selected target glyph without replacing its
name, Unicode, groups, or mark metadata. Pasted grid glyphs refresh
metadata, previews, dirty state, save routing, and the open editor if
the target glyph is already loaded.

### Done 2026-05-15 - Add xilem design-grid rendering

The Vello renderer now draws xilem's editor design grid before metric
guides and outlines. It uses the same zoom thresholds and spacing:
8-unit fine / 32-unit coarse grid at mid zoom, plus 2-unit fine /
8-unit coarse grid at close zoom, with the xilem translucent gray grid
colors.

### Done 2026-05-15 - Fit background imports to font metrics

Background image import now mirrors xilem's initial placement more
closely by fitting the raster image to the active font ascender to
descender range instead of using a hard-coded 1000-unit box. Rust
exposes `metricBounds()` over wasm, Vue uses those bounds for
`designY` and scale, and editor-mode Enter now matches xilem's
return-to-grid shortcut outside Text mode.

Follow-up completed 2026-05-15: imports now prefer the current glyph's
actual outline/component bounds when present, matching xilem's
`load_matched_to_outlines` behavior. The ascender-to-descender metric
fit remains the fallback for empty glyphs.

### Done 2026-05-15 - Add glyph-grid multi-selection

The grid now tracks a selected glyph set in addition to the primary
glyph shown in the sidebars. Plain click selects one glyph, while
Shift-click adds the range between the primary anchor and clicked
glyph in filtered grid order, matching xilem/Glyphs-style selection.
Mark-color edits and grid paste apply to the selected set, while copy
continues to copy the primary glyph.

Follow-up completed 2026-05-15: after loading fonts, switching
masters, or changing category filters, the grid now preserves visible
selections or selects the first visible glyph, matching xilem's
non-empty grid selection behavior more closely.

### Done 2026-05-15 - Add selected-point arrow-key nudging

Selected outline points now support xilem-style keyboard nudging.
Arrow keys move by 2 design units, Shift+Arrow moves by 8, and
Cmd/Ctrl+Arrow moves by 32; changes flow through wasm undo,
selection refresh, `.glif` serialization, dirty state, and Comfy sync.

Follow-up completed 2026-05-15: normal arrow nudging now carries
adjacent off-curve handles with selected on-curve points, while
Option/Alt+Arrow uses xilem's independent nudge mode and moves only
explicitly selected points.

### Done 2026-05-15 - Add editor workspace toolbar

Editor mode now includes the xilem-style top-right workspace toolbar.
Its glyph-grid button closes the current glyph editor and returns to
the grid view, matching xilem's visible workspace navigation in
addition to Comfy's existing Escape/Enter shortcuts.

Follow-up completed 2026-05-15: the editor top-right toolbar row now
also includes a compact multi-master switcher when a designspace has
more than one master, matching xilem's editor overlay placement
instead of relying only on the global top bar.

Follow-up completed 2026-05-15: the editor master switcher now uses
the active masters' rendered `n` preview SVGs, falling back to an
initial only when a master does not contain `n`, matching xilem's
master-toolbar preview intent more closely.

### Done 2026-05-15 - Broaden multi-master save persistence

Save now persists every dirty glyph across loaded masters instead of
only the currently open glyph. Dropped UFO directory handles and
Comfy workspace slots write dirty `.glif` bytes back in one save,
dirty groups/kerning are cleared per master after successful writes,
and single-glyph browser export remains the fallback when no writable
handle exists. Grid mark-color edits also serialize through a
norad-backed `glifWithMarkColor()` wasm helper, so unopened glyphs can
be marked dirty, saved, and cleared without first loading them into
the outline editor.

### Done 2026-05-15 - Make active glyph Unicode editable

The bottom active-glyph panel now edits the current glyph's first
Unicode codepoint. Vue normalizes `0041`, `U+0041`, and `0x41`
inputs, Rust/norad serializes the `.glif` codepoint list through a new
`glifWithUnicode()` wasm helper, and the grid category, metadata,
Text glyph inventory, dirty state, and Comfy sync all refresh from the
updated bytes.

### Done 2026-05-15 - Make active glyph name editable

The bottom active-glyph panel now renames the current glyph. A new
norad-backed `glifWithName()` wasm helper rewrites the `.glif` name
while preserving outlines and metadata, and Vue moves the active
master's glyph bytes, metadata, preview SVG, mark color, file handles,
groups, kerning pairs, Text inventory, dirty state, and current
selection over to the new name.

### Done 2026-05-15 - Make active glyph sidebearings editable

The active-glyph panel now exposes editable LSB and RSB fields.
Rust computes sidebearings from the live outline bbox, `setLeftSidebearing()`
translates all outlines while preserving advance width, and
`setRightSidebearing()` adjusts advance width while preserving outline
placement. Vue routes both fields through `.glif` serialization, dirty
state, Text inventory refresh, render, and Comfy sync.

### Done 2026-05-15 - Snap keyboard nudges to design grid

Selected-point arrow-key nudging now follows xilem's grid snap pass.
After the keyboard delta is applied, selected on-curve points snap to
the nearest 2-unit design grid coordinate and adjacent off-curve
handles receive the same snap offset.

Follow-up: move bitmap painting into the Vello renderer and port
local/Quiver tracing once the Comfy-side image pipeline is ready.

Trace/refit blocker noted 2026-05-15: xilem's local tracing path calls
`img2bez::trace(&bg.source_path, ...)` with a real filesystem path,
while Comfy currently stores background images as browser object URLs
and DOM overlays. A faithful port needs a Comfy/browser image pipeline
that can provide either pixel data or a workspace-backed source file to
Rust before `Cmd+T`/`Cmd+Shift+T` can become real trace/refit actions.

Follow-up completed 2026-05-15: Comfy now ports the useful browser
side of xilem's glyph-images workflow. Dropped PNG/JPEG files are
indexed by glyph name, including xilem's case-safe `A_.png` naming,
and opening a glyph auto-loads its matching image as a locked
background reference. Dropping an image directly into an open editor
imports it as the active background image without trying to reload the
font.

### Done 2026-05-15 - Text manual kerning mode

Shift-clicking a Text buffer sort now enters xilem-style manual
kerning mode when there is a previous glyph sort. Dragging updates the
direct glyph pair in the Rust Text buffer, reflows the canvas and
bottom strip live, and highlights the active/previous sort metric
marks on the Vello canvas.

Follow-up completed 2026-05-15: structural Text buffer edits now
cancel manual kerning state in Rust. Clearing the buffer, changing
direction/inventory, inserting sorts, inserting line breaks, deleting
sorts, or replacing sort metadata can no longer leave the kerning drag
session pointing at a stale sort index.

### Done 2026-05-14 - Info sidebar metadata without opening glyphs

Selecting a glyph in the grid now populates Width and Contours without
loading it into the editor. The implementation uses a `glifMetadata`
wasm helper backed by `norad` and caches metadata in each `MasterData`.

Follow-up completed 2026-05-15: cached metadata refreshes from
serialized `.glif` bytes after glyph edits.

### Done 2026-05-14 - Parse `groups.plist` for kerning groups

Glyph info sidebar now shows left/right kerning groups from UFO
`groups.plist`, formatted like xilem by stripping `public.kern1.` and
`public.kern2.` prefixes.

Follow-up completed 2026-05-15: group edits rebuild glyph-level
kerning metadata and dirty groups save through workspace, file-handle,
or browser export routes.

### Done 2026-05-14 - Coordinate panel follow-through

Edit mode now has a bottom-right coordinate panel styled after
xilem's `coordinate_panel.rs`. It reads X/Y/W/H from the wasm
`selectionBounds` helper and refreshes after render-driving selection
changes.

Follow-up completed 2026-05-15: the coordinate panel's 3x3 reference
picker is interactive and drives X/Y/W/H reference transforms.

### Done 2026-05-14 - Transform panel scaffold with real selection bounds

Edit mode now has a right-side transform panel styled after xilem's
`transform_panel.rs`. Rust computes selected point bounds in design
space, wasm exposes them as `[count, x, y, width, height]`, and Vue
uses that single state path for both coordinate and transform panels.

Follow-up completed 2026-05-15: transform panel actions are wired
through wasm for selected points and selected components.

### Done 2026-05-14 - Replace text Save button with system-toolbar parity

`TopBar.vue` now delegates file operations to `SystemToolbar.vue`,
matching xilem's `components/system_toolbar.rs` split. The visible
text `Save` button is gone; the save affordance is an icon-only
toolbar button that enables only when dirty glyph, groups, or kerning
data exists.

### Done 2026-05-14 - Enable flip transform actions

The transform panel now enables Flip Horizontal and Flip Vertical when
points are selected. Rust mirrors selected points around the current
selection-bounds center, wasm exposes `flipSelectionHorizontal()` and
`flipSelectionVertical()`, and each successful mutation pushes an undo
snapshot.

Follow-up completed 2026-05-15: transform mutations now route through
Vue's editor mutation path, refreshing serialized `.glif` bytes,
dirty state, previews, sidebars, and Comfy sync.

### Done 2026-05-14 - Enable rotate transform actions

The transform panel now enables Rotate 90 CW and Rotate 90 CCW when
points are selected. Rust rotates selected points around the current
selection-bounds center, wasm exposes `rotateSelectionClockwise()` and
`rotateSelectionCounterClockwise()`, and each successful mutation
pushes an undo snapshot.

Follow-up completed 2026-05-15: transform mutations now route through
Vue's editor mutation path, refreshing serialized `.glif` bytes,
dirty state, previews, sidebars, and Comfy sync.

### Done 2026-05-14 - Enable duplicate transform action

The transform panel now enables Duplicate when points are selected.
Rust clones every contour containing a selected point, offsets the
duplicates by (+20, +20), selects the duplicate points, wasm exposes
`duplicateSelection()`, and each successful mutation pushes an undo
snapshot. This intentionally mirrors xilem's whole-contour duplicate
behavior.

### Done 2026-05-14 - Enable duplicate-repeat transform action

Duplicate Repeat now mirrors xilem's behavior: duplicate selected
contours, then apply the previous geometric transform when one exists.
Rust stores `last_transform`, wasm exposes `duplicateRepeatSelection()`,
and the transform panel enables Duplicate Repeat when points are
selected.

Follow-up completed 2026-05-15: duplicate and duplicate-repeat now
route through Vue's editor mutation path and persist through the same
dirty `.glif` save flow as other geometry edits.

### Done 2026-05-14 - Add dirty-state plumbing for editor mutations

The top bar's "Not saved" indicator now reflects actual in-memory
mutations instead of every loaded font. Vue tracks dirty glyphs per
master, marks transform and mark-color edits dirty, and clears dirty
state on a new font/designspace load.

Follow-up completed 2026-05-15: `.glif` serialization is wired into
editor mutations, and successful saves clear dirty state.

### Done 2026-05-14 - Serialize current glyph back to `.glif` bytes

Rust now exposes `currentGlyphGlif(originalBytes, markColor)`, which
parses the original `.glif` with norad, replaces contours from the
live editor paths, preserves surrounding glyph metadata, updates
`public.markColor`, and returns encoded XML bytes. Vue uses it after
transform mutations and open-glyph mark-color changes to keep
`MasterData.glyphBytes`, metadata, Unicode, and preview SVGs aligned.

Follow-up completed 2026-05-15: dirty glyph bytes write through
dropped UFO file handles, Comfy workspace slots, or browser export
fallbacks, and successful writes clear dirty state.

### Done 2026-05-14 - Add active glyph save/export

The system toolbar save button now serializes the active glyph and
either writes it back to the dropped UFO when a writable handle is
available, writes through the browser save picker, or exports a `.glif`
download when no writable handle is available. Successful saves clear
dirty state for that glyph.

Follow-up completed 2026-05-15: save now persists dirty glyphs across
loaded masters, plus dirty groups and kerning data, while keeping
browser export fallbacks for hosts without writable handles.

### Done 2026-05-14 - Enable boolean transform actions

Union, Subtract, Intersect, and Exclude are no longer visual stubs.
Rust now routes glyph contours through `linesweeper`, wasm exposes the
four boolean actions, and the transform panel enables the boolean
buttons once the glyph has at least two contours. The edit stack still
updates dirty state and undo on successful operations.

Follow-up completed 2026-05-15: boolean output now restores matching
input on-curve point types, preserving smooth/corner metadata the same
way xilem does after `linesweeper` output is converted back to cubic
contours.

Follow-up completed 2026-05-15: boolean semantics were realigned with
xilem after checking `EditSession::boolean_op` and the xilem transform
panel. Boolean actions now operate on every contour in contour order,
not only selected contours, and no longer require a point selection to
run from the transform panel.

### Done 2026-05-14 - Explicit source-kind selection for `FONT` imports

The `Font` node now exposes a source-kind selector with
`ufo/designspace` as the default and `glyphs` as the alternate path.
Workspace slots persist that source kind in `workspace.json`, and slot
lookup uses it to prefer the right source artifact when multiple are
present.

Follow-up completed 2026-05-14: a Glyphs import adapter now runs when
the optional `glyphsLib` dependency is available, while
UFO/designspace remains the default workflow path.

Follow-up completed 2026-05-15: the optional `glyphsLib` normalization
path now has a focused unit test that fakes `glyphsLib.build_masters`
and verifies Glyphs imports become UFO/designspace workspace slots
while preserving the original `.glyphs` file.

### Done 2026-05-14 - Add a compile seam for Google Fonts sources

The workspace now has a `CompileFont` node and a `compile_slot()`
helper. It is wired for the Google Fonts-oriented `glyphspackage`
compile path via `fontc`, while UFO/designspace remains the default
editable source and Glyphs remains an alternate import/source kind.
The workspace materializes a `glyphspackage` source package
automatically when compilation needs one.

Follow-up completed 2026-05-14: the concrete Glyphs ->
UFO/designspace adapter is implemented behind optional `glyphsLib`.
Compiler backend expansion remains separate from the workspace seam.

Follow-up completed 2026-05-15: the existing `glyphsLib` adapter is
covered by tests, so the workspace seam can safely keep UFO/designspace
as the normalized editable shape after Glyphs import.

### Done 2026-05-14 - Normalize Glyphs imports when `glyphsLib` exists

Glyphs imports now attempt to normalize into UFO/designspace inside
the workspace when the optional `glyphsLib` dependency is installed.
If the conversion succeeds, the slot keeps the default editable source
shape while preserving the original Glyphs file as provenance.

Follow-up completed 2026-05-15: GlyphsPackage sources now remain
valid editable workspace sources while the compiler seam exports their
inner `sources/` payload into a generated package. The generated
package avoids case-insensitive name collisions with imported
`.glyphspackage` source directories, so downstream nodes still receive
the stable `FONT` workspace contract.

### Done 2026-05-14 - Add a workspace glyphspackage exporter layout

The workspace compile seam now materializes a concrete
`glyphspackage` package layout with a `sources/` tree and
`sources/config.yaml` before invoking `fontc`. That keeps source
conversion inside the workspace layer and keeps the graph contract
stable.

Follow-up completed 2026-05-15: the exporter has now been exercised
against the locally installed `fontc`; workspace packaging reaches
compiler source validation. The remaining live-compile blocker is a
clean compile-capable fixture, because the current Virtua test source
fails anchor validation.

### Done 2026-05-14 - Add workspace exporter smoke tests

The workspace exporter and compile seam now have unit tests that
verify the `glyphspackage` layout, package manifest, and mocked
`fontc` invocation path. This keeps the backend seam from regressing
while we wait on a live compiler install.

Follow-up completed 2026-05-15: the same path now runs against the
real local `fontc` binary. Unit coverage keeps the successful
invocation seam mocked; the real fixture still needs to be replaced or
repaired before it can become an always-on compile test.

Follow-up completed 2026-05-15: `fontc` is installed locally and real
compile attempts against `web/assets/test-fonts/VirtuaGrotesk.designspace`
showed two workspace issues before source validation: designspace
imports must copy referenced UFO masters, and this `fontc` expects a
designspace/UFO/Glyphs source rather than the generated
`.glyphspackage` directory. `create_slot_from_path()` now copies
designspace-referenced UFO sources, and `compile_slot()` keeps the
package export but invokes `fontc` on the primary source under the
package `sources/` tree. Unit coverage asserts both behaviors.

Follow-up completed 2026-05-15: failed `fontc` runs now capture
stdout/stderr and raise a workspace-level diagnostic instead of a raw
`CalledProcessError`. The live Virtua fixture still fails source
validation on invalid anchors, but the error is now surfaced directly
as `fontc failed for workspace slot ...` with the compiler message.

Remaining live-compiler gap: the bundled Virtua test font currently
reaches `fontc` source validation but fails on an invalid anchor in
`seen-ar`, so a successful end-to-end real `fontc` compile still needs
a clean compile-capable source fixture.

### Done 2026-05-14 - Fill the glyph anatomy panel with a real x-ray SVG

The right-sidebar anatomy panel now renders a dedicated SVG that
shows outline, handle lines, and point markers instead of reusing the
plain silhouette preview. This closes one of the clearest remaining
editor placeholders and matches xilem's anatomy panel much more
closely.

Follow-up completed 2026-05-15: the main edit-mode toolbar tools now
dispatch into Rust, and coordinate W/H fields are real scale controls.
Keep tightening remaining edge cases rather than treating these as
stubs.

### Done 2026-05-15 - Make the coordinate reference picker interactive

The coordinate panel's 3x3 picker now changes the active reference
point for selected-bounds X/Y display. Rust stores the selected
`Quadrant`, wasm exposes `setCoordinateQuadrant()`, and Vue updates
the active dot plus displayed coordinates when the user clicks a
reference point.

Follow-up completed 2026-05-15: W/H are editable through coordinate
panel scale transforms and have point/component Rust coverage.

### Done 2026-05-15 - Make coordinate X/Y fields move selections

The coordinate panel's X and Y fields are now editable. Changing X or
Y moves the active reference point for the selected points to the
entered design-space coordinate, matching xilem's
move-selection-by-reference behavior. Rust owns the selection delta and
undo snapshot, wasm exposes `moveSelectionReference()`, and Vue keeps
dirty state, selection bounds, glyph bytes, previews, and Comfy state
in sync after successful moves.

Follow-up completed 2026-05-15: W/H scale editing is implemented via
`resizeSelectionReference()` and now covers both point selections and
selected components.

### Done 2026-05-15 - Wire Preview and Shapes tools

The editor toolbar now dispatches selected tools into Rust instead of
only updating Vue state. Preview pans the viewport, Shapes shows a
xilem-style rectangle/ellipse sub-toolbar with live drag preview, and
dragging in the editor creates a selected contour for the chosen
primitive. Pointer-driven geometry edits now report back through wasm,
so Select drags and Shapes creation update undo, dirty state,
serialized `.glif` bytes, previews, selection bounds, and Comfy state.

Follow-up completed 2026-05-15: HyperPen, Knife, and Text all now
dispatch into Rust and have first-pass behavior. Remaining work is
edge-case parity and richer Text/session behavior.

Follow-up completed 2026-05-15: Shapes now mirrors xilem's live
Shift-lock behavior during active drags. Pressing or releasing Shift
updates the Rust Shapes tool state, recomputes the rectangle/ellipse
preview immediately, and commits the final square/circle constraint
even if the final pointer event itself does not carry Shift.

Follow-up completed 2026-05-15: Select now matches xilem's component
click priority when Shift is held. Shift-clicking a component selects
that component and clears point selection instead of falling through
to shift-marquee behavior.

Follow-up completed 2026-05-15: Select now preserves an existing
multi-point selection when clicking an already-selected point, matching
xilem's drag behavior. A normal click on an unselected point still
replaces the selection with that point.

### Done 2026-05-15 - Add xilem-style tool keyboard switching

Edit mode now supports xilem's fast tool switching keys: V selects
Select, P selects Pen, H selects HyperPen, and K selects Knife. Holding
Space temporarily switches to Preview for hand/pan behavior and
restores the previous tool on keyup.

Follow-up completed 2026-05-15: Ctrl+Space now follows xilem's
one-way Preview shortcut more closely by only consuming the key when it
actually switches from another tool into Preview.

Follow-up: as each remaining tool gains behavior, expand keyboard
coverage where xilem has mature shortcuts.

### Done 2026-05-15 - Add basic Pen corner drawing

The Pen toolbar item now dispatches into Rust. Clicking in the editor
starts or extends a cubic contour with corner on-curve points, and
clicking near the first point after at least three points closes the
path. Each committed point/close action flows through the same
pointer-change path as other geometry edits, so undo, dirty state,
serialized `.glif` bytes, previews, selection bounds, and Comfy state
stay in sync.

Follow-up completed 2026-05-15: Pen supports segment hover preview,
curve insertion, and line-grid snapping.

Follow-up completed 2026-05-15: cancelling Pen now matches xilem's
underbuilt-path behavior. A one-point unfinished Pen contour is
discarded on cancel/tool switch, while a real two-point open contour
is preserved as a finished open path.

Follow-up completed 2026-05-15: wasm tool-switch and pointer-cancel
calls now report whether tool cancellation mutated the glyph. Vue uses
that signal to refresh serialized `.glif` bytes, dirty state,
selection/contour metadata, compatibility diagnostics, render output,
and Comfy state after an unfinished Pen path is discarded.

Follow-up completed 2026-05-15: returning from the editor to the glyph
grid now cancels the active Rust tool first. This prevents a one-point
unfinished Pen contour from surviving the editor close path and keeps
the serialized glyph state aligned with xilem's transient Pen drawing
model.

### Done 2026-05-15 - Add Pen preview and smooth point drags

The Pen tool now draws a transient next-segment preview with close
feedback while the cursor moves. Dragging with Pen creates a smooth
on-curve point with mirrored handles, updates the outgoing handle live
during the drag, and keeps new open contours serializable by starting
first smooth-drag contours with an on-curve Move point.

Follow-up completed 2026-05-15: Pen preview and committed insertion
share the snapped segment insertion path.

### Done 2026-05-15 - Add basic HyperPen drawing

The HyperPen toolbar item now dispatches into Rust. Clicking in the
editor starts or extends a `Path::Hyper` contour with smooth
hyperbezier on-curve points, shows the same next-segment and close
feedback as Pen, and closes the path when clicking near the first
point after at least three points. The saved `.glif` path preserves
hyperbezier source points through the existing Hyper/HyperCorner UFO
serialization.

Follow-up completed 2026-05-15: HyperPen shares segment snapping and
insert-point-on-curve behavior with Pen. Separate HyperCorner drawing
controls remain a product decision.

### Done 2026-05-15 - Add Measure tool

The Measure toolbar item now dispatches into Rust. Dragging draws a
measurement line on the canvas and Vue shows a compact distance/angle
label near the endpoint. The measurement line intersects editable
paths, draws dots at hit points, and shows per-segment length labels
between consecutive intersections. Holding Shift constrains the
measurement to horizontal or vertical, matching xilem's axis-lock
behavior.

Follow-up completed 2026-05-15: Measure and Knife preview
intersections now use xilem-style fuzzy clustering in design-space
units, with regression coverage for nearby hit merging.

Follow-up completed 2026-05-15: Measure now intersects inactive
Text-buffer sorts as well as the active editable sort. Text-session
measurements use the Rust Text layout and glyph inventory outlines so
neighboring sorts contribute hit dots and segment-length labels like
xilem's Text buffer path.

### Done 2026-05-15 - Add Knife preview and intersection markers

The Knife toolbar item now dispatches into Rust. Dragging draws the
knife line on the canvas and marks every intersection with editable
outline segments. It shares the same line/segment intersection helper
used by Measure, so cubic, quadratic, hyperbezier, and line segments
all participate.

### Done 2026-05-15 - Add Knife contour splitting

Knife now ports xilem's cubic contour-splitting algorithm into the
WASM editor core. On mouse release, a knife line with two or more
intersections splits cubic contours, assigns fresh point ids, clears
selection, bumps the edit revision, and lets the Vue host refresh the
active glyph's serialized `.glif` bytes. Shift constrains the knife
line horizontally or vertically.

Follow-up completed 2026-05-15: quadratic contours now split as
quadratic output, and hyperbezier contours split by expanding to
explicit cubic output.

Follow-up completed 2026-05-15: Knife Shift axis-lock is now
keyboard-driven during an active drag, matching xilem's stored
`shift_locked` behavior. Pressing or releasing Shift immediately
recomputes the live knife preview and final cut line without waiting
for another pointer move.

### Done 2026-05-15 - Wire Text tool shell and direction toolbar

Text is no longer routed to the inert fallback in the Rust tool box.
Selecting Text now creates an active Text tool, Vue shows a
xilem-style LTR/RTL direction sub-toolbar beneath the edit toolbar,
and the `T` keyboard shortcut switches into Text mode.

### Done 2026-05-15 - Add first Text buffer behavior

Text mode now accepts printable keyboard input, maps characters to
glyphs through the active master's Unicode map, inserts matching sorts
at the cursor, supports arrow/Home/End cursor movement plus
Backspace/Delete, renders a bottom preview strip using the active
master's glyph SVGs, reverses the strip for RTL mode, and double-clicks
sorts back into the glyph editor.

### Done 2026-05-15 - Add Text line breaks and kerning-aware preview spacing

The browser workspace loader now parses `kerning.plist` in addition to
`groups.plist`, keeps full group and kerning maps in the active master
state, and applies UFO-style direct/group kerning lookup to the Text
preview strip. Text mode also handles Enter as a line-break sort and
renders multiple preview lines.

### Done 2026-05-15 - Add basic Arabic shaping for Text preview

When Text direction is RTL, the text buffer now recomputes Arabic
joining forms after insert/delete/direction changes and resolves
`.init`, `.medi`, and `.fina` glyph names when those glyphs exist in
the active UFO. The joining-type table mirrors xilem's current Arabic
shaper subset and falls back to the base glyph when a positional glyph
is missing.

Follow-up completed 2026-05-15: the Text session has since moved to
Rust-owned state, canvas-level multi-sort rendering, Rust layout and
hit-testing, active-sort editing, manual kerning, and refreshed glyph
inventory. The remaining Text gap is broader shaping coverage beyond
the Arabic joining subset currently shared with xilem.

### Done 2026-05-15 - Add Text sort selection and visual cursor movement

The Text preview strip now supports single-click sort selection,
updates the selected glyph in the side panels, moves the insertion
cursor next to the clicked sort, and keeps visual ArrowLeft/ArrowRight
movement direction-aware for RTL mode. Double-click still opens the
sort glyph for outline editing.

Follow-up completed 2026-05-15: canvas-level Text sort activation now
loads the newly active sort's glyph into the editable outline state,
matching the preview-strip activation path and xilem's active-sort
handoff model.

Follow-up completed 2026-05-15: the Rust Text tool path now has
focused coverage proving canvas clicks activate the hit sort and move
the insertion cursor.

Follow-up completed 2026-05-15: canvas Text rendering now visually
distinguishes the active sort from inactive sorts with active-fill and
active metric styling, moving closer to xilem's active/inactive sort
model.

Follow-up completed 2026-05-15: outline edits now refresh the Rust
Text glyph inventory whenever a Text buffer exists, so canvas sorts
and the bottom strip see updated glyph outlines after active-sort
geometry changes.

Follow-up completed 2026-05-15: the browser Text buffer strip now
uses a full-width bottom preview band sized like xilem's 85/15 editor
split, while keeping Comfy's existing interactive sort buttons for
cursor and active-sort selection.

Follow-up completed 2026-05-15: while Text mode is active, the editor
canvas now ends above the bottom Text buffer band instead of rendering
under it, matching xilem's split editor/text preview layout.

Follow-up completed 2026-05-15: Text buffer sort hit targets are now
transparent glyph preview regions instead of browser-card buttons, so
the bottom strip visually matches xilem's dark `multi_glyph_view`
preview pane while retaining click and double-click affordances.

### Done 2026-05-15 - Add Rust-side Text buffer foundation

`rust-core` now has a `text` module with a TextBuffer, glyph and
line-break sorts, cursor movement, direction-aware visual cursor
movement, active sort selection, deletion, clear/reset behavior, and
unit coverage. The wasm API exposes the buffer operations, and Vue
mirrors Text preview edits into the Rust-side buffer so the next step
can move preview rendering/session behavior out of browser-only state.

### Done 2026-05-15 - Expose Rust Text buffer snapshots to Vue

The wasm API now returns a structured Text buffer snapshot with cursor,
active sort, direction, and sort metadata. Vue refreshes cursor,
active-sort, and direction state from that snapshot after Text edits,
so Rust is now the authority for Text session mechanics while Vue still
owns preview rendering and glyph shaping.

### Done 2026-05-15 - Sync shaped Text glyph names back to Rust

Vue's Arabic shaping pass now pushes resolved glyph names and advance
widths back into the Rust Text buffer after reshaping. This keeps the
wasm Text snapshot aligned with the preview strip even when a base
glyph becomes `.init`, `.medi`, or `.fina`.

### Done 2026-05-15 - Make Rust Text snapshot drive Vue sorts

Vue now refreshes the preview strip's sort list from the Rust
`textBufferSnapshot()` after Text insert/delete/cursor operations.
This removes the browser-only parallel source for Text buffer contents:
Rust owns the sort list, cursor, direction, and active sort; Vue still
renders the preview and performs the current glyph-name shaping pass.

### Done 2026-05-15 - Add Rust Text layout calculation

The Rust Text buffer now computes X/Y layout positions for glyph sorts
and cursor placement across line breaks, with RTL lines laid out from
their line width. The wasm API exposes `textBufferLayout(lineHeight)`
so Vue can migrate the preview strip toward Rust-provided layout
coordinates before full canvas-level multi-sort rendering lands.

### Done 2026-05-15 - Drive Text preview positions from Rust layout

Vue now renders the Text preview strip from `textBufferLayout()`
coordinates instead of its previous flex-line layout. Glyph positions,
line positions, and cursor placement now come from Rust-owned Text
session layout.

### Done 2026-05-15 - Move Text kerning into Rust layout

The active master's parsed `groups.plist` and `kerning.plist` data are
now pushed into the wasm editor as a Text kerning model. Rust applies
direct and group kerning pairs during Text layout, including RTL line
width and position calculations, so Vue no longer computes extra
kerning offsets around the Rust layout snapshot.

### Done 2026-05-15 - Move basic Arabic shaping into Rust

The active master's Unicode-to-glyph map and glyph widths are now
pushed into the wasm editor as a Text glyph inventory. Rust applies
the existing Arabic positional-form shaping pass to the Text buffer,
updates shaped glyph names and advance widths, and resets glyphs back
to base forms when direction changes away from RTL. Vue now asks core
to shape the buffer instead of owning the joining table.

### Done 2026-05-15 - Render Text buffer glyphs on the canvas

The Text glyph inventory now includes outline path data from the
active master. The Vello renderer draws each Text buffer glyph at its
Rust layout position and paints a canvas cursor using the same Text
layout snapshot that drives the bottom preview strip.

### Done 2026-05-15 - Extend Knife splitting to quadratic outlines

Knife now mutates quadratic contours instead of preserving them
unchanged after preview. The quadratic splitter follows the same
line-hit ordering and recursive slice behavior as the cubic splitter,
but emits `QuadraticPath` output and preserves quadratic off-curve
controls instead of raising sliced segments to cubic curves.

### Done 2026-05-15 - Extend Knife splitting to hyperbezier outlines

Knife now mutates hyperbezier contours after preview. Because a cut
invalidates the original automatic hyper-control model, the mutation
path expands the solved hyperbezier to explicit cubic contours, then
uses the cubic splitter so the resulting sliced contours remain
editable and serializable.

### Done 2026-05-15 - Add canvas Text sort and cursor hit testing

Text mode now responds to clicks on the editor canvas. Rust hit-tests
against the Text layout in design space, activates clicked glyph sorts,
and places the insertion cursor at the nearest visual boundary when
clicking between glyphs or on empty Text-session space. Vue refreshes
the Text snapshot after pointer-up so the bottom strip, selected sort,
canvas cursor, and glyph side panels stay aligned.

Follow-up completed 2026-05-15: canvas Text hit-testing now uses the
same Rust `text_line_height()` as Text rendering, active-sort origins,
and Vue preview layout. Multi-line canvas clicks no longer fall back
to UPM-based line spacing when font ascender/descender metrics differ
from units-per-em.

### Done 2026-05-15 - Add xilem-style Text cursor and sort metrics markers

The canvas Text renderer now draws the insertion cursor from descender
to ascender with top and bottom triangular handles, matching xilem's
Glyphs-style Text cursor treatment. It also draws minimal metric
cross markers at each rendered sort's left and right edges, while text
mode renders all sorts as filled previews like xilem.

### Done 2026-05-14 - Bridge Runebender state into ComfyUI output

The Runebender Vue host now pushes live editor state into the ComfyUI
node, keyed by node id. The Python node stores that state server-side
and returns the active `FONT` handle as its workflow output, so the
editor now has a real Comfy graph contract instead of placeholder
pass-through wiring.

Follow-up completed 2026-05-15: the placeholder path-based `FONT`
input has been replaced by workspace slots with source/compiled
manifests. `Font`, `Runebender`, `ForkFont`, `CompileFont`, and
`FontPreview` now share opaque `FONT` handles through the Comfy graph.

### Done 2026-05-14 - Establish the `FONT` wire contract

The graph now treats `FONT` as the first-class data type for type
design workflows. A lightweight `Font` node resolves a filesystem path
to a `FONT` handle, and `Runebender` passes that handle through rather
than returning SVG markup. The live glyph preview remains a separate
browser-side state channel.

Follow-up completed 2026-05-14: workspace slots now carry editable
source files and optional compiled artifacts, so the `FONT` handle can
represent UFO/designspace source plus renderable output.

### Done 2026-05-14 - Add a workspace slot manager and first FONT consumer

Python now has a small workspace layer under `workspace/fonts/` that
tracks `FONT` slots with paired source/compiled files. The `Font`
node creates slots from source paths, `Runebender` preserves the slot
handle, and `FontPreview` resolves the handle into a simple raster
specimen using the compiled font side when present.

Follow-up completed 2026-05-14: slot import/fork helpers exist and
Runebender can hydrate the browser editor from a graph-owned
workspace slot instead of relying only on standalone drag/drop.

### Done 2026-05-14 - Add FONT slot forking

The workspace layer now supports cloning a slot into a new slot, and
the graph exposes that as `ForkFont`. This gives local AI workflows a
clean branching primitive: keep the original `FONT` stable, fork it,
and run a different edit or rendering path on the copy.

Follow-up completed 2026-05-15: compiled artifacts and generated
glyphspackage exports are invalidated on source writes, and
`CompileFont` rebuilds the package from fresh source when needed.

### Done 2026-05-14 - Hydrate Runebender from workspace slots

The editor now fetches slot text files from the ComfyUI server and
loads them into the existing UFO/designspace import path. That closes
the loop between a graph-owned `FONT` value and the browser editor.

Follow-up completed 2026-05-15: the workspace model now supports
explicit source-kind import choices, Google Fonts-oriented
glyphspackage export, `fontc` compilation, and downstream preview
resolution through the `FONT` handle.

### Done 2026-05-14 - Write edited glyphs back into workspace slots

The editor's save action now writes the active glyph `.glif` back to
the workspace slot through the ComfyUI server. That gives the `FONT`
wire a real round-trip path: load a slot, edit a glyph, persist the
updated source file, and keep the slot as the graph object.

Follow-up completed 2026-05-15: save/write-back now covers glyphs,
groups, and kerning, and source writes invalidate stale compiled
artifacts so downstream nodes do not reuse old outputs.

### Done 2026-05-15 - Invalidate compiled FONT artifacts after source edits

Workspace write-back now targets real `workspace/fonts/<slot>/...`
slot paths when the browser saves files from a Runebender-loaded
`FONT` handle. Source writes remove stale compiled artifacts and
generated glyphspackage exports from the slot manifest so downstream
`CompileFont` and `FontPreview` nodes do not silently reuse an old
compiled font after an edit.

Follow-up completed 2026-05-15: `CompileFont` now rebuilds the
glyphspackage export whenever the slot has no current compiled
artifact, so normal recompiles after an edit consume fresh UFO/source
files without requiring users to toggle Force.

Follow-up completed 2026-05-15: workspace write-back now rejects
parent-directory traversal in browser-supplied relative paths before
resolving slot targets.

Follow-up completed 2026-05-15: Runebender slot hydration now skips
the manifest-recorded generated `.glyphspackage` export that
`CompileFont` creates inside a slot. The browser editor receives the
editable source files instead of duplicate generated source copies,
while real glyphspackage sources remain eligible as source entries.

Follow-up completed 2026-05-15: the `Font` node now exposes
`source_kind` as a constrained Comfy option set
(`ufo/designspace`, `glyphs`, `glyphspackage`) instead of a free-text
field, keeping UFO/designspace as the default while making alternate
source paths discoverable and typo-resistant.

Follow-up completed 2026-05-15: `FontPreview` now imports its image
stack (`numpy`, Pillow, `torch`) lazily inside the preview run path,
so the custom-node package can register `FONT`, Runebender, compile,
and fork nodes even in lean environments where preview dependencies
are not installed yet.

Follow-up completed 2026-05-15: `ForkFont` now treats an empty
`fork_name` as "create the next unique fork" (`<slot>-fork`,
`<slot>-fork-002`, ...), making graph branching ergonomic for
automation while preserving explicit names when provided.

Follow-up completed 2026-05-15: tests now import the root custom-node
package with stubbed ComfyUI route modules and assert the expected node
class/display mappings plus `WEB_DIRECTORY`. This protects Comfy
registration from optional preview dependencies and route import drift.

Follow-up completed 2026-05-15: `DesignBot` is no longer a pure
placeholder. The node now writes a temporary Rust script, wraps bare
DrawBot-style commands with the requested canvas size, invokes an
installed `designbot` CLI (`DESIGNBOT_BIN` or `PATH`), decodes the PNG
output lazily into a Comfy `IMAGE` tensor, and reports a clear setup
error when the CLI is missing. Tests cover wrapper generation and the
CLI handoff without requiring the Rust toolchain or image stack.

## Larger Work

### A. Save and UFO round trip

Dirty glyph, groups, and kerning write-back is now implemented for
Comfy workspace slots, dropped browser file handles, and export
fallbacks. The direction in `docs/architecture/decisions.md` remains:
Rust owns outline semantics and serializes back through a narrow wasm
API; Vue coordinates browser file access and ComfyUI output.

Next useful slices:

- refresh or invalidate compiled font artifacts after broader source
  edits beyond glyph writes
- Done 2026-05-15: add stronger user-visible save diagnostics for
  multi-master partial failures by listing the first unsaved
  master/glyph pairs in the status message
- harden designspace/UFO round trips against unusual directory layouts

### B. ComfyUI integration

The core Python nodes now have concrete graph contracts: `FONT`
workspace slots, Runebender state bridge/output, Compile/Fork/Preview
consumers, and an optional CLI-backed DesignBot renderer. The
remaining integration work is workflow polish and broader automation
coverage rather than first-pass placeholder replacement.

First useful slice:

- Done 2026-05-15: define the serialized output contract from Vue to
  `nodes/runebender.py`.
- Done 2026-05-15: return the live active-glyph SVG from the
  Runebender node as a second `STRING` output while preserving the
  pass-through `FONT` output.
- Done 2026-05-15: add a bundle contract test that checks
  `WEB_DIRECTORY` points to `web/dist` and the built
  `runebender-comfy.js` preserves the ComfyUI extension markers:
  `/scripts/app.js`, `registerExtension`, the Runebender extension
  name, `addDOMWidget`, the preview launcher, the edit overlay, and
  the hidden `glyph_data` widget.
- Done 2026-05-16: replace the inline Runebender widget with a
  compact preview card and Edit button that opens the editor as an
  full-screen `document.body` overlay, matching comfyfont's launch
  behavior instead of embedding the editor inside the node.

### C. Theme JSON shared through `runebender-core`

Do after the UI token set stabilizes. See
`docs/architecture/decisions.md`.

First useful slice:

- Done 2026-05-15: create
  `../runebender-core/themes/runebender.json` as the checked-in
  semantic token source for xilem/comfy color and size parity.
- Done 2026-05-15: add `web/src/themeTokens.ts` and move Comfy's
  `MARK_COLORS` source to semantic mark tokens aligned with the shared
  JSON artifact.
- Done 2026-05-15: expose initial root CSS variables from
  `THEME_CHROME_COLORS` and migrate `MarkColorPanel` chrome/ring
  styling to those variables.
- Done 2026-05-15: migrate the top-right editor workspace, system,
  and master toolbar chrome to the same root CSS variables.
- Done 2026-05-15: migrate the main edit toolbar plus Shapes and Text
  direction sub-toolbar chrome to the same root CSS variables.
- Done 2026-05-15: extend `THEME_CHROME_COLORS` with the remaining
  shared chrome roles and migrate the category/sidebar, glyph grid
  cell, anatomy, top bar, welcome, coordinate, transform, active glyph,
  context-menu, text-strip, and notice chrome onto root CSS variables.
- Done 2026-05-15: add a UI-toolkit-free `theme` module in
  `runebender-core` and consume those semantic color tokens from the
  Comfy Vello renderer instead of keeping a parallel hard-coded
  palette.

### D. Grow `runebender-core` deliberately

Keep moving kurbo-free semantics first. Avoid public `kurbo` types
until xilem and comfy can share a geometry version.

Good next candidates:

- Done 2026-05-15: move UFO `public.markColor` parsing,
  canonicalization, exact xilem palette matching, and preset RGBA
  strings into `runebender-core::mark_color`; Comfy now validates and
  canonicalizes mark-color writes through the shared core helper.
- Done 2026-05-15: move the xilem-aligned Arabic joining and
  positional-form helpers into `runebender-core::shaping`, and have
  Comfy Text shaping consume that shared module instead of keeping a
  separate local copy.
- Done 2026-05-15: add `runebender-core::model::GlyphMetadata`
  with full Unicode-list metadata and have Comfy's `glifMetadata`
  wasm API serialize that shared struct so Vue no longer has to rescan
  glyph XML for codepoints.
- edit command descriptions
- UFO plist helpers that do not expose geometry

## Suggested Next Pick

Continue editor parity before returning to backend workspace work.
Good next slices are richer Text session behavior, remaining toolbar
edge cases, or shape/selection interaction polish.
Dirty glyph bytes now persist to dropped UFO handles, Comfy workspace
slots, or browser export fallback; the remaining product gap is
compiled-artifact refresh and broader ComfyUI workflow polish.
