---
slug: glyph-tracing-providers
agent: Codex (eli@desktop)
branch: main
worktree: /Users/eli/GH/repos/runebender-comfy
started: 2026-06-06
last_touched: 2026-06-06
touches:
  - docs/workflows/local-ai-glyph-tracing.md
  - docs/workflows/local-ai-glyph-tracing-rethink.md
  - docs/workflows/local-font-workflow.md
  - nodes/runebender.py
  - nodes/glyph_candidate_builder.py
  - nodes/apply_glyph_candidates.py
  - web/src/Runebender.vue
  - web/src/host/runebenderHost.ts
  - web/src/hosts/browser/browserHost.ts
  - web/src/hosts/comfy/comfyHost.ts
---

## Goal

Make background-image glyph tracing work end-to-end with the local Rust
tracing path, optional local model preprocessing, and optional QuiverAI /
Comfy Cloud SVG vectorization, while keeping all outputs as reviewed
candidate outlines.

## Status

- [x] Preflight passed; dirty state noted.
- [x] Existing stale `canvas-theme-pass` claim reviewed.
- [ ] Merge the two tracing plan docs into one canonical checklist.
- [ ] Keep the checklist updated as implementation work lands.

## Notes

The working checkout already has tracing-related local changes and
untracked workflow docs. Stage carefully and avoid touching unrelated
dirty files except where the tracing checklist explicitly requires it.
