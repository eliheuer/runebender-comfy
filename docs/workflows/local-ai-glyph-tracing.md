# Local AI Glyph Tracing Workflow

Canonical plan and live checklist for turning placed background images
into editable glyph outlines in `runebender-comfy`.

Weekend target: by Sunday, June 7, 2026, have three provider paths
working behind the same review contract:

- local Rust tracing tools for deterministic image-to-outline tracing
- local model preprocessing that outputs a better image or mask
- QuiverAI through Comfy Cloud / Partner Nodes for Image-to-SVG

Current focus, 2026-06-06: Comfy Cloud / QuiverAI is on hold. Local
readiness means the local Rust tracer, local model mask handoff, ComfyUI
node registration, and direct Runebender trace-insertion paths work
without requiring `COMFY_CLOUD_API_KEY`.

The default local workflow is direct: place a good image or AI-cleaned
mask behind an empty glyph, run `Trace Image`, inspect the result in
Runebender, and save if it is good.

## Core Decisions

- Deterministic tracing is the baseline. AI and cloud providers are
  optional front ends.
- The machine should hand the human a clean outline, correctly placed in
  design space. The human matches type style in Runebender.
- For empty codepoints, trace directly into the active glyph. Candidate
  workspaces are advanced graph experiments, not the normal workflow.
- Green-marked glyphs are references for scoring and future training,
  not required inputs for a single literal trace.
- Cleanup is mechanical and near-lossless: grid snap, despeckle, winding,
  contour thresholds, width/sidebearing preservation. No auto-restyling.
- Provider output is normalized through one strict import path. QuiverAI
  SVG, local model masks, and local tracer output all go through the same
  cleanup and design-space transform contract.

## Existing Hooks

- `web/src/Runebender.vue` owns active glyph state, mark colors,
  background image bytes, and background image design placement.
- `web/src/hosts/comfy/comfyHost.ts` already posts
  `/runebender/workspace/trace_background`.
- `nodes/runebender.py` already has `trace_background_with_img2bez(...)`
  and the `/runebender/workspace/trace_background` route.
- `nodes/mark_colors.py` mirrors editor mark colors for Python nodes.
- Candidate graph nodes remain available for advanced comparison work, but
  the editor's primary action is direct `Trace Image`.

## Architecture

Direct editor path:

```text
Runebender editor context
  -> background image bytes + design transform
  -> provider: local tracer / local model mask / QuiverAI SVG
  -> strict import + cleanup
  -> returned GLIF
  -> undoable insertion into active glyph
  -> normal Runebender save
```

Advanced graph/candidate path:

```text
Runebender editor context
  -> Build Trace Request
  -> provider: Trace To Candidate / Trace With QuiverAI / Local Model Mask
  -> Score Candidate
  -> optional Runebender review
  -> Apply Glyph Candidates
```

Provider rule:

```text
background image / cleaned mask / Quiver SVG
  -> strict import
  -> one design-space transform
  -> GLIF cleanup
  -> score + report
  -> active glyph for the default editor path
```

## Data Contracts

### `GLYPH_TRACE_REQUEST`

The request must be server-visible. Browser object URLs are not enough.
Store this per glyph in the workspace cache for graph mode; multipart
bytes are acceptable for direct editor tracing.

```json
{
  "version": 1,
  "slot": "virtua-grotesk",
  "glyph": "numbersign",
  "master": "Regular",
  "image": { "path": "...png", "width": 2048, "height": 2048 },
  "transform": {
    "designX": 0,
    "designY": -200,
    "designScaleX": 0.5,
    "designScaleY": 0.5
  },
  "metrics": {
    "advanceWidth": 600,
    "unitsPerEm": 1000,
    "ascender": 800,
    "descender": -200
  }
}
```

Do not include reference marks in the request. References are selected
by the scoring node or editor state.

### `TRACE_PROVIDER_RESULT`

Provider output must be one of:

