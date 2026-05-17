<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
// wasm-pack output lives in ../wasm/ (a normal source directory, not
// /public/). Vite resolves this as a regular ES module; the shim's
// internal `new URL('..._bg.wasm', import.meta.url)` then resolves
// to a sibling URL that Vite serves automatically in dev and rewrites
// to a bundled asset in prod.
import init, {
  GlyphEditor,
  glifCompatibility,
  glifAnatomySvg,
  glifAnatomySvgWithComponents,
  glifMetadata,
  glifToSvg,
  glifToSvgWithComponents,
  glifWithMarkColor,
  glifWithName,
  glifWithOutlinesFrom,
  glifWithUnicode,
  glyphCategoryForCodepoint,
} from "../wasm/runebender_comfy_core.js";
import CategorySidebar, {
  type Category,
} from "./components/CategorySidebar.vue";
import CoordinatePanel from "./components/CoordinatePanel.vue";
import type { CoordinateQuadrant } from "./components/CoordinatePanel.vue";
import EditModeToolbar from "./components/EditModeToolbar.vue";
import { type ToolId } from "./components/toolIds";
import ShapesToolbar from "./components/ShapesToolbar.vue";
import type { ShapeKind } from "./components/ShapesToolbar.vue";
import TextDirectionToolbar from "./components/TextDirectionToolbar.vue";
import type { TextDirection } from "./components/TextDirectionToolbar.vue";
import GlyphAnatomyPanel from "./components/GlyphAnatomyPanel.vue";
import GlyphCell from "./components/GlyphCell.vue";
import GlyphInfoSidebar from "./components/GlyphInfoSidebar.vue";
import MarkColorPanel from "./components/MarkColorPanel.vue";
import MasterToolbar from "./components/MasterToolbar.vue";
import TopBar from "./components/TopBar.vue";
import TransformPanel, {
  type TransformActionId,
} from "./components/TransformPanel.vue";
import { THEME_CHROME_COLORS } from "./themeTokens";
import WelcomePanel from "./components/WelcomePanel.vue";
import WorkspaceToolbar from "./components/WorkspaceToolbar.vue";

const props = defineProps<{
  nodeId?: string;
  fontPathRef?: { value: string };
  onGlyphDataChange?: (value: string) => void;
}>();

const currentFontPath = computed(() => props.fontPathRef?.value ?? "");
const WELCOME_DEMO_GLIF = `<?xml version="1.0" encoding="UTF-8"?>
<glyph name="R" format="2">
  <advance width="668"/>
  <unicode hex="0052"/>
  <outline>
    <contour>
      <point x="192" y="416" type="line"/>
      <point x="184" y="424" type="line"/>
      <point x="184" y="664" type="line"/>
      <point x="192" y="672" type="line"/>
      <point x="368" y="672" type="line"/>
      <point x="440" y="672"/>
      <point x="496" y="616"/>
      <point x="496" y="544" type="curve"/>
      <point x="496" y="472"/>
      <point x="440" y="416"/>
      <point x="368" y="416" type="curve"/>
    </contour>
    <contour>
      <point x="96" y="0" type="line"/>
      <point x="168" y="0" type="line"/>
      <point x="184" y="16" type="line"/>
      <point x="184" y="320" type="line"/>
      <point x="192" y="328" type="line"/>
      <point x="360" y="328" type="line"/>
      <point x="456" y="328"/>
      <point x="496" y="288"/>
      <point x="496" y="192" type="curve"/>
      <point x="496" y="16" type="line"/>
      <point x="512" y="0" type="line"/>
      <point x="584" y="0" type="line"/>
      <point x="600" y="16" type="line"/>
      <point x="600" y="208" type="line"/>
      <point x="600" y="304"/>
      <point x="544" y="360"/>
      <point x="472" y="368" type="curve"/>
      <point x="472" y="376" type="line"/>
      <point x="528" y="392"/>
      <point x="604" y="448"/>
      <point x="604" y="544" type="curve"/>
      <point x="604" y="672"/>
      <point x="504" y="768"/>
      <point x="376" y="768" type="curve"/>
      <point x="96" y="768" type="line"/>
      <point x="80" y="752" type="line"/>
      <point x="80" y="16" type="line"/>
    </contour>
  </outline>
</glyph>`;
const WELCOME_DEMO_FONTINFO = `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>unitsPerEm</key><integer>1024</integer>
  <key>ascender</key><integer>832</integer>
  <key>descender</key><integer>-256</integer>
  <key>xHeight</key><integer>576</integer>
  <key>capHeight</key><integer>768</integer>
</dict>
</plist>`;

const canvas = ref<HTMLCanvasElement | null>(null);
const gridView = ref<HTMLDivElement | null>(null);
const gridViewportWidth = ref<number>(0);
const backgroundImageInput = ref<HTMLInputElement | null>(null);
const fontDirectoryInput = ref<HTMLInputElement | null>(null);
const status = ref<string>("initializing");
const lastSavedDisplay = ref<string | null>(null);
const selectionCount = ref<number>(0);
const selectedContourCount = ref<number>(0);
const dragHover = ref<boolean>(false);
const currentGlyph = ref<string>("");
const fontLabel = ref<string>("");
const viewMode = ref<"grid" | "editor">("grid");
const selectedCategory = ref<Category>("All");
// Live metadata for the info sidebar — read from the wasm editor
// after each setGlyphGlif call. -1 / 0 means "no glyph loaded yet".
const currentWidth = ref<number>(-1);
const currentContours = ref<number>(0);
// Glyph that's selected (highlighted in the grid, shown in the info
// sidebar, mark-color target). Distinct from `currentGlyph` which
// is the glyph currently loaded into the editor.
const selectedGlyph = ref<string>("");
const selectedGlyphs = ref<Set<string>>(new Set());
const currentLeftSidebearing = ref<number>(0);
const currentRightSidebearing = ref<number>(0);
// Active tool in the editor view. Tool implementations land
// incrementally in Rust while Vue owns the selected toolbar state.
const activeTool = ref<ToolId>("Select");
const activeShape = ref<ShapeKind>("rectangle");
const textDirection = ref<TextDirection>("ltr");
const textBuffer = ref<TextSort[]>([]);
const textCursor = ref<number>(0);
const activeTextSortIndex = ref<number | null>(null);
const temporaryPreviewReturnTool = ref<ToolId | null>(null);
const coordinateQuadrant = ref<CoordinateQuadrant>("cc");
const editorPanelsVisible = ref<boolean>(true);
const chromeStyle = {
  "--rb-app-background": THEME_CHROME_COLORS.appBackground,
  "--rb-control-background": THEME_CHROME_COLORS.controlBackground,
  "--rb-panel-background": THEME_CHROME_COLORS.panelBackground,
  "--rb-panel-outline": THEME_CHROME_COLORS.panelOutline,
  "--rb-primary-text": THEME_CHROME_COLORS.primaryText,
  "--rb-secondary-text": THEME_CHROME_COLORS.secondaryText,
  "--rb-muted-text": THEME_CHROME_COLORS.mutedText,
  "--rb-subdued-text": THEME_CHROME_COLORS.subduedText,
  "--rb-accent": THEME_CHROME_COLORS.accent,
  "--rb-glyph-preview": THEME_CHROME_COLORS.glyphPreview,
  "--rb-warning": THEME_CHROME_COLORS.warning,
  "--rb-background-image-selection": THEME_CHROME_COLORS.backgroundImageSelection,
  "--rb-danger": THEME_CHROME_COLORS.danger,
  "--rb-danger-text": THEME_CHROME_COLORS.dangerText,
  "--rb-overlay-text": THEME_CHROME_COLORS.overlayText,
  "--rb-mark-selected-ring": THEME_CHROME_COLORS.markSelectedRing,
  "--rb-mark-hover-ring": THEME_CHROME_COLORS.markHoverRing,
};
const backgroundImage = ref<BackgroundImageState | null>(null);
const backgroundImageFrame = ref<Record<string, string>>({});
const backgroundImageDragStart = ref<{ x: number; y: number } | null>(null);
const backgroundImageResize = ref<BackgroundImageResizeState | null>(null);
const backgroundImageContextMenu = ref<BackgroundImageContextMenuState | null>(null);
const glyphImageFiles = ref<Map<string, File>>(new Map());
const contourContextMenu = ref<ContourContextMenuState | null>(null);
const compatErrors = ref<CompatError[]>([]);
const compatMarkers = ref<CompatMarker[]>([]);
const clipboardNotice = ref<string>("");
let clipboardNoticeTimer: number | null = null;

// ---------------------------------------------------------------------
// Master state — single source of truth
// ---------------------------------------------------------------------
//
// All per-glyph data lives inside MasterData; the active master's
// view is exposed as a set of computeds below. Switching masters
// (Regular ↔ Bold) means flipping `activeMasterName` — no need to
// imperatively swap top-level state.

type MasterData = {
  glyphBytes: Map<string, Uint8Array>;
  glyphPaths: Map<string, string>;
  glyphFileHandles: Map<string, FileSystemFileHandle>;
  groupsPath: string | null;
  groupsFileHandle: FileSystemFileHandle | null;
  kerningPath: string | null;
  kerningFileHandle: FileSystemFileHandle | null;
  glyphUnicodes: Map<string, string>;
  glyphMetadata: Map<string, GlyphMetadata>;
  glyphKerningGroups: Map<string, GlyphKerningGroups>;
  groups: Map<string, string[]>;
  kerning: Map<string, Map<string, number>>;
  glyphSvgs: Map<string, string>;
  glyphCategories: Map<string, Category>;
  glyphMarkColors: Map<string, string>;
  fontInfoBytes: Uint8Array | null;
  unitsPerEm: number;
};

type GlyphMetadata = {
  name: string;
  width: number;
  contours: number;
  unicode: string | null;
  unicodes: string[];
};

type GridGlyphItem = {
  name: string;
  index: number;
  columnSpan: number;
};

type GlyphKerningGroups = {
  left?: string;
  right?: string;
};

type BackgroundImageState = {
  url: string;
  width: number;
  height: number;
  designX: number;
  designY: number;
  designScaleX: number;
  designScaleY: number;
  locked: boolean;
  selected: boolean;
};

type BackgroundImageResizeHandle = "tl" | "tr" | "bl" | "br" | "top" | "bottom" | "left" | "right";

type BackgroundImageResizeState = {
  handle: BackgroundImageResizeHandle;
  anchorX: number;
  anchorY: number;
  initialScaleX: number;
  initialScaleY: number;
  initialDistance: number;
};

type BackgroundImageContextMenuState = {
  x: number;
  y: number;
  locked: boolean;
};

type ContourContextMenuState = {
  x: number;
  y: number;
  screenX: number;
  screenY: number;
  pathIndex: number;
  canSetStart: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
};

type CompatError = {
  kind: "missingGlyph" | "contourCountMismatch" | "pointCountMismatch" | "pointTypeMismatch";
  masterName: string;
  message: string;
  contourIndex?: number | null;
  pointIndex?: number | null;
  x?: number | null;
  y?: number | null;
  expected?: string | null;
  actual?: string | null;
};

type CompatMarker = CompatError & {
  screenX: number;
  screenY: number;
};

type SelectionBounds = {
  count: number;
  x: number;
  y: number;
  width: number;
  height: number;
};

type MeasureInfo = {
  x: number;
  y: number;
  distance: number;
  angle: number;
  labels: Array<{
    x: number;
    y: number;
    length: number;
  }>;
};

type TextSort =
  | {
      kind: "glyph";
      glyphName: string;
      char: string;
      codepoint: number;
      advanceWidth: number;
    }
  | { kind: "lineBreak" };

type TextBufferSnapshot = {
  cursor: number;
  activeSort: number | null;
  direction: TextDirection;
  sorts: Array<{
    kind: "glyph" | "lineBreak";
    glyphName?: string;
    char?: string;
    codepoint?: number;
    advanceWidth?: number;
    active: boolean;
  }>;
};

type TextLayoutSnapshot = {
  cursorX: number;
  cursorY: number;
  items: Array<{
    index: number;
    x: number;
    y: number;
    advanceWidth: number;
  }>;
};

type TextLayoutItem = TextLayoutSnapshot["items"][number] & {
  sort: Extract<TextSort, { kind: "glyph" }>;
};

const masterDataMap = ref<Map<string, MasterData>>(new Map());
const activeMasterName = ref<string>("");
const selectedBounds = ref<SelectionBounds | undefined>(undefined);
const measureInfo = ref<MeasureInfo | undefined>(undefined);
const dirtyGlyphsByMaster = ref<Map<string, Set<string>>>(new Map());
const dirtyKerningMasters = ref<Set<string>>(new Set());
const dirtyGroupsMasters = ref<Set<string>>(new Set());
const gridGlyphClipboard = ref<Uint8Array | null>(null);

const activeMasterData = computed(() => masterDataMap.value.get(activeMasterName.value));
const glyphUnicodes = computed(
  () => activeMasterData.value?.glyphUnicodes ?? (new Map<string, string>()),
);
const glyphMetadataMap = computed(
  () => activeMasterData.value?.glyphMetadata ?? (new Map<string, GlyphMetadata>()),
);
const glyphKerningGroups = computed(
  () =>
    activeMasterData.value?.glyphKerningGroups ??
    (new Map<string, GlyphKerningGroups>()),
);
const groups = computed(
  () => activeMasterData.value?.groups ?? (new Map<string, string[]>()),
);
const kerning = computed(
  () => activeMasterData.value?.kerning ?? (new Map<string, Map<string, number>>()),
);
const glyphSvgs = computed(
  () => activeMasterData.value?.glyphSvgs ?? (new Map<string, string>()),
);
const glyphXmlByName = computed(() =>
  activeMasterData.value ? glyphXmlMapJson(activeMasterData.value.glyphBytes) : "{}",
);
const glyphCategories = computed(
  () => activeMasterData.value?.glyphCategories ?? (new Map<string, Category>()),
);
const glyphMarkColors = computed(
  () => activeMasterData.value?.glyphMarkColors ?? (new Map<string, string>()),
);
const glyphNames = computed(() =>
  activeMasterData.value ? Array.from(activeMasterData.value.glyphBytes.keys()).sort() : [],
);
const masters = computed(() => Array.from(masterDataMap.value.keys()));
const activeMasterIndex = computed(() => masters.value.indexOf(activeMasterName.value));
const masterPreviewSvgs = computed(() =>
  masters.value.map((name) => {
    const data = masterDataMap.value.get(name);
    return data?.glyphSvgs.get("n") ?? data?.glyphSvgs.get("N");
  }),
);
const dirtyGlyphCount = computed(() => {
  let total = 0;
  for (const glyphs of dirtyGlyphsByMaster.value.values()) {
    total += glyphs.size;
  }
  return total;
});
const hasDirtyChanges = computed(
  () =>
    dirtyGlyphCount.value > 0 ||
    dirtyKerningMasters.value.size > 0 ||
    dirtyGroupsMasters.value.size > 0,
);

// Names filtered by the active category. The grid renders this list
// instead of glyphNames directly.
const filteredGlyphNames = computed(() => {
  if (selectedCategory.value === "All") return glyphNames.value;
  return glyphNames.value.filter(
    (n) => (glyphCategories.value.get(n) ?? "Other") === selectedCategory.value,
  );
});

const glyphGridColumns = computed(() => {
  // Mirrors xilem's AppState::grid_columns constants.
  const available = Math.max(0, gridViewportWidth.value - 6);
  const columns = Math.floor((available + 6) / (128 + 6));
  return Math.max(1, Math.min(8, columns || 1));
});

const gridStyle = computed(() => ({
  gridTemplateColumns: `repeat(${glyphGridColumns.value}, minmax(0, 1fr))`,
}));

function computeGlyphColumnSpan(name: string): number {
  const metadata = glyphMetadataMap.value.get(name);
  const upm = activeMasterData.value?.unitsPerEm ?? 1000;
  const nameSpan = name.length <= 14 ? 1 : name.length <= 26 ? 2 : 3;
  const width = metadata?.width ?? 0;
  const widthRatio = upm > 0 ? width / upm : 0;
  const widthSpan =
    widthRatio <= 1.5 ? 1 : widthRatio <= 2.8 ? 2 : widthRatio <= 4 ? 3 : 4;
  return Math.max(nameSpan, widthSpan);
}

const gridGlyphItems = computed<GridGlyphItem[]>(() => {
  const columns = glyphGridColumns.value;
  const items = filteredGlyphNames.value.map((name, index) => ({
    name,
    index,
    columnSpan: Math.min(columns, computeGlyphColumnSpan(name)),
  }));
  let rowSpan = 0;
  let lastInRow = -1;
  for (let i = 0; i < items.length; i += 1) {
    const span = items[i].columnSpan;
    if (rowSpan + span > columns && lastInRow >= 0) {
      items[lastInRow].columnSpan += columns - rowSpan;
      rowSpan = 0;
    }
    rowSpan += items[i].columnSpan;
    lastInRow = i;
    if (rowSpan === columns) {
      rowSpan = 0;
      lastInRow = -1;
    }
  }
  if (rowSpan > 0 && lastInRow >= 0) {
    items[lastInRow].columnSpan += columns - rowSpan;
  }
  return items;
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

const selectedMetadata = computed(() =>
  selectedGlyph.value ? glyphMetadataMap.value.get(selectedGlyph.value) : undefined,
);
const selectedWidth = computed(() =>
  selectedGlyph.value === currentGlyph.value
    ? currentWidth.value
    : selectedMetadata.value?.width,
);
const selectedContours = computed(() =>
  selectedGlyph.value === currentGlyph.value
    ? currentContours.value
    : selectedMetadata.value?.contours,
);
const selectedKerningGroups = computed(() =>
  selectedGlyph.value ? glyphKerningGroups.value.get(selectedGlyph.value) : undefined,
);
const selectedUnicodeDisplay = computed(() => {
  if (!selectedGlyph.value) return undefined;
  const unicodes = glyphMetadataMap.value.get(selectedGlyph.value)?.unicodes;
  return unicodes && unicodes.length > 0
    ? unicodes.join(", ")
    : glyphUnicodes.value.get(selectedGlyph.value);
});
const activeGlyphSvg = computed(() =>
  currentGlyph.value ? glyphSvgs.value.get(currentGlyph.value) : undefined,
);
const activeGlyphPreviewStyle = computed(() => {
  const svg = activeGlyphSvg.value;
  if (!svg) return {};
  const viewBox = /\bviewBox="([^"]+)"/.exec(svg)?.[1]?.trim().split(/\s+/).map(Number);
  const width = viewBox && viewBox.length === 4 ? viewBox[2] : NaN;
  const height = viewBox && viewBox.length === 4 ? viewBox[3] : NaN;
  if (!Number.isFinite(width) || !Number.isFinite(height) || height <= 0) {
    return {};
  }
  const panelWidth = Math.max(60, Math.min(200, 140 * (width / height)));
  return { width: `${panelWidth}px` };
});
const activeGlyphUnicode = computed(() =>
  currentGlyph.value ? glyphUnicodes.value.get(currentGlyph.value) : undefined,
);
const activeGlyphKerningGroups = computed(() =>
  currentGlyph.value ? glyphKerningGroups.value.get(currentGlyph.value) : undefined,
);
const activeLeftKern = computed(() => activeTextKernValue("left"));
const activeRightKern = computed(() => activeTextKernValue("right"));
const canEditActiveLeftKern = computed(() => activeTextKernPair("left") !== null);
const canEditActiveRightKern = computed(() => activeTextKernPair("right") !== null);
const activeGlyphPanelVisible = computed(
  () => !!currentGlyph.value && (activeTool.value !== "Text" || activeTextSortIndex.value !== null),
);
const selectedAnatomySvg = computed(() => {
  if (!selectedGlyph.value || !activeMasterData.value) return undefined;
  const bytes = activeMasterData.value.glyphBytes.get(selectedGlyph.value);
  if (!bytes) return undefined;
  try {
    return glifAnatomySvgWithComponents(bytes, glyphXmlByName.value) || glifAnatomySvg(bytes);
  } catch {
    return undefined;
  }
});
const TEXT_PREVIEW_SCALE = 0.12;
const textLayout = ref<TextLayoutSnapshot>({ cursorX: 0, cursorY: 0, items: [] });
const textLayoutItems = computed<TextLayoutItem[]>(() =>
  textLayout.value.items
    .map((item) => {
      const sort = textBuffer.value[item.index];
      if (!sort || sort.kind !== "glyph") return undefined;
      return { ...item, sort };
    })
    .filter((item): item is TextLayoutItem => !!item),
);
const textPreviewHeight = computed(() => {
  const minY = Math.min(0, textLayout.value.cursorY, ...textLayout.value.items.map((item) => item.y));
  return Math.max(70, Math.abs(minY) * TEXT_PREVIEW_SCALE + 70);
});
const textPreviewWidth = computed(() => {
  const maxX = Math.max(
    textLayout.value.cursorX,
    ...textLayout.value.items.map((item) => item.x + item.advanceWidth),
    600,
  );
  return Math.max(120, maxX * TEXT_PREVIEW_SCALE + 24);
});

