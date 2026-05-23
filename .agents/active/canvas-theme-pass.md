---
slug: canvas-theme-pass
agent: Codex (eli@desktop)
branch: agent/canvas-theme-pass
worktree: ~/Temp/worktrees/runebender-comfy-canvas-theme-pass
started: 2026-05-23
last_touched: 2026-05-23
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
- [ ] Finish host-side theme extraction and `setTheme` application.
- [ ] Rebuild WASM/bundle and run web bundle/workspace tests.

## Notes

Started from an existing local `rust-core/src/renderer.rs` change that
already introduces a renderer-owned `CanvasTheme`.
