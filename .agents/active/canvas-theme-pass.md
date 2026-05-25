---
slug: canvas-theme-pass
agent: Codex (eli@desktop)
branch: main
worktree: /Users/eli/GH/repos/runebender-comfy
started: 2026-05-23
last_touched: 2026-05-25
touches:
  - rust-core/src/renderer.rs
  - rust-core/src/wasm_api.rs
  - web/src/Runebender.vue
  - web/src/extension.ts
  - tests/test_web_bundle.py
---

## Goal

Wire the edit canvas to the live Runebender/ComfyUI theme palette so
Rust-rendered glyph outlines, points, guides, previews, and canvas
background stay visually aligned with the DOM chrome.

## Status

- [x] Preflight passed; no overlapping active claims found.
- [x] Added renderer-owned `CanvasTheme` with JS-facing `GlyphEditor.setTheme`.
- [x] Removed fixed inline Vue chrome variables so `.runebender-host` follows ComfyUI CSS variables.
- [x] Added host-side CSS color resolution and applies the computed palette to the WASM renderer.
- [x] Rebuilt WASM and the Vue bundle.
- [x] Ran `python3 -m unittest tests.test_web_bundle tests.test_workspace`.
- [x] Ran `cargo test` and `cargo fmt -- --check`.

## Notes

`web/wasm/` is ignored by git but was regenerated locally by
`COREPACK_ENABLE_AUTO_PIN=0 pnpm wasm:release` before the Vue bundle
was rebuilt.
