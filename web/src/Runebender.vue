<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
// wasm-pack output lives in ../wasm/ (a normal source directory, not
// /public/). Vite resolves this as a regular ES module; the shim's
// internal `new URL('..._bg.wasm', import.meta.url)` then resolves
// to a sibling URL that Vite serves automatically in dev and rewrites
// to a bundled asset in prod.
import init, {
  GlyphEditor,
  glifToSvg,
  glyphCategoryForCodepoint,
} from "../wasm/runebender_comfy_core.js";
import CategorySidebar, {
  type Category,
} from "./components/CategorySidebar.vue";
import GlyphCell from "./components/GlyphCell.vue";
import GlyphInfoSidebar from "./components/GlyphInfoSidebar.vue";
import MarkColorPanel from "./components/MarkColorPanel.vue";
import TopBar from "./components/TopBar.vue";

const canvas = ref<HTMLCanvasElement | null>(null);
const status = ref<string>("initializing");
const selectionCount = ref<number>(0);
const dragHover = ref<boolean>(false);
const glyphNames = ref<string[]>([]);
const currentGlyph = ref<string>("");
const fontLabel = ref<string>("");
const viewMode = ref<"grid" | "editor">("grid");
// Pre-computed SVG strings for the grid cells, keyed by glyph name.
// Set once per UFO load; consulted only by the template.
const glyphSvgs = ref<Map<string, string>>(new Map());
// Unicode codepoint per glyph (uppercase hex, no "U+" prefix). Same
// lifecycle as glyphSvgs.
const glyphUnicodes = ref<Map<string, string>>(new Map());
// Master switcher labels for the TopBar. Stub for now — when
// designspace lands (Phase 7) this becomes the list of master
// names parsed from the .designspace file.
const masters = ref<string[]>(["Regular"]);
// Category filter for the sidebar. "All" until the user picks.
const selectedCategory = ref<Category>("All");
// Live metadata for the info sidebar — read from the wasm editor
// after each setGlyphGlif call. -1 / 0 means "no glyph loaded yet".
const currentWidth = ref<number>(-1);
const currentContours = ref<number>(0);
// Glyph that's selected (highlighted in the grid, shown in the info
// sidebar, mark-color target). Distinct from `currentGlyph` which
// is the glyph currently loaded into the editor.
const selectedGlyph = ref<string>("");
// `public.markColor` value per glyph as RGBA string "r,g,b,a"
// with 0–1 floats. Populated on UFO load from each .glif's <lib>.
// Mutated when the user clicks a swatch.
const glyphMarkColors = ref<Map<string, string>>(new Map());
// Category per glyph name, computed in loadGlifFiles via the wasm
// `glyphCategoryForCodepoint`. Glyphs without a codepoint default
// to "Other".
const glyphCategories = ref<Map<string, Category>>(new Map());

// Names filtered by the active category. The grid renders this list
// instead of glyphNames directly.
const filteredGlyphNames = computed(() => {
  if (selectedCategory.value === "All") return glyphNames.value;
  return glyphNames.value.filter(
    (n) => (glyphCategories.value.get(n) ?? "Other") === selectedCategory.value,
  );
});

// Counts of glyphs per category for the sidebar's right-aligned
// indicator. "All" is the total.
const categoryCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {
    All: glyphNames.value.length,
    Letter: 0,
    Number: 0,
    Punctuation: 0,
    Symbol: 0,
    Mark: 0,
    Separator: 0,
    Other: 0,
  };
  for (const n of glyphNames.value) {
    const c = glyphCategories.value.get(n) ?? "Other";
    counts[c] = (counts[c] ?? 0) + 1;
  }
  return counts;
});

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
  setFontInfo(bytes: Uint8Array): void;
  fitToCanvas(w: number, h: number): void;
  setZoom(z: number): void;
  setOffset(x: number, y: number): void;
  selectionCount(): number;
  advanceWidth(): number;
  contourCount(): number;
  free(): void;
};

