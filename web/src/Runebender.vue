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
const dragHover = ref<boolean>(false);
const glyphNames = ref<string[]>([]);
const currentGlyph = ref<string>("");
const fontLabel = ref<string>("");

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
// Holds raw bytes for every loaded glyph, keyed by the glyph's UFO
// name (NOT the mangled filename). Set when a UFO is loaded; consulted
// when the dropdown changes.
const glyphBytes = new Map<string, Uint8Array>();

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

// ---------------------------------------------------------------------
// UFO loading
// ---------------------------------------------------------------------

/// Extract `<glyph name="...">` from a .glif XML buffer without
/// pulling in DOMParser overhead. The attribute is always near the
/// top of the file.
function parseGlyphName(bytes: Uint8Array): string | null {
  const head = new TextDecoder().decode(bytes.slice(0, 512));
  const match = /<glyph\s+name="([^"]+)"/.exec(head);
  return match?.[1] ?? null;
}

/// Pick a sensible glyph to land on after loading a UFO. Try "A"
/// first (common test glyph), else "a", else .notdef, else the first
/// glyph alphabetically.
function pickInitialGlyph(names: string[]): string | null {
  if (names.length === 0) return null;
  for (const preferred of ["A", "a", ".notdef"]) {
    if (names.includes(preferred)) return preferred;
  }
  return names[0];
}

async function loadGlifFiles(files: File[]) {
  if (!editor || !canvas.value) return;

  // Surface a friendlier message for designspace until Wave 3f.
  if (files.some((f) => /\.designspace$/i.test(f.name))) {
    status.value =
      "designspace files aren't supported yet — open one of the referenced .ufo folders for now";
    // Continue: if there are also .glif files in the drop, those still load.
  }

  // From a UFO directory, prefer the main `glyphs/` layer. Falls
  // back to "any .glif" so a single-file drop still works.
  let glifs = files.filter(
    (f) => /\.glif$/i.test(f.name) && /\/glyphs\//.test(relPath(f)),
  );
  if (glifs.length === 0) {
    glifs = files.filter((f) => /\.glif$/i.test(f.name));
  }

  if (glifs.length === 0) {
    status.value =
      files.length === 0
        ? "nothing dropped"
        : `no .glif files found in ${files.length} file(s) — make sure you're picking a .ufo folder, not the .designspace`;
    return;
  }

  glyphBytes.clear();
  glyphNames.value = [];
  status.value = `loading ${glifs.length} glyph${glifs.length === 1 ? "" : "s"}…`;

  // Read all glyphs in parallel. For UFOs with thousands of glyphs
  // this is bounded by browser file-read throughput, not CPU.
  const loaded = await Promise.all(
    glifs.map(async (f) => {
      const bytes = new Uint8Array(await f.arrayBuffer());
      return { name: parseGlyphName(bytes), bytes };
    }),
  );
  for (const { name, bytes } of loaded) {
    if (name) glyphBytes.set(name, bytes);
  }
  const names = Array.from(glyphBytes.keys()).sort();
  glyphNames.value = names;

  // Label the UFO by the folder name (best-effort) — match any
  // ".ufo" segment in the path, regardless of how deeply nested.
  const sample = relPath(glifs[0]);
  const ufoMatch = sample.match(/([^/]+\.ufo)\//i);
  fontLabel.value = ufoMatch ? ufoMatch[1] : "";

  const initial = pickInitialGlyph(names);
  if (initial) selectGlyph(initial);
  status.value = "ready";
}

function selectGlyph(name: string) {
  if (!editor || !canvas.value) return;
  const bytes = glyphBytes.get(name);
  if (!bytes) return;
  try {
    editor.setGlyphGlif(bytes);
    editor.fitToCanvas(canvas.value.width, canvas.value.height);
    currentGlyph.value = name;
    requestRender();
  } catch (e) {
    console.error(e);
    status.value = `failed to load ${name}: ${e}`;
  }
}

function onGlyphSelect(e: Event) {
  const name = (e.target as HTMLSelectElement).value;
  if (name) selectGlyph(name);
}

// macOS treats `.ufo` as a package bundle when any font tool is
// installed (FontLab, Glyphs, …), and the standard file-picker panel
// then refuses to let you enter it. The File System Access API uses
// a different panel that handles bundles correctly. We try it first
// and fall back to the <input webkitdirectory> route only if it isn't
// available or the user denies access.
async function onLoadButton() {
  type WithPicker = Window & {
    showDirectoryPicker?: (opts: {
      mode?: "read" | "readwrite";
    }) => Promise<FileSystemDirectoryHandle>;
  };
  const win = window as WithPicker;
  if (typeof win.showDirectoryPicker === "function") {
    try {
      const handle = await win.showDirectoryPicker({ mode: "read" });
      const files = await filesFromDirectoryHandle(handle, handle.name);
      await loadGlifFiles(files);
      return;
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError") return; // user cancelled
      // Fall through to the legacy picker on any other error.
      console.warn("showDirectoryPicker failed, falling back:", err);
    }
  }
  fileInput.value?.click();
}

/// Recursively flatten a FileSystemDirectoryHandle into File objects
/// with synthetic webkitRelativePath set, so the existing UFO filter
/// logic sees a uniform shape across all entry points.
async function filesFromDirectoryHandle(
  handle: FileSystemDirectoryHandle,
  prefix: string,
): Promise<File[]> {
  const out: File[] = [];
  const dirHandle = handle as FileSystemDirectoryHandle & {
    entries: () => AsyncIterable<[string, FileSystemHandle]>;
  };
  for await (const [name, entry] of dirHandle.entries()) {
    const path = `${prefix}/${name}`;
    if (entry.kind === "file") {
      const file = await (entry as FileSystemFileHandle).getFile();
      try {
        Object.defineProperty(file, "webkitRelativePath", {
          value: path,
          configurable: true,
        });
      } catch {
        // Some browsers disallow this; the "any .glif" fallback still
        // catches the file, just without the `/glyphs/` filter.
      }
      out.push(file);
    } else {
      out.push(
        ...(await filesFromDirectoryHandle(entry as FileSystemDirectoryHandle, path)),
      );
    }
  }
  return out;
}

function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const files = Array.from(input.files ?? []);
  loadGlifFiles(files);
  input.value = "";
}

