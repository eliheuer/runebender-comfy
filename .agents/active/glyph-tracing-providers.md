---
slug: glyph-tracing-providers
agent: Codex (eli@desktop)
branch: main
worktree: /Users/eli/GH/repos/runebender-comfy
started: 2026-06-06
last_touched: 2026-06-06 18:47 PDT
touches:
  - docs/workflows/local-ai-glyph-tracing.md
  - docs/workflows/local-ai-glyph-tracing-rethink.md
  - docs/workflows/local-font-workflow.md
  - example_workflows/local-model-mask-to-trace-template.md
  - example_workflows/quiver-image-to-svg-manual-template.md
  - nodes/runebender.py
  - nodes/glyph_trace.py
  - nodes/mark_colors.py
  - nodes/glyph_candidate_builder.py
  - nodes/apply_glyph_candidates.py
  - tests/test_workspace.py
  - tests/test_web_bundle.py
  - web/src/Runebender.vue
  - web/src/host/runebenderHost.ts
  - web/src/hosts/browser/browserHost.ts
  - web/src/hosts/comfy/comfyHost.ts
---

## Goal

Make background-image glyph tracing work end-to-end with the local Rust
tracing path, optional local model preprocessing, and optional QuiverAI /
Comfy Cloud SVG vectorization. The default editor path should trace into
the active glyph; candidate slots are only advanced graph/review plumbing.

## Status

- [x] Preflight passed; dirty state noted.
- [x] Existing stale `canvas-theme-pass` claim reviewed.
- [x] Merged the two tracing plan docs into
  `docs/workflows/local-ai-glyph-tracing.md`.
- [x] Retired `docs/workflows/local-ai-glyph-tracing-rethink.md`.
- [x] Added mocked tests for the existing direct `img2bez` helper and
  `/runebender/workspace/trace_background` route.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the phase 1 tests.
- [x] Added `TraceImageTransform` plus round-trip tests for phase 2.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the phase 2 tests.
- [x] Added `GLYPH_TRACE_REQUEST` write/reload helpers and tests for
  phase 3.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the phase 3 tests.
- [x] Added `BuildGlyphTraceRequest` and `TraceToCandidate` graph nodes,
  registration, publish-readiness mapping updates, and candidate fork
  tests for phase 4.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the phase 4 tests.
- [x] Added strict SVG import, `TraceWithQuiverAI`, fake Quiver SVG tests,
  and a manual Comfy Cloud template.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  and `git diff --check` after the Quiver import work.
- [x] Added `TraceLocalMaskToCandidate`, a local model mask handoff
  template, and tests proving mask bytes feed the local tracer.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  and `git diff --check` after the local mask handoff work.
- [x] Added `ScoreCandidate` with GLIF structural review metrics and
  tests.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the scoring work.
- [x] Routed direct editor trace requests through `TraceImageTransform`
  by sending full background transform fields from Vue to Python.
- [x] Rebuilt `web/dist`, bumped bundle fingerprint to
  `rb-bundle-2026-06-06-trace-transform-providers`, and verified tests.
- [x] Added an installed-`img2bez` integration test with a generated PNG
  fixture and valid temporary UFO; it passes on this machine.
- [x] Embedded `ScoreCandidate` output in provider reports and added green
  reference stem comparison coverage.
- [x] Added `RUNEBENDER_TRACE_TOOL` for future `img2bez`-compatible Rust
  tracers while keeping `RUNEBENDER_IMG2BEZ` compatibility.
- [x] Added PNG foreground-bbox comparison in `ScoreCandidate`.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after the tracer adapter and foreground scoring work.
- [x] Added `/runebender/workspace/trace_background_candidate` as advanced
  graph/review plumbing; removed it from the primary editor workflow after
  deciding direct tracing into empty glyphs is the right default.
- [x] Added a skippable installed-`img2bez` integration test that runs the
  placed-image candidate route end to end on this machine.
- [x] Removed the editor candidate report overlay along with the editor
  candidate action; normal editor review is the default after `Trace Image`.
- [x] Rebuilt `web/dist`, bumped bundle fingerprint to
  `rb-bundle-2026-06-06-direct-trace-primary`, and verified tests.
- [x] Added `TraceWithComfyCloudQuiverAI`, an optional automated Comfy
  Cloud provider that uploads the trace image, submits a Quiver workflow
  with Partner Node `extra_data`, polls for the SVG output, and imports it
  into the same candidate flow.
- [x] Added mocked Cloud API tests for upload, prompt submission, job
  status/history, SVG download, and candidate import without live Cloud
  access.
- [x] Added raster overlay diff scoring for PNG trace requests, including
  IoU, precision, recall, and false-positive/false-negative counts.
- [x] Added `green-reference-overlay` in editor mode and kept `Cmd+T` as
  the direct trace shortcut. `Cmd+Shift+T` no longer creates candidate
  slots from the editor.
- [x] Rebuilt `web/dist`, bumped bundle fingerprint to
  `rb-bundle-2026-06-06-direct-trace-primary`, and verified tests.
- [x] Added `scripts/check_tracing_live_ready.py` to check local tracer
  discovery, ComfyUI `/object_info` node registration, and Cloud API key
  presence without running paid Cloud jobs.
- [x] Extended `scripts/check_tracing_live_ready.py` with
  `--comfy-root` so it also verifies the running ComfyUI custom-node
  install points at this checkout.
- [x] Extended `scripts/check_tracing_live_ready.py` to verify this
  checkout's local `NODE_CLASS_MAPPINGS` declare all tracing nodes, so a
  running-host mismatch is clearly a restart/reload issue.
- [x] Added local-only readiness mode so Comfy Cloud credentials can be
  paused while local Rust tracing and local model handoff are verified.
- [x] Restarted the local ComfyUI server from
  `/Users/eli/Work/comfy/repos/ComfyUI` using its `.venv`.
- [x] Local-only live readiness now passes against the restarted ComfyUI
  host.
- [x] Live placed-image candidate route passed through ComfyUI and wrote
  `trace-live-local-candidate` with provider `placed-background-img2bez`.
- [x] Live direct route passed through ComfyUI and returned a GLIF outline
  for `trace-live-local` without creating a candidate workspace.
- [x] Runtime local model mask handoff passed and wrote
  `trace-live-local-mask-candidate` with provider
  `local-model-mask-img2bez`.
- [x] Ran the live checker outside the sandbox: local `img2bez` and
  ComfyUI `/system_stats` pass; `COMFY_CLOUD_API_KEY` is missing and the
  running host has stale Runebender node registration.
- [x] Confirmed `/Users/eli/Work/comfy/repos/ComfyUI/custom_nodes/runebender-comfy`
  is a symlink to this checkout; ComfyUI should need a restart, not a
  reinstall, to load the new tracing mappings.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  successfully after adding the checker: 116 tests.
- [x] Ran `git diff --check` successfully after adding the checker.
- [ ] Live ComfyUI-host editor verification with an actual placed image is
  still pending.
- [ ] Live Quiver Cloud run is pending user confirmation of credentials and
  budget.
- [ ] Keep the checklist updated as implementation work lands.

## Notes

The working checkout already has tracing-related local changes and
untracked workflow docs. Stage carefully and avoid touching unrelated
dirty files except where the tracing checklist explicitly requires it.