let editor: Editor | null = null;
let raf = 0;
let resizeObserver: ResizeObserver | null = null;
// Holds raw bytes for every loaded glyph, keyed by the glyph's UFO
// name (NOT the mangled filename). Set when a UFO is loaded; consulted
// when the user clicks a grid cell.
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

    resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(canvas.value);
    window.addEventListener("keydown", onKeyDown);
    // Window-level drag listeners stop the browser from "opening" a
    // dropped .ufo as a file:// URL when the drop lands outside the
    // canvas's drop zone (e.g. on the drop-hint overlay, the
    // toolbar, or empty space in grid mode).
    window.addEventListener("dragover", onWindowDragOver);
    window.addEventListener("drop", onWindowDrop);

    // Dev convenience: auto-load any UFO sitting at
    // web/assets/test-fonts/ so we don't drag-drop on every reload.
    // Tree-shaken out of prod builds because of the env check.
    if (import.meta.env.DEV) {
      const { readDevTestFontFiles } = await import("./devTestFont");
      const files = await readDevTestFontFiles();
      if (files.length > 0) {
        await loadGlifFiles(files);
      }
    }
  } catch (e) {
    console.error(e);
    status.value = `failed: ${e}`;
  }
});

function requestRender() {
  if (!editor || viewMode.value !== "editor") return;
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

/// Extract glyph metadata (name, first unicode, mark color) from a
/// .glif XML buffer. Name and unicode are near the top; the mark
/// color lives in `<lib>` at the bottom, so we have to decode the
/// whole file. Still well under a millisecond per glyph.
function parseGlyphInfo(bytes: Uint8Array): {
  name: string | null;
  unicode: string | null;
  markColor: string | null;
} {
  const xml = new TextDecoder().decode(bytes);
  const nameMatch = /<glyph\s+name="([^"]+)"/.exec(xml);
  const uniMatch = /<unicode\s+hex="([0-9a-fA-F]+)"/.exec(xml);
  const markMatch =
    /<key>\s*public\.markColor\s*<\/key>\s*<string>\s*([0-9.,\s]+)\s*<\/string>/.exec(
      xml,
    );
  return {
    name: nameMatch?.[1] ?? null,
    unicode: uniMatch?.[1]?.toUpperCase() ?? null,
    markColor: markMatch?.[1]?.replace(/\s+/g, "") ?? null,
  };
}

