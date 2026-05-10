<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

const canvas = ref<HTMLCanvasElement | null>(null);
const status = ref<string>("initializing");

let editor: { render: () => void; resize: (w: number, h: number) => void; free: () => void } | null = null;
let raf = 0;
let resizeObserver: ResizeObserver | null = null;

// Placeholder glyph — capital "I" centered around the origin in
// design space (Y-up). Real glyphs land here from the Python side
// once the ComfyUI integration is in place.
const PLACEHOLDER_SVG =
  "M -50 0 L 50 0 L 50 50 L 25 50 L 25 650 L 50 650 L 50 700 L -50 700 L -50 650 L -25 650 L -25 50 L -50 50 Z";

onMounted(async () => {
  if (!canvas.value) return;

  if (!("gpu" in navigator)) {
    status.value = "WebGPU is not available in this browser. Try Chrome 113+, Edge, or Safari Tech Preview.";
    return;
  }

  try {
    const mod = await import("/wasm/runebender_comfy_core.js");
    await mod.default();

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.value.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width * dpr));
    const height = Math.max(1, Math.floor(rect.height * dpr));
    canvas.value.width = width;
    canvas.value.height = height;

    editor = await mod.GlyphEditor.new(canvas.value, width, height);

    // Center the glyph in the canvas at 0.5x design-units-per-pixel.
    editor!.setGlyphSvg(PLACEHOLDER_SVG);
    editor!.setZoom(0.5 * dpr);
    editor!.setOffset(width / 2, height / 2 + 175 * dpr);

    status.value = "ready";
    scheduleRender();

    resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(canvas.value);
  } catch (e) {
    console.error(e);
    status.value = `failed: ${e}`;
  }
});

function scheduleRender() {
  if (!editor) return;
  raf = requestAnimationFrame(() => {
    editor?.render();
  });
}

function handleResize() {
  if (!editor || !canvas.value) return;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.value.getBoundingClientRect();
  const width = Math.max(1, Math.floor(rect.width * dpr));
  const height = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.value.width === width && canvas.value.height === height) return;
  canvas.value.width = width;
  canvas.value.height = height;
  editor.resize(width, height);
  scheduleRender();
}

onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  resizeObserver?.disconnect();
  editor?.free();
  editor = null;
});

function enterFullscreen() {
  canvas.value?.requestFullscreen?.();
}
</script>

<template>
  <div class="runebender-host">
    <canvas ref="canvas" class="runebender-canvas" />
    <button class="fs-btn" @click="enterFullscreen">Full screen</button>
    <div v-if="status !== 'ready'" class="status">{{ status }}</div>
  </div>
</template>

<style scoped>
.runebender-host {
  position: relative;
  width: 100%;
  height: 100%;
  background: #1f1a14; /* warm dark — campfire palette */
}
.runebender-canvas {
  width: 100%;
  height: 100%;
  display: block;
}
.fs-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 4px 8px;
  background: #3a2f24;
  color: #f0e6d2;
  border: 1px solid #5a4a38;
  border-radius: 4px;
  cursor: pointer;
}
.status {
  position: absolute;
  bottom: 8px;
  left: 8px;
  padding: 4px 8px;
  font: 11px ui-monospace, monospace;
  color: #f0e6d2;
  background: rgba(58, 47, 36, 0.85);
  border-radius: 4px;
  pointer-events: none;
}
</style>
