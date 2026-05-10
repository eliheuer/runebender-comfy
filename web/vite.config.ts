import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// ComfyUI loads custom-node frontends from WEB_DIRECTORY (../dist).
// We emit a single ES module that registers the Runebender widget.
//
// The wasm-pack output lives under public/wasm/ and is served at
// /wasm/... at runtime; we mark that prefix external so Rollup
// doesn't try to bundle it. ComfyUI's app singleton is provided
// by the host page at /scripts/app.js, also marked external.
export default defineConfig({
  plugins: [vue()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    lib: {
      entry: "src/extension.ts",
      formats: ["es"],
      fileName: () => "runebender-comfy.js",
    },
    rollupOptions: {
      external: [/^\/scripts\//, /^\/wasm\//],
    },
  },
});