type Editor = {
  pointerDown(x: number, y: number, button: number, mods: number): void;
  pointerMove(x: number, y: number, mods: number): void;
  pointerUp(x: number, y: number, button: number, mods: number): boolean;
  pointerCancel(): boolean;
  componentBaseAt(x: number, y: number): string;
  setTool(toolId: ToolId): boolean;
  setShapeTool(shape: ShapeKind): boolean;
  setShapeShiftLocked(locked: boolean): boolean;
  setKnifeShiftLocked(locked: boolean): boolean;
  setTextDirection(direction: TextDirection): void;
  setTextKerningModel(json: string): void;
  textKerningModel(): string;
  setTextGlyphInventory(json: string): void;
  shapeTextBuffer(): boolean;
  textActiveSort(): number;
  textBufferSnapshot(): string;
  textBufferLayout(lineHeight: number): string;
  clearTextBuffer(): void;
  insertTextGlyph(name: string, codepoint: number, advanceWidth: number): void;
  insertTextCharacter(codepoint: number): boolean;
  updateTextGlyph(index: number, name: string, codepoint: number, advanceWidth: number): boolean;
  insertTextLineBreak(): void;
  deleteTextBeforeCursor(): boolean;
  deleteTextAfterCursor(): boolean;
  setTextCursor(cursor: number): void;
  moveTextCursorVisualLeft(): void;
  moveTextCursorVisualRight(): void;
  moveTextCursorVisualUp(lineHeight: number): void;
  moveTextCursorVisualDown(lineHeight: number): void;
  moveTextCursorLineStart(): void;
  moveTextCursorLineEnd(): void;
  activateTextSort(index: number): boolean;
  wheel(x: number, y: number, deltaY: number): void;
  undo(): boolean;
  redo(): boolean;
  flipSelectionHorizontal(): boolean;
  flipSelectionVertical(): boolean;
  rotateSelectionClockwise(): boolean;
  rotateSelectionCounterClockwise(): boolean;
  duplicateSelection(): boolean;
  duplicateRepeatSelection(): boolean;
  reverseContours(): boolean;
  contourContextAt(x: number, y: number): Float64Array;
  setStartPointAt(x: number, y: number): boolean;
  reverseContourAt(x: number, y: number): boolean;
  moveContour(pathIndex: number, direction: "up" | "down"): boolean;
  convertHyperToCubic(): boolean;
  copySelection(): boolean;
  pasteSelection(): boolean;
  deleteSelection(): boolean;
  togglePointType(): boolean;
  togglePointTypeAt(x: number, y: number): boolean;
  unionSelection(): boolean;
  subtractSelection(): boolean;
  intersectSelection(): boolean;
  excludeSelection(): boolean;
  render(): void;
  resize(w: number, h: number): void;
  setGlyphSvg(svg: string): void;
  setGlyphGlif(bytes: Uint8Array): void;
  setGlyphGlifWithComponents(bytes: Uint8Array, glyphXmlByName: string): void;
  setFontInfo(bytes: Uint8Array): void;
  fitToCanvas(w: number, h: number): void;
  setZoom(z: number): void;
  zoom(): number;
  setOffset(x: number, y: number): void;
  designToScreen(x: number, y: number): Float64Array;
  screenToDesign(x: number, y: number): Float64Array;
  selectionCount(): number;
  selectedContourCount(): number;
  selectionBounds(): Float64Array;
  measureInfo(): Float64Array;
  setCoordinateQuadrant(quadrant: string): void;
  moveSelectionReference(axis: "x" | "y", value: number): boolean;
  resizeSelectionReference(axis: "width" | "height", value: number): boolean;
  nudgeSelection(
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    independent: boolean,
  ): boolean;
  setAdvanceWidth(width: number): boolean;
  leftSidebearing(): number;
  rightSidebearing(): number;
  setLeftSidebearing(value: number): boolean;
  setRightSidebearing(value: number): boolean;
  currentGlyphGlif(originalBytes: Uint8Array, markColor: string): Uint8Array;
  advanceWidth(): number;
  contourCount(): number;
  metricBounds(): Float64Array;
  glyphBounds(): Float64Array;
  free(): void;
};

type SaveFilePickerOptions = {
  suggestedName?: string;
  types?: Array<{
    description?: string;
    accept: Record<string, string[]>;
  }>;
  excludeAcceptAllOption?: boolean;
};

type SaveFilePicker = (options?: SaveFilePickerOptions) => Promise<FileSystemFileHandle>;
type DirectoryPicker = () => Promise<FileSystemDirectoryHandle>;

let editor: Editor | null = null;
let raf = 0;
let resizeObserver: ResizeObserver | null = null;
let comfySyncTimer: number | null = null;
let lastPublishedComfyState = "";

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
    updateGridViewportSize();
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("pointerdown", onWindowPointerDown);
    // Window-level drag listeners stop the browser from "opening" a
    // dropped .ufo as a file:// URL when the drop lands outside the
    // canvas's drop zone (e.g. on the drop-hint overlay, the
    // toolbar, or empty space in grid mode).
    window.addEventListener("dragenter", onWindowDragOver, { capture: true });
    window.addEventListener("dragover", onWindowDragOver, { capture: true });
    window.addEventListener("drop", onWindowDrop, { capture: true });

    if (currentFontPath.value) {
      await loadWorkspaceSlot(currentFontPath.value);
    } else {
      loadWelcomeDemoGlyph();
    }
  } catch (e) {
    console.error(e);
    status.value = `failed: ${e}`;
  }
});

watch(
  () => currentFontPath.value,
  async (slot) => {
    if (!slot || !editor || !canvas.value) return;
    await loadWorkspaceSlot(slot);
  },
);

function requestRender() {
  if (!editor || (viewMode.value !== "editor" && glyphNames.value.length > 0)) return;
  cancelAnimationFrame(raf);
  raf = requestAnimationFrame(() => {
    editor?.render();
    refreshBackgroundImageFrame();
    refreshSelectionState();
    refreshCompatibilityMarkers();
  });
}

function loadWelcomeDemoGlyph() {
  if (!editor || !canvas.value || glyphNames.value.length > 0) return;
  const encoder = new TextEncoder();
  try {
    editor.setFontInfo(encoder.encode(WELCOME_DEMO_FONTINFO));
    editor.setGlyphGlif(encoder.encode(WELCOME_DEMO_GLIF));
    editor.setTool("Select");
    editor.setOffset(500, 700);
    editor.setZoom(0.7);
    requestRender();
  } catch (e) {
    console.warn("welcome demo glyph failed to load:", e);
  }
}

function queueComfyStateSync(force = false) {
  if (!props.nodeId) return;
  if (comfySyncTimer !== null) {
    clearTimeout(comfySyncTimer);
  }
  comfySyncTimer = window.setTimeout(() => {
    comfySyncTimer = null;
    void publishComfyState(force);
  }, 0);
}

async function publishComfyState(force = false) {
  if (!props.nodeId) return;
  const payload = currentGlyph.value
    ? activeMasterData.value?.glyphSvgs.get(currentGlyph.value) ?? ""
    : "";
  const stateKey = `${currentFontPath.value}\n${payload}`;
  if (!force && stateKey === lastPublishedComfyState) return;

  try {
    props.onGlyphDataChange?.(payload);
    const body = new FormData();
    body.append("node_id", props.nodeId);
    body.append("font", currentFontPath.value);
    body.append("glyph_data", payload);
    await fetch("/runebender/set_state", {
      method: "POST",
      body,
    });
    lastPublishedComfyState = stateKey;
  } catch (e) {
    console.warn("ComfyUI state sync failed:", e);
  }
}

function refreshSelectionState() {
  if (!editor) {
    selectionCount.value = 0;
    selectedContourCount.value = 0;
    selectedBounds.value = undefined;
    return;
  }
  const bounds = editor.selectionBounds();
  selectedBounds.value =
    bounds.length >= 5
      ? {
          count: bounds[0],
          x: bounds[1],
          y: bounds[2],
          width: bounds[3],
          height: bounds[4],
        }
      : undefined;
  selectionCount.value = selectedBounds.value?.count ?? editor.selectionCount();
  selectedContourCount.value = editor.selectedContourCount();
}

function updateCompatibilityErrors() {
  const data = activeMasterData.value;
  if (!data || !currentGlyph.value || masters.value.length < 2) {
    compatErrors.value = [];
    compatMarkers.value = [];
    return;
  }
  const activeBytes = data.glyphBytes.get(currentGlyph.value);
  if (!activeBytes) {
    compatErrors.value = [];
    compatMarkers.value = [];
    return;
  }
  const decoder = new TextDecoder();
  const otherMasters: Record<string, string | null> = {};
  for (const [masterName, masterData] of masterDataMap.value) {
    if (masterName === activeMasterName.value) continue;
    const bytes = masterData.glyphBytes.get(currentGlyph.value);
    otherMasters[masterName] = bytes ? decoder.decode(bytes) : null;
  }
  try {
    compatErrors.value = JSON.parse(
      glifCompatibility(activeBytes, currentGlyph.value, JSON.stringify(otherMasters)),
    ) as CompatError[];
  } catch (e) {
    console.warn("compatibility check failed:", e);
    compatErrors.value = [];
  }
  refreshCompatibilityMarkers();
}

function refreshCompatibilityMarkers() {
  if (!editor || viewMode.value !== "editor" || compatErrors.value.length === 0) {
    compatMarkers.value = [];
    return;
  }
  compatMarkers.value = compatErrors.value.flatMap((error) => {
    if (typeof error.x !== "number" || typeof error.y !== "number") return [];
    const screen = editor.designToScreen(error.x, error.y);
    return [
      {
        ...error,
        screenX: screen[0],
        screenY: screen[1],
      },
    ];
  });
}

function refreshMeasureState() {
  if (!editor) {
    measureInfo.value = undefined;
    return;
  }
  const info = editor.measureInfo();
  measureInfo.value =
    info.length >= 4
      ? {
          x: info[0],
          y: info[1],
          distance: info[2],
          angle: info[3],
          labels: measureLabelsFromInfo(info),
        }
      : undefined;
}

function measureLabelsFromInfo(info: Float64Array): MeasureInfo["labels"] {
  const count = Math.max(0, Math.floor(info[4] ?? 0));
  const labels: MeasureInfo["labels"] = [];
  for (let i = 0; i < count; i++) {
    const offset = 5 + i * 3;
    if (info.length < offset + 3) break;
    labels.push({
      x: info[offset],
      y: info[offset + 1],
      length: info[offset + 2],
    });
  }
  return labels;
}

function formatMeasure(value: number): string {
  return Number.isFinite(value) ? value.toFixed(1) : "";
}

function refreshBackgroundImageFrame() {
  if (!editor || !backgroundImage.value) {
    backgroundImageFrame.value = {};
    return;
  }
  const bg = backgroundImage.value;
  const dpr = window.devicePixelRatio || 1;
  const topLeft = editor.designToScreen(
    bg.designX,
    bg.designY + bg.height * bg.designScaleY,
  );
  const width = (bg.width * bg.designScaleX * editor.zoom()) / dpr;
  const height = (bg.height * bg.designScaleY * editor.zoom()) / dpr;
  backgroundImageFrame.value = {
    left: `${topLeft[0] / dpr}px`,
    top: `${topLeft[1] / dpr}px`,
    width: `${width}px`,
    height: `${height}px`,
    pointerEvents: bg.locked || activeTool.value === "Preview" ? "none" : "auto",
  };
}

function pointerDesignCoords(e: PointerEvent): [number, number] | null {
  if (!editor) return null;
  const coords = canvasCoords(e);
  if (!coords) return null;
  const design = editor.screenToDesign(coords[0], coords[1]);
  return [design[0], design[1]];
}

function imageDimensions(url: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
    img.onerror = () => reject(new Error("image decode failed"));
    img.src = url;
  });
}

function isBackgroundImageFile(file: File): boolean {
  return /\.(png|jpe?g)$/i.test(file.name);
}

function clearBackgroundImage() {
  if (backgroundImage.value?.url) {
    URL.revokeObjectURL(backgroundImage.value.url);
  }
  backgroundImage.value = null;
  backgroundImageDragStart.value = null;
  backgroundImageResize.value = null;
  backgroundImageFrame.value = {};
}

function baseNameWithoutExtension(path: string): string {
  const leaf = path.split(/[\\/]/).pop() ?? path;
  return leaf.replace(/\.[^.]+$/, "");
}

function glyphNamesForImageFile(file: File): string[] {
  const path = file.webkitRelativePath || file.name;
  const baseName = baseNameWithoutExtension(path);
  const names = new Set<string>([baseName]);
  if (baseName.endsWith("_")) {
    names.add(baseName.slice(0, -1));
  }
  return Array.from(names).filter(Boolean);
}

function collectGlyphImageFiles(files: File[]): Map<string, File> {
  const images = new Map<string, File>();
  for (const file of files) {
    if (!isBackgroundImageFile(file)) continue;
    const path = file.webkitRelativePath || file.name;
    const baseName = baseNameWithoutExtension(path);
    if (baseName.endsWith("_")) {
      images.set(baseName.slice(0, -1), file);
      if (!images.has(baseName)) {
        images.set(baseName, file);
      }
      continue;
    }
    for (const glyphName of glyphNamesForImageFile(file)) {
      if (!images.has(glyphName)) images.set(glyphName, file);
    }
  }
  return images;
}

async function importBackgroundImage(file: File, options: { locked?: boolean } = {}) {
  if (!editor) return;
  const url = URL.createObjectURL(file);
  try {
    const { width, height } = await imageDimensions(url);
    clearBackgroundImage();
    const metrics = editor.metricBounds();
    const ascender = metrics.length >= 2 ? metrics[0] : 800;
    const descender = metrics.length >= 2 ? metrics[1] : -200;
    const glyphBounds = editor.glyphBounds();
    const bounds =
      glyphBounds.length >= 4 && glyphBounds[3] > 0
        ? {
            x: glyphBounds[0],
            y: glyphBounds[1],
            width: glyphBounds[2],
            height: glyphBounds[3],
          }
        : null;
    const designHeight = Math.max(1, bounds?.height ?? ascender - descender);
    const designScale = designHeight / Math.max(1, height);
    const designWidth = width * designScale;
    const designX = bounds
      ? bounds.x + (bounds.width - designWidth) / 2
      : (editor.advanceWidth() - designWidth) / 2;
    backgroundImage.value = {
      url,
      width,
      height,
      designX,
      designY: bounds?.y ?? descender,
      designScaleX: designScale,
      designScaleY: designScale,
      locked: !!options.locked,
      selected: !options.locked,
    };
    status.value = `imported ${file.name}`;
    refreshBackgroundImageFrame();
    requestRender();
  } catch (e) {
    URL.revokeObjectURL(url);
    status.value = `image import failed: ${e}`;
  }
}

async function importMatchingGlyphImage(glyphName: string) {
  const imageFile = glyphImageFiles.value.get(glyphName);
  if (!imageFile) {
    clearBackgroundImage();
    return;
  }
  await importBackgroundImage(imageFile, { locked: true });
  status.value = `loaded background image for ${glyphName}`;
}

function onBackgroundImageInput(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (file) void importBackgroundImage(file);
  if (input) input.value = "";
}

async function openFontDirectoryPicker() {
  const picker = (window as Window & {
    showDirectoryPicker?: DirectoryPicker;
  }).showDirectoryPicker;
  if (!picker) {
    fontDirectoryInput.value?.click();
    return;
  }
  try {
    const handle = await picker();
    const { files, fileHandles } = await filesFromDirectoryHandle(handle, handle.name);
    await loadGlifFiles(files, fileHandles);
  } catch (e) {
    if ((e as DOMException).name !== "AbortError") {
      console.warn("font directory picker failed:", e);
      status.value = `open failed: ${e}`;
    }
  }
}

async function onFontDirectoryInput(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const files = Array.from(input?.files ?? []);
  if (input) input.value = "";
  if (files.length === 0) return;
  await loadGlifFiles(files);
}

function toggleBackgroundImageLock(): boolean {
  if (!backgroundImage.value) return false;
  const locked = !backgroundImage.value.locked;
  backgroundImage.value = {
    ...backgroundImage.value,
    locked,
    selected: locked ? false : backgroundImage.value.selected,
  };
  backgroundImageDragStart.value = null;
  backgroundImageResize.value = null;
  status.value = backgroundImage.value.locked
    ? "background image locked"
    : "background image unlocked";
  refreshBackgroundImageFrame();
  return true;
}

function openBackgroundImageContextMenu(e: MouseEvent) {
  const bg = backgroundImage.value;
  if (!bg) return;
  e.preventDefault();
  e.stopPropagation();
  backgroundImageContextMenu.value = {
    x: e.clientX,
    y: e.clientY,
    locked: bg.locked,
  };
  if (!bg.locked) {
    backgroundImage.value = {
      ...bg,
      selected: true,
    };
    refreshBackgroundImageFrame();
  }
}

function lockedBackgroundImageContainsScreenPoint(x: number, y: number): boolean {
  const bg = backgroundImage.value;
  if (!editor || !bg?.locked) return false;
  const design = editor.screenToDesign(x, y);
  const right = bg.designX + bg.width * bg.designScaleX;
  const top = bg.designY + bg.height * bg.designScaleY;
  return (
    design[0] >= bg.designX &&
    design[0] <= right &&
    design[1] >= bg.designY &&
    design[1] <= top
  );
}

function openLockedBackgroundImageContextMenu(e: MouseEvent) {
  const bg = backgroundImage.value;
  if (!bg?.locked) return;
  e.preventDefault();
  e.stopPropagation();
  backgroundImageContextMenu.value = {
    x: e.clientX,
    y: e.clientY,
    locked: true,
  };
}

function dismissBackgroundImageContextMenu() {
  backgroundImageContextMenu.value = null;
}

