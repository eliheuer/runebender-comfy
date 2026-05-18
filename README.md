# runebender-comfy

ComfyUI custom nodes that bring Rust-powered type-design tools into the
graph: a full-screen glyph editor ported from
[`runebender-xilem`](https://github.com/eliheuer/runebender-xilem), a
workspace-backed `FONT` wire, and a specimen renderer that consumes that
wire.

## Status

Workspace-backed `FONT` wires and the editor widget are wired up. The
full type-design toolchain is still in progress.

## Nodes

- **Runebender** — full-screen glyph editor widget (Vello + Kurbo via
  WASM, Vue host). Passes a `FONT` workspace reference through the
  graph.
- **Load Font** — creates a `FONT` workspace from a source font path.
  UFO/designspace is auto-detected by default; Glyphs and glyphspackage
  are supported as alternate import/source kinds. The node also has an
  import button for selecting a local UFO/designspace bundle from disk.
  When `glyphsLib` is installed, the import path normalizes Glyphs into
  UFO/designspace inside the workspace.
- **Specimen** — DrawBot-style specimen renderer with `IMAGE` + `MASK`
  outputs for scripted type specimens and previews.
- **Compile Font** — materializes a compiled artifact for a workspace
  when a backend exists. The workspace auto-materializes a
  `glyphspackage` source package and then runs the Google Fonts
  oriented `fontc` compiler against it. The compiled artifact is
  recorded in the workspace manifest.
- **Font Preview** — renders a simple specimen from a `FONT`
  reference.
- **Fork Font** — duplicates a `FONT` workspace for parallel work.
- **DesignBot** — Rust DrawBot/Processing-style 2D graphics node.
  Invokes the optional `designbot` CLI and outputs `IMAGE`.

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

## Install And Use

### Comfy Cloud

Comfy Cloud runs ComfyUI in the browser with no local install. The
service ships with preinstalled nodes and models, and it follows the
same workflow structure as local ComfyUI.

1. Open Comfy Cloud from the official site and load a workflow.
2. If your cloud workspace exposes custom-node installation, add this
   repository there the same way you would install any other custom
   node: clone it into `ComfyUI/custom_nodes/` inside the cloud
   environment, then install the Python dependencies in that same
   environment.
3. Restart the cloud workspace after installation.
4. Load the `Load Font` node, point it at a font source path available
   to that workspace, then connect it to `Runebender`, `Compile Font`,
   and `Font Preview`.
5. Use `Fork Font` when you want to branch a workspace without
   overwriting the original.

If your Comfy Cloud workspace does not expose a way to add custom
nodes, use the local install path below.

### Local ComfyUI

1. Clone or symlink this repository into ComfyUI's `custom_nodes/`
   directory:
   ```bash
   ln -s ~/GH/repos/runebender-comfy \
         ~/Work/comfy/repos/ComfyUI/custom_nodes/runebender-comfy
   ```
2. Install the web deps and build the WASM core plus the Vue widget:
   ```bash
   cd web
   pnpm install
   pnpm wasm          # cargo + wasm-pack build into ./wasm/
   pnpm build         # vite build into ./dist/ (consumed by ComfyUI)
   ```
3. Run the setup smoke tests from the repo root:
   ```bash
   python3 -m unittest tests.test_workspace tests.test_web_bundle
   ```
4. Restart ComfyUI.
5. Open the Runebender node in the ComfyUI graph and use the starter
   workflow below as the simplest path into font work.

For a first smoke test, use the checked-in demo source at
`samples/virtua-grotesk/VirtuaGrotesk.designspace` (Regular + Bold
masters; OFL-licensed, see `samples/virtua-grotesk/OFL.txt`).

The nodes are grouped under the `Runebender` section in the ComfyUI
add-node menu:

- `Runebender / Editor`
- `Runebender / Font`
- `Runebender / Graphics`

For a concrete starter graph, see
[docs/workflows/local-font-workflow.md](docs/workflows/local-font-workflow.md).

To use the Google Fonts compile path, install `fontc` and put it on
`PATH`. The workspace will materialize a `glyphspackage` source package
automatically from the workspace's editable source when needed.

To import Glyphs files into the default UFO/designspace workspace
shape, install `glyphsLib` in the ComfyUI Python environment.

The `Load Font` node includes an import button that copies a local
UFO/designspace bundle into the workspace and binds the resulting
workspace reference into the graph.
The same node also shows a small inline specimen preview and a dropdown
of existing workspace slots.

To render DesignBot scripts, install the Rust CLI and keep it on
`PATH`, or set `DESIGNBOT_BIN` to the executable path:

```bash
cargo install --git https://github.com/eliheuer/designbot designbot-cli
```

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
