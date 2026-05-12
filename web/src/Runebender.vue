<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
// wasm-pack output lives in ../wasm/ (a normal source directory, not
// /public/). Vite resolves this as a regular ES module; the shim's
// internal `new URL('..._bg.wasm', import.meta.url)` then resolves
// to a sibling URL that Vite serves automatically in dev and rewrites
// to a bundled asset in prod.
import init, { GlyphEditor } from "../wasm/runebender_comfy_core.js";

const canvas = ref<HTMLCanvasElement | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const status = ref<string>("initializing");
const selectionCount = ref<number>(0);
const glyphName = ref<string>("");
const dragHover = ref<boolean>(false);

type Editor = {
  pointerDown(x: number, y: number, button: number, mods: number): void;
  pointerMove(x: number, y: number, mods: number): void;
  pointerUp(x: number, y: number, button: number, mods: number): void;
  pointerCancel(): void;
  wheel(x: number, y: number, deltaY: number): void;
  undo(): void;
  redo(): void;
  render(): void;
  resize(w: number, h: number): void;
  setGlyphSvg(svg: string): void;
  setGlyphGlif(bytes: Uint8Array): void;
  fitToCanvas(w: number, h: number): void;
  setZoom(z: number): void;
  setOffset(x: number, y: number): void;
  selectionCount(): number;
  free(): void;
};

let editor: Editor | null = null;
let raf = 0;
let resizeObserver: ResizeObserver | null = null;

// Placeholder glyph — capital "I" in design space (Y-up). Real
// glyphs land here from the Python side once the ComfyUI integration
// is in place.
const PLACEHOLDER_SVG =
  "M -50 0 L 50 0 L 50 50 L 25 50 L 25 650 L 50 650 L 50 700 L -50 700 L -50 650 L -25 650 L -25 50 L -50 50 Z";

onMounted(async () => {
  if (!canvas.value) return;

  if (!("gpu" in navigator)) {
    status.value =
      "WebGPU is not available in this browser. Try Chrome 113+, Edge, or Safari Tech Preview.";
    return;
  }

  try {
    await init();

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.value.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width * dpr));
    const height = Math.max(1, Math.floor(rect.height * dpr));
    canvas.value.width = width;
    canvas.value.height = height;

    editor = (await GlyphEditor.new(canvas.value, width, height)) as unknown as Editor;

    editor.setGlyphSvg(PLACEHOLDER_SVG);
    editor.setZoom(0.5 * dpr);
    editor.setOffset(width / 2, height / 2 + 175 * dpr);

    status.value = "ready";
    requestRender();

    resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(canvas.value);
    window.addEventListener("keydown", onKeyDown);
  } catch (e) {
    console.error(e);
    status.value = `failed: ${e}`;
  }
});

function requestRender() {
  if (!editor) return;
  cancelAnimationFrame(raf);
  raf = requestAnimationFrame(() => {
    editor?.render();
    if (editor) selectionCount.value = editor.selectionCount();
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
  requestRender();
}

// Map a DOM PointerEvent to canvas-backing-store coords (the renderer
// works in physical pixels, not CSS pixels).
function canvasCoords(e: PointerEvent): [number, number] | null {
  if (!canvas.value) return null;
  const rect = canvas.value.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const x = (e.clientX - rect.left) * dpr;
  const y = (e.clientY - rect.top) * dpr;
  return [x, y];
}

function modBits(e: PointerEvent): number {
  return (
    (e.shiftKey ? 1 : 0) |
    (e.ctrlKey ? 2 : 0) |
    (e.altKey ? 4 : 0) |
    (e.metaKey ? 8 : 0)
  );
}

function onPointerDown(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  (e.target as Element).setPointerCapture?.(e.pointerId);
  editor.pointerDown(c[0], c[1], e.button, modBits(e));
  requestRender();
}

function onPointerMove(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  editor.pointerMove(c[0], c[1], modBits(e));
  requestRender();
}

function onPointerUp(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  editor.pointerUp(c[0], c[1], e.button, modBits(e));
  (e.target as Element).releasePointerCapture?.(e.pointerId);
  requestRender();
}

function onPointerCancel() {
  if (!editor) return;
  editor.pointerCancel();
  requestRender();
}

async function loadGlifFile(file: File) {
  if (!editor || !canvas.value) return;
  if (!/\.glif$/i.test(file.name)) {
    status.value = `not a .glif: ${file.name}`;
    return;
  }
  try {
    const buf = await file.arrayBuffer();
    editor.setGlyphGlif(new Uint8Array(buf));
    editor.fitToCanvas(canvas.value.width, canvas.value.height);
    glyphName.value = file.name.replace(/\.glif$/i, "");
    status.value = "ready";
    requestRender();
  } catch (e) {
    console.error(e);
    status.value = `failed to load: ${e}`;
  }
}

function onLoadButton() {
  fileInput.value?.click();
}

function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) loadGlifFile(file);
  // Reset so picking the same file again still fires `change`.
  input.value = "";
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  dragHover.value = true;
}