- GLIF bytes
- simple filled SVG paths
- raster mask/image bytes to be traced locally

Provider output must also include:

- provider name and version
- source request ID
- import warnings/errors
- generated candidate mark color, default orange

### `TRACE_REPORT`

Every candidate should produce a report:

- overlay difference against the background image
- bbox and overshoot plausibility
- point and contour counts
- winding correctness
- speckle/sub-threshold contour count
- sidebearing and advance consistency
- optional stem-width comparison against green references

The report should explain why a candidate is usable or bad. This makes
the node useful even when generation fails.

## Provider Tracks

### Local Rust Tracing

This is the baseline path. It should use Eli's local tracing tools first:
`img2bez` today, and any future Rust tracing CLI behind the same command
adapter.

Required behavior:

- copy the active master UFO into a temporary directory
- write the placed image or cleaned mask to that directory
- run the tracer against the temporary UFO
- read the target `.glif`
- normalize width, Unicode, mark color, and sidebearings
- translate/snap the result into the background image's design-space
  position
- return GLIF directly or write a candidate `FONT`

### Local Model Preprocessing

Local models are upstream only. They should output a cleaner image or
mask, never final production outlines.

Useful first forms:

- image cleanup from a rough sketch or scan
- black-on-white silhouette generation
- local LoRA/style prior trained on rendered glyph images

The local model output flows back into the local tracer path.

### QuiverAI Through Comfy Cloud

QuiverAI is an optional cloud vectorization provider. Comfy Cloud /
Partner Nodes can run Quiver Image-to-SVG, so it is useful when the user
wants vector output without local model setup.

Current external contract, checked 2026-06-06:

- Quiver Image-to-SVG is available through ComfyUI Partner Nodes and
  Comfy Cloud.
- Comfy Cloud API workflow execution uses `POST
  https://cloud.comfy.org/api/prompt` with `X-API-Key`.
- Cloud API access requires a paid Comfy Cloud tier; do not run paid
  workflow calls until the user confirms credentials and budget.
- Direct API workflows that contain Partner Nodes must include
  `extra_data.api_key_comfy_org` with the Comfy API key. The browser UI
  packages this automatically, but a Runebender node or CLI bridge must
  send it explicitly.

Sources:

- https://blog.comfy.org/p/quiver-structured-svg-generation
- https://docs.comfy.org/development/cloud/overview
- https://docs.comfy.org/development/cloud/api-reference
- https://docs.comfy.org/api-reference/cloud/workflow/submit-a-workflow-for-execution

Required behavior:

- send the placed image, cleaned mask, or AI-generated silhouette through
  Quiver Image-to-SVG in Comfy Cloud / Partner Nodes
- import only returned SVG path geometry
- reject unsupported SVG constructs: strokes, masks, filters, gradients,
  text, embedded rasters, and complex transforms
- normalize SVG coordinates through the same design-space transform as
  local tracing
- return GLIF directly or write a candidate `FONT`

The Quiver path must not bypass cleanup, scoring, or human review.

Manual Comfy Cloud path:

Template: `example_workflows/quiver-image-to-svg-manual-template.md`.

1. Build a `GLYPH_TRACE_REQUEST` from the Runebender `FONT`, glyph,
   master, placed image, design transform, and metrics.
2. Upload the request image to Comfy Cloud.
3. Run a Quiver Image-to-SVG workflow on that image.
4. Export the resulting SVG.
5. Run `Trace With QuiverAI` with the original `FONT`, the
   `GLYPH_TRACE_REQUEST`, and the exported SVG path.
6. Open the candidate `FONT` in Runebender, review the orange-marked
   glyph, then promote with `Apply Glyph Candidates` if it is acceptable.

Automated Comfy Cloud path:

- keep the same `GLYPH_TRACE_REQUEST`
- run `Trace With Comfy Cloud QuiverAI` when an approved API key and
  API-format Quiver workflow exist
