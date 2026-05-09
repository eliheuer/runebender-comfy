# web

Vue 3 frontend for the Runebender ComfyUI widget. Builds a single ES
module that ComfyUI loads via `WEB_DIRECTORY` (`./dist/`).

## Dev

```bash
pnpm install
pnpm build
```

The build emits `dist/runebender-comfy.js`, which ComfyUI auto-loads
when the custom node is present.