// ---------------------------------------------------------------------
// Drag-drop: handle both single .glif files and full UFO folders.
// ---------------------------------------------------------------------

function relPath(f: File): string {
  // webkitdirectory sets webkitRelativePath; for drag-drop we attach
  // it ourselves via Object.defineProperty in `readEntry`.
  return (f as File & { webkitRelativePath?: string }).webkitRelativePath ?? f.name;
}

type FsEntry = {
  isFile: boolean;
  isDirectory: boolean;
  fullPath: string;
  file?: (cb: (f: File) => void, err?: (e: unknown) => void) => void;
  createReader?: () => FsDirReader;
};

type FsDirReader = {
  readEntries: (cb: (entries: FsEntry[]) => void, err?: (e: unknown) => void) => void;
};

async function readEntry(entry: FsEntry): Promise<File[]> {
  if (entry.isFile && entry.file) {
    return new Promise((resolve, reject) =>
      entry.file!(
        (f) => {
          // Attach the entry's fullPath as a synthetic relative path
          // so the UFO `glyphs/` filter works for drag-drop too.
          try {
            Object.defineProperty(f, "webkitRelativePath", {
              value: entry.fullPath.replace(/^\//, ""),
              configurable: true,
            });
          } catch {
            // Some browsers don't allow redefining File props; the
            // fallback filter ("any .glif") will still catch it.
          }
          resolve([f]);
        },
        (err) => reject(err),
      ),
    );
  }
  if (entry.isDirectory && entry.createReader) {
    const reader = entry.createReader();
    // readEntries returns up to ~100 entries per call; loop until empty.
    const all: FsEntry[] = [];
    while (true) {
      const batch: FsEntry[] = await new Promise((resolve, reject) =>
        reader.readEntries(resolve, (e) => reject(e)),
      );
      if (batch.length === 0) break;
      all.push(...batch);
    }
    const results = await Promise.all(all.map(readEntry));
    return results.flat();
  }
  return [];
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  dragHover.value = true;
}

function onDragLeave() {
  dragHover.value = false;
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  dragHover.value = false;
  const items = e.dataTransfer?.items;
  if (!items) return;

  const collected: File[] = [];
  const itemsCopy = Array.from(items); // items is live; snapshot first
  for (const item of itemsCopy) {
    const entry = (
      item as DataTransferItem & {
        webkitGetAsEntry?: () => FsEntry | null;
      }
    ).webkitGetAsEntry?.();
    if (entry) {
      collected.push(...(await readEntry(entry)));
    } else {
      const f = item.getAsFile();
      if (f) collected.push(f);
    }
  }
  if (collected.length > 0) loadGlifFiles(collected);
}

// ---------------------------------------------------------------------
// Wheel + keyboard
// ---------------------------------------------------------------------

function onWheel(e: WheelEvent) {
  if (!editor) return;
  e.preventDefault();
  const c = canvasCoords(e as unknown as PointerEvent);
  if (!c) return;
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
  const target = e.target as HTMLElement | null;
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
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
      <button class="btn" @click="onLoadButton">Open UFO…</button>
      <select
        v-if="glyphNames.length > 0"
        class="btn glyph-select"
        :value="currentGlyph"
        @change="onGlyphSelect"
      >
        <option v-for="name in glyphNames" :key="name" :value="name">
          {{ name }}
        </option>
      </select>
      <button class="btn" @click="enterFullscreen">Full screen</button>
    </div>
    <input
      ref="fileInput"
      type="file"
      class="file-input"
      webkitdirectory
      multiple
      @change="onFileChange"
    />
    <div v-if="status !== 'ready'" class="status">{{ status }}</div>
    <div v-else-if="currentGlyph || fontLabel" class="status">
      <span v-if="fontLabel">{{ fontLabel }} · </span>
      <span v-if="currentGlyph">{{ currentGlyph }}</span>
      <span v-if="selectionCount > 0"> · {{ selectionCount }} selected</span>
    </div>
    <div v-else-if="selectionCount > 0" class="status">
      {{ selectionCount }} selected
    </div>
    <div
      v-if="status === 'ready' && glyphNames.length === 0"
      class="drop-hint"
    >
      <div class="drop-hint-title">Drop a .ufo folder here</div>
      <div class="drop-hint-sub">
        or click <strong>Open UFO…</strong> in the toolbar
      </div>
      <div class="drop-hint-note">
        On macOS, if the picker shows .ufo as a file you can't enter,
        drag-drop the folder directly onto this canvas — it'll work.
      </div>
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
  align-items: center;
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
.glyph-select {
  min-width: 90px;
  max-width: 200px;
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
.drop-hint {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  pointer-events: none;
  text-align: center;
  padding: 24px;
  font: 13px ui-sans-serif, system-ui, sans-serif;
  color: #c8ae88;
}
.drop-hint-title {
  font-size: 18px;
  color: #f0e6d2;
  letter-spacing: 0.02em;
}
.drop-hint-sub {
  color: #a89476;
}
.drop-hint-note {
  margin-top: 16px;
  max-width: 480px;
  color: #8a6f52;
  line-height: 1.5;
  font-size: 12px;
}
</style>
