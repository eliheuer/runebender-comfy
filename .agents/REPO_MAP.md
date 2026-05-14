# Agent Repo Map

Use this as the first routing table after reading `AGENTS.md`.
It points agents to the files that usually matter for a task, plus
the sibling repo references that keep `runebender-comfy` aligned with
the native editor.

## First Checks

Before non-trivial work:

1. Run `scripts/agent-doctor.sh` from the repo root.
2. Read `.agents/active/*.md` for live claims.
3. If the task touches `../runebender-core` or `../runebender-xilem`,
   also skim those repos' `AGENTS.md` and `.agents/active/`.

## UI Parity Work

Comfy files:

- `web/src/Runebender.vue` - layout root and current state owner.
- `web/src/components/*.vue` - Vue peers for xilem panels.
- `web/src/components/markColors.ts` - mark color constants.
- `web/src/components/toolIds.ts` - edit toolbar tool ids.

Xilem references:

- `../runebender-xilem/src/views/glyph_grid/mod.rs`
- `../runebender-xilem/src/views/glyph_grid/glyph_cell.rs`
- `../runebender-xilem/src/components/*.rs`
- `../runebender-xilem/src/theme.rs`

Keep component names aligned where possible. A change to xilem's
`src/components/glyph_info_panel.rs` should be easy to mirror in
`web/src/components/GlyphInfoSidebar.vue`.

## Editing Behavior

Comfy files:

- `rust-core/src/wasm_api.rs` - JS/Vue API surface.
- `rust-core/src/editor.rs` - editable glyph state.
- `rust-core/src/editing/*` - mouse state, viewport, hit testing.
- `rust-core/src/tool.rs` - active tool behavior.
- `rust-core/src/path/*` - local path representation.

Xilem references:

- `../runebender-xilem/src/editing/*`
- `../runebender-xilem/src/editing/session/*`
- `../runebender-xilem/src/tools/*`
- `../runebender-xilem/src/path/*`

Shared logic candidate:

- `../runebender-core/src/editing/*`
- `../runebender-core/src/model/*`

Do not move public `kurbo` types into `runebender-core` while xilem
and comfy are on different kurbo versions.

## Glyph Grid, UFO, and Master Loading

Comfy files:

- `web/src/Runebender.vue` - drag/drop, master maps, glyph metadata.
- `web/src/devTestFont.ts` - dev-only auto-loader.
- `rust-core/src/wasm_api.rs` - `.glif` parsing helpers.
- `rust-core/src/model/workspace.rs` - local owned font model.

Xilem references:

- `../runebender-xilem/src/data/file_io.rs`
- `../runebender-xilem/src/model/workspace.rs`
- `../runebender-xilem/src/model/designspace.rs`

## Rendering

Comfy files:

- `rust-core/src/renderer.rs` - Vello/WebGPU rendering.
- `rust-core/src/model/glyph_renderer.rs` - contour-to-path helpers.
- `rust-core/src/editor.rs` - glyph conversion and metrics.

Xilem references:

- `../runebender-xilem/src/components/editor_canvas/paint.rs`
- `../runebender-xilem/src/components/editor_canvas/drawing.rs`
- `../runebender-xilem/src/model/glyph_renderer.rs`
- `../runebender-xilem/src/theme.rs`

Load-bearing rendering rule: fill all contours as one `BezPath`, not
one fill per contour, so counters stay holes under nonzero winding.

## ComfyUI Integration

Files:

- `__init__.py` - ComfyUI extension registration.
- `nodes/runebender.py` - Runebender Python graph node.
- `nodes/designbot.py` - DesignBot Python graph node.
- `web/src/extension.ts` - ComfyUI frontend widget registration.
- `web/vite.config.ts` - production bundle entry.

Standalone Vite remains the main dev loop until ComfyUI integration
is the task itself.

## Supply Chain and Build

Files:

- `SECURITY.md` - dependency policy.
- `scripts/audit.sh` - cargo-deny + crate-age checks.
- `tools/check-crate-age/` - Rust proactive age checker.
- `web/.npmrc` - pnpm 7-day cooldown.
- `web/pnpm-workspace.yaml` - pnpm 10 build-script approval.
- `rust-core/deny.toml` - cargo-deny policy.

## Durable Design Notes

- `docs/architecture/decisions.md` - decisions agents should preserve.
- `.agents/UI_PARITY_PLAN.md` - current UI roadmap.
- `AGENTS.md` - evergreen onboarding and gotchas.
