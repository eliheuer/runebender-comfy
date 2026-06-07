# Local Model Mask-to-Trace Template

Use this as the local model side of the Runebender glyph-tracing
workflow. The first supported contract is provider-neutral: a local model
or preprocessing graph outputs a clean black-on-white mask, then
Runebender traces that mask with the local Rust tracing provider.

## Local ComfyUI Graph

```text
Load Image / Runebender trace request image
  -> local image cleanup, segmentation, or img2img model
  -> mask cleanup / threshold
  -> Save Image
  -> Trace Local Mask To Candidate
```

## Steps

1. In Runebender, create a `GLYPH_TRACE_REQUEST` for the target glyph.
2. Use the request image as the input to a local model workflow.
3. Produce a clean black glyph silhouette on a white or transparent
   background.
4. Save the mask locally.
5. Run `Trace Local Mask To Candidate` with:
   - `font`: the original Runebender `FONT`
   - `trace_request`: the `GLYPH_TRACE_REQUEST` path
   - `mask_path`: the generated mask path
6. Open the candidate `FONT` in Runebender and review the orange-marked
   glyph.
7. Promote with `Apply Glyph Candidates` only after review.

## First Model Choice

Do not hard-code one model family into Runebender. For this weekend, the
first local-model handoff is any local image/mask workflow that produces
a clean silhouette file. SDXL, FLUX, or segmentation-specific tools can
all attach to the same `mask_path` contract later.

## Test Contract

Tests cover the mask handoff into tracing, not the model runtime. CI must
not require local model weights, GPU access, or Comfy Cloud access.