function dismissContourContextMenu() {
  contourContextMenu.value = null;
}

function showClipboardNotice(message: string) {
  clipboardNotice.value = message;
  if (clipboardNoticeTimer !== null) {
    window.clearTimeout(clipboardNoticeTimer);
  }
  clipboardNoticeTimer = window.setTimeout(() => {
    clipboardNotice.value = "";
    clipboardNoticeTimer = null;
  }, 1400);
}

function onWindowPointerDown() {
  dismissBackgroundImageContextMenu();
  dismissContourContextMenu();
}

function applyBackgroundImageContextMenuAction() {
  const menu = backgroundImageContextMenu.value;
  if (!menu || !backgroundImage.value) return;
  backgroundImage.value = {
    ...backgroundImage.value,
    locked: !menu.locked,
    selected: menu.locked ? backgroundImage.value.selected : false,
  };
  backgroundImageDragStart.value = null;
  backgroundImageResize.value = null;
  backgroundImageContextMenu.value = null;
  status.value = backgroundImage.value.locked
    ? "background image locked"
    : "background image unlocked";
  refreshBackgroundImageFrame();
  requestRender();
}

function onBackgroundPointerDown(e: PointerEvent) {
  if (
    !backgroundImage.value ||
    backgroundImage.value.locked ||
    backgroundImageResize.value
  ) {
    return;
  }
  dismissBackgroundImageContextMenu();
  const design = pointerDesignCoords(e);
  if (!design) return;
  e.preventDefault();
  e.stopPropagation();
  (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  backgroundImage.value = {
    ...backgroundImage.value,
    selected: true,
  };
  backgroundImageDragStart.value = { x: design[0], y: design[1] };
}

function onBackgroundPointerMove(e: PointerEvent) {
  if (
    !backgroundImage.value ||
    backgroundImage.value.locked ||
    backgroundImageResize.value
  ) {
    return;
  }
  const start = backgroundImageDragStart.value;
  if (!start) return;
  const design = pointerDesignCoords(e);
  if (!design) return;
  e.preventDefault();
  e.stopPropagation();
  const dx = design[0] - start.x;
  const dy = design[1] - start.y;
  backgroundImage.value = {
    ...backgroundImage.value,
    designX: backgroundImage.value.designX + dx,
    designY: backgroundImage.value.designY + dy,
  };
  backgroundImageDragStart.value = { x: design[0], y: design[1] };
  refreshBackgroundImageFrame();
}

function onBackgroundPointerUp(e: PointerEvent) {
  if (!backgroundImageDragStart.value) return;
  e.preventDefault();
  e.stopPropagation();
  (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
  backgroundImageDragStart.value = null;
}

function backgroundImageAnchor(
  handle: BackgroundImageResizeHandle,
  bg: BackgroundImageState,
): { x: number; y: number } {
  const right = bg.designX + bg.width * bg.designScaleX;
  const top = bg.designY + bg.height * bg.designScaleY;
  const centerX = bg.designX + (right - bg.designX) / 2;
  const centerY = bg.designY + (top - bg.designY) / 2;
  switch (handle) {
    case "tl":
      return { x: right, y: bg.designY };
    case "tr":
      return { x: bg.designX, y: bg.designY };
    case "bl":
      return { x: right, y: top };
    case "br":
      return { x: bg.designX, y: top };
    case "top":
      return { x: centerX, y: bg.designY };
    case "bottom":
      return { x: centerX, y: top };
    case "left":
      return { x: right, y: centerY };
    case "right":
      return { x: bg.designX, y: centerY };
  }
}

function onBackgroundResizePointerDown(
  handle: BackgroundImageResizeHandle,
  e: PointerEvent,
) {
  if (!backgroundImage.value || backgroundImage.value.locked) return;
  const design = pointerDesignCoords(e);
  if (!design) return;
  e.preventDefault();
  e.stopPropagation();
  (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  const anchor = backgroundImageAnchor(handle, backgroundImage.value);
  backgroundImage.value = {
    ...backgroundImage.value,
    selected: true,
  };
  backgroundImageResize.value = {
    handle,
    anchorX: anchor.x,
    anchorY: anchor.y,
    initialScaleX: backgroundImage.value.designScaleX,
    initialScaleY: backgroundImage.value.designScaleY,
    initialDistance: Math.max(1, Math.hypot(design[0] - anchor.x, design[1] - anchor.y)),
  };
  backgroundImageDragStart.value = null;
}

function onBackgroundResizePointerMove(e: PointerEvent) {
  const resize = backgroundImageResize.value;
  const bg = backgroundImage.value;
  if (!resize || !bg || bg.locked) return;
  const design = pointerDesignCoords(e);
  if (!design) return;
  e.preventDefault();
  e.stopPropagation();
  const distance = Math.max(
    1,
    Math.hypot(design[0] - resize.anchorX, design[1] - resize.anchorY),
  );
  let designScaleX = bg.designScaleX;
  let designScaleY = bg.designScaleY;
  if (["tl", "tr", "bl", "br"].includes(resize.handle)) {
    const ratio = distance / resize.initialDistance;
    designScaleX = Math.max(0.001, resize.initialScaleX * ratio);
    designScaleY = Math.max(0.001, resize.initialScaleY * ratio);
  } else if (resize.handle === "left" || resize.handle === "right") {
    designScaleX = Math.max(0.001, Math.abs(design[0] - resize.anchorX) / bg.width);
    designScaleY = resize.initialScaleY;
  } else {
    designScaleX = resize.initialScaleX;
    designScaleY = Math.max(0.001, Math.abs(design[1] - resize.anchorY) / bg.height);
  }
  const nextWidth = bg.width * designScaleX;
  const nextHeight = bg.height * designScaleY;
  const next = { ...bg, designScaleX, designScaleY };
  switch (resize.handle) {
    case "tl":
      next.designX = resize.anchorX - nextWidth;
      next.designY = resize.anchorY;
      break;
    case "tr":
      next.designX = resize.anchorX;
      next.designY = resize.anchorY;
      break;
    case "bl":
      next.designX = resize.anchorX - nextWidth;
      next.designY = resize.anchorY - nextHeight;
      break;
    case "br":
      next.designX = resize.anchorX;
      next.designY = resize.anchorY - nextHeight;
      break;
    case "top":
      next.designX = bg.designX;
      next.designY = resize.anchorY;
      break;
    case "bottom":
      next.designX = bg.designX;
      next.designY = resize.anchorY - nextHeight;
      break;
    case "left":
      next.designX = resize.anchorX - nextWidth;
      next.designY = bg.designY;
      break;
    case "right":
      next.designX = resize.anchorX;
      next.designY = bg.designY;
      break;
  }
  backgroundImage.value = next;
  refreshBackgroundImageFrame();
}

function onBackgroundResizePointerUp(e: PointerEvent) {
  if (!backgroundImageResize.value) return;
  e.preventDefault();
  e.stopPropagation();
  (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
  backgroundImageResize.value = null;
}

function deleteSelectedBackgroundImage(): boolean {
  if (!backgroundImage.value?.selected) return false;
  clearBackgroundImage();
  status.value = "background image removed";
  requestRender();
  return true;
}

function nudgeSelectedBackgroundImage(dx: number, dy: number): boolean {
  const bg = backgroundImage.value;
  if (!bg?.selected || bg.locked) return false;
  backgroundImage.value = {
    ...bg,
    designX: bg.designX + dx,
    designY: bg.designY + dy,
  };
  status.value = "background image moved";
  refreshBackgroundImageFrame();
  requestRender();
  return true;
}

function reportUnavailableBackgroundTrace(kind: "local" | "quiver", refit: boolean): boolean {
  if (!backgroundImage.value) {
    status.value = kind === "quiver"
      ? "no background image for Quiver trace"
      : "no background image to trace";
    return true;
  }
  if (kind === "quiver") {
    status.value = "Quiver trace is not available in the browser editor yet";
    return true;
  }
  status.value = refit
    ? "background image refit is not available in the browser editor yet"
    : "background image trace is not available in the browser editor yet";
  return true;
}

function onCoordinateQuadrant(quadrant: CoordinateQuadrant) {
  coordinateQuadrant.value = quadrant;
  editor?.setCoordinateQuadrant(quadrant);
  refreshSelectionState();
}

function onCoordinateChange(axis: "x" | "y" | "width" | "height", value: number) {
  if (!editor || !currentGlyph.value) return;
  const changed =
    axis === "x" || axis === "y"
      ? editor.moveSelectionReference(axis, value)
      : editor.resizeSelectionReference(axis, value);
  if (!changed) {
    refreshSelectionState();
    return;
  }
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
}

function onActiveGlyphWidthChange(event: Event) {
  if (!editor || !currentGlyph.value) return;
  const input = event.target as HTMLInputElement | null;
  const width = Number(input?.value);
  if (!Number.isFinite(width)) {
    if (input) input.value = Math.round(currentWidth.value).toString();
    return;
  }
  const changed = editor.setAdvanceWidth(width);
  if (!changed) {
    if (input) input.value = Math.round(currentWidth.value).toString();
    return;
  }
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  syncTextKerningModelToEditor();
  reshapeTextBuffer();
  requestRender();
  queueComfyStateSync();
}

function refreshSidebearingsFromEditor() {
  if (!editor) {
    currentLeftSidebearing.value = 0;
    currentRightSidebearing.value = 0;
    return;
  }
  currentLeftSidebearing.value = editor.leftSidebearing();
  currentRightSidebearing.value = editor.rightSidebearing();
}

function onActiveGlyphSidebearingChange(side: "left" | "right", event: Event) {
  if (!editor || !currentGlyph.value) return;
  const input = event.target as HTMLInputElement | null;
  const value = Number(input?.value);
  if (!Number.isFinite(value)) {
    if (input) {
      input.value = Math.round(
        side === "left" ? currentLeftSidebearing.value : currentRightSidebearing.value,
      ).toString();
    }
    return;
  }
  const changed =
    side === "left"
      ? editor.setLeftSidebearing(value)
      : editor.setRightSidebearing(value);
  if (!changed) {
    refreshSidebearingsFromEditor();
    if (input) {
      input.value = Math.round(
        side === "left" ? currentLeftSidebearing.value : currentRightSidebearing.value,
      ).toString();
    }
    return;
  }
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  refreshSidebearingsFromEditor();
  syncTextKerningModelToEditor();
  reshapeTextBuffer();
  requestRender();
  queueComfyStateSync();
}

function normalizeUnicodeInput(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  return trimmed
    .replace(/^U\+/i, "")
    .replace(/^0x/i, "")
    .toUpperCase()
    .padStart(4, "0");
}

function onActiveGlyphUnicodeChange(event: Event) {
  if (!editor || !currentGlyph.value) return;
  const input = event.target as HTMLInputElement | null;
  const data = activeMasterData.value;
  if (!data) return;
  const unicode = normalizeUnicodeInput(input?.value ?? "");

  if (!syncCurrentGlyphBytesFromEditor()) {
    if (input) input.value = activeGlyphUnicode.value ?? "";
    return;
  }
  const currentBytes = data.glyphBytes.get(currentGlyph.value);
  if (!currentBytes) return;

  try {
    const bytes = glifWithUnicode(currentBytes, unicode);
    const info = parseGlyphInfo(bytes);
    data.glyphBytes.set(currentGlyph.value, bytes);
    data.glyphMetadata.set(currentGlyph.value, {
      name: currentGlyph.value,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    });
    if (info.unicode) {
      data.glyphUnicodes.set(currentGlyph.value, info.unicode);
      const cp = parseInt(info.unicode, 16);
      data.glyphCategories.set(
        currentGlyph.value,
        Number.isFinite(cp) ? (glyphCategoryForCodepoint(cp) as Category) : "Other",
      );
    } else {
      data.glyphUnicodes.delete(currentGlyph.value);
      data.glyphCategories.set(currentGlyph.value, "Other");
    }
    const svg = glifToSvg(bytes);
    if (svg) data.glyphSvgs.set(currentGlyph.value, svg);
    masterDataMap.value = new Map(masterDataMap.value);
    if (input) input.value = info.unicode ?? "";
    markGlyphDirty(currentGlyph.value);
    syncTextKerningModelToEditor();
    reshapeTextBuffer();
    queueComfyStateSync();
  } catch (e) {
    console.warn("updating glyph unicode failed:", e);
    if (input) input.value = activeGlyphUnicode.value ?? "";
    status.value = `unicode update failed: ${e}`;
  }
}

function replaceGlyphNameInGroups(
  groupsMap: Map<string, string[]>,
  oldName: string,
  newName: string,
) {
  for (const [groupName, members] of groupsMap) {
    const nextMembers = members.map((member) => (member === oldName ? newName : member));
    if (nextMembers.some((member, index) => member !== members[index])) {
      groupsMap.set(groupName, Array.from(new Set(nextMembers)));
    }
  }
}

function replaceGlyphNameInKerning(
  kerningMap: Map<string, Map<string, number>>,
  oldName: string,
  newName: string,
) {
  const oldPairs = kerningMap.get(oldName);
  if (oldPairs) {
    kerningMap.delete(oldName);
    const targetPairs = kerningMap.get(newName) ?? new Map<string, number>();
    for (const [second, value] of oldPairs) {
      targetPairs.set(second === oldName ? newName : second, value);
    }
    kerningMap.set(newName, targetPairs);
  }

  for (const [first, pairs] of kerningMap) {
    if (!pairs.has(oldName)) continue;
    const value = pairs.get(oldName);
    pairs.delete(oldName);
    if (value !== undefined) pairs.set(newName, value);
    if (pairs.size === 0) kerningMap.delete(first);
  }
}

function syncRenamedTextSorts(oldName: string, newName: string, metadata: GlyphMetadata) {
  if (!editor) return;
  const codepoint = metadata.unicode ? parseInt(metadata.unicode, 16) : 0;
  for (let index = 0; index < textBuffer.value.length; index++) {
    const sort = textBuffer.value[index];
    if (sort.kind !== "glyph" || sort.glyphName !== oldName) continue;
    editor.updateTextGlyph(
      index,
      newName,
      Number.isFinite(codepoint) ? codepoint : 0,
      metadata.width,
    );
  }
  refreshTextStateFromEditor();
}

function onActiveGlyphNameChange(event: Event) {
  if (!editor || !currentGlyph.value) return;
  const input = event.target as HTMLInputElement | null;
  const data = activeMasterData.value;
  const oldName = currentGlyph.value;
  const newName = input?.value.trim() ?? "";
  if (!data || !newName || newName === oldName) {
    if (input) input.value = oldName;
    return;
  }
  if (data.glyphBytes.has(newName)) {
    if (input) input.value = oldName;
    status.value = `glyph ${newName} already exists`;
    return;
  }

  if (!syncCurrentGlyphBytesFromEditor()) {
    if (input) input.value = oldName;
    return;
  }
  const oldBytes = data.glyphBytes.get(oldName);
  if (!oldBytes) return;

  try {
    const bytes = glifWithName(oldBytes, newName);
    const info = parseGlyphInfo(bytes);
    const metadata = {
      name: newName,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    };
    const path = data.glyphPaths.get(oldName);
    const fileHandle = data.glyphFileHandles.get(oldName);
    const markColor = data.glyphMarkColors.get(oldName);

    data.glyphBytes.delete(oldName);
    data.glyphBytes.set(newName, bytes);
    data.glyphPaths.delete(oldName);
    if (path) data.glyphPaths.set(newName, path);
    data.glyphFileHandles.delete(oldName);
    if (fileHandle) data.glyphFileHandles.set(newName, fileHandle);
    data.glyphMetadata.delete(oldName);
    data.glyphMetadata.set(newName, metadata);
    data.glyphUnicodes.delete(oldName);
    if (info.unicode) data.glyphUnicodes.set(newName, info.unicode);
    data.glyphCategories.delete(oldName);
    const cp = info.unicode ? parseInt(info.unicode, 16) : NaN;
    data.glyphCategories.set(
      newName,
      Number.isFinite(cp) ? (glyphCategoryForCodepoint(cp) as Category) : "Other",
    );
    data.glyphMarkColors.delete(oldName);
    if (markColor) data.glyphMarkColors.set(newName, markColor);
    data.glyphSvgs.delete(oldName);
    const svg = glifToSvg(bytes);
    if (svg) data.glyphSvgs.set(newName, svg);

    replaceGlyphNameInGroups(data.groups, oldName, newName);
    data.glyphKerningGroups = buildGlyphKerningGroups(data.groups);
    replaceGlyphNameInKerning(data.kerning, oldName, newName);
    clearGlyphDirty(oldName);
  currentGlyph.value = newName;
  selectedGlyph.value = newName;
  selectedGlyphs.value = new Set([newName]);
  currentWidth.value = info.width;
  currentContours.value = info.contours;
  refreshSidebearingsFromEditor();
  masterDataMap.value = new Map(masterDataMap.value);
    markGlyphDirty(newName);
    markGroupsDirty();
    markKerningDirty();
    syncTextKerningModelToEditor();
    syncRenamedTextSorts(oldName, newName, metadata);
    reshapeTextBuffer();
    queueComfyStateSync();
  } catch (e) {
    console.warn("renaming glyph failed:", e);
    if (input) input.value = oldName;
    status.value = `rename failed: ${e}`;
  }
}

function onToolSelect(tool: ToolId) {
  activeTool.value = tool;
  const toolChanged = editor?.setTool(tool) ?? false;
  let shapeChanged = false;
  if (tool === "Shapes") {
    shapeChanged = editor?.setShapeTool(activeShape.value) ?? false;
  }
  if (toolChanged || shapeChanged) {
    syncEditorMutationAfterWasmChange();
  }
  refreshMeasureState();
  refreshBackgroundImageFrame();
  requestRender();
}

function eventTargetAcceptsText(event: KeyboardEvent): boolean {
  const target = event.target as HTMLElement | null;
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    !!target?.isContentEditable
  );
}

function onShapeSelect(shape: ShapeKind) {
  activeShape.value = shape;
  activeTool.value = "Shapes";
  if (editor?.setShapeTool(shape)) {
    syncEditorMutationAfterWasmChange();
  }
  requestRender();
}

function onTextDirectionSelect(direction: TextDirection) {
  textDirection.value = direction;
  editor?.setTextDirection(direction);
  refreshTextStateFromEditor();
  reshapeTextBuffer();
}

function textPreviewLineHeight(): number {
  if (!editor) return 1000;
  const metrics = editor.metricBounds();
  const ascender = metrics.length >= 2 ? metrics[0] : 800;
  const descender = metrics.length >= 2 ? metrics[1] : -200;
  return Math.max(1, ascender - descender);
}

function refreshTextStateFromEditor(syncSorts = true) {
  if (!editor) return;
  try {
    const snapshot = JSON.parse(editor.textBufferSnapshot()) as TextBufferSnapshot;
    if (syncSorts) {
      textBuffer.value = snapshot.sorts.map((sort): TextSort => {
        if (sort.kind === "lineBreak") return { kind: "lineBreak" };
        return {
          kind: "glyph",
          glyphName: sort.glyphName ?? ".notdef",
          char: sort.char ?? "",
          codepoint: sort.codepoint ?? 0,
          advanceWidth: sort.advanceWidth ?? 600,
        };
      });
    }
    textCursor.value = Math.max(0, Math.min(snapshot.sorts.length, snapshot.cursor));
    activeTextSortIndex.value =
      typeof snapshot.activeSort === "number" && snapshot.activeSort >= 0
        ? snapshot.activeSort
        : null;
    if (activeTextSortIndex.value !== null) {
      const activeSort = textBuffer.value[activeTextSortIndex.value];
      if (activeSort?.kind === "glyph") {
        selectedGlyph.value = activeSort.glyphName;
        selectedGlyphs.value = new Set([activeSort.glyphName]);
      }
    }
    textDirection.value = snapshot.direction;
    textLayout.value = JSON.parse(
      editor.textBufferLayout(textPreviewLineHeight()),
    ) as TextLayoutSnapshot;
    requestRender();
  } catch (e) {
    console.warn("failed to read text buffer snapshot:", e);
  }
}

function insertTextCharacter(char: string): boolean {
  const codepoint = char.codePointAt(0);
  if (codepoint === undefined) return false;
  if (!editor?.insertTextCharacter(codepoint)) {
    status.value = `no glyph for U+${codepoint.toString(16).toUpperCase().padStart(4, "0")}`;
    return false;
  }
  refreshTextStateFromEditor();
  loadActiveTextSortGlyphIntoEditor();
  return true;
}

function insertTextGlyphByName(glyphName: string): boolean {
  const metadata = glyphMetadataMap.value.get(glyphName);
  const unicode = glyphUnicodes.value.get(glyphName);
  const codepoint = unicode ? parseInt(unicode, 16) : 0;
  editor?.insertTextGlyph(
    glyphName,
    Number.isFinite(codepoint) ? codepoint : 0,
    metadata?.width ?? 600,
  );
  refreshTextStateFromEditor();
  reshapeTextBuffer();
  return true;
}

function insertTextLineBreak(): boolean {
  editor?.insertTextLineBreak();
  refreshTextStateFromEditor();
  reshapeTextBuffer();
  return true;
}

function deleteTextBeforeCursor(): boolean {
  if (textCursor.value <= 0) return false;
  editor?.deleteTextBeforeCursor();
  refreshTextStateFromEditor();
  reshapeTextBuffer();
  return true;
}

function deleteTextAfterCursor(): boolean {
  if (textCursor.value >= textBuffer.value.length) return false;
  editor?.deleteTextAfterCursor();
  refreshTextStateFromEditor();
  reshapeTextBuffer();
  return true;
}

function openTextSort(sort: TextSort) {
  if (sort.kind !== "glyph") return;
  openGlyph(sort.glyphName);
  activeTool.value = "Text";
  if (editor?.setTool("Text")) {
    syncEditorMutationAfterWasmChange();
  }
}

function activateTextSort(logicalIndex: number) {
  const sort = textBuffer.value[logicalIndex];
  if (!sort || sort.kind !== "glyph") return;
  editor?.activateTextSort(logicalIndex);
  refreshTextStateFromEditor();
  loadGlyphIntoEditor(sort.glyphName, { fitCanvas: false });
}

function setTextCursor(position: number) {
  const cursor = Math.max(0, Math.min(textBuffer.value.length, position));
  editor?.setTextCursor(cursor);
  refreshTextStateFromEditor();
}

function moveTextCursorVisual(delta: -1 | 1) {
  if (delta < 0) {
    editor?.moveTextCursorVisualLeft();
  } else {
    editor?.moveTextCursorVisualRight();
  }
  refreshTextStateFromEditor();
}

function moveTextCursorVertical(delta: -1 | 1) {
  if (delta < 0) {
    editor?.moveTextCursorVisualUp(textPreviewLineHeight());
  } else {
    editor?.moveTextCursorVisualDown(textPreviewLineHeight());
  }
  refreshTextStateFromEditor();
}

function moveTextCursorLineBoundary(boundary: "start" | "end") {
  if (boundary === "start") {
    editor?.moveTextCursorLineStart();
  } else {
    editor?.moveTextCursorLineEnd();
  }
  refreshTextStateFromEditor();
}

function reshapeTextBuffer() {
  editor?.shapeTextBuffer();
  refreshTextStateFromEditor();
  loadActiveTextSortGlyphIntoEditor();
  requestRender();
}

function textLayoutItemStyle(item: TextLayoutItem): Record<string, string> {
  const width = Math.max(28, Math.min(118, item.advanceWidth * TEXT_PREVIEW_SCALE));
  return {
    left: `${item.x * TEXT_PREVIEW_SCALE}px`,
    top: `${-item.y * TEXT_PREVIEW_SCALE}px`,
    width: `${width}px`,
  };
}

function textCursorStyle(): Record<string, string> {
  return {
    left: `${textLayout.value.cursorX * TEXT_PREVIEW_SCALE}px`,
    top: `${-textLayout.value.cursorY * TEXT_PREVIEW_SCALE}px`,
  };
}

function handleTextToolKey(e: KeyboardEvent): boolean {
  if (activeTool.value !== "Text" || e.metaKey || e.ctrlKey || e.altKey) {
    return false;
  }

  switch (e.key) {
    case "ArrowLeft":
      moveTextCursorVisual(-1);
      return true;
    case "ArrowRight":
      moveTextCursorVisual(1);
      return true;
    case "ArrowUp":
      moveTextCursorVertical(-1);
      return true;
    case "ArrowDown":
      moveTextCursorVertical(1);
      return true;
    case "Home":
      moveTextCursorLineBoundary("start");
      return true;
    case "End":
      moveTextCursorLineBoundary("end");
      return true;
    case "Backspace":
      deleteTextBeforeCursor();
      return true;
    case "Delete":
      deleteTextAfterCursor();
      return true;
    case "Enter":
      insertTextLineBreak();
      return true;
    default:
      if (e.key.length === 1) {
        insertTextCharacter(e.key);
        return true;
      }
      return false;
  }
}

function handleResize() {
  updateGridViewportSize();
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

function updateGridViewportSize() {
  const rect = canvas.value?.getBoundingClientRect() ?? gridView.value?.getBoundingClientRect();
  gridViewportWidth.value = rect?.width ?? 0;
}

function canvasCoords(e: PointerEvent): [number, number] | null {
  if (!canvas.value) return null;
  const rect = canvas.value.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const x = (e.clientX - rect.left) * dpr;
  const y = (e.clientY - rect.top) * dpr;
  return [x, y];
}

function canvasMouseCoords(e: MouseEvent): [number, number] | null {
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
  dismissBackgroundImageContextMenu();
  dismissContourContextMenu();
  if (backgroundImage.value?.selected) {
    backgroundImage.value = {
      ...backgroundImage.value,
      selected: false,
    };
  }
  (e.target as Element).setPointerCapture?.(e.pointerId);
  editor.pointerDown(c[0], c[1], e.button, modBits(e));
  refreshMeasureState();
  requestRender();
}

function onCanvasContextMenu(e: MouseEvent) {
  dismissBackgroundImageContextMenu();
  dismissContourContextMenu();
  if (!editor || viewMode.value !== "editor") return;
  const c = canvasMouseCoords(e);
  if (!c) return;
  const info = editor.contourContextAt(c[0], c[1]);
  if (info.length >= 4) {
    e.preventDefault();
    contourContextMenu.value = {
      x: e.clientX,
      y: e.clientY,
      screenX: info[4] ?? c[0],
      screenY: info[5] ?? c[1],
      pathIndex: info[0],
      canSetStart: info[1] > 0,
      canMoveUp: info[2] > 0,
      canMoveDown: info[3] > 0,
    };
    return;
  }
  if (lockedBackgroundImageContainsScreenPoint(c[0], c[1])) {
    openLockedBackgroundImageContextMenu(e);
  }
}

function applyContourContextMenuAction(
  action: "set-start" | "reverse" | "move-up" | "move-down",
) {
  const menu = contourContextMenu.value;
  if (!editor || !menu) return;
  const changed =
    action === "set-start"
      ? applyEditorMutation(() => editor.setStartPointAt(menu.screenX, menu.screenY))
      : action === "reverse"
        ? applyEditorMutation(() => editor.reverseContourAt(menu.screenX, menu.screenY))
        : applyEditorMutation(() =>
            editor.moveContour(menu.pathIndex, action === "move-up" ? "up" : "down"),
          );
  dismissContourContextMenu();
  if (changed) {
    status.value =
      action === "set-start"
        ? "start point set"
        : action === "reverse"
          ? "contour reversed"
          : "contour reordered";
  }
}

function onPointerMove(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  editor.pointerMove(c[0], c[1], modBits(e));
  refreshMeasureState();
  if (activeTool.value === "Text") {
    refreshTextStateFromEditor();
    syncTextKerningModelFromEditor(true);
  }
  requestRender();
}

function onPointerUp(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  const previousTextSort =
    activeTool.value === "Text" ? activeTextSortIndex.value : null;
  const changed = editor.pointerUp(c[0], c[1], e.button, modBits(e));
  (e.target as Element).releasePointerCapture?.(e.pointerId);
  if (changed && currentGlyph.value) {
    syncCurrentGlyphBytesFromEditor();
    markGlyphDirty(currentGlyph.value);
    currentContours.value = editor.contourCount();
    queueComfyStateSync();
  }
  refreshSelectionState();
  refreshMeasureState();
  if (activeTool.value === "Text") {
    refreshTextStateFromEditor();
    if (activeTextSortIndex.value !== previousTextSort) {
      loadActiveTextSortGlyphIntoEditor();
    }
    syncTextKerningModelFromEditor(true);
  }
  requestRender();
}

function onPointerCancel() {
  if (!editor) return;
  if (editor.pointerCancel()) {
    syncEditorMutationAfterWasmChange();
  }
  refreshMeasureState();
  requestRender();
}

function onCanvasDoubleClick(e: MouseEvent) {
  if (!editor) return;
  const c = canvasMouseCoords(e);
  if (!c) return;
  if (activeTool.value === "Select") {
    if (editor.togglePointTypeAt(c[0], c[1])) {
      syncCurrentGlyphBytesFromEditor();
      if (currentGlyph.value) {
        markGlyphDirty(currentGlyph.value);
      }
      refreshSelectionState();
      requestRender();
      queueComfyStateSync();
      return;
    }
    const baseName = editor.componentBaseAt(c[0], c[1]);
    if (!baseName || !activeMasterData.value?.glyphBytes.has(baseName)) return;
    openGlyph(baseName);
    return;
  }
  if (activeTool.value === "Text") {
    const baseName = editor.componentBaseAt(c[0], c[1]);
    if (!baseName || !activeMasterData.value?.glyphBytes.has(baseName)) return;
    insertTextGlyphByName(baseName);
    requestRender();
    queueComfyStateSync();
  }
}

// ---------------------------------------------------------------------
// UFO loading
// ---------------------------------------------------------------------

/// Extract glyph metadata from a .glif XML buffer. Rust/norad owns
/// structural metadata and codepoints; mark color still comes from a
/// tiny XML scan until lib metadata round-tripping moves fully into Rust.
function parseGlyphInfo(bytes: Uint8Array): {
  name: string | null;
  unicode: string | null;
  unicodes: string[];
  markColor: string | null;
  width: number;
  contours: number;
} {
  const xml = new TextDecoder().decode(bytes);
  const markMatch =
    /<key>\s*public\.markColor\s*<\/key>\s*<string>\s*([0-9.,\s]+)\s*<\/string>/.exec(
      xml,
    );
  const metadata = JSON.parse(glifMetadata(bytes)) as GlyphMetadata;
  const unicodes = metadata.unicodes.length
    ? metadata.unicodes
    : Array.from(xml.matchAll(/<unicode\b[^>]*\bhex="([0-9A-Fa-f]+)"/g))
        .map((match) => match[1].toUpperCase().padStart(4, "0"))
        .filter(Boolean);
  if (unicodes.length === 0 && metadata.unicode) {
    unicodes.push(metadata.unicode);
  }
  return {
    name: metadata.name,
    unicode: metadata.unicode,
    unicodes,
    markColor: markMatch?.[1]?.replace(/\s+/g, "") ?? null,
    width: metadata.width,
    contours: metadata.contours,
  };
}

function parseGroupsPlist(bytes: Uint8Array): Map<string, string[]> {
  const xml = new TextDecoder().decode(bytes);
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  const groups = new Map<string, string[]>();
  const dict = doc.querySelector("plist > dict");
  if (!dict) return groups;

  const children = Array.from(dict.children);
  for (let i = 0; i < children.length - 1; i += 2) {
    const key = children[i];
    const value = children[i + 1];
    if (key.tagName !== "key" || value.tagName !== "array") continue;
    const groupName = key.textContent?.trim();
    if (!groupName) continue;
    const members = Array.from(value.children)
      .filter((el) => el.tagName === "string")
      .map((el) => el.textContent?.trim() ?? "")
      .filter(Boolean);
    groups.set(groupName, members);
  }

  return groups;
}

function parseKerningPlist(bytes: Uint8Array): Map<string, Map<string, number>> {
  const xml = new TextDecoder().decode(bytes);
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  const kerning = new Map<string, Map<string, number>>();
  const rootDict = doc.querySelector("plist > dict");
  if (!rootDict) return kerning;

  const children = Array.from(rootDict.children);
  for (let i = 0; i < children.length - 1; i += 2) {
    const firstKey = children[i];
    const secondDict = children[i + 1];
    if (firstKey.tagName !== "key" || secondDict.tagName !== "dict") continue;
    const first = firstKey.textContent?.trim();
    if (!first) continue;

    const pairs = new Map<string, number>();
    const pairChildren = Array.from(secondDict.children);
    for (let j = 0; j < pairChildren.length - 1; j += 2) {
      const secondKey = pairChildren[j];
      const value = pairChildren[j + 1];
      if (secondKey.tagName !== "key") continue;
      if (!["integer", "real"].includes(value.tagName)) continue;
      const second = secondKey.textContent?.trim();
      const kernValue = Number(value.textContent?.trim());
      if (second && Number.isFinite(kernValue)) {
        pairs.set(second, kernValue);
      }
    }
    if (pairs.size > 0) kerning.set(first, pairs);
  }

  return kerning;
}

function serializeKerningPlist(kerningMap: Map<string, Map<string, number>>): string {
  const lines = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
    '<plist version="1.0">',
    '<dict>',
  ];

  for (const first of Array.from(kerningMap.keys()).sort()) {
    const pairs = kerningMap.get(first);
    if (!pairs || pairs.size === 0) continue;
    lines.push(`  <key>${escapeXml(first)}</key>`);
    lines.push("  <dict>");
    for (const second of Array.from(pairs.keys()).sort()) {
      const value = pairs.get(second);
      if (value === undefined) continue;
      lines.push(`    <key>${escapeXml(second)}</key>`);
      lines.push(`    <real>${formatPlistNumber(value)}</real>`);
    }
    lines.push("  </dict>");
  }

  lines.push("</dict>", "</plist>", "");
  return lines.join("\n");
}

function serializeGroupsPlist(groupsMap: Map<string, string[]>): string {
  const lines = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
    '<plist version="1.0">',
    '<dict>',
  ];

  for (const groupName of Array.from(groupsMap.keys()).sort()) {
    const members = groupsMap.get(groupName) ?? [];
    if (members.length === 0) continue;
    lines.push(`  <key>${escapeXml(groupName)}</key>`);
    lines.push("  <array>");
    for (const glyphName of Array.from(new Set(members)).sort()) {
      lines.push(`    <string>${escapeXml(glyphName)}</string>`);
    }
    lines.push("  </array>");
  }

  lines.push("</dict>", "</plist>", "");
  return lines.join("\n");
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function formatPlistNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(6)));
}