async function loadGlifFiles(files: File[]) {
  if (!editor || !canvas.value) return;

  // Surface a friendlier message for designspace until Wave 3f.
  if (files.some((f) => /\.designspace$/i.test(f.name))) {
    status.value =
      "designspace files aren't supported yet — open one of the referenced .ufo folders for now";
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

  // Look for fontinfo.plist alongside the glyphs/ dir. The path
  // looks like "MyFont.ufo/fontinfo.plist" — match the segment
  // sitting directly inside any *.ufo dir so we don't accidentally
  // pick up a nested copy.
  const fontInfoFile = files.find((f) =>
    /(^|\/)[^/]+\.ufo\/fontinfo\.plist$/i.test(relPath(f)),
  ) ?? files.find((f) => /\/fontinfo\.plist$/i.test(relPath(f)));
  if (fontInfoFile) {
    try {
      const bytes = new Uint8Array(await fontInfoFile.arrayBuffer());
      editor.setFontInfo(bytes);
    } catch (e) {
      console.warn("fontinfo.plist parse failed:", e);
    }
  }

  glyphBytes.clear();
  glyphUnicodes.value = new Map();
  glyphCategories.value = new Map();
  glyphMarkColors.value = new Map();
  glyphNames.value = [];
  glyphSvgs.value = new Map();
  currentGlyph.value = "";
  selectedGlyph.value = "";
  selectedCategory.value = "All";
  status.value = `reading ${glifs.length} glyph${glifs.length === 1 ? "" : "s"}…`;

  // Read all glyphs in parallel; bound by file-read throughput.
  const loaded = await Promise.all(
    glifs.map(async (f) => {
      const bytes = new Uint8Array(await f.arrayBuffer());
      const { name, unicode } = parseGlyphInfo(bytes);
      return { name, unicode, bytes };
    }),
  );
  for (const { name, unicode, markColor, bytes } of loaded) {
    if (!name) continue;
    glyphBytes.set(name, bytes);
    if (unicode) glyphUnicodes.value.set(name, unicode);
    if (markColor) glyphMarkColors.value.set(name, markColor);
    // Glyphs without a unicode default to "Other" — same convention
    // runebender-xilem uses for design-only glyphs (.notdef, etc.).
    const cp = unicode ? parseInt(unicode, 16) : NaN;
    const cat = Number.isFinite(cp)
      ? (glyphCategoryForCodepoint(cp) as Category)
      : "Other";
    glyphCategories.value.set(name, cat);
  }
  const names = Array.from(glyphBytes.keys()).sort();
  glyphNames.value = names;

  // Label the UFO by the folder name (best-effort) — match any
  // ".ufo" segment in the path, regardless of nesting depth.
  const sample = relPath(glifs[0]);
  const ufoMatch = sample.match(/([^/]+\.ufo)\//i);
  fontLabel.value = ufoMatch ? ufoMatch[1] : "";

  status.value = `rendering grid (${names.length})…`;
  // Render grid SVGs in chunks so we yield to the event loop and
  // the "loading…" status pill actually paints for big UFOs.
  await buildGridSvgs(names);

  status.value = "ready";
  viewMode.value = "grid";
}

async function buildGridSvgs(names: string[]) {
  const chunkSize = 64;
  const svgs = new Map<string, string>();
  for (let i = 0; i < names.length; i += chunkSize) {
    for (let j = i; j < Math.min(i + chunkSize, names.length); j++) {
      const name = names[j];
      const bytes = glyphBytes.get(name);
      if (!bytes) continue;
      try {
        const svg = glifToSvg(bytes);
        if (svg) svgs.set(name, svg);
      } catch {
        // Skip malformed glyphs silently — they'll appear as empty
        // cells but won't break the rest of the UFO.
      }
    }
    // Let Vue/browser breathe between chunks.
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  glyphSvgs.value = svgs;
}

function selectGlyph(name: string) {
  selectedGlyph.value = name;
  // Fill width/contours for the info sidebar by parsing on-the-fly
  // without loading into the editor (so single-clicks stay light).
  // For now we leave currentWidth / currentContours alone unless
  // the user actually opens the glyph; the info sidebar will show
  // em-dash until they double-click.
}

function openGlyph(name: string) {
  if (!editor || !canvas.value) return;
  const bytes = glyphBytes.get(name);
  if (!bytes) return;
  try {
    editor.setGlyphGlif(bytes);
    viewMode.value = "editor";
    currentGlyph.value = name;
    selectedGlyph.value = name;
    currentWidth.value = editor.advanceWidth();
    currentContours.value = editor.contourCount();
    // Canvas was visually hidden; let layout settle before sizing.
    requestAnimationFrame(() => {
      if (!editor || !canvas.value) return;
      handleResize();
      editor.fitToCanvas(canvas.value.width, canvas.value.height);
      requestRender();
    });
  } catch (e) {
    console.error(e);
    status.value = `failed to load ${name}: ${e}`;
  }
}

/// Apply (or clear) a mark color on the selected glyph. RGBA is
/// the UFO `public.markColor` string "r,g,b,a"; empty string clears.
function setMarkOnSelected(rgba: string) {
  const name = selectedGlyph.value;
  if (!name) return;
  if (rgba) {
    glyphMarkColors.value.set(name, rgba);
  } else {
    glyphMarkColors.value.delete(name);
  }
  // Force the cell to re-render with the new tint. The Map mutation
  // alone doesn't trigger Vue reactivity, so we replace it.
  glyphMarkColors.value = new Map(glyphMarkColors.value);
  // TODO: when save lands, also patch the .glif bytes in glyphBytes
  // so the change persists to disk.
}

function backToGrid() {
  viewMode.value = "grid";
}

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
      } catch {}
      out.push(file);
    } else {
      out.push(
        ...(await filesFromDirectoryHandle(entry as FileSystemDirectoryHandle, path)),
      );
    }
  }
  return out;
}

// ---------------------------------------------------------------------
// Drag-drop
// ---------------------------------------------------------------------

function relPath(f: File): string {
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
          try {
            Object.defineProperty(f, "webkitRelativePath", {
              value: entry.fullPath.replace(/^\//, ""),
              configurable: true,
            });
          } catch {}
          resolve([f]);
        },
        (err) => reject(err),
      ),
    );
  }
  if (entry.isDirectory && entry.createReader) {
    const reader = entry.createReader();
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
  const itemsCopy = Array.from(items);
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

// Window-level fallback handlers. Without these, dropping a .ufo
// onto any part of the page that isn't the canvas (e.g. the
// drop-hint overlay in grid mode, the toolbar) makes the browser
// navigate to the file:// URL of the folder, leaving the dev page
// entirely. preventDefault on dragover signals to the browser
// "I want to handle this drop myself, don't go to file://".
function onWindowDragOver(e: DragEvent) {
  if (e.dataTransfer?.types?.includes("Files")) {
    e.preventDefault();
    dragHover.value = true;
  }
}

function onWindowDrop(e: DragEvent) {
  if (e.dataTransfer?.types?.includes("Files")) {
    onDrop(e);
  }
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

  // Esc returns to the grid view from the editor.
  if (e.key === "Escape" && viewMode.value === "editor") {
    e.preventDefault();
    backToGrid();
    return;
  }

  // Undo/redo only apply in the editor.
  if (viewMode.value !== "editor") return;

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
  window.removeEventListener("dragover", onWindowDragOver);
  window.removeEventListener("drop", onWindowDrop);
  editor?.free();
  editor = null;
});
</script>

