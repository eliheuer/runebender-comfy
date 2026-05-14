# rust-core

WASM core for the Runebender ComfyUI node. Vello (renderer) + Kurbo
(geometry). No Xilem — UI is hosted by Vue in `../web/`.

## Build

```bash
wasm-pack build --target web --out-dir ../web/wasm --dev
```

The output directory is deliberately `../web/wasm/`, not
`../web/public/wasm/`. Vite treats `public/` as a static shadow tree;
if `web/public/wasm/` exists it can mask the current wasm-pack output
and leave the dev server serving stale bindings. Use `pnpm wasm` from
`../web/` for the normal dev loop.
