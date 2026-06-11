---
slug: glyph-tracing-providers
agent: Codex (eli@desktop)
branch: main
worktree: /Users/eli/GH/repos/runebender-comfy
started: 2026-06-06
last_touched: 2026-06-09 01:35 PDT
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
  - web/src/hosts/comfy/comfyHost.ts
  - web/src/host/runebenderHost.ts
  - web/src/hosts/browser/browserHost.ts
  - web/src/hosts/comfy/comfyHost.ts
  - rust-core/Cargo.toml
  - rust-core/src/image_trace.rs
  - rust-core/src/wasm_api.rs
  - ../img2bez/Cargo.toml
  - ../img2bez/src/bitmap.rs
  - ../img2bez/src/glif.rs
  - ../img2bez/src/lib.rs
  - ../img2bez/src/vectorize/curve.rs
  - ../img2bez/src/vectorize/mod.rs
  - ../img2bez/autoresearch/focused_glyphs.txt
  - ../img2bez/autoresearch/check_structural_gate.py
  - ../img2bez/autoresearch/compare_structural_runs.py
  - ../img2bez/autoresearch/inspect_glyph.sh
  - ../img2bez/autoresearch/run_structural_gate.sh
  - ../img2bez/autoresearch/run_structural_loop.sh
  - ../img2bez/autoresearch/structural-loop.md
  - ../img2bez/autoresearch/structural_report.py
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
- [x] Added browser-safe `img2bez` byte-input and GLIF-output APIs that
  build with `default-features = false`.
- [x] Added `img2bez` as a local path dependency of `rust-core` with
  default features off.
- [x] Added the `traceImageToGlif` WASM export and a single
  `rust-core/src/image_trace.rs` adapter for future `img2bez` updates.
- [x] Updated editor `Trace Image` to prefer local WASM tracing, with the
  existing Python/backend route as fallback.
- [x] Rebuilt WASM and `web/dist`; bundle fingerprint is now
  `rb-bundle-2026-06-07-trace-wasm-img2bez`.
- [x] Verified `img2bez` slim mode with `cargo test --no-default-features`.
- [x] Verified `runebender-comfy` with `cargo check --target
  wasm32-unknown-unknown`, `cargo test`, `pnpm build`, `python3 -m
  unittest tests.test_workspace tests.test_web_bundle`, and `git diff
  --check`.
- [x] Started a structural tracing improvement goal for `img2bez` using
  Virtua Grotesk Regular reference glyphs `ampersand a e s R O S n`.
- [x] Added a focused structural autoresearch loop with per-glyph SVG
  overlays and GLIF structure scoring against the reference UFO.
- [x] Baseline focused loop: mean IoU `93.77%`, mean vector score `0.824`,
  mean structural score `0.859`.
- [x] First parameter experiment `--smooth 1 --alphamax 1.0`: mean IoU
  `92.60%`, mean vector score `0.811`, mean structural score `0.867`;
  archived for visual inspection, not adopted as default yet.
- [x] Added a one-glyph inspection loop that writes the source PNG, traced
  UFO, split-debug log, structural report, raster diff, and SVG overlay for
  individual glyphs.
- [x] Verified one-glyph inspection for `s` and `n` in a clean temp copy.
  `s` baseline is `0.754`; `s --alphamax 0.94` is `0.778`. `n` baseline is
  `0.906`; `n --alphamax 0.94` is `0.917`.
- [x] Added a structural run comparison helper. Comparing `baseline` to
  `alphamax094` shows mean focused score delta `+0.006`: useful local
  evidence, but not enough for a global default because `a` regresses.
- [x] Applied the first structural tracer improvement: raised
  `CURVATURE_TRANSITION_THRESHOLD` from `0.37` to `0.50`.
- [x] Archived `transition050`: mean IoU `94.70%`, mean vector score
  `0.859`, mean structural score `0.893`; structural delta versus baseline is
  `+0.033` with 5 glyphs improved, 1 worsened, and `n` unchanged.
- [x] Verified `img2bez` with `cargo test` and
  `cargo test --no-default-features` after the transition threshold change.
- [x] Checked the app's actual trace setting: `alphamax 0.35` produced a weak
  focused structural score (`0.755`) with many extra points, so the editor
  trace path now uses `alphamax 0.8`.
- [x] Rebuilt Runebender WASM against sibling `img2bez`; `wasm-pack` compiled
  `img2bez` from `/Users/eli/GH/repos/img2bez`.
- [x] Rebuilt the ComfyUI web bundle with fingerprint
  `rb-bundle-2026-06-08-trace-transition050-app-test`.
- [x] Aligned Comfy host backend fallback to `alphamax 0.8`, matching the WASM
  trace path.
- [x] Added `alphamax` to the editor trace request log so manual app testing can
  confirm `accuracy=4 alphamax=0.8` before judging outline quality.
- [x] Added `autoresearch/check_structural_gate.py`; it passes
  `transition050` with required ampersand/`s` improvement and protected `n`/`O`,
  and it fails the bad `app-alpha035` run.
- [x] Added `autoresearch/run_structural_gate.sh` so the current checkout can
  run the focused loop, archive the result, and gate that exact archive in one
  command.
- [x] Ran `RUN_LABEL=current-transition050 ./autoresearch/run_structural_gate.sh`
  in `img2bez`: focused structural gate passed with mean score delta `+0.033`.
- [x] Ran `python3 -m unittest tests.test_workspace tests.test_web_bundle`:
  117 tests pass.
- [x] Ran `git diff --check` in `runebender-comfy` and `img2bez`: both pass.
- [x] Live readiness checker passes outside the sandbox against
  `http://127.0.0.1:8188`; ComfyUI is running, this checkout is symlinked, and
  tracing nodes are registered.
- [x] In-app browser verification loaded ComfyUI and captured the runtime
  extension log from
  `/extensions/runebender-comfy/runebender-comfy.js`:
  `rb-bundle-2026-06-08-trace-transition050-app-test`.
- [x] Live ComfyUI-host editor verification with an actual placed ampersand
  image confirms better point placement in the app. Current result is an
  improved checkpoint, but still has major structural mistakes to address next.
- [x] Folded OH no Type's vector drawing rule into the img2bez autoresearch
  target: minimal points, smooth points at extrema, inflections where curves
  change direction, H/V handles by default, and clustered/diagonal handles only
  for true corners or terminals.
- [x] Tested broader false-corner suppression thresholds. `1.10` helped `s` but
  regressed `ampersand`, `a`, `e`, `S`, and `n`; `1.00` passed the old baseline
  gate but was still worse than the `transition050` checkpoint.
- [x] Added a tracked `transition050` structural baseline in `img2bez` and
  changed `run_structural_gate.sh` into a regression gate against that current
  known-good checkpoint.
- [x] Accepted a targeted contour-complexity tracing improvement: contours with
  at least 100 polygon vertices use a stricter curvature-transition threshold
  (`0.55` instead of `0.50`). Focused structural score improves from `0.893` to
  `0.896`; `S` improves from `0.850` to `0.878`; the rest of the focused set is
  unchanged against the previous checkpoint.
- [x] Updated the tracked `img2bez` current structural baseline to
  `complex-transition055`.
- [ ] Live Quiver Cloud run is pending user confirmation of credentials and
  budget.
- [ ] Keep the checklist updated as implementation work lands.

## Notes

The working checkout already has tracing-related local changes and
untracked workflow docs. Stage carefully and avoid touching unrelated
dirty files except where the tracing checklist explicitly requires it.
