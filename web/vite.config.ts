import { defineConfig, type Plugin, type ResolvedConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { copyFileSync, mkdirSync } from "fs";
import { resolve } from "path";

// ComfyUI loads custom-node frontends from WEB_DIRECTORY (../dist).
// We emit a single ES module (runebender-comfy.js) plus the wasm binary
// as a separate file in dist/assets/ so the browser can use
// WebAssembly.instantiateStreaming — compile while downloading, not
// decode-then-compile from a base64 blob.
//
// Vite lib mode inlines any wasm it sees (via new URL(...) or ?url).
// wasmStreamingPlugin avoids this with two hooks:
//
//  transform (build only, enforce:"pre"):
//    Rewrites the wasm-pack shim's built-in URL lookup from
//      new URL('...bg.wasm', import.meta.url)
//    to
//      new URL(/*@vite-ignore*/ './assets/runebender_web_bg.wasm',
//               import.meta.url)
//    The @vite-ignore comment prevents Vite from seeing a wasm reference
//    here and trying to inline it.  At runtime the URL resolves relative
//    to the bundle (which is in the same dist/ directory as assets/).
//
//  writeBundle:
//    Copies wasm/runebender_web_bg.wasm → dist/assets/ with a
//    stable filename so the URL above stays correct across rebuilds.
//    ComfyUI cache-busts at the node version level, not the file level.
//
// `/scripts/app.js` is provided by ComfyUI's host page; mark external.

function wasmStreamingPlugin(): Plugin {
  let isBuildMode = false;
  let outDir = "dist";

  return {
    name: "wasm-streaming",
    enforce: "pre",

    configResolved(config: ResolvedConfig) {
      isBuildMode = config.command === "build";
      outDir = config.build.outDir ?? "dist";
    },

    transform(code: string, id: string) {
      if (!isBuildMode) return;
      if (!id.includes("runebender_web")) return;
      // Rewrite the shim's default URL to the stable assets/ path and
      // suppress Vite's asset-inlining pass with @vite-ignore.
      return code.replace(
        /new URL\(['"]runebender_web_bg\.wasm['"],\s*import\.meta\.url\)/g,
        "new URL(/*@vite-ignore*/ './assets/runebender_web_bg.wasm', import.meta.url)",
      );
    },

    writeBundle() {
      const assetsDir = resolve(process.cwd(), outDir, "assets");
      mkdirSync(assetsDir, { recursive: true });
      copyFileSync(
        resolve(process.cwd(), "node_modules/runebender-web/wasm/runebender_web_bg.wasm"),
        resolve(assetsDir, "runebender_web_bg.wasm"),
      );
    },
  };
}

export default defineConfig({
  plugins: [vue(), wasmStreamingPlugin()],
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
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