function onDragLeave() {
  dragHover.value = false;
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  dragHover.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (file) loadGlifFile(file);
}

function onWheel(e: WheelEvent) {
  if (!editor) return;
  e.preventDefault();
  const c = canvasCoords(e as unknown as PointerEvent);
  if (!c) return;
  // Normalize wheel deltas across deltaMode (pixels / lines / pages).
  // Lines ~= 16px, pages ~= a screen (~800px is fine as a rough scale).
  const lineFactor = 16;
  const pageFactor = 800;
  const dy =
    e.deltaMode === 1
      ? e.deltaY * lineFactor
      : e.deltaMode === 2
        ? e.deltaY * pageFactor
        : e.deltaY;
  editor.wheel(c[0], c[1], dy);
  requestRender();
}

function onKeyDown(e: KeyboardEvent) {
  if (!editor) return;
  // Ignore when the user is typing in another field.
  const target = e.target as HTMLElement | null;
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target?.isContentEditable
  ) {
    return;
  }
  const meta = e.metaKey || e.ctrlKey;
  if (meta && e.key.toLowerCase() === "z") {
    e.preventDefault();
    if (e.shiftKey) editor.redo();
    else editor.undo();
    requestRender();
  } else if (meta && e.key.toLowerCase() === "y") {
    // Windows-style redo.
    e.preventDefault();
    editor.redo();
    requestRender();
  }
}

onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  resizeObserver?.disconnect();
  window.removeEventListener("keydown", onKeyDown);
  editor?.free();
  editor = null;
});

function enterFullscreen() {
  canvas.value?.requestFullscreen?.();
}
</script>

<template>
  <div class="runebender-host">
    <canvas
      ref="canvas"
      class="runebender-canvas"
      :class="{ 'drag-hover': dragHover }"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerCancel"
      @wheel.prevent="onWheel"
      @contextmenu.prevent
      @dragover="onDragOver"
      @dragleave="onDragLeave"
      @drop="onDrop"
    />
    <div class="toolbar">
      <button class="btn" @click="onLoadButton">Load .glif</button>
      <button class="btn" @click="enterFullscreen">Full screen</button>
    </div>
    <input
      ref="fileInput"
      type="file"
      accept=".glif"
      class="file-input"
      @change="onFileChange"
    />
    <div v-if="status !== 'ready'" class="status">{{ status }}</div>
    <div v-else-if="glyphName" class="status">
      {{ glyphName }}{{ selectionCount > 0 ? ` — ${selectionCount} selected` : "" }}
    </div>
    <div v-else-if="selectionCount > 0" class="status">
      {{ selectionCount }} selected
    </div>
  </div>
</template>

<style scoped>
.runebender-host {
  position: relative;
  width: 100%;
  height: 100%;
  background: #1f1a14;
}
.runebender-canvas {
  width: 100%;
  height: 100%;
  display: block;
  cursor: crosshair;
  touch-action: none;
}
.toolbar {
  position: absolute;
  top: 8px;
  right: 8px;
  display: flex;
  gap: 6px;
}
.btn {
  padding: 4px 8px;
  background: #3a2f24;
  color: #f0e6d2;
  border: 1px solid #5a4a38;
  border-radius: 4px;
  cursor: pointer;
  font: 11px ui-sans-serif, system-ui, sans-serif;
}
.btn:hover {
  background: #4a3d2e;
}
.file-input {
  display: none;
}
.runebender-canvas.drag-hover {
  outline: 2px dashed #ffa640;
  outline-offset: -2px;
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