- upload the request image to `POST /api/upload/image`
- inject the uploaded filename into the configured workflow image node
- submit the workflow to `POST /api/prompt` with `X-API-Key` and
  `extra_data.api_key_comfy_org`
- poll job status and read history for the returned SVG output
- download the returned SVG into the workspace cache
- call the same strict SVG import used by the manual path

Do not require a live Cloud run for local CI. Tests should use mocked API
responses and local recorded/fake Quiver SVG output.

## Implementation Checklist

### 0. Coordination And Plan

- [x] Run `scripts/agent-doctor.sh`.
- [x] Review active claims.
- [x] File and push `.agents/active/glyph-tracing-providers.md`.
- [x] Merge the original tracing doc and the rethink doc into this
  canonical plan.
- [x] Retire `docs/workflows/local-ai-glyph-tracing-rethink.md`.

### 1. Current Baseline Audit

- [x] Verify the existing direct `img2bez` route works through live
  ComfyUI with a placed image payload.
- [x] Confirm the route returns GLIF with correct glyph name, Unicode,
  width, and mark metadata.
- [x] Confirm failures produce clear UI status and Python error JSON.
- [x] Add or update tests covering the existing route without requiring
  real Comfy Cloud access.
- [x] Update this checklist with the exact command/test evidence.

Evidence, 2026-06-06:

- `command -v img2bez` returned `/Users/eli/.cargo/bin/img2bez`.
- Added mocked tests for `trace_background_with_img2bez`,
  `/runebender/workspace/trace_background` request forwarding, and route
  error JSON.
- Live local ComfyUI direct-route smoke test posted a placed PNG to
  `/runebender/workspace/trace_background` and received a GLIF for the
  active glyph without creating a candidate workspace.
- Added `/runebender/workspace/trace_background_candidate` for placed
  background images. It stores a `GLYPH_TRACE_REQUEST`, runs the local
  tracer candidate provider, writes an orange-marked candidate slot, and
  returns the provider report. This is now advanced graph/review plumbing,
  not the default editor workflow.
- Added a skippable installed-`img2bez` integration test for the candidate
  route. On this machine it traced a generated PNG through a valid
  temporary UFO and wrote a `trace-demo-placed-candidate` report with
  foreground comparison.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 109 tests.

### 2. Design-Space Transform

- [x] Extract the background-image transform into a shared helper.
- [x] Define the transform from image pixels to font design units.
- [x] Add round-trip tests: design point -> image pixel -> design point.
- [x] Cover baseline, negative descender, scaled image, nonzero x/y, and
  active glyph advance width.
- [x] Use the helper in the direct editor route.
- [x] Use the helper in future SVG import and candidate nodes.

Evidence, 2026-06-06:

- Added `TraceImageTransform` in `nodes/runebender.py`.
- Added tests for baseline points, negative descender placement,
  nonzero origin, scaled image, snapped origin, and negative
  `designScaleY` target-height handling.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 89 tests.
- Direct editor tracing now sends full image dimensions and design
  transform from Vue; Python derives target height and snapped origin
  through `TraceImageTransform`.
- Rebuilt `web/dist` and bumped the bundle fingerprint to
  `rb-bundle-2026-06-06-trace-transform-providers`.

### 3. Trace Request Artifact

- [x] Add a server-visible `GLYPH_TRACE_REQUEST` artifact.
- [x] Store request image bytes in the workspace cache.
- [x] Store request JSON per glyph/master.
- [x] Add validation for missing glyph, master, image, transform, and
  metrics.
- [x] Add tests for request creation and reloading.

Evidence, 2026-06-06:

- Added `GlyphTraceRequestArtifact`, `write_glyph_trace_request`, and
  `load_glyph_trace_request` in `nodes/runebender.py`.
- Trace request files are written under
  `trace-requests/<master>/<glyph>/` inside the workspace slot.
- Request JSON records image path, dimensions, SHA-256, design transform,
  and font metrics.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 91 tests.

### 4. Advanced Candidate Nodes