function buildGlyphKerningGroups(
  groups: Map<string, string[]>,
): Map<string, GlyphKerningGroups> {
  const byGlyph = new Map<string, GlyphKerningGroups>();
  for (const [groupName, members] of groups) {
    const side = groupName.startsWith("public.kern1.")
      ? "left"
      : groupName.startsWith("public.kern2.")
        ? "right"
        : null;
    if (!side) continue;
    for (const glyphName of members) {
      const existing = byGlyph.get(glyphName) ?? {};
      if (!existing[side]) existing[side] = groupName;
      byGlyph.set(glyphName, existing);
    }
  }
  return byGlyph;
}

function displayKerningGroup(group: string | undefined, prefix: string): string {
  return group?.startsWith(prefix) ? group.slice(prefix.length) : (group ?? "");
}

function normalizeKerningGroup(side: "left" | "right", value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "-") return null;
  const prefix = side === "left" ? "public.kern1." : "public.kern2.";
  return trimmed.startsWith(prefix) ? trimmed : `${prefix}${trimmed}`;
}

function textGlyphNameAt(index: number | null): string | null {
  if (index === null || index < 0 || index >= textBuffer.value.length) return null;
  const sort = textBuffer.value[index];
  return sort?.kind === "glyph" ? sort.glyphName : null;
}

function kerningGroupsForGlyph(glyphName: string, prefix: string): string[] {
  const groupsForGlyph = glyphKerningGroups.value.get(glyphName);
  const group =
    prefix === "public.kern1." ? groupsForGlyph?.left : groupsForGlyph?.right;
  return group?.startsWith(prefix) ? [group] : [];
}

function lookupKerningValue(left: string, right: string): number {
  const leftGroups = kerningGroupsForGlyph(left, "public.kern1.");
  const rightGroups = kerningGroupsForGlyph(right, "public.kern2.");
  const pairs = kerning.value.get(left);
  const direct = pairs?.get(right);
  if (direct !== undefined) return direct;
  for (const rightGroup of rightGroups) {
    const value = pairs?.get(rightGroup);
    if (value !== undefined) return value;
  }
  for (const leftGroup of leftGroups) {
    const value = kerning.value.get(leftGroup)?.get(right);
    if (value !== undefined) return value;
  }
  for (const leftGroup of leftGroups) {
    const groupPairs = kerning.value.get(leftGroup);
    if (!groupPairs) continue;
    for (const rightGroup of rightGroups) {
      const value = groupPairs.get(rightGroup);
      if (value !== undefined) return value;
    }
  }
  return 0;
}

function activeTextKernPair(side: "left" | "right"): [string, string] | null {
  const activeIndex = activeTextSortIndex.value;
  const activeName = textGlyphNameAt(activeIndex);
  if (activeIndex === null || !activeName) return null;
  if (side === "left") {
    const previousName = textGlyphNameAt(activeIndex - 1);
    return previousName ? [previousName, activeName] : null;
  }
  const nextName = textGlyphNameAt(activeIndex + 1);
  return nextName ? [activeName, nextName] : null;
}

function activeTextKernValue(side: "left" | "right"): number | null {
  const pair = activeTextKernPair(side);
  if (!pair) return null;
  const value = lookupKerningValue(pair[0], pair[1]);
  return value === 0 ? null : value;
}

function updateActiveTextKern(side: "left" | "right", value: string) {
  const data = activeMasterData.value;
  const pair = activeTextKernPair(side);
  if (!data || !pair) return;
  const trimmed = value.trim();
  const pairs = data.kerning.get(pair[0]) ?? new Map<string, number>();

  if (!trimmed || trimmed === "-") {
    pairs.delete(pair[1]);
  } else {
    const kernValue = Number(trimmed);
    if (!Number.isFinite(kernValue)) return;
    if (Math.abs(kernValue) < Number.EPSILON) {
      pairs.delete(pair[1]);
    } else {
      pairs.set(pair[1], kernValue);
    }
  }

  if (pairs.size > 0) {
    data.kerning.set(pair[0], pairs);
  } else {
    data.kerning.delete(pair[0]);
  }
  masterDataMap.value = new Map(masterDataMap.value);
  markKerningDirty();
  syncTextKerningModelToEditor();
  reshapeTextBuffer();
  queueComfyStateSync();
}

