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

Current state is in-memory. Save is not wired.

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