- [x] Add `Build Trace Request`: `FONT`, glyph, master, image/bytes,
  transform -> `GLYPH_TRACE_REQUEST`.
- [x] Add `Trace To Candidate`: `FONT`, `GLYPH_TRACE_REQUEST` -> candidate
  `FONT` + report.
- [x] Reuse `Glyph Candidate Builder` fork mechanics.
- [x] Mark generated candidates orange by default.
- [x] Keep `Apply Glyph Candidates` as the only promotion step.
- [x] Add tests for candidate fork isolation.
- [x] Keep candidate nodes out of the default editor workflow.

Evidence, 2026-06-06:

- Added `nodes/glyph_trace.py` with `BuildGlyphTraceRequest` and
  `TraceToCandidate`.
- Registered both nodes in `NODE_CLASS_MAPPINGS` and
  `NODE_DISPLAY_NAME_MAPPINGS`.
- Removed the editor context-menu and shortcut entry points for candidate
  tracing. Direct `Trace Image` is the default path for filling empty
  glyphs.
- Updated the publish readiness node-list check.
- Added tests for node declarations, trace request creation from an image
  path, candidate forking, orange mark color, report writing, and source
  isolation.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 95 tests.

### 5. Local Rust Tracer Provider

- [x] Audit `img2bez` command resolution and document setup.
- [x] Add provider adapter around `img2bez`.
- [x] Support a future Rust tracing CLI behind the same adapter.
- [x] Add fixture image -> GLIF test.
- [x] Add cleanup checks for winding, speckles, width, and point count.
- [x] Add report fields for local tracer settings.

Evidence, 2026-06-06:

- Existing command resolution finds `$RUNEBENDER_IMG2BEZ`, PATH, Cargo
  install, release sibling build, or sibling Cargo manifest fallback.
- Added `$RUNEBENDER_TRACE_TOOL` as the generic override for any
  `img2bez`-compatible Rust tracing CLI, while keeping
  `$RUNEBENDER_IMG2BEZ` as the compatibility override.
- `TraceToCandidate` and `TraceLocalMaskToCandidate` both route through
  `trace_background_with_img2bez`.
- Candidate reports now include provider, command, trace tool, trace image
  path, and tracer settings.
- Added a skippable installed-`img2bez` integration test that generates a
  PNG fixture, traces it through a valid temporary UFO, and verifies GLIF
  outline output.
- Provider reports now embed `Score Candidate` output with width,
  contour count, point count, winding approximation, speckles, sidebearing
  metrics, and background bbox comparison.
- Added a unit test proving `$RUNEBENDER_TRACE_TOOL` resolves to a future
  Rust tracer command without depending on the real machine state.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 107 tests.

### 6. Strict SVG Import

- [x] Add SVG path parser/import helper.
- [x] Accept simple filled paths.
- [x] Reject strokes, masks, filters, gradients, text, embedded rasters,
  and unsupported transforms.
- [x] Convert SVG path coordinates to font design units with the shared
  transform.
- [x] Convert accepted paths to GLIF.
- [x] Add fixtures for accepted and rejected SVGs.

Evidence, 2026-06-06:

- Added strict `svg_to_glif` import in `nodes/glyph_trace.py`.
- Accepted simple filled `path` data with `M`, `L`, `H`, `V`, `C`, and
  `Z` commands.
- Rejected unsupported stroke and transform examples in tests.
- Added fake Quiver SVG import coverage.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 99 tests.

### 7. QuiverAI / Comfy Cloud Provider

- [x] Create a minimal Quiver Image-to-SVG workflow template.
- [x] Document the required Comfy Cloud / Partner Node setup.
- [x] Define the manual path: run Quiver in Comfy Cloud, download/export
  SVG, import into Runebender candidate flow.
- [x] Define the automated path if Cloud API/auth is available.
- [x] Add `Trace With QuiverAI` as an optional provider node or helper.
- [x] Add `Trace With Comfy Cloud QuiverAI` as an optional automated
  provider node.