function updateGlyphKerningGroup(side: "left" | "right", value: string) {
  const glyphName = currentGlyph.value;
  const data = activeMasterData.value;
  if (!glyphName || !data) return;
  const prefix = side === "left" ? "public.kern1." : "public.kern2.";
  const targetGroup = normalizeKerningGroup(side, value);
  let changed = false;

  for (const [groupName, members] of Array.from(data.groups.entries())) {
    if (!groupName.startsWith(prefix)) continue;
    const nextMembers = members.filter((member) => member !== glyphName);
    if (nextMembers.length !== members.length) {
      changed = true;
      if (nextMembers.length > 0) {
        data.groups.set(groupName, nextMembers);
      } else {
        data.groups.delete(groupName);
      }
    }
  }

  if (targetGroup) {
    const members = data.groups.get(targetGroup) ?? [];
    if (!members.includes(glyphName)) {
      data.groups.set(targetGroup, [...members, glyphName]);
      changed = true;
    }
  }

  if (!changed) return;
  data.glyphKerningGroups = buildGlyphKerningGroups(data.groups);
  masterDataMap.value = new Map(masterDataMap.value);
  markGroupsDirty();
  syncTextKerningModelToEditor();
  reshapeTextBuffer();
  queueComfyStateSync();
}

function syncTextKerningModelToEditor() {
  if (!editor) return;
  try {
    editor.setTextKerningModel(
      JSON.stringify({
        groups: stringArrayMapToRecord(groups.value),
        kerning: nestedNumberMapToRecord(kerning.value),
      }),
    );
    editor.setTextGlyphInventory(
      JSON.stringify({
        unicode: glyphUnicodeMapToRecord(glyphUnicodes.value),
        widths: glyphWidthMapToRecord(glyphMetadataMap.value),
        outlines: glyphOutlineMapToRecord(glyphSvgs.value),
      }),
    );
    refreshTextStateFromEditor(false);
  } catch (e) {
    console.warn("syncing Text model to editor failed:", e);
  }
}

function syncTextKerningModelFromEditor(markDirty = false) {
  if (!editor || !activeMasterData.value) return false;
  try {
    const model = JSON.parse(editor.textKerningModel()) as {
      kerning?: Record<string, Record<string, number>>;
    };
    const nextKerning = recordToNestedNumberMap(model.kerning ?? {});
    if (nestedNumberMapsEqual(activeMasterData.value.kerning, nextKerning)) {
      return false;
    }
    activeMasterData.value.kerning = nextKerning;
    masterDataMap.value = new Map(masterDataMap.value);
    if (markDirty) {
      markKerningDirty();
      queueComfyStateSync();
    }
    return true;
  } catch (e) {
    console.warn("syncing Text kerning model from editor failed:", e);
    return false;
  }
}

function stringArrayMapToRecord(map: Map<string, string[]>): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const [key, value] of map) {
    out[key] = value;
  }
  return out;
}

function nestedNumberMapToRecord(
  map: Map<string, Map<string, number>>,
): Record<string, Record<string, number>> {
  const out: Record<string, Record<string, number>> = {};
  for (const [first, pairs] of map) {
    const pairOut: Record<string, number> = {};
    for (const [second, value] of pairs) {
      pairOut[second] = value;
    }
    out[first] = pairOut;
  }
  return out;
}

function recordToNestedNumberMap(
  record: Record<string, Record<string, number>>,
): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  for (const [first, pairs] of Object.entries(record)) {
    const pairMap = new Map<string, number>();
    for (const [second, value] of Object.entries(pairs ?? {})) {
      if (Number.isFinite(value)) {
        pairMap.set(second, value);
      }
    }
    if (pairMap.size > 0) {
      out.set(first, pairMap);
    }
  }
  return out;
}

function nestedNumberMapsEqual(
  a: Map<string, Map<string, number>>,
  b: Map<string, Map<string, number>>,
): boolean {
  if (a.size !== b.size) return false;
  for (const [first, pairsA] of a) {
    const pairsB = b.get(first);
    if (!pairsB || pairsA.size !== pairsB.size) return false;
    for (const [second, valueA] of pairsA) {
      if (pairsB.get(second) !== valueA) return false;
    }
  }
  return true;
}

function glyphUnicodeMapToRecord(map: Map<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [glyphName, unicode] of map) {
    const codepoint = parseInt(unicode, 16);
    if (Number.isFinite(codepoint)) {
      out[String(codepoint)] = glyphName;
    }
  }
  return out;
}

function glyphWidthMapToRecord(map: Map<string, GlyphMetadata>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [glyphName, metadata] of map) {
    out[glyphName] = metadata.width;
  }
  return out;
}

function glyphOutlineMapToRecord(map: Map<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [glyphName, svg] of map) {
    const path = /<path\b[^>]*\sd="([^"]+)"/.exec(svg)?.[1];
    if (path) {
      out[glyphName] = path;
    }
  }
  return out;
}

async function loadGlifFiles(
  files: File[],
  fileHandles: Map<string, FileSystemFileHandle> = new Map(),
) {
  if (!editor || !canvas.value) return;

  // Reset all selection state regardless of which load path runs.
  currentGlyph.value = "";
  selectedGlyph.value = "";
  selectedGlyphs.value = new Set();
  selectedCategory.value = "All";
  textBuffer.value = [];
  textCursor.value = 0;
  activeTextSortIndex.value = null;
  clearBackgroundImage();
  glyphImageFiles.value = collectGlyphImageFiles(files);
  editor?.clearTextBuffer();
  dirtyGlyphsByMaster.value = new Map();
  dirtyKerningMasters.value = new Set();
  dirtyGroupsMasters.value = new Set();
  lastSavedDisplay.value = null;

  const dsFile = files.find((f) => /\.designspace$/i.test(f.name));
  if (dsFile) {
    await loadDesignspace(dsFile, files, fileHandles);
  } else {
    await loadSingleUfo(files, fileHandles);
  }
  viewMode.value = "grid";
  queueComfyStateSync(true);
}

async function loadDesignspace(
  dsFile: File,
  allFiles: File[],
  fileHandles: Map<string, FileSystemFileHandle>,
) {
  status.value = "parsing designspace…";
  const xml = await dsFile.text();
  const sources = parseDesignspace(xml);
  if (sources.length === 0) {
    status.value = "designspace has no <source> entries";
    return;
  }

  // Designspace dir = everything before the .designspace filename.
  // UFO references in the designspace are relative to this dir.
  const dsPath = relPath(dsFile);
  const dsDir = dsPath.includes("/") ? dsPath.slice(0, dsPath.lastIndexOf("/")) : "";
  const ufoRoots = collectUfoRoots(allFiles);

  const map = new Map<string, MasterData>();
  for (const src of sources) {
    const ufoRel = resolveRelativePath(dsDir, src.filename);
    const ufoRoot = resolveUfoRoot(ufoRel, ufoRoots);
    const ufoFiles = ufoRoot
      ? allFiles.filter((f) => relPath(f).startsWith(`${ufoRoot}/`))
      : [];
    if (ufoFiles.length === 0) {
      console.warn(`master "${src.styleName}" UFO not found at ${ufoRel}`);
      continue;
    }
    status.value = `building master ${src.styleName}…`;
    map.set(src.styleName, await buildMasterData(ufoFiles, fileHandles));
  }

  if (map.size === 0) {
    status.value = "designspace had no resolvable masters";
    return;
  }

  masterDataMap.value = map;
  fontLabel.value = dsFile.name;
  activateMaster(Array.from(map.keys())[0]);
  ensureGridSelection();
  status.value = "ready";
}

function collectUfoRoots(files: File[]): string[] {
  const roots = new Set<string>();
  for (const file of files) {
    const match = relPath(file).match(/^(.*?\.ufo)(?:\/|$)/i);
    if (match?.[1]) {
      roots.add(normalizeRelativePath(match[1]));
    }
  }
  return Array.from(roots).sort();
}

function resolveUfoRoot(requested: string, roots: string[]): string | null {
  const normalized = normalizeRelativePath(requested);
  if (roots.includes(normalized)) {
    return normalized;
  }

  const lower = normalized.toLowerCase();
  const insensitive = roots.filter((root) => root.toLowerCase() === lower);
  if (insensitive.length === 1) {
    return insensitive[0];
  }

  const basename = normalized.split("/").pop()?.toLowerCase();
  if (!basename) return null;
  const basenameMatches = roots.filter(
    (root) => root.split("/").pop()?.toLowerCase() === basename,
  );
  return basenameMatches.length === 1 ? basenameMatches[0] : null;
}

async function loadWorkspaceSlot(slot: string) {
  status.value = `loading workspace ${slot}…`;
  const res = await fetch(`/runebender/workspace/${encodeURIComponent(slot)}`);
  if (!res.ok) {
    status.value = `failed to load workspace ${slot}`;
    return;
  }
  const data = (await res.json()) as {
    slot: string;
    files: Array<{ path: string; text: string }>;
  };
  const files = data.files.map((entry) => {
    const name = entry.path.split("/").pop() ?? entry.path;
    const file = new File([entry.text], name, { type: "text/plain" });
    try {
      Object.defineProperty(file, "webkitRelativePath", {
        value: `${data.slot}/${entry.path}`,
        configurable: true,
      });
    } catch {}
    return file;
  });
  await loadGlifFiles(files);
  fontLabel.value = slot;
}

