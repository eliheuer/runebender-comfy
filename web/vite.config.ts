import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// ComfyUI loads custom-node frontends from WEB_DIRECTORY (../dist).
// We emit a single ES module that registers the Runebender widget,
// plus a sibling .wasm asset that Rollup emits from the wasm-pack
// shim's `new URL(...)` reference.
//
// `/scripts/app.js` is provided by ComfyUI's host page; mark external.
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
      external: [/^\/scripts\//],
    },
  },
});
