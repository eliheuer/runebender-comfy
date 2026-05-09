# runebender-comfy-nodes

ComfyUI custom nodes that bring Rust-powered type-design tools into the
graph: a full-screen glyph editor ported from
[`runebender-xilem`](https://github.com/eliheuer/runebender-xilem), and a
DrawBot-style scripting node backed by
[`designbot`](https://github.com/eliheuer/designbot).

## Status

Scaffolding — node skeletons in place, Rust/WASM core and Vue widget
wired up but not yet implemented.

## Nodes

- **Runebender** — full-screen glyph editor widget (Vello + Kurbo via
  WASM, Vue host). Outputs UFO/SVG/contour data.
- **DesignBot** — Rust DrawBot/Processing-style 2D graphics node.
  Outputs `IMAGE`.

## Architecture

```
┌──────────────────────────────────────────┐
│ ComfyUI graph (Python)                   │
│   nodes/runebender.py   nodes/designbot.py
└─────────────┬────────────────────────────┘
              │ custom widget registration
┌─────────────▼────────────────────────────┐
│ ComfyUI frontend (Vue 3)                 │
│   web/src/Runebender.vue                 │
└─────────────┬────────────────────────────┘
              │ wasm-bindgen
┌─────────────▼────────────────────────────┐
│ rust-core (Vello + Kurbo, no Xilem)      │
└──────────────────────────────────────────┘
```

The Rust core deliberately skips Xilem — for an embedded canvas inside
an existing JS host (ComfyUI's Vue frontend), Vello + Kurbo via
`wasm-bindgen` is the right layer.

## Install (dev)

1. Clone into ComfyUI's `custom_nodes/`:
   ```bash
   ln -s ~/GH/repos/runebender-comfy-nodes \
         ~/Work/comfy/repos/ComfyUI/custom_nodes/runebender-comfy-nodes
   ```

2. Build the WASM core:
   ```bash
   cd rust-core
   wasm-pack build --target web --out-dir ../web/public/wasm
   ```

3. Build the Vue widget:
   ```bash
   cd web
   pnpm install && pnpm build
   ```

4. Restart ComfyUI.

## License

GPL-3.0. Sources mined from `runebender-xilem` (Apache-2.0) and
`designbot` (see its repo) — both GPL-3.0 compatible.