async function loadSingleUfo(
  files: File[],
  fileHandles: Map<string, FileSystemFileHandle>,
) {
  const glifs = files.filter(
    (f) => /\.glif$/i.test(f.name) && /\/glyphs\//.test(relPath(f)),
  );
  if (glifs.length === 0) {
    status.value =
      files.length === 0
        ? "nothing dropped"
        : `no .glif files found in ${files.length} file(s)`;
    return;
  }

  status.value = `reading ${glifs.length} glyph${glifs.length === 1 ? "" : "s"}…`;
  const data = await buildMasterData(files, fileHandles);

  // Label by the UFO folder name (best-effort).
  const sample = relPath(glifs[0]);
  const ufoMatch = sample.match(/([^/]+\.ufo)\//i);
  fontLabel.value = ufoMatch ? ufoMatch[1] : "";

  // Master name from fontinfo.plist's styleName, or "Regular" fallback.
  const styleName = data.fontInfoBytes
    ? (extractStyleName(data.fontInfoBytes) ?? "Regular")
    : "Regular";

  masterDataMap.value = new Map([[styleName, data]]);
  activateMaster(styleName);
  ensureGridSelection();
  status.value = "ready";
}

/// Read every .glif in `ufoFiles` (filtered to the `glyphs/` layer),
/// parse glyph metadata + render SVGs, and bundle everything along
/// with the matching fontinfo.plist bytes into a MasterData.
async function buildMasterData(
  ufoFiles: File[],
  fileHandles: Map<string, FileSystemFileHandle>,
): Promise<MasterData> {
  const glifs = ufoFiles.filter(
    (f) => /\.glif$/i.test(f.name) && /\/glyphs\//.test(relPath(f)),
  );
  const loaded = await Promise.all(
    glifs.map(async (f) => {
      const bytes = new Uint8Array(await f.arrayBuffer());
      try {
        const info = parseGlyphInfo(bytes);
        return { ...info, bytes, path: relPath(f) };
      } catch (e) {
        console.warn(`skipping malformed glyph ${relPath(f)}:`, e);
        return null;
      }
    }),
  );

  const glyphBytes = new Map<string, Uint8Array>();
  const glyphPaths = new Map<string, string>();
  const glyphFileHandles = new Map<string, FileSystemFileHandle>();
  const glyphUnicodes = new Map<string, string>();
  const glyphMetadata = new Map<string, GlyphMetadata>();
  const glyphCategories = new Map<string, Category>();
  const glyphMarkColors = new Map<string, string>();
  for (const item of loaded) {
    if (!item) continue;
    const { name, unicode, unicodes, markColor, width, contours, bytes, path } = item;
    if (!name) continue;
    glyphBytes.set(name, bytes);
    glyphPaths.set(name, path);
    glyphMetadata.set(name, { name, width, contours, unicode, unicodes });
    if (unicode) glyphUnicodes.set(name, unicode);
    if (markColor) glyphMarkColors.set(name, markColor);
    const cp = unicode ? parseInt(unicode, 16) : NaN;
    const cat = Number.isFinite(cp)
      ? (glyphCategoryForCodepoint(cp) as Category)
      : "Other";
    glyphCategories.set(name, cat);
    const fileHandle = fileHandles.get(path);
    if (fileHandle) {
      glyphFileHandles.set(name, fileHandle);
    }
  }

  const glyphSvgs = await buildGridSvgsForMap(glyphBytes);

  const groupsFile = ufoFiles.find((f) =>
    /\/groups\.plist$/i.test(relPath(f)),
  );
  const groups = groupsFile
    ? parseGroupsPlist(new Uint8Array(await groupsFile.arrayBuffer()))
    : new Map<string, string[]>();
  const groupsPath = groupsFile ? relPath(groupsFile) : inferGroupsPath(glifs);
  const groupsFileHandle = groupsPath ? (fileHandles.get(groupsPath) ?? null) : null;
  const glyphKerningGroups = buildGlyphKerningGroups(groups);
  const kerningFile = ufoFiles.find((f) =>
    /\/kerning\.plist$/i.test(relPath(f)),
  );
  const kerningPath = kerningFile ? relPath(kerningFile) : inferKerningPath(glifs);
  const kerningFileHandle = kerningPath ? (fileHandles.get(kerningPath) ?? null) : null;
  const kerning = kerningFile
    ? parseKerningPlist(new Uint8Array(await kerningFile.arrayBuffer()))
    : new Map<string, Map<string, number>>();

  const fontInfoFile = ufoFiles.find((f) =>
    /\/fontinfo\.plist$/i.test(relPath(f)),
  );
  const fontInfoBytes = fontInfoFile
    ? new Uint8Array(await fontInfoFile.arrayBuffer())
    : null;
  const unitsPerEm = fontInfoBytes ? extractUnitsPerEm(fontInfoBytes) : 1000;

  return {
    glyphBytes,
    glyphPaths,
    glyphFileHandles,
    groupsPath,
    groupsFileHandle,
    kerningPath,
    kerningFileHandle,
    glyphUnicodes,
    glyphMetadata,
    glyphKerningGroups,
    groups,
    kerning,
    glyphSvgs,
    glyphCategories,
    glyphMarkColors,
    fontInfoBytes,
    unitsPerEm,
  };
}

async function buildGridSvgsForMap(
  glyphBytes: Map<string, Uint8Array>,
): Promise<Map<string, string>> {
  const names = Array.from(glyphBytes.keys()).sort();
  const chunkSize = 64;
  const svgs = new Map<string, string>();
  const glyphXmlByName = glyphXmlMapJson(glyphBytes, names);
  for (let i = 0; i < names.length; i += chunkSize) {
    for (let j = i; j < Math.min(i + chunkSize, names.length); j++) {
      const name = names[j];
      const bytes = glyphBytes.get(name);
      if (!bytes) continue;
      try {
        const svg = glifToSvgWithComponents(bytes, glyphXmlByName) || glifToSvg(bytes);
        if (svg) svgs.set(name, svg);
      } catch {
        // Skip malformed glyphs silently.
      }
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  return svgs;
}

function glyphXmlMapJson(
  glyphBytes: Map<string, Uint8Array>,
  sortedNames = Array.from(glyphBytes.keys()).sort(),
): string {
  const decoder = new TextDecoder();
  return JSON.stringify(
    Object.fromEntries(
      sortedNames
        .map((name) => {
          const bytes = glyphBytes.get(name);
          return bytes ? [name, decoder.decode(bytes)] : null;
        })
        .filter((entry): entry is [string, string] => entry !== null),
    ),
  );
}

function parseDesignspace(
  xml: string,
): Array<{ name: string; styleName: string; filename: string }> {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  return Array.from(doc.querySelectorAll("source"))
    .map((el) => {
      const filename =
        el.getAttribute("filename") ??
        el.getAttribute("path") ??
        el.querySelector("filename")?.textContent ??
        "";
      return {
        name: el.getAttribute("name") ?? "",
        styleName:
          el.getAttribute("stylename") ?? el.getAttribute("name") ?? "Master",
        filename: normalizeRelativePath(filename),
      };
    })
    .filter((s) => s.filename);
}

function extractStyleName(fontInfoBytes: Uint8Array): string | null {
  const xml = new TextDecoder().decode(fontInfoBytes);
  const m = /<key>styleName<\/key>\s*<string>([^<]+)<\/string>/.exec(xml);
  return m?.[1] ?? null;
}

function extractUnitsPerEm(fontInfoBytes: Uint8Array): number {
  const xml = new TextDecoder().decode(fontInfoBytes);
  const m = /<key>unitsPerEm<\/key>\s*<(?:integer|real)>([^<]+)<\/(?:integer|real)>/.exec(xml);
  const units = m ? Number(m[1]) : NaN;
  return Number.isFinite(units) && units > 0 ? units : 1000;
}

/// Swap the active master. If a glyph is open in the editor, reload
/// it from the new master's bytes so the canvas tracks the switch.
function activateMaster(name: string) {
  if (!masterDataMap.value.has(name)) return;
  activeMasterName.value = name;
  const data = masterDataMap.value.get(name);
  if (!data || !editor) return;
  syncTextKerningModelToEditor();
  // Push the master's fontinfo so the metric guides reflect it.
  if (data.fontInfoBytes) {
    try {
      editor.setFontInfo(data.fontInfoBytes);
    } catch (e) {
      console.warn("setFontInfo failed:", e);
    }
  }
  // If the editor is showing a glyph that exists in this master,
  // reload it from the master's bytes.
  if (currentGlyph.value && canvas.value) {
    const bytes = data.glyphBytes.get(currentGlyph.value);
    if (bytes) {
      try {
        editor.setGlyphGlifWithComponents(bytes, glyphXmlMapJson(data.glyphBytes));
        coordinateQuadrant.value = "cc";
        currentWidth.value = editor.advanceWidth();
        currentContours.value = editor.contourCount();
        refreshSidebearingsFromEditor();
        refreshSelectionState();
        updateCompatibilityErrors();
        requestRender();
        queueComfyStateSync(true);
      } catch (e) {
        console.warn("reloading glyph for master switch failed:", e);
      }
    }
  }
  ensureGridSelection();
}

function onSelectMaster(index: number) {
  const name = masters.value[index];
  if (name) activateMaster(name);
}

function ensureGridSelection() {
  const names = filteredGlyphNames.value;
  const visibleSelected = Array.from(selectedGlyphs.value).filter((name) =>
    names.includes(name),
  );
  if (selectedGlyph.value && names.includes(selectedGlyph.value)) {
    selectedGlyphs.value = new Set(
      visibleSelected.length > 0 ? visibleSelected : [selectedGlyph.value],
    );
    return;
  }
  if (visibleSelected.length > 0) {
    selectedGlyph.value = visibleSelected[0];
    selectedGlyphs.value = new Set(visibleSelected);
    scrollSelectedGlyphIntoView();
    return;
  }
  setPrimarySelectedGlyph(names[0] ?? "");
}

function onSelectCategory(category: Category) {
  selectedCategory.value = category;
  if (gridView.value) {
    gridView.value.scrollTop = 0;
  }
  ensureGridSelection();
}

function setPrimarySelectedGlyph(name: string) {
  selectedGlyph.value = name;
  selectedGlyphs.value = new Set(name ? [name] : []);
  scrollSelectedGlyphIntoView();
}

function selectGlyph(name: string, event?: MouseEvent) {
  if (!event?.shiftKey) {
    setPrimarySelectedGlyph(name);
    return;
  }

  const names = filteredGlyphNames.value;
  const anchor = selectedGlyph.value;
  const anchorIndex = anchor ? names.indexOf(anchor) : -1;
  const targetIndex = names.indexOf(name);
  if (anchorIndex >= 0 && targetIndex >= 0) {
    const lo = Math.min(anchorIndex, targetIndex);
    const hi = Math.max(anchorIndex, targetIndex);
    const next = new Set(selectedGlyphs.value);
    for (const glyphName of names.slice(lo, hi + 1)) {
      next.add(glyphName);
    }
    selectedGlyphs.value = next;
  } else {
    const next = new Set(selectedGlyphs.value);
    if (next.has(name)) {
      next.delete(name);
    } else {
      next.add(name);
    }
    selectedGlyphs.value = next;
    if (!selectedGlyph.value || !next.has(selectedGlyph.value)) {
      selectedGlyph.value = next.values().next().value ?? "";
    }
  }
  scrollSelectedGlyphIntoView();
}

function gridColumnCount(): number {
  return glyphGridColumns.value;
}

function scrollSelectedGlyphIntoView() {
  const index = filteredGlyphNames.value.indexOf(selectedGlyph.value);
  if (index < 0) return;
  requestAnimationFrame(() => {
    const cell = gridView.value?.querySelector<HTMLElement>(
      `[data-glyph-index="${index}"]`,
    );
    cell?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });
}

function navigateGridSelection(direction: "left" | "right" | "up" | "down"): boolean {
  const names = filteredGlyphNames.value;
  if (names.length === 0) return false;
  const columns = gridColumnCount();
  const current = selectedGlyph.value
    ? names.indexOf(selectedGlyph.value)
    : -1;
  const delta =
    direction === "left"
      ? -1
      : direction === "right"
        ? 1
        : direction === "up"
          ? -columns
          : columns;
  const nextIndex =
    current < 0 ? 0 : Math.min(names.length - 1, Math.max(0, current + delta));
  setPrimarySelectedGlyph(names[nextIndex]);
  return true;
}

function copyGridGlyph(): boolean {
  const data = activeMasterData.value;
  const name = selectedGlyph.value;
  const bytes = name && data?.glyphBytes.get(name);
  if (!bytes) return false;
  gridGlyphClipboard.value = new Uint8Array(bytes);
  status.value = `copied ${name}`;
  return true;
}

function pasteGridGlyph(): boolean {
  const data = activeMasterData.value;
  const source = gridGlyphClipboard.value;
  if (!data || !source) return false;
  const targets = selectedGridGlyphNames();
  if (targets.length === 0) return false;

  try {
    for (const name of targets) {
      const target = data.glyphBytes.get(name);
      if (!target) continue;
      const bytes = glifWithOutlinesFrom(source, target);
      const info = parseGlyphInfo(bytes);
      data.glyphBytes.set(name, bytes);
      data.glyphMetadata.set(name, {
        name,
        width: info.width,
        contours: info.contours,
        unicode: info.unicode,
        unicodes: info.unicodes,
      });
      if (info.unicode) {
        data.glyphUnicodes.set(name, info.unicode);
      } else {
        data.glyphUnicodes.delete(name);
      }
      const svg = glifToSvgWithComponents(bytes, glyphXmlMapJson(data.glyphBytes)) || glifToSvg(bytes);
      if (svg) data.glyphSvgs.set(name, svg);
      markGlyphDirty(name);
      if (name === currentGlyph.value && editor) {
        editor.setGlyphGlifWithComponents(bytes, glyphXmlMapJson(data.glyphBytes));
        currentWidth.value = editor.advanceWidth();
        currentContours.value = editor.contourCount();
        refreshSidebearingsFromEditor();
        refreshSelectionState();
        updateCompatibilityErrors();
        requestRender();
      }
    }
    masterDataMap.value = new Map(masterDataMap.value);
    status.value = `pasted outlines into ${targets.length} glyph${targets.length === 1 ? "" : "s"}`;
    queueComfyStateSync(true);
    return true;
  } catch (e) {
    console.warn("grid paste failed:", e);
    status.value = `paste failed: ${e}`;
    return false;
  }
}

function selectedGridGlyphNames(): string[] {
  const names = Array.from(selectedGlyphs.value).filter((name) =>
    activeMasterData.value?.glyphBytes.has(name),
  );
  if (names.length > 0) return names;
  return selectedGlyph.value ? [selectedGlyph.value] : [];
}

function handleGridKeyDown(e: KeyboardEvent): boolean {
  if (viewMode.value !== "grid") return false;
  const meta = e.metaKey || e.ctrlKey;
  if (meta && !e.shiftKey && e.key.toLowerCase() === "s") {
    void onSave();
    return true;
  }
  if (meta && !e.shiftKey && e.key.toLowerCase() === "c") {
    return copyGridGlyph();
  }
  if (meta && !e.shiftKey && e.key.toLowerCase() === "v") {
    return pasteGridGlyph();
  }
  if (meta) return false;
  const direction =
    e.key === "ArrowLeft"
      ? "left"
      : e.key === "ArrowRight"
        ? "right"
        : e.key === "ArrowUp"
          ? "up"
          : e.key === "ArrowDown"
            ? "down"
            : null;
  if (direction) {
    return navigateGridSelection(direction);
  }
  if (e.key === "Enter" && selectedGlyph.value) {
    openGlyph(selectedGlyph.value);
    return true;
  }
  return false;
}

function loadGlyphIntoEditor(name: string, options: { fitCanvas?: boolean } = {}) {
  if (!editor || !canvas.value) return;
  const data = activeMasterData.value;
  if (!data) return;
  const bytes = data.glyphBytes.get(name);
  if (!bytes) return;
  try {
    editor.setGlyphGlifWithComponents(bytes, glyphXmlMapJson(data.glyphBytes));
    coordinateQuadrant.value = "cc";
    refreshSelectionState();
    viewMode.value = "editor";
    currentGlyph.value = name;
    selectedGlyph.value = name;
    selectedGlyphs.value = new Set([name]);
    currentWidth.value = editor.advanceWidth();
    currentContours.value = editor.contourCount();
    refreshSidebearingsFromEditor();
    updateCompatibilityErrors();
    void importMatchingGlyphImage(name);
    if (options.fitCanvas) {
      // Canvas was visually hidden; let layout settle before sizing.
      requestAnimationFrame(() => {
        if (!editor || !canvas.value) return;
        handleResize();
        editor.fitToCanvas(canvas.value.width, canvas.value.height);
        requestRender();
      });
    } else {
      requestRender();
    }
    queueComfyStateSync(true);
  } catch (e) {
    console.error(e);
    status.value = `failed to load ${name}: ${e}`;
  }
}

function loadActiveTextSortGlyphIntoEditor() {
  const glyphName = textGlyphNameAt(activeTextSortIndex.value);
  if (glyphName) {
    loadGlyphIntoEditor(glyphName, { fitCanvas: false });
  }
}

function openGlyph(name: string) {
  loadGlyphIntoEditor(name, { fitCanvas: true });
}

/// Apply (or clear) a mark color on the selected glyph. RGBA is
/// the UFO `public.markColor` string "r,g,b,a"; empty string clears.
/// Affects only the active master's MasterData.
function setMarkOnSelected(rgba: string) {
  const data = activeMasterData.value;
  const names = selectedGridGlyphNames();
  if (!data || names.length === 0) return;
  for (const name of names) {
    if (rgba) {
      data.glyphMarkColors.set(name, rgba);
    } else {
      data.glyphMarkColors.delete(name);
    }
  }
  // Trigger reactivity — the inner Map mutation isn't observable;
  // replace the outer masterDataMap reference so dependent computeds
  // (glyphMarkColors, the cells) re-run.
  masterDataMap.value = new Map(masterDataMap.value);
  // If this glyph is open, keep its in-memory .glif bytes aligned.
  // Browser/UFO file writes still land in the later save slice.
  if (names.includes(currentGlyph.value)) {
    syncCurrentGlyphBytesFromEditor();
  }
  for (const name of names) {
    if (name === currentGlyph.value) continue;
    const originalBytes = data.glyphBytes.get(name);
    if (originalBytes) {
      try {
        const bytes = glifWithMarkColor(originalBytes, rgba);
        data.glyphBytes.set(name, bytes);
        const svg = glifToSvg(bytes);
        if (svg) data.glyphSvgs.set(name, svg);
      } catch (e) {
        console.warn("serializing mark color failed:", e);
      }
    }
  }
  for (const name of names) {
    markGlyphDirty(name);
  }
  queueComfyStateSync();
}

function backToGrid() {
  if (editor?.pointerCancel()) {
    syncEditorMutationAfterWasmChange();
  }
  viewMode.value = "grid";
  refreshSelectionState();
}

function onTransform(action: TransformActionId) {
  if (!editor) return;
  const changed =
    action === "flip-h"
      ? editor.flipSelectionHorizontal()
      : action === "flip-v"
        ? editor.flipSelectionVertical()
        : action === "rot-cw"
          ? editor.rotateSelectionClockwise()
          : action === "rot-ccw"
            ? editor.rotateSelectionCounterClockwise()
            : action === "duplicate"
          ? editor.duplicateSelection()
          : action === "duplicate-repeat"
              ? editor.duplicateRepeatSelection()
              : action === "union"
                ? editor.unionSelection()
                : action === "subtract"
                  ? editor.subtractSelection()
                  : action === "intersect"
                    ? editor.intersectSelection()
                    : action === "exclude"
                      ? editor.excludeSelection()
                      : false;
  if (!changed) return;
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  updateCompatibilityErrors();
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
}

function applySelectionEdit(action: "delete" | "toggle-point") {
  if (!editor || !currentGlyph.value) return false;
  const changed =
    action === "delete" ? editor.deleteSelection() : editor.togglePointType();
  if (!changed) return false;
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  updateCompatibilityErrors();
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
  return true;
}

function applyEditorMutation(mutate: () => boolean): boolean {
  if (!editor || !currentGlyph.value) return false;
  const changed = mutate();
  if (!changed) return false;
  syncEditorMutationAfterWasmChange();
  return true;
}

function syncEditorMutationAfterWasmChange(): boolean {
  if (!editor || !currentGlyph.value) return false;
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  updateCompatibilityErrors();
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
  return true;
}

function applyEditorHistoryChange(change: () => boolean): boolean {
  if (!editor || !currentGlyph.value) return false;
  if (!change()) return false;
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  updateCompatibilityErrors();
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
  return true;
}

function copySelection(): boolean {
  if (!editor || selectionCount.value === 0) return false;
  const copied = editor.copySelection();
  if (copied) {
    showClipboardNotice(
      `Copied ${selectionCount.value} selected point${selectionCount.value === 1 ? "" : "s"}`,
    );
  }
  return copied;
}

function pasteSelection(): boolean {
  if (!editor || !currentGlyph.value) return false;
  const changed = editor.pasteSelection();
  if (!changed) return false;
  syncCurrentGlyphBytesFromEditor();
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  currentContours.value = editor.contourCount();
  requestRender();
  queueComfyStateSync();
  showClipboardNotice("Pasted outline selection");
  return true;
}

function handleZoomShortcut(key: string): boolean {
  if (!editor) return false;
  if (key === "+" || key === "=") {
    editor.setZoom(Math.min(editor.zoom() * 1.1, 1e4));
    requestRender();
    return true;
  }
  if (key === "-" || key === "_") {
    editor.setZoom(Math.max(editor.zoom() / 1.1, 1e-3));
    requestRender();
    return true;
  }
  if (key === "0" && canvas.value) {
    editor.fitToCanvas(canvas.value.width, canvas.value.height);
    refreshSelectionState();
    requestRender();
    return true;
  }
  return false;
}

async function onSave() {
  const data = activeMasterData.value;
  if (!data || !activeMasterName.value) return;

  try {
    status.value = "saving…";
    if (currentGlyph.value && editor && !syncCurrentGlyphBytesFromEditor()) {
      status.value = "save failed";
      return;
    }

    let savedGlyphs = 0;
    const unsavedGlyphs: Array<{ masterName: string; glyphName: string }> = [];
    const dirtyEntries = Array.from(dirtyGlyphsByMaster.value.entries());
    for (const [masterName, glyphs] of dirtyEntries) {
      const masterData = masterDataMap.value.get(masterName);
      if (!masterData) continue;
      for (const glyphName of glyphs) {
        if (await persistGlyphData(masterData, glyphName)) {
          clearGlyphDirty(glyphName, masterName);
          savedGlyphs += 1;
        } else {
          unsavedGlyphs.push({ masterName, glyphName });
        }
      }
    }

    if (unsavedGlyphs.length === 1 && dirtyGlyphCount.value === 1) {
      const unsaved = unsavedGlyphs[0];
      const masterData = masterDataMap.value.get(unsaved.masterName);
      if (masterData && (await exportGlyphData(masterData, unsaved.glyphName))) {
        clearGlyphDirty(unsaved.glyphName, unsaved.masterName);
        unsavedGlyphs.length = 0;
        savedGlyphs += 1;
      }
    }

    const unsavedGroups = await persistDirtyGroups();
    const unsavedKerning = await persistDirtyKerning();
    if (unsavedGlyphs.length === 0 && unsavedGroups.length === 0 && unsavedKerning.length === 0) {
      lastSavedDisplay.value = formatLastSavedDisplay();
    }

    const suffix = saveWarningSuffix(unsavedGroups.length === 0, unsavedKerning.length === 0);
    if (unsavedGlyphs.length > 0) {
      status.value = `saved ${savedGlyphs} glyph${savedGlyphs === 1 ? "" : "s"}; ${unsavedGlyphs.length} glyph${unsavedGlyphs.length === 1 ? "" : "s"} not saved (${formatUnsavedGlyphs(unsavedGlyphs)})${suffix}`;
      return;
    }

    if (savedGlyphs > 0) {
      status.value = `saved ${savedGlyphs} glyph${savedGlyphs === 1 ? "" : "s"}${suffix}`;
    } else if (unsavedGroups.length === 0 && unsavedKerning.length === 0) {
      status.value = `saved metadata${suffix}`;
    } else {
      status.value = `save incomplete${suffix}`;
    }
    queueComfyStateSync(true);
  } catch (e) {
    console.warn("save failed:", e);
    status.value = `save failed: ${e}`;
  }
}

async function persistGlyphData(data: MasterData, glyphName: string): Promise<boolean> {
  const bytes = data.glyphBytes.get(glyphName);
  if (!bytes) return false;

  const slotPath = data.glyphPaths.get(glyphName);
  if (currentFontPath.value && slotPath) {
    const body = new FormData();
    body.append("path", slotPath);
    body.append("text", new TextDecoder().decode(bytes));
    const res = await fetch("/runebender/workspace/write", {
      method: "POST",
      body,
    });
    return res.ok;
  }

  const fileHandle = data.glyphFileHandles.get(glyphName);
  if (!fileHandle) return false;
  const writable = await fileHandle.createWritable();
  await writable.write(bytes);
  await writable.close();
  return true;
}

async function exportGlyphData(data: MasterData, glyphName: string): Promise<boolean> {
  const bytes = data.glyphBytes.get(glyphName);
  if (!bytes) return false;

  const suggestedName = `${glyphName}.glif`;
  const picker = (window as Window & {
    showSaveFilePicker?: SaveFilePicker;
  }).showSaveFilePicker;

  if (picker) {
    const handle = await picker({
      suggestedName,
      types: [
        {
          description: "UFO GLIF",
          accept: {
            "application/xml": [".glif"],
            "text/xml": [".glif"],
          },
        },
      ],
      excludeAcceptAllOption: false,
    });
    const writable = await handle.createWritable();
    await writable.write(bytes);
    await writable.close();
    return true;
  }

  const blob = new Blob([bytes], { type: "application/xml" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = suggestedName;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 0);
  return true;
}

async function exportTextFile(
  text: string,
  suggestedName: string,
  description: string,
  extensions: string[],
): Promise<boolean> {
  const picker = (window as Window & {
    showSaveFilePicker?: SaveFilePicker;
  }).showSaveFilePicker;

  if (picker) {
    const handle = await picker({
      suggestedName,
      types: [
        {
          description,
          accept: {
            "application/xml": extensions,
            "text/xml": extensions,
          },
        },
      ],
      excludeAcceptAllOption: false,
    });
    const writable = await handle.createWritable();
    await writable.write(new TextEncoder().encode(text));
    await writable.close();
    return true;
  }

  const blob = new Blob([text], { type: "application/xml" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = suggestedName;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 0);
  return true;
}

function filenameFromPath(path: string | null, fallback: string): string {
  if (!path) return fallback;
  return path.split("/").filter(Boolean).pop() ?? fallback;
}

function saveWarningSuffix(groupsSaved: boolean, kerningSaved: boolean): string {
  const missing = [];
  if (!groupsSaved) missing.push("groups");
  if (!kerningSaved) missing.push("kerning");
  return missing.length ? `; ${missing.join(" and ")} not saved` : "";
}

function formatLastSavedDisplay(date = new Date()): string {
  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatUnsavedGlyphs(
  glyphs: Array<{ masterName: string; glyphName: string }>,
): string {
  const shown = glyphs
    .slice(0, 4)
    .map(({ masterName, glyphName }) => `${masterName}/${glyphName}`);
  const remaining = glyphs.length - shown.length;
  return remaining > 0 ? `${shown.join(", ")} +${remaining} more` : shown.join(", ");
}

async function persistDirtyGroups(): Promise<string[]> {
  const unsaved: string[] = [];
  for (const masterName of Array.from(dirtyGroupsMasters.value)) {
    const data = masterDataMap.value.get(masterName);
    if (!data || !(await persistGroupsData(data, masterName))) {
      unsaved.push(masterName);
    } else {
      clearGroupsDirty(masterName);
    }
  }
  return unsaved;
}

async function persistGroupsData(data: MasterData, masterName = activeMasterName.value): Promise<boolean> {
  if (!dirtyGroupsMasters.value.has(masterName)) {
    return true;
  }

  const text = serializeGroupsPlist(data.groups);

  if (currentFontPath.value && data.groupsPath) {
    const body = new FormData();
    body.append("path", data.groupsPath);
    body.append("text", text);
    const res = await fetch("/runebender/workspace/write", {
      method: "POST",
      body,
    });
    return res.ok;
  }

  if (data.groupsFileHandle) {
    const writable = await data.groupsFileHandle.createWritable();
    await writable.write(new TextEncoder().encode(text));
    await writable.close();
    return true;
  }

  if (data.groups.size === 0) {
    return true;
  }

  return exportTextFile(
    text,
    filenameFromPath(data.groupsPath, "groups.plist"),
    "UFO groups plist",
    [".plist"],
  );
}

async function persistDirtyKerning(): Promise<string[]> {
  const unsaved: string[] = [];
  for (const masterName of Array.from(dirtyKerningMasters.value)) {
    const data = masterDataMap.value.get(masterName);
    if (!data || !(await persistKerningData(data, masterName))) {
      unsaved.push(masterName);
    } else {
      clearKerningDirty(masterName);
    }
  }
  return unsaved;
}

async function persistKerningData(data: MasterData, masterName = activeMasterName.value): Promise<boolean> {
  if (!dirtyKerningMasters.value.has(masterName)) {
    return true;
  }

  const text = serializeKerningPlist(data.kerning);

  if (currentFontPath.value && data.kerningPath) {
    const body = new FormData();
    body.append("path", data.kerningPath);
    body.append("text", text);
    const res = await fetch("/runebender/workspace/write", {
      method: "POST",
      body,
    });
    return res.ok;
  }

  if (data.kerningFileHandle) {
    const writable = await data.kerningFileHandle.createWritable();
    await writable.write(new TextEncoder().encode(text));
    await writable.close();
    return true;
  }

  if (data.kerning.size === 0) {
    return true;
  }

  return exportTextFile(
    text,
    filenameFromPath(data.kerningPath, "kerning.plist"),
    "UFO kerning plist",
    [".plist"],
  );
}

function syncCurrentGlyphBytesFromEditor(): boolean {
  if (!editor || !currentGlyph.value) return false;
  const data = activeMasterData.value;
  const originalBytes = data?.glyphBytes.get(currentGlyph.value);
  if (!data || !originalBytes) return false;

  try {
    const markColor = data.glyphMarkColors.get(currentGlyph.value) ?? "";
    const bytes = editor.currentGlyphGlif(originalBytes, markColor);
    const info = parseGlyphInfo(bytes);
    data.glyphBytes.set(currentGlyph.value, bytes);
    data.glyphMetadata.set(currentGlyph.value, {
      name: currentGlyph.value,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    });
    if (info.unicode) {
      data.glyphUnicodes.set(currentGlyph.value, info.unicode);
    } else {
      data.glyphUnicodes.delete(currentGlyph.value);
    }
    const svg = glifToSvg(bytes);
    if (svg) data.glyphSvgs.set(currentGlyph.value, svg);
    currentWidth.value = info.width;
    currentContours.value = info.contours;
    refreshSidebearingsFromEditor();
    masterDataMap.value = new Map(masterDataMap.value);
    updateCompatibilityErrors();
    if (textBuffer.value.length > 0) {
      syncTextKerningModelToEditor();
    }
    queueComfyStateSync();
    return true;
  } catch (e) {
    console.warn("serializing current glyph failed:", e);
    return false;
  }
}

function markGlyphDirty(glyphName: string) {
  if (!glyphName || !activeMasterName.value) return;
  const next = new Map(dirtyGlyphsByMaster.value);
  const glyphs = new Set(next.get(activeMasterName.value) ?? []);
  glyphs.add(glyphName);
  next.set(activeMasterName.value, glyphs);
  dirtyGlyphsByMaster.value = next;
}

function markKerningDirty() {
  if (!activeMasterName.value) return;
  const next = new Set(dirtyKerningMasters.value);
  next.add(activeMasterName.value);
  dirtyKerningMasters.value = next;
}

function markGroupsDirty() {
  if (!activeMasterName.value) return;
  const next = new Set(dirtyGroupsMasters.value);
  next.add(activeMasterName.value);
  dirtyGroupsMasters.value = next;
}

function clearKerningDirty(masterName = activeMasterName.value) {
  if (!masterName) return;
  const next = new Set(dirtyKerningMasters.value);
  next.delete(masterName);
  dirtyKerningMasters.value = next;
}

function clearGroupsDirty(masterName = activeMasterName.value) {
  if (!masterName) return;
  const next = new Set(dirtyGroupsMasters.value);
  next.delete(masterName);
  dirtyGroupsMasters.value = next;
}

function clearGlyphDirty(glyphName: string, masterName = activeMasterName.value) {
  if (!glyphName || !masterName) return;
  const next = new Map(dirtyGlyphsByMaster.value);
  const glyphs = new Set(next.get(masterName) ?? []);
  glyphs.delete(glyphName);
  if (glyphs.size === 0) {
    next.delete(masterName);
  } else {
    next.set(masterName, glyphs);
  }
  dirtyGlyphsByMaster.value = next;
}

async function filesFromDirectoryHandle(
  handle: FileSystemDirectoryHandle,
  prefix: string,
): Promise<{ files: File[]; fileHandles: Map<string, FileSystemFileHandle> }> {
  const out: File[] = [];
  const fileHandles = new Map<string, FileSystemFileHandle>();
  const dirHandle = handle as FileSystemDirectoryHandle & {
    entries: () => AsyncIterable<[string, FileSystemHandle]>;
  };
  for await (const [name, entry] of dirHandle.entries()) {
    const path = `${prefix}/${name}`;
    if (entry.kind === "file") {
      const fileHandle = entry as FileSystemFileHandle;
      const file = await fileHandle.getFile();
      try {
        Object.defineProperty(file, "webkitRelativePath", {
          value: path,
          configurable: true,
        });
      } catch {}
      out.push(file);
      fileHandles.set(path, fileHandle);
    } else {
      const nested = await filesFromDirectoryHandle(
        entry as FileSystemDirectoryHandle,
        path,
      );
      out.push(...nested.files);
      for (const [nestedPath, nestedHandle] of nested.fileHandles) {
        fileHandles.set(nestedPath, nestedHandle);
      }
    }
  }
  return { files: out, fileHandles };
}

// ---------------------------------------------------------------------
// Drag-drop
// ---------------------------------------------------------------------

function relPath(f: File): string {
  return normalizeRelativePath(
    (f as File & { webkitRelativePath?: string }).webkitRelativePath ?? f.name,
  );
}

function normalizeRelativePath(path: string): string {
  const parts: string[] = [];
  for (const part of path.replace(/\\/g, "/").split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      parts.pop();
      continue;
    }
    parts.push(part);
  }
  return parts.join("/");
}

function resolveRelativePath(base: string, path: string): string {
  return normalizeRelativePath(base ? `${base}/${path}` : path);
}

function inferKerningPath(glifs: File[]): string | null {
  const sample = glifs[0] ? relPath(glifs[0]) : "";
  const match = sample.match(/^(.*?\.ufo)\//i);
  return match ? `${match[1]}/kerning.plist` : null;
}

function inferGroupsPath(glifs: File[]): string | null {
  const sample = glifs[0] ? relPath(glifs[0]) : "";
  const match = sample.match(/^(.*?\.ufo)\//i);
  return match ? `${match[1]}/groups.plist` : null;
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
  e.stopPropagation();
  dragHover.value = true;
}

function onDragLeave() {
  dragHover.value = false;
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  dragHover.value = false;
  const items = e.dataTransfer?.items;
  if (!items) return;

  const collected: File[] = [];
  const collectedFileHandles = new Map<string, FileSystemFileHandle>();
  const itemsCopy = Array.from(items);
  for (const item of itemsCopy) {
    const fsHandleItem = item as DataTransferItem & {
      getAsFileSystemHandle?: () => Promise<FileSystemHandle>;
      webkitGetAsEntry?: () => FsEntry | null;
    };
    if (fsHandleItem.getAsFileSystemHandle) {
      try {
        const handle = await fsHandleItem.getAsFileSystemHandle();
        if (handle.kind === "file") {
          const fileHandle = handle as FileSystemFileHandle;
          const file = await fileHandle.getFile();
          try {
            Object.defineProperty(file, "webkitRelativePath", {
              value: fileHandle.name,
              configurable: true,
            });
          } catch {}
          collected.push(file);
          collectedFileHandles.set(fileHandle.name, fileHandle);
          continue;
        }
        const nested = await filesFromDirectoryHandle(
          handle as FileSystemDirectoryHandle,
          handle.name,
        );
        collected.push(...nested.files);
        for (const [path, fileHandle] of nested.fileHandles) {
          collectedFileHandles.set(path, fileHandle);
        }
        continue;
      } catch {
        // Fall through to the older entry API.
      }
    }
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
  if (collected.length > 0) {
    const imageFiles = collected.filter(isBackgroundImageFile);
    const hasFontPayload = collected.some((file) =>
      /\.(designspace|glif|plist)$/i.test(file.name),
    );
    if (!hasFontPayload && imageFiles.length > 0 && viewMode.value === "editor") {
      void importBackgroundImage(imageFiles[0]);
      return;
    }
    loadGlifFiles(collected, collectedFileHandles);
  }
}

// Window-level fallback handlers. Without these, dropping a .ufo
// onto any part of the page that isn't the canvas (e.g. the
// drop-hint overlay in grid mode, the toolbar, or the document
// background) can make the browser navigate to a file:// URL.
// Capture-phase handlers make the app claim file drops before the
// browser, a parent page, or an overlay can treat them as navigation.
function hasDroppedFiles(e: DragEvent): boolean {
  const types = e.dataTransfer?.types;
  return Array.from(types ?? []).includes("Files");
}

function onWindowDragOver(e: DragEvent) {
  if (hasDroppedFiles(e)) {
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "copy";
    }
    dragHover.value = true;
  }
}

function onWindowDrop(e: DragEvent) {
  if (hasDroppedFiles(e)) {
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
  if (eventTargetAcceptsText(e)) {
    return;
  }
  if (handleGridKeyDown(e)) {
    e.preventDefault();
    return;
  }

  // Esc returns to the grid view from the editor.
  if (e.key === "Escape" && (backgroundImageContextMenu.value || contourContextMenu.value)) {
    e.preventDefault();
    dismissBackgroundImageContextMenu();
    dismissContourContextMenu();
    return;
  }
  if (e.key === "Escape" && viewMode.value === "editor") {
    e.preventDefault();
    backToGrid();
    return;
  }

  // Undo/redo only apply in the editor.
  if (viewMode.value !== "editor") return;

  const meta = e.metaKey || e.ctrlKey;
  if ((activeTool.value === "Shapes" || activeTool.value === "Knife") && e.key === "Shift") {
    const changed =
      activeTool.value === "Shapes"
        ? editor.setShapeShiftLocked(true)
        : editor.setKnifeShiftLocked(true);
    if (changed) {
      requestRender();
    }
  }
  if (handleTextToolKey(e)) {
    e.preventDefault();
    return;
  }

  if (e.key === "Enter" && activeTool.value !== "Text") {
    e.preventDefault();
    backToGrid();
    return;
  }

  if (e.key === "Tab" && activeTool.value !== "Text") {
    e.preventDefault();
    editorPanelsVisible.value = !editorPanelsVisible.value;
    return;
  }

  if (e.ctrlKey && !e.metaKey && e.key === " ") {
    if (activeTool.value !== "Preview") {
      e.preventDefault();
      onToolSelect("Preview");
      return;
    }
  }

  if (
    e.key === " " &&
    !e.repeat &&
    activeTool.value !== "Text" &&
    temporaryPreviewReturnTool.value === null
  ) {
    e.preventDefault();
    if (activeTool.value !== "Preview") {
      temporaryPreviewReturnTool.value = activeTool.value;
      onToolSelect("Preview");
    }
    return;
  }

  if (activeTool.value !== "Text") {
    const nudge =
      e.key === "ArrowLeft"
        ? [-1, 0]
        : e.key === "ArrowRight"
          ? [1, 0]
          : e.key === "ArrowUp"
            ? [0, 1]
            : e.key === "ArrowDown"
              ? [0, -1]
              : null;
    if (nudge && nudgeSelectedBackgroundImage(nudge[0], nudge[1])) {
      e.preventDefault();
      return;
    }
    if (
      nudge &&
      selectionCount.value > 0 &&
      applyEditorMutation(() =>
        editor.nudgeSelection(nudge[0], nudge[1], e.shiftKey, meta, e.altKey),
      )
    ) {
      e.preventDefault();
      return;
    }
  }

  if (meta && e.key.toLowerCase() === "z") {
    e.preventDefault();
    applyEditorHistoryChange(() => (e.shiftKey ? editor.redo() : editor.undo()));
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "y") {
    e.preventDefault();
    applyEditorHistoryChange(() => editor.redo());
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "i") {
    e.preventDefault();
    backgroundImageInput.value?.click();
  } else if (
    meta &&
    !e.shiftKey &&
    e.key.toLowerCase() === "l" &&
    activeTool.value !== "Text"
  ) {
    if (toggleBackgroundImageLock()) {
      e.preventDefault();
    }
  } else if (meta && e.key.toLowerCase() === "t" && activeTool.value !== "Text") {
    if (reportUnavailableBackgroundTrace("local", e.shiftKey)) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "y" &&
    activeTool.value !== "Text"
  ) {
    if (reportUnavailableBackgroundTrace("quiver", false)) {
      e.preventDefault();
    }
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void onSave();
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "c") {
    if (copySelection()) {
      e.preventDefault();
    }
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "v") {
    if (pasteSelection()) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "h" &&
    activeTool.value !== "Text"
  ) {
    if (applyEditorMutation(() => editor.convertHyperToCubic())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "r" &&
    activeTool.value !== "Text"
  ) {
    if (applyEditorMutation(() => editor.rotateSelectionClockwise())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "l" &&
    activeTool.value !== "Text"
  ) {
    if (applyEditorMutation(() => editor.rotateSelectionCounterClockwise())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.key.toLowerCase() === "d" &&
    activeTool.value !== "Text"
  ) {
    const duplicate = e.shiftKey
      ? () => editor.duplicateRepeatSelection()
      : () => editor.duplicateSelection();
    if (applyEditorMutation(duplicate)) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "o" &&
    activeTool.value !== "Text"
  ) {
    if (applyEditorMutation(() => editor.unionSelection())) {
      e.preventDefault();
    }
  } else if (meta && handleZoomShortcut(e.key)) {
    e.preventDefault();
  } else if (e.key === "Backspace" || e.key === "Delete") {
    if (deleteSelectedBackgroundImage()) {
      e.preventDefault();
    } else if (selectionCount.value > 0 && applySelectionEdit("delete")) {
      e.preventDefault();
    }
  } else if (!meta && !e.shiftKey && e.key.toLowerCase() === "t" && selectionCount.value > 0) {
    if (applySelectionEdit("toggle-point")) {
      e.preventDefault();
    }
  } else if (!meta && e.shiftKey && e.key.toLowerCase() === "h" && activeTool.value !== "Text") {
    if (applyEditorMutation(() => editor.flipSelectionHorizontal())) {
      e.preventDefault();
    }
  } else if (!meta && e.shiftKey && e.key.toLowerCase() === "v" && activeTool.value !== "Text") {
    if (applyEditorMutation(() => editor.flipSelectionVertical())) {
      e.preventDefault();
    }
  } else if (!meta && !e.shiftKey && e.key.toLowerCase() === "r" && activeTool.value !== "Text") {
    if (applyEditorMutation(() => editor.reverseContours())) {
      e.preventDefault();
    }
  } else if (!meta && !e.shiftKey) {
    const key = e.key.toLowerCase();
    const tool =
      key === "v"
        ? "Select"
        : key === "p"
          ? "Pen"
          : key === "h"
            ? "HyperPen"
            : key === "k"
              ? "Knife"
              : key === "t"
                ? "Text"
                : null;
    if (tool) {
      e.preventDefault();
      onToolSelect(tool);
    }
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (!editor || viewMode.value !== "editor" || eventTargetAcceptsText(e)) {
    return;
  }
  if ((activeTool.value === "Shapes" || activeTool.value === "Knife") && e.key === "Shift") {
    const changed =
      activeTool.value === "Shapes"
        ? editor.setShapeShiftLocked(false)
        : editor.setKnifeShiftLocked(false);
    if (changed) {
      requestRender();
    }
    return;
  }
  if (activeTool.value === "Text") {
    return;
  }
  if (e.key !== " ") return;
  e.preventDefault();
  const previous = temporaryPreviewReturnTool.value;
  temporaryPreviewReturnTool.value = null;
  if (previous && activeTool.value === "Preview") {
    onToolSelect(previous);
  }
}

onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  if (comfySyncTimer !== null) {
    clearTimeout(comfySyncTimer);
    comfySyncTimer = null;
  }
  if (clipboardNoticeTimer !== null) {
    window.clearTimeout(clipboardNoticeTimer);
    clipboardNoticeTimer = null;
  }
  resizeObserver?.disconnect();
  clearBackgroundImage();
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("keyup", onKeyUp);
  window.removeEventListener("pointerdown", onWindowPointerDown);
  window.removeEventListener("dragenter", onWindowDragOver, { capture: true });
  window.removeEventListener("dragover", onWindowDragOver, { capture: true });
  window.removeEventListener("drop", onWindowDrop, { capture: true });
  editor?.free();
  editor = null;
});
</script>

<template>
  <div class="runebender-host" :style="chromeStyle">
    <input
      ref="backgroundImageInput"
      class="hidden-file-input"
      type="file"
      accept="image/png,image/jpeg"
      @change="onBackgroundImageInput"
    />
    <input
      ref="fontDirectoryInput"
      class="hidden-file-input"
      type="file"
      multiple
      webkitdirectory
      @change="onFontDirectoryInput"
    />
    <TopBar
      v-if="glyphNames.length > 0 && viewMode === 'grid'"
      :font-label="fontLabel"
      :unsaved="hasDirtyChanges"
      :last-saved="lastSavedDisplay"
      :masters="masters"
      :active-master="activeMasterIndex"
      :master-previews="masterPreviewSvgs"
      :save-enabled="glyphNames.length > 0"
      @select-master="onSelectMaster"
      @save="onSave"
    />

    <!-- Content row: left rail switches based on view mode
         (categories+marks in grid, tool palette in editor). The
         stage holds the canvas + grid. Right sidebar shows glyph
         info whenever a font is loaded. -->
    <div class="content">
      <div v-if="glyphNames.length > 0 && viewMode === 'grid'" class="left-col">
          <CategorySidebar
            :selected="selectedCategory"
            :counts="categoryCounts"
            @select="onSelectCategory"
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
            'is-hidden': viewMode !== 'editor' && glyphNames.length > 0,
            'text-buffer-visible': viewMode === 'editor' && activeTool === 'Text',
          }"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerCancel"
          @dblclick="onCanvasDoubleClick"
          @wheel.prevent="onWheel"
          @contextmenu.prevent="onCanvasContextMenu"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
        />

        <div
          ref="gridView"
          v-if="viewMode === 'grid' && glyphNames.length > 0"
          class="grid-view"
          :style="gridStyle"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
        >
          <GlyphCell
            v-for="item in gridGlyphItems"
            :key="item.name"
            :data-glyph-index="item.index"
            :name="item.name"
            :unicode="glyphUnicodes.get(item.name)"
            :svg="glyphSvgs.get(item.name)"
            :selected="selectedGlyphs.has(item.name)"
            :column-span="item.columnSpan"
            :mark-color="glyphMarkColors.get(item.name)"
            @click="selectGlyph(item.name, $event)"
            @dblclick="openGlyph(item.name)"
          />
        </div>

        <div
          v-if="viewMode === 'editor'"
          class="editor-tools-overlay"
        >
          <EditModeToolbar
            :active="activeTool"
            @select="onToolSelect"
          />
          <ShapesToolbar
            v-if="activeTool === 'Shapes'"
            :active="activeShape"
            @select="onShapeSelect"
          />
          <TextDirectionToolbar
            v-if="activeTool === 'Text'"
            :active="textDirection"
            @select="onTextDirectionSelect"
          />
        </div>

        <div
          v-if="viewMode === 'editor'"
          class="workspace-overlay"
        >
          <MasterToolbar
            v-if="masters.length > 1"
            :masters="masters"
            :active-master="activeMasterIndex"
            :previews="masterPreviewSvgs"
            @select-master="onSelectMaster"
          />
          <WorkspaceToolbar @glyph-grid="backToGrid" />
        </div>

        <WelcomePanel
          v-if="glyphNames.length === 0"
          @open-ufo="openFontDirectoryPicker"
        />

        <div
          v-if="viewMode === 'editor' && backgroundImage"
          class="background-image"
          :class="{ locked: backgroundImage.locked, selected: backgroundImage.selected }"
          :style="backgroundImageFrame"
          @pointerdown="onBackgroundPointerDown"
          @pointermove="onBackgroundPointerMove"
          @pointerup="onBackgroundPointerUp"
          @pointercancel="onBackgroundPointerUp"
          @contextmenu="openBackgroundImageContextMenu"
        >
          <img :src="backgroundImage.url" alt="" draggable="false" />
          <template v-if="!backgroundImage.locked && backgroundImage.selected">
            <span
              v-for="handle in ['tl', 'tr', 'bl', 'br', 'top', 'bottom', 'left', 'right'] as const"
              :key="handle"
              class="background-image-handle"
              :class="handle"
              @pointerdown="onBackgroundResizePointerDown(handle, $event)"
              @pointermove="onBackgroundResizePointerMove"
              @pointerup="onBackgroundResizePointerUp"
              @pointercancel="onBackgroundResizePointerUp"
            />
          </template>
        </div>

        <div
          v-if="backgroundImageContextMenu"
          class="background-image-menu"
          :style="{
            left: `${backgroundImageContextMenu.x}px`,
            top: `${backgroundImageContextMenu.y}px`,
          }"
          role="menu"
          @pointerdown.stop
          @contextmenu.prevent.stop
        >
          <button
            type="button"
            role="menuitem"
            @click="applyBackgroundImageContextMenuAction"
          >
            {{ backgroundImageContextMenu.locked ? "Unlock Image" : "Lock Image" }}
          </button>
        </div>

        <div
          v-if="contourContextMenu"
          class="context-menu"
          :style="{
            left: `${contourContextMenu.x}px`,
            top: `${contourContextMenu.y}px`,
          }"
          role="menu"
          @pointerdown.stop
          @contextmenu.prevent.stop
        >
          <button
            v-if="contourContextMenu.canSetStart"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('set-start')"
          >
            Set Start Point
          </button>
          <button
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('reverse')"
          >
            Reverse Contour
          </button>
          <button
            v-if="contourContextMenu.canMoveUp"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('move-up')"
          >
            Move Contour Up ({{ contourContextMenu.pathIndex }} -> {{ contourContextMenu.pathIndex - 1 }})
          </button>
          <button
            v-if="contourContextMenu.canMoveDown"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('move-down')"
          >
            Move Contour Down ({{ contourContextMenu.pathIndex }} -> {{ contourContextMenu.pathIndex + 1 }})
          </button>
        </div>

        <template v-if="viewMode === 'editor' && compatErrors.length">
          <span
            v-for="(marker, index) in compatMarkers"
            :key="`compat-${index}-${marker.masterName}-${marker.contourIndex ?? 'c'}-${marker.pointIndex ?? 'p'}`"
            class="compat-marker"
            :style="{ left: `${marker.screenX}px`, top: `${marker.screenY}px` }"
            :title="marker.message"
            aria-hidden="true"
          />
          <div class="compat-badge" role="status" aria-live="polite">
            <strong>
              {{ compatErrors.length }} interpolation
              {{ compatErrors.length === 1 ? "error" : "errors" }}
            </strong>
            <span
              v-for="(error, index) in compatErrors.slice(0, 4)"
              :key="`compat-error-${index}-${error.masterName}`"
            >
              {{ error.message }}
            </span>
            <span v-if="compatErrors.length > 4">
              and {{ compatErrors.length - 4 }} more
            </span>
          </div>
        </template>

        <div
          v-if="viewMode === 'editor' && editorPanelsVisible && currentGlyph && activeTool !== 'Text'"
          class="glyph-preview-overlay"
          :style="activeGlyphPreviewStyle"
          aria-label="Active glyph preview"
        >
          <span
            v-if="activeGlyphSvg"
            class="glyph-preview-shape"
            v-html="activeGlyphSvg"
          />
        </div>

        <div
          v-if="viewMode === 'editor' && editorPanelsVisible && activeGlyphPanelVisible"
          class="active-glyph-overlay"
          :class="{ 'text-mode': activeTool === 'Text' }"
          aria-label="Active glyph metrics"
        >
          <div class="active-glyph-row top">
            <input
              type="text"
              :value="currentGlyph"
              aria-label="Glyph name"
              @change="onActiveGlyphNameChange"
              @keydown.enter.prevent="onActiveGlyphNameChange"
            />
            <input
              type="text"
              :value="activeGlyphUnicode ?? ''"
              aria-label="Unicode"
              @change="onActiveGlyphUnicodeChange"
              @keydown.enter.prevent="onActiveGlyphUnicodeChange"
            />
          </div>
          <div class="active-glyph-row kern-sidebearings">
            <input
              type="number"
              :value="activeLeftKern ?? ''"
              aria-label="Left kern"
              placeholder="Kern"
              :disabled="activeTool !== 'Text' || !canEditActiveLeftKern"
              @change="updateActiveTextKern('left', ($event.target as HTMLInputElement).value)"
              @keydown.enter.prevent="updateActiveTextKern('left', ($event.target as HTMLInputElement).value)"
            />
            <input
              type="number"
              :value="Math.round(currentLeftSidebearing)"
              aria-label="Left sidebearing"
              @change="onActiveGlyphSidebearingChange('left', $event)"
              @keydown.enter.prevent="onActiveGlyphSidebearingChange('left', $event)"
            />
            <input
              type="number"
              :value="Math.round(currentRightSidebearing)"
              aria-label="Right sidebearing"
              @change="onActiveGlyphSidebearingChange('right', $event)"
              @keydown.enter.prevent="onActiveGlyphSidebearingChange('right', $event)"
            />
            <input
              type="number"
              :value="activeRightKern ?? ''"
              aria-label="Right kern"
              placeholder="Kern"
              :disabled="activeTool !== 'Text' || !canEditActiveRightKern"
              @change="updateActiveTextKern('right', ($event.target as HTMLInputElement).value)"
              @keydown.enter.prevent="updateActiveTextKern('right', ($event.target as HTMLInputElement).value)"
            />
          </div>
          <div class="active-glyph-row metrics">
            <input
              type="text"
              :value="displayKerningGroup(activeGlyphKerningGroups?.left, 'public.kern1.')"
              aria-label="Left kerning group"
              @change="updateGlyphKerningGroup('left', ($event.target as HTMLInputElement).value)"
              @keydown.enter.prevent="updateGlyphKerningGroup('left', ($event.target as HTMLInputElement).value)"
            />
            <input
              type="number"
              :value="Math.round(currentWidth)"
              aria-label="Advance width"
              @change="onActiveGlyphWidthChange"
              @keydown.enter.prevent="onActiveGlyphWidthChange"
            />
            <input
              type="text"
              :value="displayKerningGroup(activeGlyphKerningGroups?.right, 'public.kern2.')"
              aria-label="Right kerning group"
              @change="updateGlyphKerningGroup('right', ($event.target as HTMLInputElement).value)"
              @keydown.enter.prevent="updateGlyphKerningGroup('right', ($event.target as HTMLInputElement).value)"
            />
          </div>
        </div>

        <div
          v-if="viewMode === 'editor' && clipboardNotice"
          class="clipboard-notice"
          role="status"
          aria-live="polite"
        >
          {{ clipboardNotice }}
        </div>

        <CoordinatePanel
          v-if="viewMode === 'editor' && editorPanelsVisible"
          class="coordinate-overlay"
          :value="selectedBounds"
          :selection-count="selectionCount"
          :quadrant="coordinateQuadrant"
          @select-quadrant="onCoordinateQuadrant"
          @change-coordinate="onCoordinateChange"
        />

        <TransformPanel
          v-if="viewMode === 'editor' && editorPanelsVisible"
          class="transform-overlay"
          :bounds="selectedBounds"
          :contour-count="currentContours"
          @transform="onTransform"
        />

        <div
          v-if="viewMode === 'editor' && measureInfo"
          class="measure-overlay"
          :style="{ left: `${measureInfo.x}px`, top: `${measureInfo.y}px` }"
        >
          <span>{{ formatMeasure(measureInfo.distance) }}</span>
          <span>{{ formatMeasure(measureInfo.angle) }}deg</span>
        </div>

        <div
          v-for="(label, index) in measureInfo?.labels ?? []"
          :key="`measure-${index}`"
          class="measure-overlay measure-segment"
          :style="{ left: `${label.x}px`, top: `${label.y}px` }"
        >
          <span>{{ formatMeasure(label.length) }}</span>
        </div>

        <div
          v-if="viewMode === 'editor' && activeTool === 'Text'"
          class="text-buffer-panel"
          dir="ltr"
          @click.self="setTextCursor(textBuffer.length)"
        >
          <div
            class="text-layout-surface"
            :style="{
              width: `${textPreviewWidth}px`,
              height: `${textPreviewHeight}px`,
            }"
          >
            <span
              class="text-cursor"
              :style="textCursorStyle()"
              role="presentation"
            />
            <button
              v-for="item in textLayoutItems"
              :key="`sort-${item.index}-${item.sort.glyphName}`"
              type="button"
              class="text-sort"
              :class="{
                active: item.sort.glyphName === currentGlyph,
                selected: item.index === activeTextSortIndex,
              }"
              :style="textLayoutItemStyle(item)"
              :title="item.sort.glyphName"
              :aria-label="item.sort.glyphName"
              :aria-pressed="item.index === activeTextSortIndex"
              @click.stop="activateTextSort(item.index)"
              @dblclick.stop="openTextSort(item.sort)"
            >
              <span
                v-if="glyphSvgs.get(item.sort.glyphName)"
                class="text-sort-glyph"
                v-html="glyphSvgs.get(item.sort.glyphName)"
              />
              <span v-else class="text-sort-fallback">{{ item.sort.char }}</span>
            </button>
            <span
              v-if="!textLayoutItems.length"
              class="text-empty"
            >
              Text
            </span>
          </div>
        </div>
      </div>

      <div v-if="glyphNames.length > 0 && viewMode === 'grid'" class="right-col">
        <GlyphInfoSidebar
          :master="masters.length > 1 ? activeMasterName : '(single UFO)'"
          :name="selectedGlyph"
          :unicode="selectedUnicodeDisplay"
          :width="selectedWidth"
          :contours="selectedContours"
          :left-group="selectedKerningGroups?.left"
          :right-group="selectedKerningGroups?.right"
        />
        <GlyphAnatomyPanel
          :name="selectedGlyph"
          :svg="selectedAnatomySvg"
        />
      </div>
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
  background: var(--rb-app-background, #101010);
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-sizing: border-box;
}

.hidden-file-input {
  display: none;
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

/* Right column: info on top, anatomy fills remaining height. */
.right-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
  width: 220px;
}
.right-col > :deep(.info-sidebar) {
  width: auto;
  flex-shrink: 0;
}

/* Stage = the area inside .content where canvas + grid live,
   stacked on the same coordinates. */
.stage {
  position: relative;
  flex: 1;
  min-width: 0;
}

.coordinate-overlay {
  position: absolute;
  right: 12px;
  bottom: 12px;
  z-index: 3;
}

.transform-overlay {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
  z-index: 3;
}

.editor-tools-overlay {
  position: absolute;
  left: 12px;
  top: 12px;
  z-index: 4;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0;
  pointer-events: auto;
}

.workspace-overlay {
  position: absolute;
  right: 12px;
  top: 12px;
  z-index: 4;
  display: flex;
  gap: 6px;
  align-items: flex-start;
}

.background-image {
  position: absolute;
  z-index: 1;
  user-select: none;
  touch-action: none;
  cursor: move;
  border: 1.5px solid transparent;
  box-sizing: border-box;
}
.background-image img {
  width: 100%;
  height: 100%;
  display: block;
  opacity: 0.3;
  object-fit: fill;
  pointer-events: none;
}
.background-image.locked {
  border-color: transparent;
  cursor: default;
}
.background-image.selected {
  border-color: var(--rb-background-image-selection, #4488ff);
  border-style: dashed;
  border-width: 2px;
}

.background-image-handle {
  position: absolute;
  width: 10px;
  height: 10px;
  margin: -5px 0 0 -5px;
  border-radius: 50%;
  background: var(--rb-mark-selected-ring, #ffffff);
  border: 1.5px solid var(--rb-background-image-selection, #4488ff);
  box-sizing: border-box;
  touch-action: none;
}
.background-image-handle.tl {
  left: 0;
  top: 0;
  cursor: nwse-resize;
}
.background-image-handle.tr {
  left: 100%;
  top: 0;
  cursor: nesw-resize;
}
.background-image-handle.bl {
  left: 0;
  top: 100%;
  cursor: nesw-resize;
}
.background-image-handle.br {
  left: 100%;
  top: 100%;
  cursor: nwse-resize;
}
.background-image-handle.top {
  left: 50%;
  top: 0;
  border-radius: 2px;
  cursor: ns-resize;
}
.background-image-handle.bottom {
  left: 50%;
  top: 100%;
  border-radius: 2px;
  cursor: ns-resize;
}
.background-image-handle.left {
  left: 0;
  top: 50%;
  border-radius: 2px;
  cursor: ew-resize;
}
.background-image-handle.right {
  left: 100%;
  top: 50%;
  border-radius: 2px;
  cursor: ew-resize;
}

.background-image-menu,
.context-menu {
  position: fixed;
  z-index: 20;
  min-width: 140px;
  padding: 6px;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}
.background-image-menu button,
.context-menu button {
  appearance: none;
  width: 100%;
  height: 28px;
  padding: 0 10px;
  color: var(--rb-overlay-text, #f0f0f0);
  background: transparent;
  border: 0;
  border-radius: 4px;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  text-align: left;
  cursor: pointer;
}
.background-image-menu button:hover,
.context-menu button:hover {
  background: var(--rb-control-background, #303030);
}

.compat-marker {
  position: absolute;
  z-index: 5;
  width: 16px;
  height: 16px;
  margin: -8px 0 0 -8px;
  border: 2px solid var(--rb-danger, #ff3333);
  border-radius: 50%;
  box-sizing: border-box;
  pointer-events: none;
}

.compat-badge {
  position: absolute;
  left: 12px;
  top: 12px;
  z-index: 5;
  width: min(360px, calc(100% - 24px));
  max-height: 160px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  background: color-mix(in srgb, var(--rb-panel-background, #1c1c1c) 94%, transparent);
  border: 1.5px solid var(--rb-danger, #ff3333);
  border-radius: 6px;
  color: var(--rb-overlay-text, #f0f0f0);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  pointer-events: none;
}
.compat-badge strong {
  color: var(--rb-danger-text, #ff7777);
  font-weight: 700;
}
.compat-badge span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.glyph-preview-overlay,
.active-glyph-overlay {
  position: absolute;
  z-index: 3;
  bottom: 12px;
  height: 140px;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
  pointer-events: auto;
}

.glyph-preview-overlay {
  left: 12px;
  width: 140px;
  padding: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.glyph-preview-shape {
  width: 100%;
  height: 100%;
  color: var(--rb-primary-text, #909090);
  display: flex;
  align-items: center;
  justify-content: center;
}
.glyph-preview-shape :deep(svg) {
  width: 100%;
  height: 100%;
}

.active-glyph-overlay {
  left: 50%;
  width: 488px;
  padding: 12px;
  transform: translateX(-50%);
  display: grid;
  align-content: center;
  gap: 8px;
}
.active-glyph-overlay.text-mode {
  bottom: calc(max(96px, 15%) + 18px);
}

.active-glyph-row {
  display: grid;
  gap: 8px;
}
.active-glyph-row.top {
  grid-template-columns: 346px 110px;
}
.active-glyph-row.kern-sidebearings {
  grid-template-columns: repeat(4, 110px);
}
.active-glyph-row.metrics {
  grid-template-columns: 149px 150px 149px;
}
.active-glyph-row output,
.active-glyph-row input {
  height: 30px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: var(--rb-app-background, #101010);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-primary-text, #909090);
  font: 12px ui-monospace, monospace;
  text-align: center;
}
.active-glyph-row input {
  appearance: textfield;
  width: 100%;
}
.active-glyph-row input:disabled {
  color: var(--rb-panel-outline, #606060);
  border-color: var(--rb-control-background, #303030);
}
.measure-overlay {
  position: absolute;
  z-index: 4;
  transform: translate(12px, -50%);
  pointer-events: none;
  display: flex;
  gap: 6px;
  padding: 4px 6px;
  background: var(--rb-app-background, #101010);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-accent, #66ee88);
  font: 11px ui-monospace, monospace;
  white-space: nowrap;
}

.clipboard-notice {
  position: absolute;
  z-index: 5;
  left: 50%;
  top: 12px;
  transform: translateX(-50%);
  max-width: min(360px, calc(100% - 240px));
  padding: 6px 10px;
  box-sizing: border-box;
  background: color-mix(in srgb, var(--rb-panel-background, #1c1c1c) 94%, transparent);
  border: 1.5px solid var(--rb-accent, #66ee88);
  border-radius: 6px;
  color: var(--rb-overlay-text, #f0f0f0);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  pointer-events: none;
}

.text-buffer-panel {
  position: absolute;
  z-index: 4;
  left: 0;
  right: 0;
  bottom: 0;
  height: max(96px, 15%);
  max-height: 180px;
  overflow-x: auto;
  overflow-y: auto;
  padding: 8px;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 0;
  box-sizing: border-box;
}

.text-layout-surface {
  position: relative;
  min-width: 100%;
  min-height: 70px;
}

.text-sort {
  appearance: none;
  position: absolute;
  min-width: 28px;
  max-width: 118px;
  height: 70px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  color: var(--rb-primary-text, #909090);
  background: transparent;
  border: 0;
  border-radius: 0;
  cursor: pointer;
}
.text-sort:hover,
.text-sort.active,
.text-sort.selected {
  color: var(--rb-accent, #66ee88);
}
.text-sort.selected {
  box-shadow: inset 0 -2px 0 var(--rb-accent, #66ee88);
}

.text-sort-glyph {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.text-sort-glyph :deep(svg) {
  width: 100%;
  height: 100%;
}

.text-sort-fallback {
  font: 32px ui-serif, Georgia, serif;
  line-height: 1;
}

.text-cursor {
  position: absolute;
  width: 2px;
  height: 70px;
  background: var(--rb-accent, #66ee88);
  border-radius: 2px;
  pointer-events: none;
}

.text-empty {
  position: absolute;
  left: 10px;
  bottom: 10px;
  color: var(--rb-panel-outline, #606060);
  font: 12px ui-monospace, monospace;
  pointer-events: none;
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
.runebender-canvas.text-buffer-visible {
  bottom: max(96px, 15%);
  height: auto;
}
.runebender-canvas.is-hidden {
  visibility: hidden;
  pointer-events: none;
}
.runebender-canvas.drag-hover {
  outline: 2px dashed var(--rb-accent, #66ee88);
  outline-offset: -2px;
}

/* ----- Grid ----- */
/* BENTO_GAP = 6px from xilem's views/glyph_grid/mod.rs */
.grid-view {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  display: grid;
  grid-auto-rows: 192px;
  gap: 6px;
  background: var(--rb-app-background, #101010);
}
</style>
