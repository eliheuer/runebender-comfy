# Architecture Decisions

Durable decisions for agents and humans. Task plans live in
`.agents/`; this file is for choices that should outlive a single
phase.

## Vue Host, Rust/WASM Core

`runebender-comfy` is a ComfyUI custom-node package. ComfyUI's
frontend is already a browser/Vue environment, so this repo embeds a
Vue widget and calls a Rust core compiled with `wasm-bindgen`.

The Rust side owns glyph editing state, hit testing, undo, and Vello
rendering. Vue owns host integration, file drag/drop, panel chrome,
and ComfyUI widget registration. Xilem's view tree and Masonry
widgets are intentionally not compiled into the ComfyUI frontend.

Practical consequence: port behavior and model code from
`runebender-xilem`, not entire Xilem modules. Anything coupled to
`app_logic()`, Masonry widgets, or native file dialogs needs a host
specific adapter.

## `runebender-core` Boundary

`runebender-core` is the shared Apache-2.0 crate used by both
`runebender-xilem` and `runebender-comfy`. It should contain
platform-independent editing/model logic that does not expose host UI
types.

Current hard boundary: no public `kurbo` types in `runebender-core`.
Xilem is pinned to kurbo 0.12 through Masonry 0.4; comfy uses kurbo
0.13 through Vello/Peniko. A shared crate that exposes one kurbo
version cannot be consumed cleanly by both frontends.

Good candidates for `runebender-core`:

- selection sets and entity IDs
- undo/redo grouping
- glyph categories
- kerning lookup
- mark-color parsing and serialization
- host-neutral edit command descriptions
- UFO metadata helpers that do not expose geometry types

Keep local to each frontend for now:

- viewport transforms
- hit testing over `kurbo::Point`
- path geometry and `BezPath` conversion
- renderer adapters
- mouse/pointer state machines with geometry payloads

If geometry must move before the kurbo versions align, add tiny
owned core geometry types and convert to each frontend's kurbo at the
edge. Do not leak `kurbo::Point`, `kurbo::Affine`, or
`kurbo::BezPath` through core public APIs.

## Source of Truth

The native `runebender-xilem` app is the canonical UI/UX reference.
The ComfyUI port should mirror visible behavior and naming wherever
that does not fight the browser host.

`runebender-core` should become the canonical source for semantics:
categories, kerning, edit command meanings, metadata, and eventually
geometry once the dependency graph allows it.

`runebender-comfy` remains the canonical source for browser-specific
constraints: drag/drop, WebGPU canvas lifecycle, wasm-pack output,
ComfyUI widget registration, and Vite packaging.

## Google Fonts Sidebar Data

The glyph-grid sidebar should use the local Google Fonts upstream
checkouts under `/Users/eli/GF/repos` for language and glyphset data:

- `lang`: <https://github.com/googlefonts/lang>
- `glyphsets`: <https://github.com/googlefonts/glyphsets>

Do not hand-maintain Google Fonts character sets in Vue. Regenerate
`web/src/gfSidebarData.generated.ts` with
`scripts/generate-gf-sidebar-data.mjs`, which reads the upstream
`glyphsets` YAML definitions and generated Glyphs-compatible
nice-name lists. This keeps Runebender's sidebar filters aligned with
Google Fonts names like `GF Latin Core` while still shipping a local,
versioned manifest in the web bundle.

## Theme Direction

The long-term theme shape is a checked-in semantic theme definition
in `runebender-core`, consumed by both frontends:

- xilem maps tokens to Rust theme constants or runtime theme state.
- comfy maps tokens to CSS custom properties and renderer values.
- a ComfyUI adapter can map ComfyUI palette tokens onto Runebender's
  semantic tokens.

Until that lands, keep the palette byte-for-byte aligned with xilem's
`theme.rs`. Mark colors are semantic workflow slots, not decorative
colors; do not tweak them casually.

## Save and Round Trip