<template>
  <div class="runebender-host">
    <TopBar
      :font-label="fontLabel"
      :unsaved="glyphNames.length > 0"
      :masters="masters"
      :active-master="0"
    />

    <!-- Content row: left column (categories + mark colors) +
         stage (canvas/grid) + info sidebar (right). -->
    <div class="content">
      <div
        v-if="viewMode === 'grid' && glyphNames.length > 0"
        class="left-col"
      >
        <CategorySidebar
          :selected="selectedCategory"
          :counts="categoryCounts"
          @select="selectedCategory = $event"
        />
        <MarkColorPanel
          :active="selectedGlyph ? glyphMarkColors.get(selectedGlyph) : ''"
          :enabled="!!selectedGlyph"
          @set="setMarkOnSelected"
        />
      </div>

      <!-- Stage = canvas + grid stacked on the same area. Canvas
           stays in the DOM (visibility-hidden in grid mode) so the
           WebGPU surface stays bound. -->
      <div class="stage">
        <canvas
          ref="canvas"
          class="runebender-canvas"
          :class="{
            'drag-hover': dragHover,
            'is-hidden': viewMode !== 'editor',
          }"
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

        <div
          v-if="viewMode === 'grid' && glyphNames.length > 0"
          class="grid-view"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
        >
          <GlyphCell
            v-for="name in filteredGlyphNames"
            :key="name"
            :name="name"
            :unicode="glyphUnicodes.get(name)"
            :svg="glyphSvgs.get(name)"
            :selected="name === selectedGlyph"
            :mark-color="glyphMarkColors.get(name)"
            @click="selectGlyph(name)"
            @dblclick="openGlyph(name)"
          />
        </div>
      </div>

      <GlyphInfoSidebar
        v-if="glyphNames.length > 0"
        :master="masters[0]"
        :name="selectedGlyph"
        :unicode="selectedGlyph ? glyphUnicodes.get(selectedGlyph) : undefined"
        :width="selectedGlyph === currentGlyph ? currentWidth : undefined"
        :contours="selectedGlyph === currentGlyph ? currentContours : undefined"
      />
    </div>
  </div>
</template>

<style scoped>
/*
 * Colors mirror runebender-xilem/src/theme.rs verbatim:
 *   APP_BACKGROUND       #101010 (BASE_A)
 *   PANEL_BACKGROUND     #1C1C1C
 *   PANEL_OUTLINE / BASE_F  #606060
 *   PRIMARY_UI_TEXT / BASE_I  #909090
 *   SECONDARY_UI_TEXT / BASE_G  #707070
 *   GRID_CELL_TEXT / BASE_H  #808080
 *   GRID_GLYPH_COLOR / BASE_J  #a0a0a0
 *   GRID_CELL_SELECTED_OUTLINE / METRICS_GUIDE  #66EE88
 *   SELECTION_RECT_STROKE / TOOL_PREVIEW  #ffaa33
 */

.runebender-host {
  width: 100%;
  height: 100%;
  background: #101010;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-sizing: border-box;
}

/* Content row: left column + stage + right sidebar, separated by
   BENTO_GAP. */
.content {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 6px;
}

/* Left column: categories stretch, mark colors fixed at the bottom. */
.left-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}

/* Stage = the area inside .content where canvas + grid live,
   stacked on the same coordinates. */
.stage {
  position: relative;
  flex: 1;
  min-width: 0;
}

.runebender-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
  cursor: crosshair;
  touch-action: none;
}
.runebender-canvas.is-hidden {
  visibility: hidden;
  pointer-events: none;
}
.runebender-canvas.drag-hover {
  outline: 2px dashed #66ee88;
  outline-offset: -2px;
}

/* ----- Grid ----- */
/* BENTO_GAP = 6px from xilem's views/glyph_grid/mod.rs */
.grid-view {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(144px, 1fr));
  gap: 6px;
  background: #101010;
}
</style>
