# rust-core

WASM core for the Runebender ComfyUI node. Vello (renderer) + Kurbo
(geometry). No Xilem — UI is hosted by Vue in `../web/`.

## Build

```bash
wasm-pack build --target web --out-dir ../web/public/wasm
```