Current state is in-memory first. The Comfy host now supports saving
or exporting the active glyph as a `.glif`, and when the browser
exposes a writable file handle it writes back in place. Export fallback
still covers hosts without writable handles.

The intended direction is that edits mutate an owned glyph/session
model in Rust, then serialize back to UFO `.glif`/metadata formats
through a narrow wasm API. Vue should coordinate browser file access
and ComfyUI output plumbing, but it should not become the long-term
owner of outline semantics.

When implementing save, preserve UFO semantics explicitly:

- contours and components round-trip without changing meaning
- mark colors use `public.markColor`
- kerning groups come from `groups.plist`
- designspace masters remain distinct workspaces

## ComfyUI State Bridge

The ComfyUI graph should carry a `FONT` wire, not raw glyph bytes or
preview SVG. The `FONT` value is a workspace reference that downstream
nodes can reuse for editing, rendering, forking, and AI transforms.

The Runebender node still keeps a live preview snapshot in the Python
host, keyed by node id, but that is side-channel state only. The
workflow output is the `FONT` reference, not the preview.

That bridge is intentionally narrow:

- Vue remains responsible for editing UX and local glyph rendering.
- Python remains responsible for Comfy graph inputs, outputs, and
  queued execution.
- the exchange format stays simple for now: workspace path + preview
  state, with the preview kept separate from graph data.

If the workflow contract grows beyond a workspace path, add a new
serialized payload shape deliberately rather than letting ad hoc
fields accrete in the bridge.

## `FONT` Workspace References

The graph-level `FONT` type is a workspace reference, not raw glyph
data. A workspace owns the font source side and, when available, a
compiled renderable side. Nodes should pass the reference around
unchanged and resolve it only when they need a concrete file path.

A minimal local workflow is `Load Font -> Runebender -> Compile Font
-> Font Preview`, with `Fork Font` as the branching primitive. The
checked-in starter notes live in
`docs/workflows/local-font-workflow.md`.

Source format policy:

- UFO/designspace is the default editable source.
- Glyphs is supported as an alternate source/import format.
- Glyphs imports should normalize into UFO/designspace in the
  workspace when the optional `glyphsLib` dependency is available.
- glyphspackage is the compile-oriented Google Fonts interchange
  format; the workspace can materialize one from the editable source
  before invoking `fontc`.
- the workspace exporter writes a package `sources/` tree plus
  `sources/config.yaml` so the compile seam stays isolated from the
  graph.
- the `Load Font` node exposes this choice explicitly so the default
  stays on the UFO/designspace path.
- downstream nodes should not assume a specific source extension; they
  should only rely on the `FONT` reference and ask the workspace for
  concrete paths when needed.

Current workspace shape:

- `Load Font` node creates or imports a workspace from a source path.
- `CompileFont` materializes a compiled artifact when the backend is
  available, auto-building a `glyphspackage` source package in the
  workspace first when needed.
- `ForkFont` clones a workspace for parallel exploration.
- `Runebender` edits the workspace's source state.
- the Vue editor can hydrate a workspace by fetching its text files from
  the ComfyUI server.
- the Vue editor can write edited `.glif` text back into the workspace
  through the ComfyUI server.
- preview/render nodes resolve the workspace to a compiled font if one
  exists.

Keep this contract opaque to downstream nodes. The workspace reference
is the stable wire value; the filesystem layout remains an
implementation detail.

## Multi-Agent Context

Checked-in context should be durable and layered:

- `AGENTS.md` - onboarding, commands, policies, gotchas.
- `.agents/REPO_MAP.md` - where to look for each kind of task.
- `.agents/UI_PARITY_PLAN.md` - current UI parity roadmap.
- `docs/architecture/decisions.md` - durable architecture decisions.
- `.agents/active/*.md` - short-lived in-flight claims only.

Avoid putting one-off investigation notes in `AGENTS.md`. Put them in
a task plan under `.agents/` if they are useful beyond the current
session, or delete them when the task is complete.
