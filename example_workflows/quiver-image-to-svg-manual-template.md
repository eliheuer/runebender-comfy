# Quiver Image-to-SVG Template

Use this as the Comfy Cloud / Partner Nodes side of the Runebender
glyph-tracing workflow.

## Comfy Cloud Graph

```text
Load Image
  -> Quiver Image-to-SVG
  -> Save / Export SVG
```

## Manual Steps

1. In Runebender, create a `GLYPH_TRACE_REQUEST` for the target glyph.
2. Upload the request image from the trace request directory to Comfy
   Cloud.
3. Search the node or template library for `Quiver`.
4. Choose the Image-to-SVG workflow/template.
5. Connect the uploaded image to Quiver Image-to-SVG.
6. Run the workflow.
7. Export the generated SVG locally.
8. In local ComfyUI, run `Trace With QuiverAI` with:
   - `font`: the original Runebender `FONT`
   - `trace_request`: the `GLYPH_TRACE_REQUEST` path
   - `svg_path`: the exported SVG path
9. Open the candidate `FONT` in Runebender and review the orange-marked
   glyph.
10. Promote with `Apply Glyph Candidates` only after review.

## Automated Node Path

Use `Trace With Comfy Cloud QuiverAI` when you have a paid Comfy Cloud
API key and a saved Quiver workflow in API JSON format.

Inputs:

- `font`: original Runebender `FONT`
- `trace_request`: the `GLYPH_TRACE_REQUEST` path
- `workflow_api_json`: inline API-format JSON or a path to a JSON file
- `image_node_id`: node id for the workflow's `Load Image`
- `image_input_name`: usually `image`
- `svg_output_node_id`: optional output node to search first
- `api_key`: optional; otherwise use `COMFY_CLOUD_API_KEY`

Behavior:

1. Uploads the trace request image to `POST /api/upload/image`.
2. Injects the uploaded filename into the configured image input.
3. Submits the workflow to `POST /api/prompt` with `X-API-Key`.
4. Adds `extra_data.api_key_comfy_org` so Partner Nodes can run.
5. Polls job status, reads history, downloads the SVG output, and saves it
   next to the trace request as `comfy-cloud-quiver.svg`.
6. Routes that SVG through the same strict importer as `Trace With
   QuiverAI`.

Do not run this path until credentials and Cloud budget are approved.

## Import Contract

`Trace With QuiverAI` currently accepts only simple filled SVG paths. It
rejects strokes, masks, filters, gradients, text, embedded rasters, and
unsupported transforms. If Quiver returns one of those constructs, clean
or simplify the SVG before importing it.