- [x] Route Quiver SVG through strict SVG import.
- [x] Add tests that use recorded/fake Quiver SVG output, not live cloud
  calls.

Evidence, 2026-06-06:

- Added `TraceWithQuiverAI` as a manual SVG import provider node.
- Added `TraceWithComfyCloudQuiverAI` as an automated Cloud provider node.
- Registered both Quiver providers in Comfy node mappings.
- Added `example_workflows/quiver-image-to-svg-manual-template.md`.
- Added tests using local fake SVG output rather than live Comfy Cloud
  calls.
- Documented the Cloud API automation contract: submit the Quiver workflow
  to `POST /api/prompt`, send `X-API-Key`, include
  `extra_data.api_key_comfy_org` for Partner Nodes, then feed the returned
  SVG through the same strict importer.
- Added mocked API tests for image upload, prompt submission, Partner Node
  `extra_data`, job polling, history lookup, SVG download, and candidate
  import through strict SVG parsing.
- Updated the local workflow docs so `Trace With Comfy Cloud QuiverAI` is
  discoverable alongside the manual Quiver SVG import path.
- Live Cloud execution is intentionally not run yet because it needs
  paid-tier API access, credentials, and budget confirmation.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 113 tests.

### 8. Local Model Provider

- [x] Choose the first local preprocessing path for this weekend.
- [x] Create a workflow template that turns sketch/scan/image into a clean
  mask.
- [x] Feed the mask into the local Rust tracer provider.
- [x] Keep model setup optional and documented.
- [x] Add tests around the mask handoff, not the model runtime.

Evidence, 2026-06-06:

- Chosen first path is provider-neutral mask handoff: local models output
  a clean silhouette file, then Runebender traces that mask.
- Added `TraceLocalMaskToCandidate`.
- Added `example_workflows/local-model-mask-to-trace-template.md`.
- Added tests proving the local mask bytes are used instead of the
  original request image.

### 9. Score Candidate

- [x] Add reusable `Score Candidate`: candidate `FONT`, optional
  background, optional `mark:green` references -> report JSON.
- [x] Measure bbox, contour count, point count, winding, and sidebearing.
- [x] Add stem-width comparison against active-master green references.
- [x] Compare PNG trace-request foreground bbox against candidate bbox.
- [x] Add full rasterized glyph-vs-background pixel overlay diff.
- [x] Make the report usable for donor-copy candidates too.

Current scope note, 2026-06-06:

- Background comparison currently reports candidate bbox deltas against
  the request image's design-space bbox.
- Foreground comparison reports black-pixel PNG bbox deltas and area ratio.
- Raster overlay comparison reports sampled GLIF-vs-PNG foreground overlap
  with true/false positives, false negatives, precision, recall, and IoU.
  It uses an even-odd polygon fill approximation for dependency-free CI.

Evidence, 2026-06-06:

- Added `ScoreCandidate` and `score_candidate_glyph`.
- Registered `ScoreCandidate` in Comfy node mappings.
- Added tests for contour count, point count, bbox, approximate winding,
  speckle count, sidebearings, background bbox comparison, and green
  reference stem comparison.
- Added a generated PNG-mask fixture that verifies foreground pixel bbox
  mapping into design-space coordinates.
- Added raster overlay tests for a perfect candidate/background match and
  a shifted candidate that reports false positives and false negatives.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 116 tests.

### 10. Editor Review Tools

- [x] Show green-reference overlays in the editor.
- [x] Keep manual style matching in Runebender after direct tracing.
- [x] Keep `Cmd+T` as the direct image-to-glyph trace shortcut.
- [x] Remove candidate tracing from the editor's primary context menu.
- [x] Match xilem-style edit review by drawing active outlines as strokes,
  not filled silhouettes.

Evidence, 2026-06-06:

- Added a `green-reference-overlay` that shows green-marked reference
  glyph thumbnails, width, and contour count in editor mode.
