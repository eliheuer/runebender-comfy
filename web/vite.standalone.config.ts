import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Standalone static-site build of the Runebender editor.
//
// Unlike vite.config.ts (which emits a ComfyUI custom-node *library*
// bundle), this builds the regular HTML app from index.html + main.ts:
// the Vue app mounts the Runebender widget with the browser host and
// auto-loads the bundled VirtuaGrotesk demo font. The output is a
// self-contained SPA suitable for hosting under a subpath.
//
// `base` is the subpath it will be served from. eliheuer.com is a
// custom-domain GitHub Pages site rooted at /, so dropping this build's
// output into that site's `public/runebender-web/` serves it at
// https://eliheuer.com/runebender-web/.
//
// The wasm-pack `--target web` shim loads its binary via
//   new URL('runebender_comfy_core_bg.wasm', import.meta.url)
// which Vite resolves natively in app mode: it emits the wasm to
// assets/ and rewrites the URL with `base` applied. No custom plugin
// (the one in vite.config.ts is lib-mode specific) is needed here.

export default defineConfig({
  base: "/runebender-web/",
  plugins: [vue()],
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
  build: {
    outDir: "dist-site",
    emptyOutDir: true,
    target: "esnext",
  },
});
