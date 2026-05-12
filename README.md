# runebender-comfy

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
   ln -s ~/GH/repos/runebender-comfy \
         ~/Work/comfy/repos/ComfyUI/custom_nodes/runebender-comfy
   ```

2. Install web deps and build the WASM core + the Vue widget:
   ```bash
   cd web
   pnpm install
   pnpm wasm          # cargo + wasm-pack build into ./wasm/
   pnpm build         # vite build into ./dist/ (consumed by ComfyUI)
   ```

3. Restart ComfyUI.

## Dev preview (without ComfyUI)

```bash
cd web
pnpm dev
```

Opens at `http://localhost:5173/` — the editor runs standalone in
Chrome/Edge/Safari (needs WebGPU).

## Supply-chain checks

See [SECURITY.md](SECURITY.md) for the npm and cargo audit setup.
Run all checks at once:

```bash
./scripts/audit.sh
```

## License

GPL-3.0. Sources mined from `runebender-xilem` (Apache-2.0) and
`designbot` — both GPL-3.0 compatible inbound. The pnpm
`minimum-release-age` cooldown plus `cargo-deny` + `check-crate-age`
on the Rust side cover supply-chain hygiene.