- `Trace Image` fills the current glyph directly. Human review happens in
  the normal editor before save; no extra candidate font slot is required
  for the empty-codepoint workflow.
- Removed the `Trace To Candidate` editor menu item and stopped routing
  `Cmd+Shift+T` to candidate tracing.
- Rebuilt `web/dist` and bumped the bundle fingerprint to
  `rb-bundle-2026-06-06-direct-trace-primary`.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 116 tests.
- Built bundle contains `Trace Image` and
  `rb-bundle-2026-06-06-direct-trace-primary`; the editor no longer
  contains `Trace To Candidate` or `trace-report-overlay`.
- After screenshot review, found that the editor renderer was filling
  active edit outlines before stroking them. That made traced and existing
  glyphs look like black filled silhouettes instead of the expected
  outline-only point editing view.
- Removed edit-mode outline fill in `rust-core/src/renderer.rs`, kept
  preview/grid fills intact, and moved edit-mode contour strokes into the
  same screen-space `EditControlsGeometry` pass that draws visible handles
  and points.
- When direct cubic path strokes still did not show in Comfy, changed the
  edit contour to a flattened polyline generated from the editor segment
  iterator. This is a general editor regression fix for any opened glyph,
  not a tracing-specific fix.
- Rebuilt WASM and `web/dist`, and bumped the bundle fingerprint to
  `rb-bundle-2026-06-06-flattened-edit-outline`.
- Added a bundle test guard that rejects reintroducing `path_edit_fill`.
- Audited `workspace/fonts/demo` Regular and Bold UFOs: 333 glyphs,
  no missing glyphs, no contour-count mismatches, and no point-count
  mismatches. The screenshot interpolation error is therefore caused by
  in-memory direct tracing into one master while the other master remains
  empty, not by checked-in demo source corruption.
- `COREPACK_ENABLE_AUTO_PIN=0 pnpm wasm` passed.
- `COREPACK_ENABLE_AUTO_PIN=0 pnpm build` passed.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 116 tests.
- `cargo test` in `rust-core` passed: 179 tests.
- `git diff --check` passed.

### 10.5. Live Readiness Checker

- [x] Add a repeatable command that checks local tracer discovery,
  running ComfyUI node registration, and Comfy Cloud credential presence
  without running paid Cloud jobs.
- [x] Add tests for the checker helpers and current external blocker
  reporting.

Command:

```sh
python3 scripts/check_tracing_live_ready.py --strict \
  --comfy-root /Users/eli/Work/comfy/repos/ComfyUI
```

Local-only command while Comfy Cloud is paused:

```sh
python3 scripts/check_tracing_live_ready.py --strict --local-only \
  --comfy-root /Users/eli/Work/comfy/repos/ComfyUI
```

Evidence, 2026-06-06:

- Added `scripts/check_tracing_live_ready.py`.
- The checker verifies `RUNEBENDER_TRACE_TOOL` / `RUNEBENDER_IMG2BEZ` /
  `img2bez`, `COMFY_CLOUD_API_KEY`, the ComfyUI custom-node install path,
  local `NODE_CLASS_MAPPINGS`, ComfyUI `/system_stats`, and
  `/object_info` registration for Runebender tracing nodes.
- The checker does not submit Cloud workflows or spend Comfy Cloud
  credits.
- `--local-only` skips the Comfy Cloud API key check so local tracing can
  be verified while QuiverAI is paused.
- `python3 -m unittest tests.test_workspace tests.test_web_bundle`
  passed: 116 tests.
- `git diff --check` passed.
- Live checker result: local `img2bez` is available, ComfyUI responds to
  `/system_stats`, and
  `/Users/eli/Work/comfy/repos/ComfyUI/custom_nodes/runebender-comfy` is
  a symlink to this checkout. The checkout's local `NODE_CLASS_MAPPINGS`
  declares all tracing nodes. `COMFY_CLOUD_API_KEY` is not set, and the
  running ComfyUI host only sees older Runebender node mappings. It is
  missing `BuildGlyphTraceRequest`, `TraceToCandidate`,
  `TraceWithQuiverAI`, `TraceWithComfyCloudQuiverAI`,
  `TraceLocalMaskToCandidate`, and `ScoreCandidate`. Restart ComfyUI to
  reload the symlinked checkout, then rerun the checker.
- After restart, local-only live readiness passed:
  `python3 scripts/check_tracing_live_ready.py --strict --local-only
  --comfy-root /Users/eli/Work/comfy/repos/ComfyUI`.
- After the glyph-cache-clear-outline rebuild, local-only live readiness passed
  again against `127.0.0.1:8188`; ComfyUI registered all tracing nodes from
  the symlinked checkout.
- Live placed-image candidate smoke test passed through ComfyUI
  `/runebender/workspace/trace_background_candidate`. It wrote
  `trace-live-local-candidate`, used provider `placed-background-img2bez`,
  used `/Users/eli/.cargo/bin/img2bez`, wrote
  `workspace/fonts/trace-live-local-candidate/glyph-trace-report.json`,
  and returned score data with 1 contour and 5 points.
- Runtime local model mask handoff passed with `TraceLocalMaskToCandidate`.
  It wrote `trace-live-local-mask-candidate`, used provider
  `local-model-mask-img2bez`, used `img2bez`, wrote
  `workspace/fonts/trace-live-local-mask-candidate/glyph-trace-report.json`,
  and returned score data with 1 contour and 5 points.
- Local `img2bez` quality pass, 2026-06-06: reinstalled
  `/Users/eli/GH/repos/img2bez` into `/Users/eli/.cargo/bin/img2bez`.
  The installed binary now defaults to the corner-aware pipeline and keeps
  experimental smooth-only tracing behind `--smooth-only`. A controlled
  `uni0061.png` run changed the bad installed-output class from a large
  diagonal counter intrusion to a recognizable but still rough outline:
  2 contours, 9 curves, 20 lines, 47 points, and correct `1 outer, 1 counter`
  classification. `cargo test --manifest-path /Users/eli/GH/repos/img2bez/Cargo.toml`
  passed before reinstall.
- After reinstalling `img2bez`, local-only live readiness passed again:
  `python3 scripts/check_tracing_live_ready.py --strict --local-only
  --comfy-root /Users/eli/Work/comfy/repos/ComfyUI`.
- Autoresearch structural comparison, 2026-06-06: the ground-truth UFO is
  `/Users/eli/GH/repos/img2bez/autoresearch/reference.ufo`, currently a
  symlink to Virtua Grotesk Regular. The traced result is written to
  `/Users/eli/GH/repos/img2bez/autoresearch/work/output.ufo`.
- Ampersand ground truth vs traced structure showed why screenshots alone
  were misleading: the reference ampersand has 3 contours, 16 curves, 19
  lines, and 32 off-curves. The traced ampersand had correct contour count
  but too many straight spans: 3 contours, 14 curves, 27 lines, and 28
  off-curves. This points at curve/line segmentation, not only placement.
- Focused autoresearch over `ampersand a e s R` improved with
  `--smooth 0 --accuracy 2`: mean IoU moved from 93.14% to 93.54%, and
  mean score moved from 0.784 to 0.801. Full A-Z/a-z autoresearch also
  improved after the same setting change: mean IoU 96.26% to 96.35%, and
  mean score 0.895 to 0.897.
- Updated Runebender direct tracing and candidate/local-mask graph nodes to
  use `accuracy=2`, `smooth=0`, `alphamax=0.8`. Reinstalled `img2bez` with
  matching CLI defaults. A small `img2bez` source experiment also tightened
  `SHORT_SECTION_TOLERANCE` from 0.07 to 0.04; this slightly improved
  vector score while preserving the full-set IoU gain.
- Follow-up visual-quality check, 2026-06-06: the setting-only pass was not
  enough for complex glyphs such as ampersands. `IMG2BEZ_DEBUG_POLYGON=1`
  showed the extracted polygon still matched the raster well (ampersand IoU
  about 98.5%), so the major damage was in the custom curve fitting and split
  logic, not in image thresholding or contour extraction.
- Added `img2bez --global-fit` as the direct-trace default for Runebender.
  Focused autoresearch over `ampersand a e s R` with
  `--smooth 0 --accuracy 2 --global-fit` produced mean IoU 97.14%, with
  ampersand IoU 95.80%. This is the first meaningful local improvement for
  visually placing a recognizable outline into an empty glyph.
- Ampersand-specific follow-up: `--smooth 0 --accuracy 1 --global-fit`
  improved the ampersand preview to IoU 97.70%. The same setting regressed
  simpler glyphs in the focused set (`s` and `e` in particular), so treat it
  as the current direct editor trace setting for visual reproduction, not as a
  proven global optimum.
- `--global-fit` is intentionally visual-first. It produces weaker
  type-design point structure than the long-term goal, because it uses global
  fitted curves instead of the ideal mix of straight spans, extrema, and
  disciplined H/V handles. Keep it as the current direct `Trace Image`
  default, then continue improving `img2bez` by hybridizing this visual fit
  with better line classification and source-UFO-aware evaluation.

### 11. Weekend Done Criteria

Local-only focus status, 2026-06-06: local Rust tracing is functioning
against the restarted ComfyUI host with direct GLIF insertion into the
active glyph. Local model output is treated as a better placed image/mask
for the same `Trace Image` path. Comfy Cloud / QuiverAI remains
intentionally paused.

- [x] Local Rust tracing path returns a GLIF outline from a placed
  background image and inserts it into the active glyph.
- [x] Local model preprocessing can produce a mask/image that feeds the
  same local tracer path.
- [ ] QuiverAI / Comfy Cloud can produce SVG that imports into the same
  direct trace/import flow.
- [x] Advanced candidate scoring reports useful pass/fail evidence when
  using graph comparison workflows.
- [x] Review happens in Runebender before normal save; candidate promotion
  is no longer part of the default local workflow.
- [x] Tests and docs cover implemented provider paths without requiring live cloud
  access in CI.

Remaining live checks, 2026-06-06:

- Open any existing glyph and visually confirm contours connect the on-curve
  points after hard-refreshing ComfyUI to load
  `rb-bundle-2026-06-06-img2bez-global-fit-accuracy1`.
- Run the direct `Trace Image` action from the editor UI and visually
  review the inserted outline against the placed image after the `img2bez`
  reinstall and bundle reload.
- Improve `img2bez` curve segmentation/fitting beyond the first reinstall.
  The current local binary avoids the worst shortcut artifact in visual-first
  mode, but it still produces rough point structure on complex glyphs such as
  ampersands.
- Decide whether WIP direct traces into one master should suppress or soften
  interpolation errors until the matching master is traced.
- Comfy Cloud / QuiverAI is paused. Resume by setting
  `COMFY_CLOUD_API_KEY`, confirming budget, and running the manual or
  automated Quiver path.

## Stop Points

Stop and ask before:

- committing to a paid Comfy Cloud plan or API credential flow
- choosing a first local model family if more than one is viable
- accepting a Quiver SVG format that needs broad SVG support
- changing source save semantics or bypassing human review
- auto-restyling outlines based on references

## References

- Comfy Cloud docs: https://docs.comfy.org/get_started/cloud
- Quiver announcement:
  https://blog.comfy.org/p/quiver-structured-svg-generation
- ComfyUI custom node docs: https://docs.comfy.org/custom-nodes/overview
- VTracer: https://github.com/visioncortex/vtracer
- diffvg: https://github.com/BachiLi/diffvg
- DeepVecFont: https://github.com/yizhiwang96/deepvecfont
- DeepVecFont-v2: https://github.com/yizhiwang96/deepvecfont-v2
