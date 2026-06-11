<script setup lang="ts">
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
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
  glifMapToSvgs,
  glifMetadata,
  glifToGridSvgWithComponents,
  glifToSvg,
  glifToSvgWithComponents,
  glifWithKerningGroup,
  glifWithMarkColor,
  glifWithName,
  glifWithOutlinesFrom,
  glifWithUnicode,
  glyphCategoryForCodepoint,
  traceImageToGlifReport,
} from "../wasm/runebender_comfy_core.js";
import CategorySidebar, {
  type Category,
} from "./components/CategorySidebar.vue";
import {
  CATEGORY_GROUPS,
  SIDEBAR_FILTERS,
  SIDEBAR_LANGUAGE_GROUPS,
  type GlyphSidebarFilter,
  type GlyphSortMode,
  type SidebarBuiltinFilter,
  type SidebarCharacterFilter,
  type SidebarSearchMode,
} from "./glyphSidebarData";
import AnchorPanel, {
  type AnchorPanelValue,
} from "./components/AnchorPanel.vue";
import CoordinatePanel from "./components/CoordinatePanel.vue";
import type { CoordinateQuadrant } from "./components/CoordinatePanel.vue";
import EditModeToolbar from "./components/EditModeToolbar.vue";
import GeneratedIcon from "./components/GeneratedIcon.vue";
import { type ToolId } from "./components/toolIds";
import ShapesToolbar from "./components/ShapesToolbar.vue";
import type { ShapeKind } from "./components/ShapesToolbar.vue";
import SystemToolbar from "./components/SystemToolbar.vue";
import TextDirectionToolbar from "./components/TextDirectionToolbar.vue";
import type { TextDirection } from "./components/TextDirectionToolbar.vue";
import GlyphAnatomyPanel from "./components/GlyphAnatomyPanel.vue";
import GlyphCell from "./components/GlyphCell.vue";
import GlyphInfoSidebar from "./components/GlyphInfoSidebar.vue";
import HelperPanel from "./components/HelperPanel.vue";
import MarkColorPanel from "./components/MarkColorPanel.vue";
import MasterToolbar from "./components/MasterToolbar.vue";
import TopBar from "./components/TopBar.vue";
import TransformPanel, {
  type TransformActionId,
} from "./components/TransformPanel.vue";
import WelcomePanel from "./components/WelcomePanel.vue";
import WorkspaceToolbar from "./components/WorkspaceToolbar.vue";
import { runebenderHostKey } from "./host/runebenderHost";
import { browserHost } from "./hosts/browser/browserHost";

const props = defineProps<{
  nodeId?: string;
  fontPathRef?: { value: string };
  initialFiles?: () => Promise<File[]>;
  onGlyphDataChange?: (value: string) => void;
  onWorkspaceSaved?: () => void;
  // Host-provided callback. When set (e.g. ComfyUI embedded mode), the
  // editor shows a Close button next to Save and invokes this on click.
  onCloseRequested?: () => void;
}>();

const runebenderHost = inject(runebenderHostKey, browserHost);
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
const stage = ref<HTMLDivElement | null>(null);
const gridViewportWidth = ref<number>(0);
const gridViewportHeight = ref<number>(0);
const backgroundImageInput = ref<HTMLInputElement | null>(null);
const fontDirectoryInput = ref<HTMLInputElement | null>(null);
const status = ref<string>("initializing");
const lastSavedDisplay = ref<string | null>(null);
const mirroredSaveWrites = ref<number>(0);
const sourceSaveLabel = ref<string | null>(null);
const designspacePath = ref<string | null>(null);
const designspaceText = ref<string>("");
const designspaceFileHandle = ref<FileSystemFileHandle | null>(null);
const designspaceDirty = ref<boolean>(false);
const selectionCount = ref<number>(0);
const selectedContourCount = ref<number>(0);
const currentGlyph = ref<string>("");
const fontLabel = ref<string>("");
const viewMode = ref<"grid" | "editor">("grid");
const selectedCategory = ref<Category>("All");
const selectedSidebarFilter = ref<GlyphSidebarFilter>({ kind: "all" });
const sidebarSearchQuery = ref("");
const sidebarSearchMode = ref<SidebarSearchMode>("all");
const sidebarSearchMatchCase = ref(false);
const sidebarSearchRegex = ref(false);
// Live metadata for the info sidebar — read from the wasm editor
// after each setGlyphGlif call. -1 / 0 means "no glyph loaded yet".
const currentWidth = ref<number>(-1);
const currentContours = ref<number>(0);
// Glyph that's selected (highlighted in the grid, shown in the info
// sidebar, mark-color target). Distinct from `currentGlyph` which
// is the glyph currently loaded into the editor.
const selectedGlyph = ref<string>("");
const selectedGlyphs = ref<Set<string>>(new Set());
let pendingGridSelectionName = "";
let pendingGridSelectionRaf: number | null = null;
let pendingGridScrollIndex = -1;
let pendingGridScrollRaf: number | null = null;
const currentLeftSidebearing = ref<number>(0);
const currentRightSidebearing = ref<number>(0);
// Active tool in the editor view. Tool implementations land
// incrementally in Rust while Vue owns the selected toolbar state.
const activeTool = ref<ToolId>("Select");
const activeShape = ref<ShapeKind>("rectangle");
const textDirection = ref<TextDirection>("ltr");
const hasTextBufferSession = ref<boolean>(false);
const textBuffer = ref<TextSort[]>([]);
const textCursor = ref<number>(0);
const activeTextSortIndex = ref<number | null>(null);
const temporaryPreviewReturnTool = ref<ToolId | null>(null);
let selectIdleHoverActive = false;
let traceMenuLastPointerUpAt = 0;
const coordinateQuadrant = ref<CoordinateQuadrant>("cc");
const editorPanelsVisible = ref<boolean>(true);
const editorBottomPreviewHeight = ref<number>(124);
const editorBottomPreviewDragStart = ref<{ y: number; height: number } | null>(null);
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
  glyphXmlByName: string | null;
  glyphXmlVersion: number;
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
  leftKerningGroup?: string | null;
  rightKerningGroup?: string | null;
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
  file: File;
  width: number;
  height: number;
  designX: number;
  designY: number;
  designScaleX: number;
  designScaleY: number;
  locked: boolean;
  selected: boolean;
  traceXOffset?: number;
  traceYOffset?: number;
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

type ForegroundPixelBounds = {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  width: number;
  height: number;
};

type WasmTraceReport = {
  glif: string;
  contours: number;
  curves: number;
  lines: number;
  onCurves: number;
  offCurves: number;
  advanceWidth: number;
  repositionShiftX: number;
  repositionShiftY: number;
  diagnostics: {
    missedExtremaFixed: number;
    highDeviationSplits: number;
    strongTangentOverrides: number;
    cleanTangentOverrides: number;
    visibleTangentOverrides: number;
    rejectedTangentNearMisses: number;
    oversegmentedSplitsRemoved: number;
    finalOutlineDivergences: number;
    finalOutlineRepairs: number;
  };
};

type ContourContextMenuState = {
  x: number;
  y: number;
  screenX: number;
  screenY: number;
  designX: number;
  designY: number;
  pathIndex: number | null;
  canSetStart: boolean;
  canRoundCorners: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
  canAddAnchor: boolean;
  canEditAnchor: boolean;
  anchorName?: string;
  anchorX?: number;
  anchorY?: number;
};

type AnchorContext = {
  name?: string | null;
  x: number;
  y: number;
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
  hasTextSession: boolean;
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

type TextBufferStateSnapshot = {
  buffer: TextBufferSnapshot;
  layout: TextLayoutSnapshot;
};

const masterDataMap = ref<Map<string, MasterData>>(new Map());
const activeMasterName = ref<string>("");
const selectedBounds = ref<SelectionBounds | undefined>(undefined);
const selectedAnchor = ref<AnchorPanelValue | null>(null);
const measureInfo = ref<MeasureInfo | undefined>(undefined);
const dirtyGlyphsByMaster = ref<Map<string, Set<string>>>(new Map());
const dirtyKerningMasters = ref<Set<string>>(new Set());
const dirtyGroupsMasters = ref<Set<string>>(new Set());
const gridGlyphClipboard = ref<Uint8Array | null>(null);
const markColorApplyAllMasters = ref(false);
const glyphSortMode = ref<GlyphSortMode>("unicode");

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
  activeMasterData.value ? cachedGlyphXmlByName(activeMasterData.value) : "{}",
);
const glyphCategories = computed(
  () => activeMasterData.value?.glyphCategories ?? (new Map<string, Category>()),
);
const glyphMarkColors = computed(
  () => activeMasterData.value?.glyphMarkColors ?? (new Map<string, string>()),
);
function firstGlyphCodepointFromUnicode(unicode?: string): number | null {
  if (!unicode) return null;
  for (const hex of unicode.split(/[\s,]+/)) {
    const codepoint = Number.parseInt(hex.replace(/^U\+/i, ""), 16);
    if (Number.isFinite(codepoint)) return codepoint;
  }
  return null;
}

function compareGlyphNamesByCodepoint(
  a: string,
  b: string,
  unicodes: Map<string, string>,
): number {
  const aCodepoint = firstGlyphCodepointFromUnicode(unicodes.get(a));
  const bCodepoint = firstGlyphCodepointFromUnicode(unicodes.get(b));
  if (aCodepoint !== null && bCodepoint !== null && aCodepoint !== bCodepoint) {
    return aCodepoint - bCodepoint;
  }
  if (aCodepoint !== null && bCodepoint === null) return -1;
  if (aCodepoint === null && bCodepoint !== null) return 1;
  return a.localeCompare(b);
}

const glyphNames = computed(() => {
  const data = activeMasterData.value;
  if (!data) return [];
  return Array.from(data.glyphBytes.keys()).sort((a, b) => {
    if (glyphSortMode.value === "unicode") {
      return compareGlyphNamesByCodepoint(a, b, data.glyphUnicodes);
    }
    return a.localeCompare(b);
  });
});
const masters = computed(() => Array.from(masterDataMap.value.keys()));
const activeMasterIndex = computed(() => masters.value.indexOf(activeMasterName.value));
const masterPreviewSvgs = computed(() =>
  masters.value.map((name) => {
    const data = masterDataMap.value.get(name);
    if (!data) return undefined;
    const bytes = data.glyphBytes.get("n") ?? data.glyphBytes.get("N");
    if (!bytes) return data.glyphSvgs.get("n") ?? data.glyphSvgs.get("N");
    try {
      return glifToSvgWithComponents(bytes, cachedGlyphXmlByName(data)) || glifToSvg(bytes);
    } catch {
      return data.glyphSvgs.get("n") ?? data.glyphSvgs.get("N");
    }
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
    dirtyGroupsMasters.value.size > 0 ||
    designspaceDirty.value,
);
const designspaceSummary = computed(() => {
  if (!designspaceText.value) return "";
  try {
    const doc = new DOMParser().parseFromString(designspaceText.value, "application/xml");
    const parserError = doc.querySelector("parsererror");
    if (parserError) return "Invalid XML";
    const sources = doc.querySelectorAll("source").length;
    const axes = doc.querySelectorAll("axis").length;
    return `${sources} source${sources === 1 ? "" : "s"} · ${axes} axis${axes === 1 ? "" : "es"}`;
  } catch {
    return "Invalid XML";
  }
});

// Names filtered by the Glyphs-style sidebar. The grid renders this
// list instead of glyphNames directly.
const filteredGlyphNames = computed(() =>
  glyphNames.value.filter(
    (name) => glyphMatchesSidebarFilter(name) && glyphMatchesSidebarSearch(name),
  ),
);

const glyphGridColumns = computed(() => {
  // Mirrors xilem's AppState::grid_columns constants.
  const available = Math.max(0, gridViewportWidth.value - 6);
  const columns = Math.floor((available + 6) / (128 + 6));
  return Math.max(1, Math.min(8, columns || 1));
});

const glyphGridRowHeight = computed(() => {
  const bentoGap = 6;
  const targetRowHeight = 192;
  const height = Math.max(0, gridViewportHeight.value);
  if (height <= 0) return targetRowHeight;
  const visibleRows = Math.max(
    1,
    Math.floor((height + bentoGap) / (targetRowHeight + bentoGap)),
  );
  return (height - bentoGap * (visibleRows - 1)) / visibleRows;
});

const gridStyle = computed(() => ({
  gridTemplateColumns: `repeat(${glyphGridColumns.value}, minmax(0, 1fr))`,
  gridAutoRows: `${glyphGridRowHeight.value}px`,
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
  for (const group of CATEGORY_GROUPS) {
    counts[sidebarFilterKey({ kind: "category", category: group.category })] =
      counts[group.category] ?? 0;
    for (const sub of group.subfilters ?? []) {
      counts[
        sidebarFilterKey({
          kind: "category",
          category: group.category,
          subcategory: sub.id,
        })
      ] = glyphNames.value.filter((name) =>
        glyphMatchesCategorySubfilter(name, group.category, sub.id),
      ).length;
    }
  }
  for (const group of SIDEBAR_LANGUAGE_GROUPS) {
    counts[sidebarFilterKey({ kind: "languageGroup", id: group.id })] =
      glyphNames.value.filter((name) =>
        group.filters.some((filter) => glyphMatchesCharacterFilter(name, filter)),
      ).length;
    for (const filter of group.filters) {
      counts[sidebarFilterKey({ kind: "language", id: filter.id })] =
        glyphNames.value.filter((name) => glyphMatchesCharacterFilter(name, filter)).length;
    }
  }
  for (const filter of SIDEBAR_FILTERS) {
    const key =
      filter.source === "google-fonts-glyphsets"
        ? sidebarFilterKey({ kind: "gfGlyphset", id: filter.id })
        : sidebarFilterKey({ kind: "builtin", id: filter.id });
    counts[key] = glyphNames.value.filter((name) => glyphMatchesBuiltinFilter(name, filter))
      .length;
  }
  return counts;
});

function sidebarFilterKey(filter: GlyphSidebarFilter): string {
  if (filter.kind === "all") return "all";
  if (filter.kind === "category") {
    return filter.subcategory
      ? `category:${filter.category}:${filter.subcategory}`
      : `category:${filter.category}`;
  }
  return `${filter.kind}:${filter.id}`;
}

function glyphCodepoints(name: string): number[] {
  const value = glyphUnicodes.value.get(name) ?? "";
  return value
    .split(/[\s,]+/)
    .map((hex) => Number.parseInt(hex.replace(/^U\+/i, ""), 16))
    .filter((cp) => Number.isFinite(cp));
}

function glyphMatchesCharacterFilter(
  name: string,
  filter: SidebarCharacterFilter | SidebarBuiltinFilter,
): boolean {
  if (filter.glyphNames?.includes(name)) return true;
  const codepoints = glyphCodepoints(name);
  if (codepoints.length === 0) return false;
  if (filter.chars) {
    const chars = new Set(Array.from(filter.chars));
    if (codepoints.some((cp) => chars.has(String.fromCodePoint(cp)))) return true;
  }
  if (filter.ranges) {
    return codepoints.some((cp) =>
      filter.ranges?.some(([start, end]) => cp >= start && cp <= end),
    );
  }
  return false;
}

function glyphMatchesCategorySubfilter(
  name: string,
  category: Category,
  subcategory: string,
): boolean {
  if ((glyphCategories.value.get(name) ?? "Other") !== category) return false;
  const lowerName = name.toLowerCase();
  const codepoints = glyphCodepoints(name);
  const first = codepoints[0];
  switch (subcategory) {
    case "uppercase":
      return !!first && String.fromCodePoint(first).toUpperCase() === String.fromCodePoint(first);
    case "lowercase":
      return !!first && String.fromCodePoint(first).toLowerCase() === String.fromCodePoint(first);
    case "ligature":
      return name.includes("_") || lowerName.includes("liga");
    case "decimal":
      return codepoints.some((cp) => cp >= 0x30 && cp <= 0x39);
    case "fraction":
      return /fraction|\.dnom|\.numr/.test(lowerName) || "¼½¾⅓⅔⁄".includes(String.fromCodePoint(first || 0));
    case "superior-inferior":
      return /\.sups|\.subs|superior|inferior|\.numr|\.dnom/.test(lowerName);
    case "space":
      return ["space", "nbspace", "nonbreakingspace"].includes(lowerName) || first === 0x20 || first === 0xa0;
    case "line":
      return lowerName.includes("line") || first === 0x2028 || first === 0x2029;
    case "quote":
      return /quote|quotedbl|guillemet|single/.test(lowerName) || "'\"‘’“”«»".includes(String.fromCodePoint(first || 0));
    case "dash":
      return /dash|hyphen|minus/.test(lowerName) || "-–—−".includes(String.fromCodePoint(first || 0));
    case "paren":
      return /paren|bracket|brace/.test(lowerName) || "()[]{}".includes(String.fromCodePoint(first || 0));
    case "currency":
      return /dollar|cent|sterling|yen|euro|currency|peso|rupee|won/.test(lowerName);
    case "math":
      return /plus|minus|equal|less|greater|divide|multiply|integral|summation/.test(lowerName);
    case "arrow":
      return lowerName.includes("arrow") || codepoints.some((cp) => cp >= 0x2190 && cp <= 0x21ff);
    case "nonspacing":
      return lowerName.includes("comb") || lowerName.includes("mark");
    case "spacing":
      return !lowerName.includes("comb") && !lowerName.includes("mark");
    default:
      return true;
  }
}

function glyphMatchesBuiltinFilter(name: string, filter: SidebarBuiltinFilter): boolean {
  if (filter.source === "google-fonts-glyphsets") {
    return glyphMatchesCharacterFilter(name, filter);
  }
  switch (filter.id) {
    case "exporting":
      return true;
    case "incompatible":
      return compatErrors.value.length > 0 && name === currentGlyph.value;
    default:
      return true;
  }
}

function glyphMatchesSidebarFilter(name: string): boolean {
  const filter = selectedSidebarFilter.value;
  if (filter.kind === "all") return true;
  if (filter.kind === "category") {
    if (filter.subcategory) {
      return glyphMatchesCategorySubfilter(name, filter.category, filter.subcategory);
    }
    return (glyphCategories.value.get(name) ?? "Other") === filter.category;
  }
  if (filter.kind === "language") {
    const languageFilter = SIDEBAR_LANGUAGE_GROUPS.flatMap((group) => group.filters).find(
      (item) => item.id === filter.id,
    );
    return languageFilter ? glyphMatchesCharacterFilter(name, languageFilter) : false;
  }
  if (filter.kind === "languageGroup") {
    const languageGroup = SIDEBAR_LANGUAGE_GROUPS.find((group) => group.id === filter.id);
    return languageGroup
      ? languageGroup.filters.some((item) => glyphMatchesCharacterFilter(name, item))
      : false;
  }
  if (filter.kind === "gfGlyphset") {
    const gfFilter = SIDEBAR_FILTERS.find((item) => item.id === filter.id);
    return gfFilter ? glyphMatchesCharacterFilter(name, gfFilter) : false;
  }
  if (filter.kind === "builtin") {
    const builtin = SIDEBAR_FILTERS.find((item) => item.id === filter.id);
    return builtin ? glyphMatchesBuiltinFilter(name, builtin) : false;
  }
  return true;
}

function glyphMatchesSidebarSearch(name: string): boolean {
  const query = sidebarSearchQuery.value.trim();
  if (!query) return true;
  const unicode = glyphUnicodes.value.get(name) ?? "";
  const chars = glyphCodepoints(name)
    .map((cp) => String.fromCodePoint(cp))
    .join("");
  const haystacks =
    sidebarSearchMode.value === "name"
      ? [name]
      : sidebarSearchMode.value === "unicode"
        ? [unicode, chars]
        : [name, unicode, chars];
  if (sidebarSearchRegex.value) {
    try {
      const regex = new RegExp(query, sidebarSearchMatchCase.value ? "" : "i");
      return haystacks.some((value) => regex.test(value));
    } catch {
      return true;
    }
  }
  const needle = sidebarSearchMatchCase.value ? query : query.toLowerCase();
  return haystacks.some((value) =>
    (sidebarSearchMatchCase.value ? value : value.toLowerCase()).includes(needle),
  );
}

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
const selectedGridTextGlyphCount = computed(() => selectedGridGlyphTextPieces().length);
const activeGlyphSvg = computed(() =>
  currentGlyph.value ? glyphSvgs.value.get(currentGlyph.value) : undefined,
);
const activeGlyphPreviewSvg = computed(() => {
  if (!currentGlyph.value || !activeMasterData.value) return undefined;
  const bytes = activeMasterData.value.glyphBytes.get(currentGlyph.value);
  if (!bytes) return activeGlyphSvg.value;
  try {
    return glifToSvgWithComponents(bytes, glyphXmlByName.value) || glifToSvg(bytes);
  } catch {
    return activeGlyphSvg.value;
  }
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
const textModeActive = computed(() => activeTool.value === "Text" && hasTextBufferSession.value);
const activeGlyphPanelVisible = computed(
  () => !!currentGlyph.value && (!textModeActive.value || activeTextSortIndex.value !== null),
);
const textBufferPreviewVisible = computed(() => hasTextBufferSession.value);
const editorBottomPreviewVisible = computed(
  () =>
    viewMode.value === "editor" &&
    (textBufferPreviewVisible.value || (!!currentGlyph.value && !!activeGlyphPreviewSvg.value)),
);
const editorBottomPreviewStyle = computed(() => ({
  "--rb-editor-bottom-preview-height": `${editorBottomPreviewHeight.value}px`,
}));
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
const textLayout = ref<TextLayoutSnapshot>({ cursorX: 0, cursorY: 0, items: [] });
const textPreviewRevision = ref(0);
function bumpTextPreviewRevision() {
  textPreviewRevision.value += 1;
}
const textBufferPreviewSvg = computed(() => {
  textPreviewRevision.value;
  textBuffer.value;
  textLayout.value;
  activeTextSortIndex.value;
  currentGlyph.value;
  try {
    return editor?.textBufferPreviewSvg() ?? "";
  } catch (e) {
    console.warn("failed to render text buffer preview:", e);
    return "";
  }
});

type Editor = {
  pointerDown(x: number, y: number, button: number, mods: number): void;
  pointerMove(x: number, y: number, mods: number): void;
  pointerMoveVisualChanged(x: number, y: number, mods: number): boolean;
  pointerMoveSelectionState(x: number, y: number, mods: number): Float64Array;
  clearSegmentHover(): boolean;
  pointerUp(x: number, y: number, button: number, mods: number): boolean;
  pointerCancel(): boolean;
  componentBaseAt(x: number, y: number): string;
  clearComponentSelection(): void;
  anchorContextAt(x: number, y: number): string;
  selectedAnchorInfo(): string;
  selectAnchorAt(x: number, y: number): boolean;
  addAnchorAt(x: number, y: number, name: string): boolean;
  updateSelectedAnchor(name: string, x: number, y: number): boolean;
  setTool(toolId: ToolId): boolean;
  setShapeTool(shape: ShapeKind): boolean;
  setShapeShiftLocked(locked: boolean): boolean;
  setKnifeShiftLocked(locked: boolean): boolean;
  setTextDirection(direction: TextDirection): void;
  setTextKerningModel(json: string): void;
  textKerningModel(): string;
  setTextGlyphInventory(json: string): void;
  shapeTextBuffer(): boolean;
  textBufferSnapshot(): string;
  textBufferLayout(lineHeight: number): string;
  textBufferState(): string;
  textLayoutState(): Float64Array;
  textBufferPreviewSvg(): string;
  clearTextBuffer(): void;
  insertTextGlyph(name: string, codepoint: number, advanceWidth: number): void;
  insertInactiveTextGlyph(name: string, codepoint: number, advanceWidth: number): void;
  activateTextSort(index: number): boolean;
  insertTextCharacter(codepoint: number): boolean;
  updateTextGlyph(index: number, name: string, codepoint: number, advanceWidth: number): boolean;
  insertTextLineBreak(): void;
  deleteTextBeforeCursor(): boolean;
  deleteTextAfterCursor(): boolean;
  moveTextCursorVisualLeft(): void;
  moveTextCursorVisualRight(): void;
  activateTextSortAt(x: number, y: number): boolean;
  activateTextSortAtIndex(x: number, y: number): number;
  activateTextSortAtState(x: number, y: number): Float64Array;
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
  roundSelectedCorners(): boolean;
  togglePointTypeAt(x: number, y: number): boolean;
  selectContourAt(x: number, y: number): boolean;
  unionSelection(): boolean;
  subtractSelection(): boolean;
  intersectSelection(): boolean;
  excludeSelection(): boolean;
  render(): void;
  resize(w: number, h: number): void;
  setDeviceScale(scale: number): void;
  setTheme(themeJson: string): void;
  setGlyphSvg(svg: string): void;
  setGlyphGlif(bytes: Uint8Array): void;
  setComponentGlyphs(glyphXmlByName: string): void;
  setComponentGlyph(name: string, bytes: Uint8Array): void;
  deleteComponentGlyph(name: string): void;
  setGlyphGlifWithComponents(bytes: Uint8Array, glyphXmlByName: string): void;
  setGlyphGlifWithComponentsPreserveHistory(bytes: Uint8Array, glyphXmlByName: string): void;
  setGlyphGlifWithCachedComponents(bytes: Uint8Array): void;
  setGlyphGlifWithCachedComponentsPreserveHistory(bytes: Uint8Array): void;
  setGlyphNameWithCachedComponents(name: string): boolean;
  setGlyphNameWithCachedComponentsPreserveHistory(name: string): boolean;
  setFontInfo(bytes: Uint8Array): void;
  fitToCanvas(w: number, h: number): void;
  setZoom(z: number): void;
  zoom(): number;
  setOffset(x: number, y: number): void;
  designToScreen(x: number, y: number): Float64Array;
  screenToDesign(x: number, y: number): Float64Array;
  selectionCount(): number;
  selectedContourCount(): number;
  cycleSelectedPoint(backwards: boolean): boolean;
  selectionBounds(): Float64Array;
  selectionState(): Float64Array;
  measureInfo(): Float64Array;
  setCoordinateQuadrant(quadrant: string): void;
  moveSelectionReference(axis: "x" | "y", value: number): boolean;
  moveSelectionReferenceState(axis: "x" | "y", value: number): Float64Array;
  resizeSelectionReference(axis: "width" | "height", value: number): boolean;
  resizeSelectionReferenceState(axis: "width" | "height", value: number): Float64Array;
  nudgeSelection(
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    independent: boolean,
  ): boolean;
  nudgeSelectionFastState(
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    independent: boolean,
  ): Float64Array;
  nudgeSelectionState(
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    independent: boolean,
  ): Float64Array;
  finishNudgeSelection(): void;
  setAdvanceWidth(width: number): boolean;
  leftSidebearing(): number;
  rightSidebearing(): number;
  editorMetricsState(): Float64Array;
  editorPanelState(): Float64Array;
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
type SaveAsDestination = {
  destination: string;
  relink: boolean;
};

type NudgePerfSample = {
  id: number;
  start: number;
  mutation?: number;
  renderStart?: number;
  renderEnd?: number;
  panel?: number;
  syncStart?: number;
  syncEnd?: number;
  queued: boolean;
};

const NUDGE_IDLE_COMMIT_DELAY_MS = 500;
const NUDGE_PANEL_REFRESH_INTERVAL_MS = 80;
const NUDGE_PERF_LOG_EVERY = 12;

let editor: Editor | null = null;
let editorComponentGlyphsData: MasterData | null = null;
let editorComponentGlyphsVersion = -1;
let editorGlyphNeedsSync = false;
let raf: number | null = null;
let rafNeedsBackgroundImageFrame = false;
let rafNeedsSelectionState = false;
let rafNeedsCompatibilityMarkers = false;
let rafNeedsCompatibilityErrors = false;
// Latest unprocessed pointermove — coalesced into the rAF callback so
// WASM is called once per frame regardless of input device Hz.
let pendingPointerMove: PointerEvent | null = null;
let compatibilityErrorRefreshTimer: number | null = null;
let pendingNudgeSelectionState: Float64Array | null = null;
let pendingNudgeSelectionRefresh = false;
let resizeObserver: ResizeObserver | null = null;
let themeObserver: MutationObserver | null = null;
let comfySyncTimer: number | null = null;
let deferredGlyphSyncTimer: number | null = null;
let deferredGlyphSyncCommitRaf: number | null = null;
let deferredGlyphSyncCommitTimer: number | null = null;
let deferredGlyphDerivedSyncTimer: number | null = null;
let deferredGlyphDerivedSyncGlyph = "";
let deferredGlyphDerivedSyncMaster = "";
let postPaintNudgeSelectionState: Float64Array | null = null;
let postPaintNudgeSelectionRefresh = false;
let postPaintNudgeSelectionPerf: NudgePerfSample | null = null;
let postPaintNudgeSelectionRaf: number | null = null;
let postPaintNudgeSelectionTimer: number | null = null;
let lastNudgeSelectionRefreshAt = 0;
let nudgePreviewActive = false;
let nudgePerfLoggingEnabled = false;
let nudgePerfNextId = 1;
let pendingNudgePerf: NudgePerfSample | null = null;
let lastRenderedNudgePerf: NudgePerfSample | null = null;
let completedNudgePerfSamples: NudgePerfSample[] = [];
let textPointerMayMutate = false;
let textKerningNeedsSync = false;
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
    editor.setDeviceScale(dpr);
    applyCanvasTheme();
    nudgePerfLoggingEnabled = readNudgePerfEnabled();
    if (nudgePerfLoggingEnabled) {
      console.info("[runebender-perf] nudge telemetry enabled");
    }

    status.value = "ready";

    resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(canvas.value);
    themeObserver = new MutationObserver(() => {
      applyCanvasTheme();
      requestRender();
    });
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class", "style", "data-theme"],
    });
    if (document.body) {
      themeObserver.observe(document.body, {
        attributes: true,
        attributeFilter: ["class", "style", "data-theme"],
      });
    }
    updateGridViewportSize();
    window.addEventListener("keydown", onGlobalKeyDownCapture, { capture: true });
    window.addEventListener("keyup", onGlobalKeyUpCapture, { capture: true });
    window.addEventListener("blur", onWindowBlur);
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
    } else if (props.initialFiles) {
      const loaded = await loadDevTestFont();
      if (!loaded) {
        loadWelcomeDemoGlyph();
      }
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

type RenderRequestOptions = {
  refreshDerivedState?: boolean;
  refreshBackgroundImageFrame?: boolean;
  refreshSelectionState?: boolean;
  refreshCompatibilityMarkers?: boolean;
  refreshCompatibilityErrors?: boolean;
};

function readNudgePerfEnabled(): boolean {
  try {
    return (
      window.localStorage?.getItem("runebender:perf") === "1" ||
      new URLSearchParams(window.location.search).has("rb_perf")
    );
  } catch {
    return false;
  }
}

function nudgePerfEnabled(): boolean {
  return nudgePerfLoggingEnabled;
}

function startNudgePerf(queued: boolean): NudgePerfSample | undefined {
  if (!nudgePerfEnabled()) return undefined;
  return {
    id: nudgePerfNextId++,
    start: performance.now(),
    queued,
  };
}

function markNudgePerfMutation(sample: NudgePerfSample | undefined) {
  if (sample) sample.mutation = performance.now();
}

function queueNudgePerfForRender(sample: NudgePerfSample | undefined) {
  if (sample) pendingNudgePerf = sample;
}

function recordCompletedNudgePerf(sample: NudgePerfSample | null) {
  if (!sample || !nudgePerfEnabled()) return;
  lastRenderedNudgePerf = sample;
  completedNudgePerfSamples.push(sample);
  if (completedNudgePerfSamples.length < NUDGE_PERF_LOG_EVERY) return;
  const samples = completedNudgePerfSamples;
  completedNudgePerfSamples = [];
  const avg = (values: number[]) =>
    values.length === 0
      ? 0
      : values.reduce((sum, value) => sum + value, 0) / values.length;
  const mutation = samples
    .filter((sample) => sample.mutation !== undefined)
    .map((sample) => sample.mutation! - sample.start);
  const render = samples
    .filter((sample) => sample.renderStart !== undefined && sample.renderEnd !== undefined)
    .map((sample) => sample.renderEnd! - sample.renderStart!);
  const visible = samples
    .filter((sample) => sample.renderEnd !== undefined)
    .map((sample) => sample.renderEnd! - sample.start);
  const panel = samples
    .filter((sample) => sample.panel !== undefined)
    .map((sample) => sample.panel! - sample.start);
  const queued = samples.filter((sample) => sample.queued).length;
  console.info(
    `[runebender-perf] nudge avg over ${samples.length}: ` +
      `mutate ${avg(mutation).toFixed(1)}ms, ` +
      `render ${avg(render).toFixed(1)}ms, ` +
      `visible ${avg(visible).toFixed(1)}ms, ` +
      `panel ${avg(panel).toFixed(1)}ms, ` +
      `queued ${queued}`,
  );
}

function logNudgeSyncPerf(sample: NudgePerfSample | undefined) {
  if (!sample || !nudgePerfEnabled() || sample.syncStart === undefined || sample.syncEnd === undefined) {
    return;
  }
  console.info(
    `[runebender-perf] nudge sync ${sample.id}: ` +
      `${(sample.syncEnd - sample.syncStart).toFixed(1)}ms, ` +
      `total ${(sample.syncEnd - sample.start).toFixed(1)}ms`,
  );
}

function requestRender(options: RenderRequestOptions = {}) {
  if (!editor || (viewMode.value !== "editor" && glyphNames.value.length > 0)) return;
  const refreshAll = options.refreshDerivedState !== false;
  rafNeedsBackgroundImageFrame ||=
    options.refreshBackgroundImageFrame ?? refreshAll;
  rafNeedsSelectionState ||= options.refreshSelectionState ?? refreshAll;
  rafNeedsCompatibilityMarkers ||=
    options.refreshCompatibilityMarkers ?? refreshAll;
  rafNeedsCompatibilityErrors ||= options.refreshCompatibilityErrors ?? false;
  if (raf !== null) return;
  raf = requestAnimationFrame(() => {
    const refreshBackground = rafNeedsBackgroundImageFrame;
    const refreshSelection = rafNeedsSelectionState;
    const refreshCompatibility = rafNeedsCompatibilityMarkers;
    const refreshCompatibilityErrors = rafNeedsCompatibilityErrors;
    raf = null;
    rafNeedsBackgroundImageFrame = false;
    rafNeedsSelectionState = false;
    rafNeedsCompatibilityMarkers = false;
    rafNeedsCompatibilityErrors = false;
    if (!editor || (viewMode.value !== "editor" && glyphNames.value.length > 0)) return;
    // Drain the coalesced pointermove: one WASM call per frame regardless
    // of input device Hz (mice at 1000Hz, tablets at 200Hz, etc.).
    const pm = pendingPointerMove;
    pendingPointerMove = null;
    if (pm && editor) {
      const c = canvasCoords(pm);
      if (c) {
        const pointerActive = pm.buttons !== 0;
        if (!pointerActive && activeTool.value === "Select") {
          const visualChanged = editor.pointerMoveVisualChanged(c[0], c[1], modBits(pm));
          selectIdleHoverActive = pm.altKey;
          if (!visualChanged) {
            // suppress the render below if nothing changed
          }
        } else if (pointerActive && activeTool.value === "Select") {
          const selectionState = editor.pointerMoveSelectionState(c[0], c[1], modBits(pm));
          if (!(selectionState.length >= 2 && selectionState[0] <= 0 && selectionState[1] <= 0)) {
            if (selectionState.length >= 13) {
              if (selectionState[1] > 0) editorGlyphNeedsSync = true;
              applySelectionState(selectionState, { reuseAnchorName: true }, 2);
            }
          }
        } else {
          editor.pointerMove(c[0], c[1], modBits(pm));
          if (pointerActive) refreshMeasureState();
          if (textPointerMayMutate && pointerActive) {
            refreshTextLayoutFromEditor();
            textKerningNeedsSync = true;
          }
        }
      }
    }
    const nudgePerf = pendingNudgePerf;
    if (nudgePerf) nudgePerf.renderStart = performance.now();
    editor?.render();
    if (nudgePerf) nudgePerf.renderEnd = performance.now();
    const nudgeSelectionState = pendingNudgeSelectionState;
    const nudgeSelectionRefresh = pendingNudgeSelectionRefresh;
    pendingNudgeSelectionState = null;
    pendingNudgeSelectionRefresh = false;
    if (nudgeSelectionState) {
      schedulePostPaintNudgeSelectionState(nudgeSelectionState, nudgePerf ?? null);
    } else if (nudgeSelectionRefresh) {
      schedulePostPaintNudgeSelectionState(null, nudgePerf ?? null, true);
    } else if (nudgePerf) {
      recordCompletedNudgePerf(nudgePerf);
      if (pendingNudgePerf === nudgePerf) pendingNudgePerf = null;
    }
    if (refreshBackground) {
      refreshBackgroundImageFrame();
    }
    if (refreshSelection) {
      refreshSelectionState();
    }
    if (refreshCompatibilityErrors) {
      scheduleCompatibilityErrorRefresh();
    } else if (refreshCompatibility) {
      refreshCompatibilityMarkers();
    }
  });
}

function scheduleCompatibilityErrorRefresh() {
  if (compatibilityErrorRefreshTimer !== null) {
    window.clearTimeout(compatibilityErrorRefreshTimer);
  }
  compatibilityErrorRefreshTimer = window.setTimeout(() => {
    compatibilityErrorRefreshTimer = null;
    updateCompatibilityErrors();
  }, 0);
}

async function loadDevTestFont(): Promise<boolean> {
  try {
    const files = await props.initialFiles?.();
    if (files.length === 0) return false;
    await loadGlifFiles(files);
    fontLabel.value ||= "Dev test font";
    return true;
  } catch (e) {
    console.warn("dev test font auto-load failed:", e);
    return false;
  }
}

type Rgba = [number, number, number, number];

function parseCssColor(color: string, fallback: Rgba, alpha?: number): Rgba {
  const value = color.trim();
  const hex = value.match(/^#([0-9a-f]{3,8})$/i)?.[1];
  const clampByte = (value: number, fallback: number) => {
    if (!Number.isFinite(value)) return fallback;
    return Math.max(0, Math.min(255, Math.round(value)));
  };
  if (hex) {
    const expand = (part: string) => (part.length === 1 ? part + part : part);
    const r = parseInt(expand(hex.slice(0, hex.length === 3 || hex.length === 4 ? 1 : 2)), 16);
    const gStart = hex.length === 3 || hex.length === 4 ? 1 : 2;
    const gLen = hex.length === 3 || hex.length === 4 ? 1 : 2;
    const g = parseInt(expand(hex.slice(gStart, gStart + gLen)), 16);
    const bStart = gStart + gLen;
    const b = parseInt(expand(hex.slice(bStart, bStart + gLen)), 16);
    const aStart = bStart + gLen;
    const a =
      hex.length === 4 || hex.length === 8
        ? parseInt(expand(hex.slice(aStart, aStart + gLen)), 16)
        : 255;
    return [
      clampByte(r, fallback[0]),
      clampByte(g, fallback[1]),
      clampByte(b, fallback[2]),
      clampByte(alpha ?? a, fallback[3]),
    ];
  }

  const numeric = value.match(/rgba?\(([^)]+)\)/i)?.[1];
  if (numeric) {
    const parts = numeric
      .replace(/\//g, " ")
      .split(/[,\s]+/)
      .map((part) => part.trim())
      .filter(Boolean);
    if (parts.length >= 3) {
      const channel = (part: string, fallback: number) => {
        if (part.endsWith("%")) return clampByte((Number(part.slice(0, -1)) / 100) * 255, fallback);
        return clampByte(Number(part), fallback);
      };
      const alphaChannel = (part: string) => {
        if (part.endsWith("%")) return clampByte((Number(part.slice(0, -1)) / 100) * 255, fallback[3]);
        const value = Number(part);
        return clampByte(value <= 1 ? value * 255 : value, fallback[3]);
      };
      const parsedAlpha =
        parts.length >= 4
          ? alphaChannel(parts[3])
          : 255;
      return [
        channel(parts[0], fallback[0]),
        channel(parts[1], fallback[1]),
        channel(parts[2], fallback[2]),
        clampByte(alpha ?? parsedAlpha, fallback[3]),
      ];
    }
  }

  const modern = value.match(/^color\((?:srgb|display-p3)\s+(.+)\)$/i)?.[1];
  if (modern) {
    const parts = modern
      .replace(/\//g, " ")
      .split(/\s+/)
      .map((part) => part.trim())
      .filter(Boolean);
    if (parts.length >= 3) {
      const channel = (part: string, fallback: number) => {
        if (part.endsWith("%")) return clampByte((Number(part.slice(0, -1)) / 100) * 255, fallback);
        return clampByte(Number(part) * 255, fallback);
      };
      const alphaChannel = (part: string) => {
        if (part.endsWith("%")) return clampByte((Number(part.slice(0, -1)) / 100) * 255, fallback[3]);
        const value = Number(part);
        return clampByte(value <= 1 ? value * 255 : value, fallback[3]);
      };
      const parsedAlpha = parts.length >= 4 ? alphaChannel(parts[3]) : 255;
      return [
        channel(parts[0], fallback[0]),
        channel(parts[1], fallback[1]),
        channel(parts[2], fallback[2]),
        clampByte(alpha ?? parsedAlpha, fallback[3]),
      ];
    }
  }

  return [fallback[0], fallback[1], fallback[2], alpha ?? fallback[3]];
}

function resolveHostColor(variableName: string, fallback: Rgba, alpha?: number): Rgba {
  const host = canvas.value?.closest(".runebender-host") ?? document.documentElement;
  const probe = document.createElement("span");
  probe.style.color = `var(${variableName}, rgba(${fallback[0]}, ${fallback[1]}, ${fallback[2]}, ${fallback[3] / 255}))`;
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  host.appendChild(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();
  return parseCssColor(resolved, fallback, alpha);
}

function applyCanvasTheme() {
  if (!editor) return;
  const accent = resolveHostColor("--rb-accent", [0x18, 0xb8, 0x6f, 0xff]);
  const selected = resolveHostColor("--rb-warning", [0xff, 0xdc, 0x32, 0xff]);
  const selection = resolveHostColor("--rb-canvas-selection", [0xff, 0x98, 0x0f, 0xff]);
  const primaryText = resolveHostColor("--rb-primary-text", [0x90, 0x90, 0x90, 0xff]);
  const panelText = resolveHostColor("--rb-secondary-text", [0x70, 0x70, 0x70, 0xff]);

  editor.setTheme(
    JSON.stringify({
      bg: resolveHostColor("--rb-canvas-background", [0x10, 0x10, 0x10, 0xff]),
      pathStroke: resolveHostColor("--rb-canvas-path-stroke", [0xb0, 0xb0, 0xb0, 0xff]),
      previewFill: resolveHostColor("--rb-glyph-preview", [0x80, 0x80, 0x80, 0xff]),
      componentFill: resolveHostColor("--rb-canvas-component", [0x66, 0x99, 0xcc, 0xff]),
      componentSelectedFill: resolveHostColor(
        "--rb-canvas-component-selected",
        [0x88, 0xbb, 0xff, 0xff],
      ),
      handleLine: primaryText,
      pointSmoothInner: resolveHostColor("--rb-canvas-point-smooth-inner", [0x18, 0x18, 0x18, 0xff]),
      pointSmoothOuter: resolveHostColor("--rb-canvas-point-smooth-outer", [0x18, 0xb8, 0x6f, 0xff]),
      pointCornerInner: resolveHostColor("--rb-canvas-point-corner-inner", [0x18, 0x18, 0x18, 0xff]),
      pointCornerOuter: resolveHostColor("--rb-canvas-point-corner-outer", [0xff, 0x98, 0x0f, 0xff]),
      pointOffcurveInner: resolveHostColor("--rb-canvas-point-offcurve-inner", [0x18, 0x18, 0x18, 0xff]),
      pointOffcurveOuter: resolveHostColor("--rb-canvas-point-offcurve-outer", [0x8c, 0x6c, 0xff, 0xff]),
      pointHyperInner: resolveHostColor("--rb-canvas-point-hyper-inner", [0x18, 0x18, 0x18, 0xff]),
      pointHyperOuter: resolveHostColor("--rb-canvas-point-hyper-outer", [0x8c, 0x6c, 0xff, 0xff]),
      pointSelectedInner: selected,
      pointSelectedOuter: selection,
      startNodeOuter: resolveHostColor("--rb-canvas-start-node", [0xff, 0x98, 0x0f, 0xff]),
      marqueeFill: [selection[0], selection[1], selection[2], 0x20],
      marqueeStroke: selection,
      toolPreview: selection,
      metricGuide: accent,
      designGridFine: [panelText[0], panelText[1], panelText[2], 0x52],
      designGridCoarse: [panelText[0], panelText[1], panelText[2], 0x70],
      textPreviewFill: resolveHostColor("--rb-canvas-text-preview-fill", [0x80, 0x80, 0x80, 0xff]),
      textCursor: resolveHostColor("--rb-canvas-text-cursor", [0xff, 0x98, 0x0f, 0xff]),
      textKernActive: resolveHostColor("--rb-canvas-kern-active", [0x45, 0x6f, 0xff, 0xff]),
      textKernPrevious: resolveHostColor("--rb-canvas-kern-previous", [0xff, 0x98, 0x0f, 0xff]),
    }),
  );
}

function loadWelcomeDemoGlyph() {
  if (!editor || !canvas.value || glyphNames.value.length > 0) return;
  const encoder = new TextEncoder();
  try {
    editor.setFontInfo(encoder.encode(WELCOME_DEMO_FONTINFO));
    editor.setGlyphGlif(encoder.encode(WELCOME_DEMO_GLIF));
    editorComponentGlyphsData = null;
    editorComponentGlyphsVersion = -1;
    editorGlyphNeedsSync = false;
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
    await runebenderHost.publishState({
      nodeId: props.nodeId,
      font: currentFontPath.value,
      glyphData: payload,
    });
    lastPublishedComfyState = stateKey;
  } catch (e) {
    console.warn("ComfyUI state sync failed:", e);
  }
}

function applySelectionState(
  state: ArrayLike<number>,
  options: { reuseAnchorName?: boolean } = {},
  offset = 0,
) {
  cancelPostPaintNudgeSelectionState();
  postPaintNudgeSelectionState = null;
  postPaintNudgeSelectionPerf = null;
  if (!editor) {
    setSelectionState(0, 0, undefined);
    selectedAnchor.value = null;
    return;
  }
  const hasBounds = state.length >= offset + 8 && state[offset + 2] > 0;
  setSelectionStateValues(
    state[offset] ?? 0,
    state[offset + 1] ?? 0,
    hasBounds,
    state[offset + 3] ?? 0,
    state[offset + 4] ?? 0,
    state[offset + 5] ?? 0,
    state[offset + 6] ?? 0,
    state[offset + 7] ?? 0,
  );
  const hasAnchor = state.length >= offset + 11 && state[offset + 8] > 0;
  const nextAnchor = hasAnchor
    ? options.reuseAnchorName && selectedAnchor.value
      ? {
          name: selectedAnchor.value.name ?? null,
          x: state[offset + 9],
          y: state[offset + 10],
        }
      : {
          ...(parseAnchorContext(editor.selectedAnchorInfo()) ?? {
            name: selectedAnchor.value?.name ?? null,
            x: state[offset + 9],
            y: state[offset + 10],
          }),
          x: state[offset + 9],
          y: state[offset + 10],
        }
    : null;
  if (!sameAnchorContext(selectedAnchor.value, nextAnchor)) {
    selectedAnchor.value = nextAnchor;
  }
}

function refreshSelectionState(options: { reuseAnchorName?: boolean } = {}) {
  if (!editor) {
    setSelectionState(0, 0, undefined);
    selectedAnchor.value = null;
    return;
  }
  applySelectionState(editor.selectionState(), options);
}

function applyNudgeSelectionState(state: ArrayLike<number>) {
  const hasBounds = state.length >= 8 && state[2] > 0;
  setSelectionStateValues(
    state[1] ?? 0,
    selectedContourCount.value,
    hasBounds,
    state[3] ?? 0,
    state[4] ?? 0,
    state[5] ?? 0,
    state[6] ?? 0,
    state[7] ?? 0,
  );
  const hasAnchor = state.length >= 11 && state[8] > 0;
  const nextAnchor = hasAnchor
    ? {
        name: selectedAnchor.value?.name ?? null,
        x: state[9],
        y: state[10],
      }
    : null;
  if (!sameAnchorContext(selectedAnchor.value, nextAnchor)) {
    selectedAnchor.value = nextAnchor;
  }
}

function cancelPostPaintNudgeSelectionState() {
  if (postPaintNudgeSelectionRaf !== null) {
    cancelAnimationFrame(postPaintNudgeSelectionRaf);
    postPaintNudgeSelectionRaf = null;
  }
  if (postPaintNudgeSelectionTimer !== null) {
    window.clearTimeout(postPaintNudgeSelectionTimer);
    postPaintNudgeSelectionTimer = null;
  }
}

function flushPostPaintNudgeSelectionState() {
  cancelPostPaintNudgeSelectionState();
  const state = postPaintNudgeSelectionState;
  const refresh = postPaintNudgeSelectionRefresh;
  const nudgePerf = postPaintNudgeSelectionPerf;
  postPaintNudgeSelectionState = null;
  postPaintNudgeSelectionRefresh = false;
  postPaintNudgeSelectionPerf = null;
  if (state) {
    applyNudgeSelectionState(state);
  } else if (refresh) {
    refreshSelectionState({ reuseAnchorName: true });
  } else {
    return;
  }
  lastNudgeSelectionRefreshAt = performance.now();
  if (nudgePerf) {
    nudgePerf.panel = performance.now();
    recordCompletedNudgePerf(nudgePerf);
    if (pendingNudgePerf === nudgePerf) pendingNudgePerf = null;
  }
}

function schedulePostPaintNudgeSelectionState(
  state: Float64Array | null,
  nudgePerf: NudgePerfSample | null,
  refresh = false,
) {
  postPaintNudgeSelectionState = state;
  postPaintNudgeSelectionRefresh = refresh;
  postPaintNudgeSelectionPerf = nudgePerf;
  cancelPostPaintNudgeSelectionState();
  const now = performance.now();
  const elapsed = now - lastNudgeSelectionRefreshAt;
  const delay =
    nudgePreviewActive && elapsed < NUDGE_PANEL_REFRESH_INTERVAL_MS
      ? NUDGE_PANEL_REFRESH_INTERVAL_MS - elapsed
      : 0;
  if (delay > 0) {
    postPaintNudgeSelectionTimer = window.setTimeout(() => {
      postPaintNudgeSelectionTimer = null;
      postPaintNudgeSelectionRaf = requestAnimationFrame(() => {
        postPaintNudgeSelectionRaf = null;
        flushPostPaintNudgeSelectionState();
      });
    }, delay);
    return;
  }
  postPaintNudgeSelectionRaf = requestAnimationFrame(() => {
    postPaintNudgeSelectionRaf = null;
    postPaintNudgeSelectionTimer = window.setTimeout(() => {
      postPaintNudgeSelectionTimer = null;
      flushPostPaintNudgeSelectionState();
    }, 0);
  });
}

function applyEditorPanelState(state: ArrayLike<number>, offset = 0) {
  setRefNumber(currentWidth, state[offset] ?? 0);
  setRefNumber(currentContours, state[offset + 1] ?? 0);
  setRefNumber(currentLeftSidebearing, state[offset + 2] ?? 0);
  setRefNumber(currentRightSidebearing, state[offset + 3] ?? 0);
  applySelectionState(state, {}, offset + 4);
}

function applyEditorMetricsState(state: ArrayLike<number>) {
  setRefNumber(currentWidth, state[0] ?? 0);
  setRefNumber(currentContours, state[1] ?? 0);
  setRefNumber(currentLeftSidebearing, state[2] ?? 0);
  setRefNumber(currentRightSidebearing, state[3] ?? 0);
  setSelectionState(0, 0, undefined);
  if (selectedAnchor.value !== null) {
    selectedAnchor.value = null;
  }
}

function refreshEditorPanelState() {
  if (!editor) {
    setRefNumber(currentWidth, 0);
    setRefNumber(currentContours, 0);
    setRefNumber(currentLeftSidebearing, 0);
    setRefNumber(currentRightSidebearing, 0);
    setSelectionState(0, 0, undefined);
    selectedAnchor.value = null;
    return;
  }
  applyEditorPanelState(editor.editorPanelState());
}

function sameAnchorContext(
  a: AnchorPanelValue | null,
  b: AnchorPanelValue | null,
): boolean {
  if (!a || !b) return a === b;
  return a.name === b.name && a.x === b.x && a.y === b.y;
}

function setRefNumber(target: { value: number }, value: number) {
  if (target.value !== value) {
    target.value = value;
  }
}

function setSelectionState(
  nextSelectionCount: number,
  nextSelectedContourCount: number,
  nextBounds: SelectionBounds | undefined,
) {
  setSelectionStateValues(
    nextSelectionCount,
    nextSelectedContourCount,
    Boolean(nextBounds),
    nextBounds?.count ?? 0,
    nextBounds?.x ?? 0,
    nextBounds?.y ?? 0,
    nextBounds?.width ?? 0,
    nextBounds?.height ?? 0,
  );
}

function setSelectionStateValues(
  nextSelectionCount: number,
  nextSelectedContourCount: number,
  hasBounds: boolean,
  boundsCount: number,
  boundsX: number,
  boundsY: number,
  boundsWidth: number,
  boundsHeight: number,
) {
  if (selectionCount.value !== nextSelectionCount) {
    selectionCount.value = nextSelectionCount;
  }
  if (selectedContourCount.value !== nextSelectedContourCount) {
    selectedContourCount.value = nextSelectedContourCount;
  }
  if (!hasBounds) {
    if (selectedBounds.value) {
      selectedBounds.value = undefined;
    }
    return;
  }
  if (!selectedBounds.value) {
    selectedBounds.value = {
      count: boundsCount,
      x: boundsX,
      y: boundsY,
      width: boundsWidth,
      height: boundsHeight,
    };
    return;
  }
  if (
    selectedBounds.value.count !== boundsCount ||
    selectedBounds.value.x !== boundsX ||
    selectedBounds.value.y !== boundsY ||
    selectedBounds.value.width !== boundsWidth ||
    selectedBounds.value.height !== boundsHeight
  ) {
    selectedBounds.value.count = boundsCount;
    selectedBounds.value.x = boundsX;
    selectedBounds.value.y = boundsY;
    selectedBounds.value.width = boundsWidth;
    selectedBounds.value.height = boundsHeight;
  }
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
    if (compatMarkers.value.length > 0) {
      compatMarkers.value = [];
    }
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
  if (!editor || activeTool.value !== "Measure") {
    if (measureInfo.value) {
      measureInfo.value = undefined;
    }
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

function imageDataForFile(file: File): Promise<ImageData> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = Math.max(1, img.naturalWidth);
        canvas.height = Math.max(1, img.naturalHeight);
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          reject(new Error("image canvas unavailable"));
          return;
        }
        ctx.drawImage(img, 0, 0);
        resolve(ctx.getImageData(0, 0, canvas.width, canvas.height));
      } catch (error) {
        reject(error);
      } finally {
        URL.revokeObjectURL(url);
      }
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("image decode failed"));
    };
    img.src = url;
  });
}

function compositedLuma(data: Uint8ClampedArray, offset: number): number {
  const alpha = data[offset + 3] / 255;
  const luma = data[offset] * 0.299 + data[offset + 1] * 0.587 + data[offset + 2] * 0.114;
  return Math.round(luma * alpha + 255 * (1 - alpha));
}

function otsuThresholdFromImageData(imageData: ImageData): number {
  const histogram = new Array<number>(256).fill(0);
  const data = imageData.data;
  for (let i = 0; i < data.length; i += 4) {
    histogram[compositedLuma(data, i)] += 1;
  }
  const total = imageData.width * imageData.height;
  let sum = 0;
  for (let i = 0; i < histogram.length; i++) {
    sum += i * histogram[i];
  }
  let sumBackground = 0;
  let weightBackground = 0;
  let bestVariance = -1;
  let threshold = 128;
  for (let i = 0; i < histogram.length; i++) {
    weightBackground += histogram[i];
    if (weightBackground === 0) continue;
    const weightForeground = total - weightBackground;
    if (weightForeground === 0) break;
    sumBackground += i * histogram[i];
    const meanBackground = sumBackground / weightBackground;
    const meanForeground = (sum - sumBackground) / weightForeground;
    const variance =
      weightBackground *
      weightForeground *
      (meanBackground - meanForeground) *
      (meanBackground - meanForeground);
    if (variance > bestVariance) {
      bestVariance = variance;
      threshold = i;
    }
  }
  return threshold;
}

function highContrastTraceThresholdFromImageData(imageData: ImageData): number {
  const otsu = otsuThresholdFromImageData(imageData);
  const data = imageData.data;
  const total = Math.max(1, imageData.width * imageData.height);
  let darkPixels = 0;
  let lightPixels = 0;
  for (let i = 0; i < data.length; i += 4) {
    const luma = compositedLuma(data, i);
    if (luma <= 32) darkPixels += 1;
    if (luma >= 240) lightPixels += 1;
  }
  const darkRatio = darkPixels / total;
  const lightRatio = lightPixels / total;
  if (darkRatio >= 0.005 && lightRatio >= 0.55) {
    return Math.max(otsu, 192);
  }
  return otsu;
}

async function thresholdForBackgroundTrace(file: File): Promise<number> {
  return highContrastTraceThresholdFromImageData(await imageDataForFile(file));
}

async function foregroundPixelBoundsForTrace(
  file: File,
  threshold?: number,
  invert = false,
): Promise<ForegroundPixelBounds | null> {
  const imageData = await imageDataForFile(file);
  const resolvedThreshold =
    typeof threshold === "number" && Number.isFinite(threshold)
      ? threshold
      : otsuThresholdFromImageData(imageData);
  const data = imageData.data;
  let minX = imageData.width;
  let minY = imageData.height;
  let maxX = -1;
  let maxY = -1;
  for (let y = 0; y < imageData.height; y++) {
    for (let x = 0; x < imageData.width; x++) {
      const offset = (y * imageData.width + x) * 4;
      if (data[offset + 3] < 8) continue;
      const isDark = compositedLuma(data, offset) <= resolvedThreshold;
      if (invert ? !isDark : isDark) {
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
    }
  }
  if (maxX < minX || maxY < minY) return null;
  return {
    minX,
    minY,
    maxX,
    maxY,
    width: imageData.width,
    height: imageData.height,
  };
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
      file,
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

function parseAnchorContext(raw: string): AnchorContext | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as Partial<AnchorContext>;
    if (typeof value.x !== "number" || typeof value.y !== "number") return null;
    return {
      name: typeof value.name === "string" ? value.name : null,
      x: value.x,
      y: value.y,
    };
  } catch {
    return null;
  }
}

function defaultAnchorNameForPosition(y: number): string {
  return y < 0 ? "bottom" : "top";
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
  requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
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
    traceXOffset:
      backgroundImage.value.traceXOffset === undefined
        ? undefined
        : backgroundImage.value.traceXOffset + dx,
    traceYOffset:
      backgroundImage.value.traceYOffset === undefined
        ? undefined
        : backgroundImage.value.traceYOffset + dy,
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
  const next: BackgroundImageState = {
    ...bg,
    designScaleX,
    designScaleY,
    traceXOffset: undefined,
    traceYOffset: undefined,
  };
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
    traceXOffset: bg.traceXOffset === undefined ? undefined : bg.traceXOffset + dx,
    traceYOffset: bg.traceYOffset === undefined ? undefined : bg.traceYOffset + dy,
  };
  status.value = "background image moved";
  refreshBackgroundImageFrame();
  requestRender();
  return true;
}

function backgroundTraceArgs() {
  if (!backgroundImage.value || !editor || !currentGlyph.value || !currentFontPath.value) {
    return null;
  }
  const data = activeMasterData.value;
  if (!data) {
    return null;
  }
  const bg = backgroundImage.value;
  const glyphName = currentGlyph.value;
  const metadata = data.glyphMetadata.get(glyphName);
  const width =
    currentWidth.value > 0
      ? currentWidth.value
      : metadata?.width && metadata.width > 0
        ? metadata.width
        : editor.advanceWidth();
  const unicode = metadata?.unicodes?.[0] ?? metadata?.unicode ?? "";
  const targetHeight = Math.max(1, Math.abs(bg.height * bg.designScaleY));
  const grid = 2;
  const xOffset =
    bg.traceXOffset === undefined
      ? Math.round(bg.designX / grid) * grid
      : Math.round(bg.traceXOffset / grid) * grid;
  const yOffset =
    bg.traceYOffset === undefined
      ? Math.round(bg.designY / grid) * grid
      : Math.round(bg.traceYOffset / grid) * grid;
  const metrics = editor.metricBounds();
  const ascender = metrics.length >= 2 ? metrics[0] : 800;
  const descender = metrics.length >= 2 ? metrics[1] : -200;
  // Use img2bez's current structural default. Lower values preserve too many
  // bitmap stair-step corners and produce noisy point structure.
  const traceFitAccuracy = 4;
  const traceAlphaMax = 0.8;
  return {
    slot: currentFontPath.value,
    master: activeMasterName.value,
    glyph: glyphName,
    image: bg.file,
    unicode,
    width,
    targetHeight,
    xOffset,
    yOffset,
    imageWidth: bg.width,
    imageHeight: bg.height,
    designX: bg.designX,
    designY: bg.designY,
    designScaleX: bg.designScaleX,
    designScaleY: bg.designScaleY,
    unitsPerEm: data.unitsPerEm,
    ascender,
    descender,
    grid,
    accuracy: traceFitAccuracy,
    smooth: 0,
    alphamax: traceAlphaMax,
    globalFit: false,
    invert: false,
    threshold: undefined as number | undefined,
  };
}

function snapTraceValue(value: number, grid: number): number {
  return grid > 0 ? Math.round(value / grid) * grid : value;
}

async function alignBackgroundImageToTrace(
  args: NonNullable<ReturnType<typeof backgroundTraceArgs>>,
) {
  const bg = backgroundImage.value;
  if (!bg || bg.file !== args.image) return;
  const bounds = await foregroundPixelBoundsForTrace(args.image, args.threshold, args.invert);
  if (!bounds || !backgroundImage.value || backgroundImage.value.file !== args.image) return;

  const scale = args.targetHeight / Math.max(1, args.imageHeight);
  const designX = snapTraceValue(args.xOffset - bounds.minX * scale, args.grid);
  const designY = snapTraceValue(args.yOffset, args.grid);
  backgroundImage.value = {
    ...backgroundImage.value,
    designX,
    designY,
    designScaleX: scale,
    designScaleY: scale,
    traceXOffset: args.xOffset,
    traceYOffset: args.yOffset,
  };
  runebenderHost.log?.(
    "info",
    `[runebender] trace aligned image x=${designX.toFixed(2)} y=${designY.toFixed(2)} fg=(${bounds.minX},${bounds.minY})-(${bounds.maxX},${bounds.maxY}) scale=${scale.toFixed(4)}`,
  );
  refreshBackgroundImageFrame();
}

function wasmTraceConfig(args: NonNullable<ReturnType<typeof backgroundTraceArgs>>) {
  return {
    glyph: args.glyph,
    unicode: args.unicode,
    width: args.width,
    targetHeight: args.targetHeight,
    xOffset: args.xOffset,
    yOffset: args.yOffset,
    grid: args.grid,
    accuracy: args.accuracy,
    smooth: args.smooth,
    alphamax: args.alphamax,
    globalFit: args.globalFit,
    invert: args.invert,
    threshold: args.threshold,
  };
}

async function traceBackgroundImageToGlyph(refit = false): Promise<boolean> {
  if (!backgroundImage.value) {
    status.value = "no background image to trace";
    return false;
  }
  if (!editor || !currentGlyph.value) {
    status.value = "open a glyph before tracing";
    return false;
  }
  if (!currentFontPath.value) {
    status.value = "background tracing requires a workspace font";
    return false;
  }
  const data = activeMasterData.value;
  if (!data) {
    status.value = "no active master to trace into";
    return false;
  }
  const originalBytes = data.glyphBytes.get(currentGlyph.value);
  if (!originalBytes) {
    status.value = "current glyph is not loaded";
    return false;
  }
  if (refit) {
    status.value = "trace refit is not wired yet; running a fresh trace";
  } else {
    status.value = `tracing ${currentGlyph.value} with img2bez`;
  }
  const glyphName = currentGlyph.value;

  try {
    const args = backgroundTraceArgs();
    if (!args) {
      status.value = "background tracing is not ready";
      return false;
    }
    const traceThreshold =
      args.threshold === undefined ? await thresholdForBackgroundTrace(args.image) : args.threshold;
    const traceArgs = { ...args, threshold: traceThreshold };
    runebenderHost.log?.(
      "info",
      `[runebender] trace request glyph=${glyphName} slot=${traceArgs.slot} master=${traceArgs.master} image=${traceArgs.image.name} accuracy=${traceArgs.accuracy} alphamax=${traceArgs.alphamax} threshold=${traceArgs.threshold}`,
    );
    let trace;
    try {
      const imageBytes = new Uint8Array(await traceArgs.image.arrayBuffer());
      const report = JSON.parse(
        traceImageToGlifReport(imageBytes, JSON.stringify(wasmTraceConfig(traceArgs))),
      ) as WasmTraceReport;
      const glif = report.glif;
      trace = { success: true, glyph: glyphName, glif, wasmReport: report };
      runebenderHost.log?.(
        "info",
        `[runebender] trace wasm ok glyph=${glyphName} glif_bytes=${glif.length} contours=${report.contours} curves=${report.curves} lines=${report.lines} on=${report.onCurves} off=${report.offCurves} width=${report.advanceWidth.toFixed(1)} shift=(${report.repositionShiftX.toFixed(1)},${report.repositionShiftY.toFixed(1)})`,
      );
      runebenderHost.log?.(
        "info",
        `[runebender] trace wasm diagnostics extrema=${report.diagnostics.missedExtremaFixed} deviation_splits=${report.diagnostics.highDeviationSplits} tangent_overrides=${report.diagnostics.strongTangentOverrides}/${report.diagnostics.cleanTangentOverrides}/${report.diagnostics.visibleTangentOverrides} tangent_rejects=${report.diagnostics.rejectedTangentNearMisses} oversegment_removed=${report.diagnostics.oversegmentedSplitsRemoved} final_divergences=${report.diagnostics.finalOutlineDivergences} final_repairs=${report.diagnostics.finalOutlineRepairs}`,
      );
    } catch (wasmError) {
      runebenderHost.log?.(
        "warn",
        `[runebender] trace wasm failed, falling back to backend: ${wasmError}`,
      );
      const { response, data } = await runebenderHost.traceBackgroundGlyph(traceArgs);
      trace = data;
      runebenderHost.log?.(
        "info",
        `[runebender] trace response status=${response.status} ok=${response.ok} glif_bytes=${trace.glif?.length ?? 0} error=${trace.error ?? ""}`,
      );
      if (!response.ok || !trace.glif) {
        status.value = `trace failed: ${trace.error || response.statusText}`;
        runebenderHost.log?.("error", `[runebender] trace response rejected: ${status.value}`);
        return false;
      }
    }
    if (trace.command?.length) {
      console.info("[runebender] trace command:", trace.command.join(" "));
    }
    const bytes = new TextEncoder().encode(trace.glif);
    const info = parseGlyphInfo(bytes);
    runebenderHost.log?.(
      "info",
      `[runebender] trace parse ok glyph=${glyphName} bytes=${bytes.length} contours=${info.contours} width=${info.width}`,
    );
    setGlyphBytes(data, glyphName, bytes);
    data.glyphMetadata.set(glyphName, {
      name: glyphName,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    });
    if (info.unicode) {
      data.glyphUnicodes.set(glyphName, info.unicode);
    } else {
      data.glyphUnicodes.delete(glyphName);
    }
    refreshGridGlyphSvg(data, glyphName, bytes);
    ensureEditorComponentGlyphs(data);
    runebenderHost.log?.("info", `[runebender] trace applying glif to editor glyph=${glyphName}`);
    editor.setGlyphGlifWithCachedComponentsPreserveHistory(bytes);
    runebenderHost.log?.("info", `[runebender] trace editor apply ok glyph=${glyphName}`);
    try {
      await alignBackgroundImageToTrace(traceArgs);
    } catch (alignError) {
      runebenderHost.log?.(
        "warn",
        `[runebender] trace image alignment skipped: ${alignError}`,
      );
    }
    editorGlyphNeedsSync = false;
    applyEditorPanelState(editor.editorPanelState());
    updateCompatibilityErrors();
    refreshSidebearingsFromEditor();
    markGlyphDirty(glyphName);
    masterDataMap.value = new Map(masterDataMap.value);
    if (hasTextBufferSession.value) {
      syncTextSortsForGlyph(glyphName, glyphName, data.glyphMetadata.get(glyphName)!);
      syncTextKerningModelToEditor();
      bumpTextPreviewRevision();
    }
    requestRender();
    queueComfyStateSync(true);
    const wasmReport = "wasmReport" in trace ? (trace.wasmReport as WasmTraceReport) : undefined;
    const unrepairedFinalDivergences = wasmReport
      ? Math.max(
          0,
          wasmReport.diagnostics.finalOutlineDivergences -
            wasmReport.diagnostics.finalOutlineRepairs,
        )
      : 0;
    if (unrepairedFinalDivergences > 0) {
      status.value = `traced ${glyphName}; ${unrepairedFinalDivergences} outline divergence${unrepairedFinalDivergences === 1 ? "" : "s"} need review`;
      runebenderHost.log?.(
        "warn",
        `[runebender] trace applied with ${unrepairedFinalDivergences} unrepaired final outline divergence${unrepairedFinalDivergences === 1 ? "" : "s"}`,
      );
    } else {
      status.value = `traced ${glyphName} with img2bez`;
    }
    return true;
  } catch (e) {
    console.warn("background trace failed:", e);
    status.value = `trace failed: ${e}`;
    runebenderHost.log?.("error", `[runebender] trace frontend failed: ${e}`);
    return false;
  }
}

async function traceBackgroundImageFromMenu(source: "click" | "pointer" = "click") {
  const now = Date.now();
  if (source === "click" && now - traceMenuLastPointerUpAt < 500) {
    return;
  }
  if (source === "pointer") {
    traceMenuLastPointerUpAt = now;
  }
  status.value = "trace image command received";
  runebenderHost.log?.("info", `[runebender] trace image menu ${source}`);
  dismissBackgroundImageContextMenu();
  try {
    await traceBackgroundImageToGlyph(false);
  } catch (e) {
    console.warn("background trace click failed:", e);
    status.value = `trace failed: ${e}`;
  }
}

function reportUnavailableBackgroundTrace(kind: "quiver"): boolean {
  if (!backgroundImage.value) {
    status.value = "no background image for Quiver trace";
    return true;
  }
  if (kind === "quiver") {
    status.value = "Quiver trace is not available in the browser editor yet";
    return true;
  }
  return true;
}

function onCoordinateQuadrant(quadrant: CoordinateQuadrant) {
  coordinateQuadrant.value = quadrant;
  editor?.setCoordinateQuadrant(quadrant);
  refreshSelectionState();
}

function onCoordinateChange(axis: "x" | "y" | "width" | "height", value: number) {
  if (!editor || !currentGlyph.value || !activeMasterName.value) return;
  const glyphName = currentGlyph.value;
  const masterName = activeMasterName.value;
  const state =
    axis === "x" || axis === "y"
      ? editor.moveSelectionReferenceState(axis, value)
      : editor.resizeSelectionReferenceState(axis, value);
  if (state.length === 0 || state[0] <= 0) {
    refreshSelectionState();
    return;
  }
  editorGlyphNeedsSync = true;
  applyEditorPanelState(state, 1);
  requestRender({ refreshDerivedState: false });
  scheduleDeferredGlyphSync(glyphName, masterName);
}

function onAnchorChange(field: "name" | "x" | "y", value: string | number) {
  if (!editor || !currentGlyph.value || !selectedAnchor.value) return;
  const next = {
    name: selectedAnchor.value.name ?? "",
    x: selectedAnchor.value.x,
    y: selectedAnchor.value.y,
  };
  if (field === "name") {
    next.name = String(value).trim();
  } else if (field === "x" && typeof value === "number" && Number.isFinite(value)) {
    next.x = value;
  } else if (field === "y" && typeof value === "number" && Number.isFinite(value)) {
    next.y = value;
  } else {
    refreshSelectionState();
    return;
  }
  const changed = applyEditorMutation(() =>
    editor.updateSelectedAnchor(next.name, next.x, next.y),
  );
  if (changed) status.value = "anchor updated";
}

function onActiveGlyphWidthChange(event: Event) {
  if (!editor || !currentGlyph.value) return;
  const input = event.target as HTMLInputElement | null;
  const value = input?.value ?? "";
  const width = Number(value);
  if (!value || value.trim() !== value || !Number.isFinite(width)) {
    if (input) input.value = Math.round(currentWidth.value).toString();
    return;
  }
  const changed = editor.setAdvanceWidth(width);
  if (!changed) {
    if (input) input.value = Math.round(currentWidth.value).toString();
    return;
  }
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  syncTextKerningModelToEditor();
  requestRender({ refreshCompatibilityErrors: true });
  queueComfyStateSync();
}

function refreshSidebearingsFromEditor() {
  if (!editor) {
    setRefNumber(currentLeftSidebearing, 0);
    setRefNumber(currentRightSidebearing, 0);
    return;
  }
  setRefNumber(currentLeftSidebearing, editor.leftSidebearing());
  setRefNumber(currentRightSidebearing, editor.rightSidebearing());
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
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  refreshSidebearingsFromEditor();
  syncTextKerningModelToEditor();
  requestRender({ refreshCompatibilityErrors: true });
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
    const metadata = {
      name: currentGlyph.value,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    };
    setGlyphBytes(data, currentGlyph.value, bytes);
    data.glyphMetadata.set(currentGlyph.value, metadata);
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
    refreshGridGlyphSvg(data, currentGlyph.value, bytes);
    masterDataMap.value = new Map(masterDataMap.value);
    if (input) input.value = info.unicode ?? "";
    markGlyphDirty(currentGlyph.value);
    syncCurrentTextSorts(metadata);
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

function syncTextSortsForGlyph(oldName: string, newName: string, metadata: GlyphMetadata) {
  if (!editor) return;
  const codepoint = metadata.unicode ? parseInt(metadata.unicode, 16) : 0;
  let changed = false;
  for (let index = 0; index < textBuffer.value.length; index++) {
    const sort = textBuffer.value[index];
    if (sort.kind !== "glyph" || sort.glyphName !== oldName) continue;
    changed =
      editor.updateTextGlyph(
        index,
        newName,
        Number.isFinite(codepoint) ? codepoint : 0,
        metadata.width,
      ) || changed;
  }
  if (changed) {
    refreshTextStateFromEditor();
  }
}

function syncRenamedTextSorts(oldName: string, newName: string, metadata: GlyphMetadata) {
  syncTextSortsForGlyph(oldName, newName, metadata);
}

function syncCurrentTextSorts(metadata: GlyphMetadata) {
  if (!currentGlyph.value) return;
  if (!editor || activeTextSortIndex.value === null) return;
  const activeSort = textBuffer.value[activeTextSortIndex.value];
  if (activeSort?.kind !== "glyph" || activeSort.glyphName !== currentGlyph.value) return;
  const codepoint = metadata.unicode ? parseInt(metadata.unicode, 16) : 0;
  if (
    editor.updateTextGlyph(
      activeTextSortIndex.value,
      currentGlyph.value,
      Number.isFinite(codepoint) ? codepoint : 0,
      metadata.width,
    )
  ) {
    refreshTextStateFromEditor();
  }
}

function syncTextSortMetricsToActiveMaster(): boolean {
  if (!editor || !hasTextBufferSession.value) return false;
  let changed = false;
  for (let index = 0; index < textBuffer.value.length; index++) {
    const sort = textBuffer.value[index];
    if (sort.kind !== "glyph") continue;
    const metadata = glyphMetadataMap.value.get(sort.glyphName);
    if (!metadata) continue;
    const codepoint = metadata.unicode ? parseInt(metadata.unicode, 16) : sort.codepoint;
    changed =
      editor.updateTextGlyph(
        index,
        sort.glyphName,
        Number.isFinite(codepoint) ? codepoint : 0,
        metadata.width,
      ) || changed;
  }
  if (changed) {
    refreshTextStateFromEditor();
  }
  return changed;
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

    deleteGlyphBytes(data, oldName);
    setGlyphBytes(data, newName, bytes);
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
    refreshGridGlyphSvg(data, newName, bytes);

    replaceGlyphNameInGroups(data.groups, oldName, newName);
    data.glyphKerningGroups = buildGlyphKerningGroups(data.groups);
    replaceGlyphNameInKerning(data.kerning, oldName, newName);
    clearGlyphDirty(oldName);
  currentGlyph.value = newName;
  selectedGlyph.value = newName;
  selectedGlyphs.value = new Set([newName]);
  setRefNumber(currentWidth, info.width);
  setRefNumber(currentContours, info.contours);
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
  const wasTextSession = activeTool.value === "Text" && activeTextSortIndex.value !== null;
  selectIdleHoverActive = false;
  activeTool.value = tool;
  const toolChanged = editor?.setTool(tool) ?? false;
  let shapeChanged = false;
  if (tool === "Shapes") {
    shapeChanged = editor?.setShapeTool(activeShape.value) ?? false;
  }
  if (tool === "Text") {
    ensureTextSessionForCurrentGlyph();
  }
  if (wasTextSession && tool !== "Text") {
    loadActiveTextSortGlyphIntoEditor();
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
  requestRender();
}

function ensureTextSessionForCurrentGlyph() {
  if (!editor) return;
  syncTextKerningModelToEditor();
  if (!hasTextBufferSession.value && currentGlyph.value) {
    seedTextBufferWithGlyph(currentGlyph.value);
  } else {
    refreshTextStateFromEditor();
  }
}

function refreshTextStateFromEditor(
  syncSorts = true,
  renderOptions: RenderRequestOptions = { refreshDerivedState: false },
) {
  if (!editor) return;
  try {
    const state = JSON.parse(editor.textBufferState()) as TextBufferStateSnapshot;
    const snapshot = state.buffer;
    hasTextBufferSession.value = snapshot.hasTextSession;
    if (syncSorts) {
      textBuffer.value = snapshot.sorts.map((sort): TextSort => {
        if (sort.kind === "lineBreak") return { kind: "lineBreak" };
        return {
          kind: "glyph",
          glyphName: sort.glyphName ?? ".notdef",
          char: sort.char ?? "",
          codepoint: sort.codepoint ?? 0,
          advanceWidth: sort.advanceWidth ?? 500,
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
    setTextLayoutSnapshot(state.layout);
    bumpTextPreviewRevision();
    requestRender(renderOptions);
  } catch (e) {
    console.warn("failed to read text buffer snapshot:", e);
  }
}

function setTextLayoutSnapshot(snapshot: TextLayoutSnapshot) {
  const layout = textLayout.value;
  layout.cursorX = snapshot.cursorX;
  layout.cursorY = snapshot.cursorY;
  const items = layout.items;
  const nextItems = snapshot.items;
  for (let i = 0; i < nextItems.length; i++) {
    const next = nextItems[i];
    const item = items[i];
    if (item) {
      item.index = next.index;
      item.x = next.x;
      item.y = next.y;
      item.advanceWidth = next.advanceWidth;
    } else {
      items[i] = { ...next };
    }
  }
  items.length = nextItems.length;
}

function applyTextLayoutState(state: ArrayLike<number>) {
  const itemCount = Math.max(0, Math.floor(state[2] ?? 0));
  const layout = textLayout.value;
  layout.cursorX = state[0] ?? 0;
  layout.cursorY = state[1] ?? 0;
  const items = layout.items;
  let writeCount = 0;
  for (let i = 0; i < itemCount; i++) {
    const offset = 3 + i * 4;
    if (state.length < offset + 4) break;
    const item = items[writeCount];
    const index = Math.max(0, Math.floor(state[offset] ?? 0));
    const x = state[offset + 1] ?? 0;
    const y = state[offset + 2] ?? 0;
    const advanceWidth = state[offset + 3] ?? 0;
    if (item) {
      item.index = index;
      item.x = x;
      item.y = y;
      item.advanceWidth = advanceWidth;
    } else {
      items[writeCount] = { index, x, y, advanceWidth };
    }
    writeCount += 1;
  }
  items.length = writeCount;
  bumpTextPreviewRevision();
}

function refreshTextLayoutFromEditor(
  renderOptions: RenderRequestOptions = { refreshDerivedState: false },
) {
  if (!editor) return;
  applyTextLayoutState(editor.textLayoutState());
  requestRender(renderOptions);
}

function insertTextCharacter(char: string): boolean {
  const codepoint = char.codePointAt(0);
  if (codepoint === undefined) return false;
  if (!editor?.insertTextCharacter(codepoint)) {
    status.value = `no glyph for U+${codepoint.toString(16).toUpperCase().padStart(4, "0")}`;
    return false;
  }
  refreshTextStateFromEditor();
  return true;
}

function textGlyphPayload(glyphName: string): {
  codepoint: number;
  advanceWidth: number;
} {
  const metadata = glyphMetadataMap.value.get(glyphName);
  const unicode = glyphUnicodes.value.get(glyphName);
  const codepoint = unicode ? parseInt(unicode, 16) : 0;
  return {
    codepoint: Number.isFinite(codepoint) ? codepoint : 0,
    advanceWidth: metadata?.width ?? 500,
  };
}

function insertTextGlyphByName(glyphName: string): boolean {
  const { codepoint, advanceWidth } = textGlyphPayload(glyphName);
  editor?.insertTextGlyph(
    glyphName,
    codepoint,
    advanceWidth,
  );
  refreshTextStateFromEditor();
  return true;
}

function insertInactiveTextGlyphByName(glyphName: string): boolean {
  const { codepoint, advanceWidth } = textGlyphPayload(glyphName);
  editor?.insertInactiveTextGlyph(glyphName, codepoint, advanceWidth);
  refreshTextStateFromEditor();
  return true;
}

function seedTextBufferWithGlyph(glyphName: string) {
  if (!editor) return;
  const metadata = glyphMetadataMap.value.get(glyphName);
  const unicode = glyphUnicodes.value.get(glyphName);
  const codepoint = unicode ? parseInt(unicode, 16) : 0;
  const advanceWidth =
    metadata?.width ?? (currentWidth.value > 0 ? currentWidth.value : 500);
  editor.clearTextBuffer();
  editor.insertTextGlyph(
    glyphName,
    Number.isFinite(codepoint) ? codepoint : 0,
    advanceWidth,
  );
  hasTextBufferSession.value = true;
  refreshTextStateFromEditor();
}

function textGlyphArgs(glyphName: string): {
  name: string;
  codepoint: number;
  advanceWidth: number;
} | null {
  const metadata = glyphMetadataMap.value.get(glyphName);
  const unicode = glyphUnicodes.value.get(glyphName);
  const codepoint = unicode ? parseInt(unicode, 16) : 0;
  const advanceWidth = metadata?.width ?? 500;
  return {
    name: glyphName,
    codepoint: Number.isFinite(codepoint) ? codepoint : 0,
    advanceWidth,
  };
}

function seedTextBufferWithGlyphs(glyphNames: string[], activeGlyph: string) {
  if (!editor) return;
  const names = glyphNames.filter((name) => activeMasterData.value?.glyphBytes.has(name));
  if (names.length === 0) return;
  editor.clearTextBuffer();
  for (const name of names) {
    const args = textGlyphArgs(name);
    if (!args) continue;
    editor.insertInactiveTextGlyph(args.name, args.codepoint, args.advanceWidth);
  }
  const activeIndex = Math.max(0, names.indexOf(activeGlyph));
  editor.activateTextSort(activeIndex);
  hasTextBufferSession.value = true;
  activeTool.value = "Text";
  editor.setTool("Text");
  refreshTextStateFromEditor();
  syncTextKerningModelToEditor();
}

function insertTextLineBreak(): boolean {
  editor?.insertTextLineBreak();
  refreshTextStateFromEditor();
  return true;
}

function deleteTextBeforeCursor(): boolean {
  const changed = editor?.deleteTextBeforeCursor() ?? false;
  refreshTextStateFromEditor();
  return changed;
}

function deleteTextAfterCursor(): boolean {
  const changed = editor?.deleteTextAfterCursor() ?? false;
  refreshTextStateFromEditor();
  return changed;
}

function moveTextCursorVisual(delta: -1 | 1) {
  if (delta < 0) {
    editor?.moveTextCursorVisualLeft();
  } else {
    editor?.moveTextCursorVisualRight();
  }
  refreshTextStateFromEditor();
}

function reshapeTextBuffer() {
  editor?.shapeTextBuffer();
  refreshTextStateFromEditor();
  requestRender();
}

const NON_TEXT_KEY_VALUES = new Set([
  "Alt",
  "AltGraph",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "Backspace",
  "CapsLock",
  "Clear",
  "ContextMenu",
  "Control",
  "Dead",
  "Delete",
  "End",
  "Enter",
  "Escape",
  "Fn",
  "FnLock",
  "Help",
  "Home",
  "Hyper",
  "Insert",
  "Meta",
  "NumLock",
  "PageDown",
  "PageUp",
  "Pause",
  "PrintScreen",
  "Process",
  "ScrollLock",
  "Shift",
  "Super",
  "Symbol",
  "SymbolLock",
  "Tab",
  "Unidentified",
]);

function singleInputCharacter(key: string): string | null {
  const chars = Array.from(key);
  if (!chars.length) return null;
  if (chars.length === 1) return chars[0];
  if (NON_TEXT_KEY_VALUES.has(key)) return null;
  if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return null;
  if (/^[A-Z][A-Za-z0-9]*$/.test(key)) return null;
  return chars[0];
}

function handleTextToolKey(e: KeyboardEvent): boolean {
  if (!textModeActive.value) {
    return false;
  }
  const commandModified = e.metaKey || e.ctrlKey;

  switch (e.key) {
    case "ArrowLeft":
      moveTextCursorVisual(-1);
      return true;
    case "ArrowRight":
      moveTextCursorVisual(1);
      return true;
    case "Backspace":
      if (!deleteTextBeforeCursor()) requestRender();
      return true;
    case "Delete":
      if (!deleteTextAfterCursor()) requestRender();
      return true;
    case "Enter":
      insertTextLineBreak();
      return true;
    default:
      {
        if (commandModified) return false;
        const char = singleInputCharacter(e.key);
        if (char) {
          return insertTextCharacter(char) || char === " ";
        }
      }
      return false;
  }
}

function handleResize() {
  updateGridViewportSize();
  editorBottomPreviewHeight.value = clampEditorBottomPreviewHeight(editorBottomPreviewHeight.value);
  if (!editor || !canvas.value) return;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.value.getBoundingClientRect();
  const width = Math.max(1, Math.floor(rect.width * dpr));
  const height = Math.max(1, Math.floor(rect.height * dpr));
  editor.setDeviceScale(dpr);
  if (canvas.value.width === width && canvas.value.height === height) {
    requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
    return;
  }
  canvas.value.width = width;
  canvas.value.height = height;
  editor.resize(width, height);
  requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
}

function updateGridViewportSize() {
  const rect = canvas.value?.getBoundingClientRect() ?? gridView.value?.getBoundingClientRect();
  gridViewportWidth.value = rect?.width ?? 0;
  gridViewportHeight.value = rect?.height ?? 0;
}

function clampEditorBottomPreviewHeight(height: number): number {
  const stageRect = stage.value?.getBoundingClientRect();
  const stageHeight = stageRect?.height ?? window.innerHeight;
  const toolbarRect = stage.value
    ?.querySelector(".editor-tools-overlay")
    ?.getBoundingClientRect();
  const toolbarClearance =
    stageRect && toolbarRect
      ? Math.max(96, stageRect.bottom - toolbarRect.bottom - 8)
      : Math.floor(stageHeight * 0.75);
  const max = Math.max(72, Math.min(Math.floor(stageHeight - 36), Math.floor(toolbarClearance)));
  return Math.max(36, Math.min(max, Math.round(height)));
}

function onBottomPreviewResizePointerDown(e: PointerEvent) {
  if (!editorBottomPreviewVisible.value) return;
  e.preventDefault();
  e.stopPropagation();
  startBottomPreviewResize(e.clientY);
  (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
}

function onBottomPreviewResizePointerMove(e: PointerEvent) {
  if (!editorBottomPreviewDragStart.value) return;
  e.preventDefault();
  e.stopPropagation();
  updateBottomPreviewResize(e.clientY);
}

function onBottomPreviewResizePointerUp(e: PointerEvent) {
  if (!editorBottomPreviewDragStart.value) return;
  e.preventDefault();
  e.stopPropagation();
  stopBottomPreviewResize();
  (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
}

function startBottomPreviewResize(clientY: number) {
  stopBottomPreviewResize();
  editorBottomPreviewDragStart.value = {
    y: clientY,
    height: editorBottomPreviewHeight.value,
  };
  window.addEventListener("mousemove", onBottomPreviewResizeMouseMove);
  window.addEventListener("mouseup", onBottomPreviewResizeMouseUp);
}

function updateBottomPreviewResize(clientY: number) {
  const drag = editorBottomPreviewDragStart.value;
  if (!drag) return;
  editorBottomPreviewHeight.value = clampEditorBottomPreviewHeight(
    drag.height + drag.y - clientY,
  );
}

function stopBottomPreviewResize() {
  editorBottomPreviewDragStart.value = null;
  window.removeEventListener("mousemove", onBottomPreviewResizeMouseMove);
  window.removeEventListener("mouseup", onBottomPreviewResizeMouseUp);
}

function onBottomPreviewResizeMouseDown(e: MouseEvent) {
  if (!editorBottomPreviewVisible.value) return;
  e.preventDefault();
  e.stopPropagation();
  startBottomPreviewResize(e.clientY);
}

function onBottomPreviewResizeMouseMove(e: MouseEvent) {
  if (!editorBottomPreviewDragStart.value) return;
  e.preventDefault();
  updateBottomPreviewResize(e.clientY);
}

function onBottomPreviewResizeMouseUp(e: MouseEvent) {
  if (!editorBottomPreviewDragStart.value) return;
  e.preventDefault();
  stopBottomPreviewResize();
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

function isPlainTextLeftPointer(e: PointerEvent): boolean {
  return activeTool.value === "Text" && e.button === 0 && !e.shiftKey;
}

function isPlainTextLeftDrag(e: PointerEvent): boolean {
  return activeTool.value === "Text" && (e.buttons & 1) !== 0 && !e.shiftKey;
}

function onPointerDown(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  selectIdleHoverActive = false;
  textPointerMayMutate = activeTool.value === "Text" && e.shiftKey;
  textKerningNeedsSync = false;
  const previousTextSort = textPointerMayMutate ? activeTextSortIndex.value : null;
  dismissBackgroundImageContextMenu();
  dismissContourContextMenu();
  if (backgroundImage.value?.selected) {
    backgroundImage.value = {
      ...backgroundImage.value,
      selected: false,
    };
    if (isPlainTextLeftPointer(e)) {
      requestRender({ refreshDerivedState: false });
      return;
    }
  } else if (isPlainTextLeftPointer(e)) {
    return;
  }
  (e.target as Element).setPointerCapture?.(e.pointerId);
  editor.pointerDown(c[0], c[1], e.button, modBits(e));
  refreshMeasureState();
  if (textPointerMayMutate) {
    refreshTextStateFromEditor();
    if (activeTextSortIndex.value !== previousTextSort) {
      loadActiveTextSortGlyphIntoEditor();
    }
  }
  requestRender({
    refreshDerivedState: false,
    refreshSelectionState: activeTool.value === "Select",
  });
}

function onCanvasContextMenu(e: MouseEvent) {
  dismissBackgroundImageContextMenu();
  dismissContourContextMenu();
  if (!editor || viewMode.value !== "editor") return;
  const c = canvasMouseCoords(e);
  if (!c) return;
  const design = editor.screenToDesign(c[0], c[1]);
  const anchor = parseAnchorContext(editor.anchorContextAt(c[0], c[1]));
  if (anchor) {
    editor.selectAnchorAt(c[0], c[1]);
    refreshSelectionState();
    requestRender({ refreshDerivedState: false });
  }
  const info = editor.contourContextAt(c[0], c[1]);
  if (info.length >= 4) {
    e.preventDefault();
    contourContextMenu.value = {
      x: e.clientX,
      y: e.clientY,
      screenX: info[4] ?? c[0],
      screenY: info[5] ?? c[1],
      designX: design[0],
      designY: design[1],
      pathIndex: info[0],
      canSetStart: info[1] > 0,
      canRoundCorners: selectionCount.value > 0,
      canMoveUp: info[2] > 0,
      canMoveDown: info[3] > 0,
      canAddAnchor: true,
      canEditAnchor: Boolean(anchor),
      anchorName: anchor?.name ?? undefined,
      anchorX: anchor?.x,
      anchorY: anchor?.y,
    };
    return;
  }
  if (currentGlyph.value) {
    e.preventDefault();
    contourContextMenu.value = {
      x: e.clientX,
      y: e.clientY,
      screenX: c[0],
      screenY: c[1],
      designX: design[0],
      designY: design[1],
      pathIndex: null,
      canSetStart: false,
      canRoundCorners: false,
      canMoveUp: false,
      canMoveDown: false,
      canAddAnchor: true,
      canEditAnchor: Boolean(anchor),
      anchorName: anchor?.name ?? undefined,
      anchorX: anchor?.x,
      anchorY: anchor?.y,
    };
    return;
  }
  if (lockedBackgroundImageContainsScreenPoint(c[0], c[1])) {
    openLockedBackgroundImageContextMenu(e);
  }
}

function applyContourContextMenuAction(
  action:
    | "set-start"
    | "reverse"
    | "round-corners"
    | "move-up"
    | "move-down"
    | "add-anchor"
    | "delete-anchor",
) {
  const menu = contourContextMenu.value;
  if (!editor || !menu) return;
  if (action === "add-anchor") {
    const name = defaultAnchorNameForPosition(menu.designY);
    const changed = applyEditorMutation(() => editor.addAnchorAt(menu.screenX, menu.screenY, name));
    dismissContourContextMenu();
    if (changed) status.value = "anchor added";
    return;
  }
  if (action === "delete-anchor") {
    const changed = applySelectionEdit("delete");
    dismissContourContextMenu();
    if (changed) status.value = "anchor deleted";
    return;
  }
  if (menu.pathIndex === null) return;
  const changed =
    action === "set-start"
      ? applyEditorMutation(() => editor.setStartPointAt(menu.screenX, menu.screenY))
      : action === "reverse"
        ? applyEditorMutation(() => editor.reverseContourAt(menu.screenX, menu.screenY))
        : action === "round-corners"
          ? applyEditorMutation(() => editor.roundSelectedCorners())
        : applyEditorMutation(() =>
            editor.moveContour(menu.pathIndex ?? 0, action === "move-up" ? "up" : "down"),
          );
  dismissContourContextMenu();
  if (changed) {
    status.value =
      action === "set-start"
        ? "start point set"
        : action === "reverse"
          ? "contour reversed"
          : action === "round-corners"
            ? "corners rounded"
          : "contour reordered";
  }
}

function onPointerMove(e: PointerEvent) {
  if (!editor) return;
  const pointerActive = e.buttons !== 0;
  if (!textPointerMayMutate && isPlainTextLeftDrag(e)) return;
  if (
    !pointerActive &&
    (activeTool.value === "Knife" ||
      activeTool.value === "Measure" ||
      activeTool.value === "Shapes" ||
      activeTool.value === "Text" ||
      activeTool.value === "Preview")
  ) {
    return;
  }
  if (!pointerActive && activeTool.value === "Select" && !e.altKey && !selectIdleHoverActive) {
    return;
  }
  // Store the latest event; the rAF callback drains it once per frame.
  // This coalesces high-Hz input (tablets at 200Hz, gaming mice at 1000Hz)
  // down to one WASM call per rendered frame.
  pendingPointerMove = e;
  requestRender({ refreshDerivedState: false });
}

function onPointerUp(e: PointerEvent) {
  if (!editor) return;
  const c = canvasCoords(e);
  if (!c) return;
  if (!textPointerMayMutate && isPlainTextLeftPointer(e)) {
    textPointerMayMutate = false;
    return;
  }
  const shouldRefreshTextPointer = textPointerMayMutate;
  const previousTextSort = shouldRefreshTextPointer ? activeTextSortIndex.value : null;
  const changed = editor.pointerUp(c[0], c[1], e.button, modBits(e));
  textPointerMayMutate = false;
  (e.target as Element).releasePointerCapture?.(e.pointerId);
  if (changed && currentGlyph.value) {
    const glyphName = currentGlyph.value;
    const masterName = activeMasterName.value;
    if (activeTool.value === "Select" && masterName) {
      editorGlyphNeedsSync = true;
      markGlyphDirty(glyphName, masterName);
      scheduleDeferredGlyphSync(glyphName, masterName);
    } else {
      const glyphChanged = syncCurrentGlyphBytesFromEditor({ skipUnchanged: true });
      if (glyphChanged) {
        markGlyphDirty(glyphName);
        queueComfyStateSync();
      }
    }
  }
  if (activeTool.value === "Select" && !shouldRefreshTextPointer) {
    applyEditorPanelState(editor.editorPanelState());
    requestRender({ refreshDerivedState: false });
    return;
  }
  refreshSelectionState();
  refreshMeasureState();
  if (shouldRefreshTextPointer) {
    refreshTextStateFromEditor();
    if (activeTextSortIndex.value !== previousTextSort) {
      loadActiveTextSortGlyphIntoEditor();
    }
    if (textKerningNeedsSync) {
      syncTextKerningModelFromEditor(true);
      textKerningNeedsSync = false;
    }
  }
  requestRender({ refreshDerivedState: false });
}

function onPointerCancel() {
  if (!editor) return;
  if (textKerningNeedsSync) {
    syncTextKerningModelFromEditor(true);
    textKerningNeedsSync = false;
  }
  textPointerMayMutate = false;
  const changed = editor.pointerCancel();
  if (changed) {
    syncEditorMutationAfterWasmChange();
    return;
  }
  refreshMeasureState();
  requestRender({ refreshDerivedState: false });
}

function onCanvasDoubleClick(e: MouseEvent) {
  if (!editor) return;
  const c = canvasMouseCoords(e);
  if (!c) return;
  if (hasTextBufferSession.value && activateInactiveTextSortAt(c[0], c[1])) {
    return;
  }
  if (editor.togglePointTypeAt(c[0], c[1])) {
    syncEditorMutationAfterWasmChange();
    return;
  }
  if (editor.selectContourAt(c[0], c[1])) {
    refreshSelectionState();
    requestRender();
    return;
  }
  const baseName = editor.componentBaseAt(c[0], c[1]);
  if (baseName && hasTextBufferSession.value && activeMasterData.value?.glyphBytes.has(baseName)) {
    insertInactiveTextGlyphByName(baseName);
    editor.clearComponentSelection();
    refreshSelectionState();
    requestRender();
    queueComfyStateSync();
    return;
  }
  activateInactiveTextSortAt(c[0], c[1]);
}

function activateInactiveTextSortAt(x: number, y: number): boolean {
  if (!editor) return false;
  const activatedTextSortState = editor.activateTextSortAtState(x, y);
  if (activatedTextSortState.length >= 2) {
    const activatedTextSort = activatedTextSortState[0];
    activeTextSortIndex.value = activatedTextSort;
    textCursor.value = activatedTextSortState[1];
    const glyphName = textGlyphNameAt(activatedTextSort);
    if (glyphName) {
      if (selectedGlyph.value !== glyphName) {
        selectedGlyph.value = glyphName;
      }
      if (!selectedGlyphs.value.has(glyphName) || selectedGlyphs.value.size !== 1) {
        selectedGlyphs.value = new Set([glyphName]);
      }
      if (glyphName === currentGlyph.value) {
        requestRender({ refreshDerivedState: false });
        return;
      }
      loadActiveTextSortGlyphIntoEditor();
    } else {
      refreshTextStateFromEditor();
      requestRender();
    }
    return true;
  }
  return false;
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
  leftKerningGroup: string | null;
  rightKerningGroup: string | null;
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
    leftKerningGroup: metadata.leftKerningGroup ?? null,
    rightKerningGroup: metadata.rightKerningGroup ?? null,
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

function plistKerningGroupsForGlyph(
  groupsMap: Map<string, string[]>,
  glyphName: string,
): GlyphKerningGroups {
  const groups: GlyphKerningGroups = {};
  for (const [groupName, members] of groupsMap) {
    if (!members.includes(glyphName)) continue;
    if (groupName.startsWith("public.kern1.") && !groups.left) {
      groups.left = groupName;
    } else if (groupName.startsWith("public.kern2.") && !groups.right) {
      groups.right = groupName;
    }
  }
  return groups;
}

function setGlyphKerningGroupsFromInfo(
  data: MasterData,
  glyphName: string,
  info: { leftKerningGroup: string | null; rightKerningGroup: string | null },
) {
  const nextGroups = {
    ...plistKerningGroupsForGlyph(data.groups, glyphName),
    ...(info.leftKerningGroup ? { left: info.leftKerningGroup } : {}),
    ...(info.rightKerningGroup ? { right: info.rightKerningGroup } : {}),
  };
  if (nextGroups.left || nextGroups.right) {
    data.glyphKerningGroups.set(glyphName, nextGroups);
  } else {
    data.glyphKerningGroups.delete(glyphName);
  }
}

function textKerningGroupsForEditor(): Map<string, string[]> {
  const merged = new Map<string, string[]>();
  for (const [groupName, members] of groups.value) {
    merged.set(groupName, [...members]);
  }
  for (const [glyphName, glyphGroups] of glyphKerningGroups.value) {
    for (const groupName of [glyphGroups.left, glyphGroups.right]) {
      if (!groupName) continue;
      const members = merged.get(groupName) ?? [];
      if (!members.includes(glyphName)) {
        merged.set(groupName, [...members, glyphName]);
      }
    }
  }
  return merged;
}

function textKerningGroupHintsForEditor(side: "left" | "right"): Record<string, string> {
  const hints: Record<string, string> = {};
  for (const [glyphName, glyphGroups] of glyphKerningGroups.value) {
    const groupName = glyphGroups[side];
    if (groupName) hints[glyphName] = groupName;
  }
  return hints;
}

function textGlyphNameAt(index: number | null): string | null {
  if (index === null || index < 0 || index >= textBuffer.value.length) return null;
  const sort = textBuffer.value[index];
  return sort?.kind === "glyph" ? sort.glyphName : null;
}

function kerningGroupsForGlyph(glyphName: string, hint?: string): string[] {
  const groupMap = textKerningGroupsForEditor();
  const groupNames: string[] = [];
  if (hint && groupMap.get(hint)?.includes(glyphName)) {
    groupNames.push(hint);
  }
  for (const [groupName, members] of groupMap) {
    if (members.includes(glyphName) && !groupNames.includes(groupName)) {
      groupNames.push(groupName);
    }
  }
  return groupNames;
}

function lookupKerningValue(left: string, right: string): number {
  const leftGroups = kerningGroupsForGlyph(left, glyphKerningGroups.value.get(left)?.right);
  const rightGroups = kerningGroupsForGlyph(right, glyphKerningGroups.value.get(right)?.left);
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
  const pairs = data.kerning.get(pair[0]) ?? new Map<string, number>();

  if (!value || value === "-") {
    pairs.delete(pair[1]);
  } else {
    if (value.trim() !== value) return;
    const kernValue = Number(value);
    if (!Number.isFinite(kernValue)) return;
    pairs.set(pair[1], kernValue);
  }

  if (pairs.size > 0) {
    data.kerning.set(pair[0], pairs);
  } else {
    data.kerning.delete(pair[0]);
  }
  masterDataMap.value = new Map(masterDataMap.value);
  markKerningDirty();
  syncTextKerningModelToEditor();
  requestRender();
  queueComfyStateSync();
}

function updateGlyphKerningGroup(side: "left" | "right", value: string) {
  const glyphName = currentGlyph.value;
  const data = activeMasterData.value;
  if (!glyphName || !data) return;
  const bytes = data.glyphBytes.get(glyphName);
  if (!bytes) return;
  const nextGroup = value;
  const currentGroups = data.glyphKerningGroups.get(glyphName);
  const currentGroup = side === "left" ? currentGroups?.left : currentGroups?.right;
  if ((currentGroup ?? "") === nextGroup) return;

  const nextBytes = glifWithKerningGroup(bytes, side, nextGroup);
  setGlyphBytes(data, glyphName, nextBytes);
  const nextGroups = { ...(currentGroups ?? {}) };
  if (!nextGroup || nextGroup === "-") {
    delete nextGroups[side];
  } else {
    nextGroups[side] = nextGroup;
  }
  if (nextGroups.left || nextGroups.right) {
    data.glyphKerningGroups.set(glyphName, nextGroups);
  } else {
    data.glyphKerningGroups.delete(glyphName);
  }
  masterDataMap.value = new Map(masterDataMap.value);
  markGlyphDirty(glyphName);
  syncTextKerningModelToEditor();
  requestRender();
  queueComfyStateSync();
}

function syncTextKerningModelToEditor() {
  if (!editor) return;
  try {
    editor.setTextKerningModel(
      JSON.stringify({
        groups: stringArrayMapToRecord(textKerningGroupsForEditor()),
        leftGroups: textKerningGroupHintsForEditor("left"),
        rightGroups: textKerningGroupHintsForEditor("right"),
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

function gridGlyphSvgWithComponents(
  bytes: Uint8Array,
  glyphXmlByName: string,
  unitsPerEm: number,
): string {
  // Grid thumbnails must use the same em-based viewBox on load and after
  // edits. The single-glyph SVG helper fits to ink bounds, which makes edited
  // glyphs visibly resize relative to untouched glyphs.
  try {
    const svg = glifToGridSvgWithComponents(bytes, glyphXmlByName, unitsPerEm);
    if (svg) return svg;
  } catch (err) {
    console.warn("[runebender] edited glyph grid SVG refresh failed", err);
  }
  return glifToSvgWithComponents(bytes, glyphXmlByName) || glifToSvg(bytes);
}

function refreshGridGlyphSvg(
  data: MasterData,
  glyphName: string,
  bytes: Uint8Array,
): string {
  const svg = gridGlyphSvgWithComponents(
    bytes,
    cachedGlyphXmlByName(data),
    data.unitsPerEm,
  );
  if (svg) {
    data.glyphSvgs.set(glyphName, svg);
  } else {
    data.glyphSvgs.delete(glyphName);
  }
  return svg;
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
  selectedSidebarFilter.value = { kind: "all" };
  sidebarSearchQuery.value = "";
  hasTextBufferSession.value = false;
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
  sourceSaveLabel.value = null;
  designspacePath.value = null;
  designspaceText.value = "";
  designspaceFileHandle.value = null;
  designspaceDirty.value = false;

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
  designspacePath.value = relPath(dsFile);
  designspaceText.value = xml;
  designspaceFileHandle.value = fileHandles.get(designspacePath.value) ?? null;
  designspaceDirty.value = false;
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
  const data = await runebenderHost.loadWorkspaceSlot(slot);
  if (!data) {
    status.value = `failed to load workspace ${slot}`;
    return;
  }
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
  sourceSaveLabel.value = data.linked_source
    ? `Editing ${data.origin_source || data.origin_root || "source"}`
    : "Managed copy (workspace cache)";
  if (data.refreshed_from_source) {
    status.value = "reloaded source changes from disk";
  }
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
  const glyphLibKerningGroups = new Map<string, GlyphKerningGroups>();
  for (const item of loaded) {
    if (!item) continue;
    const {
      name,
      unicode,
      unicodes,
      markColor,
      leftKerningGroup,
      rightKerningGroup,
      width,
      contours,
      bytes,
      path,
    } = item;
    if (!name) continue;
    glyphBytes.set(name, bytes);
    glyphPaths.set(name, path);
    glyphMetadata.set(name, { name, width, contours, unicode, unicodes });
    if (unicode) glyphUnicodes.set(name, unicode);
    if (markColor) glyphMarkColors.set(name, markColor);
    if (leftKerningGroup || rightKerningGroup) {
      glyphLibKerningGroups.set(name, {
        ...(leftKerningGroup ? { left: leftKerningGroup } : {}),
        ...(rightKerningGroup ? { right: rightKerningGroup } : {}),
      });
    }
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

  // Read unitsPerEm before building grid thumbnails: the grid SVGs use
  // a constant em-based viewBox so every glyph shares one scale/baseline
  // (matching runebender-xilem). Without the correct UPM the thumbnails
  // would be scaled against a 1000-unit default and look slightly off.
  const fontInfoFile = ufoFiles.find((f) =>
    /\/fontinfo\.plist$/i.test(relPath(f)),
  );
  const fontInfoBytes = fontInfoFile
    ? new Uint8Array(await fontInfoFile.arrayBuffer())
    : null;
  const unitsPerEm = fontInfoBytes ? extractUnitsPerEm(fontInfoBytes) : 1000;

  const glyphSvgs = await buildGridSvgsForMap(glyphBytes, unitsPerEm);

  const groupsFile = ufoFiles.find((f) =>
    /\/groups\.plist$/i.test(relPath(f)),
  );
  const groups = groupsFile
    ? parseGroupsPlist(new Uint8Array(await groupsFile.arrayBuffer()))
    : new Map<string, string[]>();
  const groupsPath = groupsFile ? relPath(groupsFile) : inferGroupsPath(glifs);
  const groupsFileHandle = groupsPath ? (fileHandles.get(groupsPath) ?? null) : null;
  const glyphKerningGroups = buildGlyphKerningGroups(groups);
  for (const [glyphName, glyphGroups] of glyphLibKerningGroups) {
    glyphKerningGroups.set(glyphName, {
      ...(glyphKerningGroups.get(glyphName) ?? {}),
      ...glyphGroups,
    });
  }
  const kerningFile = ufoFiles.find((f) =>
    /\/kerning\.plist$/i.test(relPath(f)),
  );
  const kerningPath = kerningFile ? relPath(kerningFile) : inferKerningPath(glifs);
  const kerningFileHandle = kerningPath ? (fileHandles.get(kerningPath) ?? null) : null;
  const kerning = kerningFile
    ? parseKerningPlist(new Uint8Array(await kerningFile.arrayBuffer()))
    : new Map<string, Map<string, number>>();

  return {
    glyphBytes,
    glyphXmlByName: glyphXmlMapJson(glyphBytes),
    glyphXmlVersion: 0,
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
  unitsPerEm: number,
): Promise<Map<string, string>> {
  // Single WASM call processes the entire master in Rust. Previous
  // version called glifToSvgWithComponents 600+ times per master from
  // a JS loop; the JS↔WASM boundary crossings, not the rendering work
  // itself, made the edit-to-grid load take ~2.5s. The batched
  // implementation is ~25x faster on a 333-glyph master.
  const names = Array.from(glyphBytes.keys()).sort();
  const glyphXmlByName = glyphXmlMapJson(glyphBytes, names);
  try {
    const out = glifMapToSvgs(glyphXmlByName, unitsPerEm);
    const parsed = JSON.parse(out) as Record<string, string>;
    const svgs = new Map<string, string>();
    for (const name of names) {
      const svg = parsed[name];
      if (svg) svgs.set(name, svg);
    }
    return svgs;
  } catch (err) {
    console.warn(
      "[runebender] glifMapToSvgs batch failed, falling back to per-glyph loop",
      err,
    );
    // Defensive fallback: if the batch call ever fails, fall back to
    // the old per-glyph path so the editor still loads with thumbnails.
    const svgs = new Map<string, string>();
    for (const name of names) {
      const bytes = glyphBytes.get(name);
      if (!bytes) continue;
      try {
        const svg = glifToSvgWithComponents(bytes, glyphXmlByName) || glifToSvg(bytes);
        if (svg) svgs.set(name, svg);
      } catch {
        // Skip malformed glyphs silently.
      }
    }
    return svgs;
  }
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

function cachedGlyphXmlByName(data: MasterData): string {
  if (data.glyphXmlByName === null) {
    data.glyphXmlByName = glyphXmlMapJson(data.glyphBytes);
  }
  return data.glyphXmlByName;
}

function setGlyphBytes(
  data: MasterData,
  name: string,
  bytes: Uint8Array,
  options: { syncComponentCache?: boolean } = {},
) {
  data.glyphBytes.set(name, bytes);
  data.glyphXmlByName = null;
  data.glyphXmlVersion += 1;
  if (options.syncComponentCache !== false) {
    syncEditorComponentGlyphCacheEntry(data, name, bytes);
  }
}

function deleteGlyphBytes(data: MasterData, name: string): boolean {
  const deleted = data.glyphBytes.delete(name);
  if (deleted) {
    data.glyphXmlByName = null;
    data.glyphXmlVersion += 1;
    syncEditorComponentGlyphCacheEntry(data, name, null);
  }
  return deleted;
}

function syncEditorComponentGlyphCacheEntry(
  data: MasterData,
  name: string,
  bytes: Uint8Array | null,
) {
  if (!editor || editorComponentGlyphsData !== data) return;
  try {
    if (bytes) {
      editor.setComponentGlyph(name, bytes);
    } else {
      editor.deleteComponentGlyph(name);
    }
    editorComponentGlyphsVersion = data.glyphXmlVersion;
  } catch (e) {
    console.warn("[runebender] incremental component cache update failed:", e);
    editorComponentGlyphsData = null;
    editorComponentGlyphsVersion = -1;
  }
}

function ensureEditorComponentGlyphs(data: MasterData) {
  if (!editor) return;
  if (
    editorComponentGlyphsData === data &&
    editorComponentGlyphsVersion === data.glyphXmlVersion
  ) {
    return;
  }
  editor.setComponentGlyphs(cachedGlyphXmlByName(data));
  editorComponentGlyphsData = data;
  editorComponentGlyphsVersion = data.glyphXmlVersion;
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
  flushDeferredGlyphSync();
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
  // If a Text sort is active, it owns the editable glyph when leaving
  // Text mode and when switching masters, matching xilem's active_sort_name.
  syncTextSortMetricsToActiveMaster();
  const glyphToReload = textGlyphNameAt(activeTextSortIndex.value) ?? currentGlyph.value;
  if (glyphToReload && canvas.value) {
    const bytes = data.glyphBytes.get(glyphToReload);
    if (bytes) {
      try {
        ensureEditorComponentGlyphs(data);
        if (!editor.setGlyphNameWithCachedComponentsPreserveHistory(glyphToReload)) {
          editor.setGlyphGlifWithCachedComponentsPreserveHistory(bytes);
        }
        editorGlyphNeedsSync = false;
        currentGlyph.value = glyphToReload;
        selectedGlyph.value = glyphToReload;
        selectedGlyphs.value = new Set([glyphToReload]);
        applyEditorPanelState(editor.editorPanelState());
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

function onSelectSidebarFilter(filter: GlyphSidebarFilter) {
  selectedSidebarFilter.value = filter;
  selectedCategory.value = filter.kind === "category" ? filter.category : "All";
  if (gridView.value) {
    gridView.value.scrollTop = 0;
  }
  ensureGridSelection();
}

function setPrimarySelectedGlyph(name: string) {
  pendingGridSelectionName = "";
  if (pendingGridSelectionRaf !== null) {
    cancelAnimationFrame(pendingGridSelectionRaf);
    pendingGridSelectionRaf = null;
  }
  if (pendingGridScrollRaf !== null) {
    cancelAnimationFrame(pendingGridScrollRaf);
    pendingGridScrollRaf = null;
    pendingGridScrollIndex = -1;
  }
  selectedGlyph.value = name;
  selectedGlyphs.value = new Set(name ? [name] : []);
  scrollSelectedGlyphIntoView();
}

function setPrimarySelectedGlyphFromKeyboard(name: string, index: number) {
  const previousIndex = filteredGlyphNames.value.indexOf(
    pendingGridSelectionName || selectedGlyph.value,
  );
  applyImmediateGridSelection(previousIndex, index);
  pendingGridSelectionName = name;
  scrollGlyphIndexIntoView(index);
  if (pendingGridSelectionRaf !== null) return;
  pendingGridSelectionRaf = requestAnimationFrame(() => {
    pendingGridSelectionRaf = null;
    const next = pendingGridSelectionName;
    pendingGridSelectionName = "";
    selectedGlyph.value = next;
    selectedGlyphs.value = new Set(next ? [next] : []);
  });
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
  scrollGlyphIndexIntoView(index);
}

function scrollGlyphIndexIntoView(index: number) {
  pendingGridScrollIndex = index;
  if (pendingGridScrollRaf !== null) return;
  pendingGridScrollRaf = requestAnimationFrame(() => {
    pendingGridScrollRaf = null;
    const nextIndex = pendingGridScrollIndex;
    pendingGridScrollIndex = -1;
    const cell = gridView.value?.querySelector<HTMLElement>(
      `[data-glyph-index="${nextIndex}"]`,
    );
    cell?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });
}

function applyImmediateGridSelection(previousIndex: number, nextIndex: number) {
  const grid = gridView.value;
  if (!grid) return;
  if (previousIndex >= 0 && previousIndex !== nextIndex) {
    grid
      .querySelector<HTMLElement>(`[data-glyph-index="${previousIndex}"]`)
      ?.classList.remove("selected");
  }
  if (nextIndex >= 0) {
    grid
      .querySelector<HTMLElement>(`[data-glyph-index="${nextIndex}"]`)
      ?.classList.add("selected");
  }
}

function navigateGridSelection(direction: "left" | "right" | "up" | "down"): boolean {
  const names = filteredGlyphNames.value;
  if (names.length === 0) return false;
  const columns = gridColumnCount();
  const currentName = pendingGridSelectionName || selectedGlyph.value;
  const current = currentName
    ? names.indexOf(currentName)
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
  setPrimarySelectedGlyphFromKeyboard(names[nextIndex], nextIndex);
  return true;
}

function copyGridGlyph(): boolean {
  const data = activeMasterData.value;
  const name = currentPrimarySelectedGlyph();
  const bytes = name && data?.glyphBytes.get(name);
  if (!bytes) return false;
  gridGlyphClipboard.value = new Uint8Array(bytes);
  status.value = `copied ${name}`;
  return true;
}

function selectedGridGlyphNamesInVisibleOrder(): string[] {
  const data = activeMasterData.value;
  if (!data) return [];
  const selected = selectedGlyphs.value;
  const names = filteredGlyphNames.value.filter((name) =>
    selected.has(name) && data.glyphBytes.has(name),
  );
  if (names.length > 0) return names;
  const primary = currentPrimarySelectedGlyph();
  return primary && data.glyphBytes.has(primary) ? [primary] : [];
}

function glyphUnicodeText(name: string): string {
  const unicodes =
    glyphMetadataMap.value.get(name)?.unicodes ??
    glyphUnicodes.value.get(name)?.split(/[\s,]+/) ??
    [];
  return unicodes
    .map((hex) => Number.parseInt(hex.replace(/^U\+/i, ""), 16))
    .filter((cp) => Number.isFinite(cp) && cp >= 0 && cp <= 0x10ffff)
    .map((cp) => String.fromCodePoint(cp))
    .join("");
}

function selectedGridGlyphTextPieces(): Array<{ name: string; text: string }> {
  return selectedGridGlyphNamesInVisibleOrder()
    .map((name) => ({ name, text: glyphUnicodeText(name) }))
    .filter((item) => item.text.length > 0);
}

async function writeTextToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // Fall through to the selection-based copy path.
    }
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  document.body.appendChild(textarea);
  textarea.focus({ preventScroll: true });
  textarea.select();
  try {
    if (!document.execCommand("copy")) {
      throw new Error("copy command failed");
    }
  } finally {
    textarea.remove();
  }
}

async function copySelectedGridGlyphText(): Promise<boolean> {
  const pieces = selectedGridGlyphTextPieces();
  if (pieces.length === 0) {
    status.value = "selected glyphs have no Unicode text";
    return false;
  }
  const text = pieces.map((item) => item.text).join("");
  try {
    await writeTextToClipboard(text);
    status.value = `copied ${pieces.length} selected glyph${pieces.length === 1 ? "" : "s"} as text`;
    return true;
  } catch (e) {
    console.warn("copying selected glyph text failed:", e);
    status.value = `copy text failed: ${e}`;
    return false;
  }
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
      const metadata = {
        name,
        width: info.width,
        contours: info.contours,
        unicode: info.unicode,
        unicodes: info.unicodes,
      };
      setGlyphBytes(data, name, bytes);
      data.glyphMetadata.set(name, metadata);
      if (info.unicode) {
        data.glyphUnicodes.set(name, info.unicode);
      } else {
        data.glyphUnicodes.delete(name);
      }
      refreshGridGlyphSvg(data, name, bytes);
      if (hasTextBufferSession.value) {
        syncTextSortsForGlyph(name, name, metadata);
      }
      markGlyphDirty(name);
      if (name === currentGlyph.value && editor) {
        ensureEditorComponentGlyphs(data);
        if (!editor.setGlyphNameWithCachedComponents(name)) {
          editor.setGlyphGlifWithCachedComponents(bytes);
        }
        editorGlyphNeedsSync = false;
        applyEditorPanelState(editor.editorPanelState());
        updateCompatibilityErrors();
        requestRender();
      }
    }
    masterDataMap.value = new Map(masterDataMap.value);
    if (hasTextBufferSession.value) {
      syncTextKerningModelToEditor();
      bumpTextPreviewRevision();
    }
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
  const primary = currentPrimarySelectedGlyph();
  return primary ? [primary] : [];
}

function currentPrimarySelectedGlyph(): string {
  return pendingGridSelectionName || selectedGlyph.value;
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
  const primary = currentPrimarySelectedGlyph();
  if (e.key === "Enter" && primary) {
    setPrimarySelectedGlyph(primary);
    openGlyph(primary);
    return true;
  }
  return false;
}

function loadGlyphIntoEditor(
  name: string,
  options: {
    fitCanvas?: boolean;
    seedTextBuffer?: boolean;
    preserveHistory?: boolean;
    refreshCompatibility?: boolean;
    importImage?: boolean;
    syncComfy?: boolean;
    metricsOnly?: boolean;
  } = {},
) {
  if (!editor || !canvas.value) return;
  const previousGlyphName = currentGlyph.value;
  if (previousGlyphName && previousGlyphName !== name) {
    if (flushDeferredGlyphSync()) {
      markGlyphDirty(previousGlyphName);
    }
  }
  const data = activeMasterData.value;
  if (!data) return;
  const bytes = data.glyphBytes.get(name);
  if (!bytes) return;
  try {
    ensureEditorComponentGlyphs(data);
    if (options.preserveHistory) {
      if (!editor.setGlyphNameWithCachedComponentsPreserveHistory(name)) {
        editor.setGlyphGlifWithCachedComponentsPreserveHistory(bytes);
      }
    } else {
      if (!editor.setGlyphNameWithCachedComponents(name)) {
        editor.setGlyphGlifWithCachedComponents(bytes);
      }
      coordinateQuadrant.value = "cc";
    }
    editorGlyphNeedsSync = false;
    const previousSelection = new Set(selectedGlyphs.value);
    viewMode.value = "editor";
    currentGlyph.value = name;
    selectedGlyph.value = name;
    selectedGlyphs.value = previousSelection.has(name) ? previousSelection : new Set([name]);
    if (options.metricsOnly) {
      applyEditorMetricsState(editor.editorMetricsState());
    } else {
      applyEditorPanelState(editor.editorPanelState());
    }
    if (options.seedTextBuffer !== false) {
      syncTextKerningModelToEditor();
      seedTextBufferWithGlyph(name);
    }
    if (options.refreshCompatibility !== false) {
      updateCompatibilityErrors();
    }
    if (options.importImage !== false) {
      void importMatchingGlyphImage(name);
    }
    const renderOptions = {
      refreshDerivedState: options.refreshCompatibility !== false || options.importImage !== false,
    };
    if (options.fitCanvas) {
      // Canvas was visually hidden; let layout settle before sizing.
      requestAnimationFrame(() => {
        if (!editor || !canvas.value) return;
        handleResize();
        editor.fitToCanvas(canvas.value.width, canvas.value.height);
        requestRender(renderOptions);
      });
    } else {
      requestRender(renderOptions);
    }
    if (options.syncComfy !== false) {
      queueComfyStateSync(true);
    }
  } catch (e) {
    console.error(e);
    status.value = `failed to load ${name}: ${e}`;
  }
}

function loadActiveTextSortGlyphIntoEditor() {
  const glyphName = textGlyphNameAt(activeTextSortIndex.value);
  if (glyphName) {
    loadGlyphIntoEditor(glyphName, {
      fitCanvas: false,
      seedTextBuffer: false,
      preserveHistory: true,
      refreshCompatibility: false,
      importImage: false,
      syncComfy: false,
      metricsOnly: true,
    });
  }
}

function openGlyph(name: string) {
  loadGlyphIntoEditor(name, { fitCanvas: true });
}

function openGridSelectionInEditor(name: string) {
  const selected = selectedGridGlyphNames();
  const visibleOrder = filteredGlyphNames.value;
  const names = selected.includes(name)
    ? visibleOrder.filter((glyphName) => selected.includes(glyphName))
    : [name];
  selectedGlyph.value = name;
  selectedGlyphs.value = new Set(names);
  loadGlyphIntoEditor(name, { fitCanvas: true, seedTextBuffer: names.length <= 1 });
  if (names.length > 1) {
    seedTextBufferWithGlyphs(names, name);
    status.value = `opened ${names.length} glyphs`;
    requestRender();
  }
}

/// Apply (or clear) a mark color on the selected glyph. RGBA is
/// the UFO `public.markColor` string "r,g,b,a"; empty string clears.
/// Defaults to the active master; the color panel can opt into all masters.
function setMarkOnSelected(rgba: string) {
  const names = selectedGridGlyphNames();
  if (names.length === 0) return;
  const targetMasterNames = markColorApplyAllMasters.value
    ? masters.value
    : activeMasterName.value
      ? [activeMasterName.value]
      : [];
  const targets = targetMasterNames
    .map((masterName) => [masterName, masterDataMap.value.get(masterName)] as const)
    .filter((entry): entry is readonly [string, MasterData] => !!entry[1]);
  if (targets.length === 0) return;

  for (const [, data] of targets) {
    for (const name of names) {
      if (!data.glyphBytes.has(name)) continue;
      if (rgba) {
        data.glyphMarkColors.set(name, rgba);
      } else {
        data.glyphMarkColors.delete(name);
      }
    }
  }

  // Trigger reactivity — the inner Map mutation isn't observable;
  // replace the outer masterDataMap reference so dependent computeds
  // (glyphMarkColors, the cells) re-run.
  masterDataMap.value = new Map(masterDataMap.value);

  // If this glyph is open, keep the active master's in-memory .glif
  // bytes aligned without losing unsaved outline edits from the editor.
  if (names.includes(currentGlyph.value)) {
    syncCurrentGlyphBytesFromEditor();
  }

  for (const [masterName, data] of targets) {
    for (const name of names) {
      if (masterName === activeMasterName.value && name === currentGlyph.value) continue;
      const originalBytes = data.glyphBytes.get(name);
      if (originalBytes) {
        try {
          const bytes = glifWithMarkColor(originalBytes, rgba);
          setGlyphBytes(data, name, bytes);
          refreshGridGlyphSvg(data, name, bytes);
        } catch (e) {
          console.warn("serializing mark color failed:", e);
        }
      }
    }
  }

  for (const [masterName, data] of targets) {
    for (const name of names) {
      if (data.glyphBytes.has(name)) markGlyphDirty(name, masterName);
    }
  }
  masterDataMap.value = new Map(masterDataMap.value);
  queueComfyStateSync();
}

function backToGrid() {
  flushDeferredGlyphSync();
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
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  requestRender({ refreshCompatibilityErrors: true });
  queueComfyStateSync();
}

function applySelectionEdit(action: "delete" | "toggle-point") {
  if (!editor || !currentGlyph.value) return false;
  const changed =
    action === "delete" ? editor.deleteSelection() : editor.togglePointType();
  if (!changed) return false;
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  requestRender({ refreshCompatibilityErrors: true });
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
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  requestRender({ refreshCompatibilityErrors: true });
  queueComfyStateSync();
  return true;
}

function flushDeferredGlyphSync(): boolean {
  cancelDeferredGlyphSyncCommit();
  flushPostPaintNudgeSelectionState();
  if (deferredGlyphSyncTimer !== null) {
    window.clearTimeout(deferredGlyphSyncTimer);
    deferredGlyphSyncTimer = null;
  }
  if (!editorGlyphNeedsSync) {
    return flushDeferredGlyphDerivedRefresh();
  }
  if (deferredGlyphDerivedSyncTimer !== null) {
    window.clearTimeout(deferredGlyphDerivedSyncTimer);
    deferredGlyphDerivedSyncTimer = null;
    deferredGlyphDerivedSyncGlyph = "";
    deferredGlyphDerivedSyncMaster = "";
  }
  editor?.finishNudgeSelection();
  nudgePreviewActive = false;
  const syncPerf = pendingNudgePerf ?? lastRenderedNudgePerf ?? undefined;
  if (syncPerf) syncPerf.syncStart = performance.now();
  const glyphName = currentGlyph.value;
  const masterName = activeMasterName.value;
  const changed = syncCurrentGlyphBytesFromEditor({
    skipUnchanged: true,
    preserveMetadata: true,
  });
  if (syncPerf) {
    syncPerf.syncEnd = performance.now();
    logNudgeSyncPerf(syncPerf);
    if (pendingNudgePerf === syncPerf) pendingNudgePerf = null;
    if (lastRenderedNudgePerf === syncPerf) lastRenderedNudgePerf = null;
  }
  if (changed && glyphName) {
    markGlyphDirty(glyphName, masterName);
  }
  return changed;
}

function refreshDeferredGlyphDerivedState(glyphName: string, masterName: string): boolean {
  const data = masterDataMap.value.get(masterName);
  const bytes = data?.glyphBytes.get(glyphName);
  if (!data || !bytes) return false;
  syncEditorComponentGlyphCacheEntry(data, glyphName, bytes);
  refreshGridGlyphSvg(data, glyphName, bytes);
  if (hasTextBufferSession.value) {
    const metadata = data.glyphMetadata.get(glyphName);
    if (metadata) {
      syncCurrentTextSorts(metadata);
      bumpTextPreviewRevision();
    }
  }
  if (currentGlyph.value === glyphName && activeMasterName.value === masterName) {
    updateCompatibilityErrors();
    queueComfyStateSync();
  }
  masterDataMap.value = new Map(masterDataMap.value);
  return true;
}

function flushDeferredGlyphDerivedRefresh(): boolean {
  if (deferredGlyphDerivedSyncTimer === null) return false;
  window.clearTimeout(deferredGlyphDerivedSyncTimer);
  deferredGlyphDerivedSyncTimer = null;
  const glyphName = deferredGlyphDerivedSyncGlyph;
  const masterName = deferredGlyphDerivedSyncMaster;
  deferredGlyphDerivedSyncGlyph = "";
  deferredGlyphDerivedSyncMaster = "";
  return refreshDeferredGlyphDerivedState(glyphName, masterName);
}

function cancelDeferredGlyphSyncCommit() {
  if (deferredGlyphSyncCommitRaf !== null) {
    cancelAnimationFrame(deferredGlyphSyncCommitRaf);
    deferredGlyphSyncCommitRaf = null;
  }
  if (deferredGlyphSyncCommitTimer !== null) {
    window.clearTimeout(deferredGlyphSyncCommitTimer);
    deferredGlyphSyncCommitTimer = null;
  }
}

function scheduleDeferredGlyphSyncCommitAfterPaint() {
  cancelDeferredGlyphSyncCommit();
  if (deferredGlyphSyncTimer !== null) {
    window.clearTimeout(deferredGlyphSyncTimer);
    deferredGlyphSyncTimer = null;
  }
  deferredGlyphSyncCommitRaf = requestAnimationFrame(() => {
    deferredGlyphSyncCommitRaf = null;
    deferredGlyphSyncCommitTimer = window.setTimeout(() => {
      deferredGlyphSyncCommitTimer = null;
      flushDeferredGlyphSync();
    }, 0);
  });
}

function scheduleDeferredGlyphDerivedRefresh(glyphName: string, masterName: string) {
  if (deferredGlyphDerivedSyncTimer !== null) {
    window.clearTimeout(deferredGlyphDerivedSyncTimer);
  }
  deferredGlyphDerivedSyncGlyph = glyphName;
  deferredGlyphDerivedSyncMaster = masterName;
  deferredGlyphDerivedSyncTimer = window.setTimeout(() => {
    deferredGlyphDerivedSyncTimer = null;
    const pendingGlyph = deferredGlyphDerivedSyncGlyph;
    const pendingMaster = deferredGlyphDerivedSyncMaster;
    deferredGlyphDerivedSyncGlyph = "";
    deferredGlyphDerivedSyncMaster = "";
    if (currentGlyph.value !== pendingGlyph || activeMasterName.value !== pendingMaster) return;
    refreshDeferredGlyphDerivedState(pendingGlyph, pendingMaster);
  }, 360);
}

function scheduleDeferredGlyphSync(glyphName: string, masterName: string) {
  if (deferredGlyphSyncTimer !== null) {
    window.clearTimeout(deferredGlyphSyncTimer);
  }
  // Keyboard repeat needs xilem-like "move then paint" latency. Keep
  // GLIF serialization, SVG refresh, compatibility checks, and Comfy
  // state sync off the immediate keydown path.
  deferredGlyphSyncTimer = window.setTimeout(() => {
    deferredGlyphSyncTimer = null;
    if (currentGlyph.value !== glyphName || activeMasterName.value !== masterName) return;
    editor?.finishNudgeSelection();
    nudgePreviewActive = false;
    if (
      syncCurrentGlyphBytesFromEditor({
        skipUnchanged: true,
        preserveMetadata: true,
        refreshGridSvg: false,
        refreshCompatibility: false,
        notifyMasterData: false,
        syncComfy: false,
        syncTextPreview: false,
        syncComponentCache: false,
      })
    ) {
      markGlyphDirty(glyphName, masterName);
      scheduleDeferredGlyphDerivedRefresh(glyphName, masterName);
    }
  }, NUDGE_IDLE_COMMIT_DELAY_MS);
}

function applyImmediateEditorNudge(
  dx: number,
  dy: number,
  shift: boolean,
  ctrl: boolean,
  independent: boolean,
  perf?: NudgePerfSample,
): boolean {
  const state = editor?.nudgeSelectionFastState(dx, dy, shift, ctrl, independent);
  if (!state || state[0] <= 0) return false;
  markNudgePerfMutation(perf);
  applyNudgeSelectionState(state);
  if (perf) perf.panel = performance.now();
  editorGlyphNeedsSync = true;
  nudgePreviewActive = true;
  queueNudgePerfForRender(perf);
  return true;
}

function applyEditorNudge(
  dx: number,
  dy: number,
  shift: boolean,
  ctrl: boolean,
  independent: boolean,
): boolean {
  if (!editor || !currentGlyph.value || !activeMasterName.value) return false;
  cancelDeferredGlyphSyncCommit();
  const glyphName = currentGlyph.value;
  const masterName = activeMasterName.value;
  const perf = startNudgePerf(raf !== null);
  if (!applyImmediateEditorNudge(dx, dy, shift, ctrl, independent, perf)) return false;
  requestRender({ refreshDerivedState: false });
  scheduleDeferredGlyphSync(glyphName, masterName);
  return true;
}

function applyEditorHistoryChange(change: () => boolean): boolean {
  if (!editor || !currentGlyph.value) return false;
  if (!change()) return false;
  if (activeTool.value === "Text" || hasTextBufferSession.value) {
    refreshTextStateFromEditor();
  }
  const glyphChanged = syncCurrentGlyphBytesFromEditor({
    skipUnchanged: true,
    refreshCompatibility: false,
  });
  if (glyphChanged) {
    markGlyphDirty(currentGlyph.value);
  }
  refreshSelectionState();
  requestRender({ refreshCompatibilityErrors: true });
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
  syncCurrentGlyphBytesFromEditor({ refreshCompatibility: false });
  markGlyphDirty(currentGlyph.value);
  refreshSelectionState();
  requestRender({ refreshCompatibilityErrors: true });
  queueComfyStateSync();
  showClipboardNotice("Pasted outline selection");
  return true;
}

function handleZoomShortcut(key: string): boolean {
  if (!editor) return false;
  if (key === "+" || key === "=") {
    editor.setZoom(Math.min(editor.zoom() * 1.1, 1e4));
    requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
    return true;
  }
  if (key === "-" || key === "_") {
    editor.setZoom(Math.max(editor.zoom() / 1.1, 1e-3));
    requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
    return true;
  }
  if (key === "0" && canvas.value) {
    editor.fitToCanvas(canvas.value.width, canvas.value.height);
    refreshSelectionState();
    requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
    return true;
  }
  return false;
}

async function onSave(): Promise<boolean> {
  const data = activeMasterData.value;
  if ((!data || !activeMasterName.value) && !designspaceDirty.value) return true;

  try {
    status.value = "saving…";
    mirroredSaveWrites.value = 0;
    const needsEditorFlush =
      currentGlyph.value &&
      editor &&
      (editorGlyphNeedsSync || nudgePreviewActive);
    if (needsEditorFlush) {
      flushDeferredGlyphSync();
    }
    if (editorGlyphNeedsSync) {
      status.value = "save failed";
      return false;
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
    const designspaceSaved = await persistDirtyDesignspace();
    if (unsavedGlyphs.length === 0 && unsavedGroups.length === 0 && unsavedKerning.length === 0 && designspaceSaved) {
      lastSavedDisplay.value = formatLastSavedDisplay();
    }

    const suffix = saveWarningSuffix(unsavedGroups.length === 0, unsavedKerning.length === 0, designspaceSaved);
    const sourceSuffix = mirroredSaveWrites.value > 0 ? " to source" : "";
    if (unsavedGlyphs.length > 0) {
      status.value = `saved ${savedGlyphs} glyph${savedGlyphs === 1 ? "" : "s"}${sourceSuffix}; ${unsavedGlyphs.length} glyph${unsavedGlyphs.length === 1 ? "" : "s"} not saved (${formatUnsavedGlyphs(unsavedGlyphs)})${suffix}`;
      return false;
    }

    if (savedGlyphs > 0) {
      status.value = `saved ${savedGlyphs} glyph${savedGlyphs === 1 ? "" : "s"}${sourceSuffix}${suffix}`;
    } else if (unsavedGroups.length === 0 && unsavedKerning.length === 0 && designspaceSaved) {
      status.value = `saved metadata${sourceSuffix}${suffix}`;
    } else {
      status.value = `save incomplete${suffix}`;
    }
    queueComfyStateSync(true);
    props.onWorkspaceSaved?.();
    return unsavedGroups.length === 0 && unsavedKerning.length === 0 && designspaceSaved;
  } catch (e) {
    console.warn("save failed:", e);
    status.value = `save failed: ${e}`;
    return false;
  }
}

async function persistGlyphData(data: MasterData, glyphName: string): Promise<boolean> {
  const bytes = data.glyphBytes.get(glyphName);
  if (!bytes) return false;

  const slotPath = data.glyphPaths.get(glyphName);
  if (currentFontPath.value && slotPath) {
    const res = await runebenderHost.writeWorkspaceFile(slotPath, new TextDecoder().decode(bytes));
    await recordWorkspaceWriteResult(res);
    return res.ok;
  }

  const fileHandle = data.glyphFileHandles.get(glyphName);
  if (!fileHandle) return false;
  const writable = await fileHandle.createWritable();
  await writable.write(bytes);
  await writable.close();
  await invalidateCompiledWorkspacePath(slotPath);
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

async function chooseDestinationFolder(): Promise<string | null> {
  const electronApi = (window as Window & {
    electronAPI?: { showDirectoryPicker?: () => Promise<string> };
  }).electronAPI;
  if (typeof electronApi?.showDirectoryPicker === "function") {
    const path = await electronApi.showDirectoryPicker();
    return String(path ?? "").trim() || null;
  }

  const data = await runebenderHost.chooseSource("folder");
  if (data.cancelled) return null;
  return String(data.path ?? "").trim() || null;
}

function requestSaveAsDestination(defaultValue: string): Promise<SaveAsDestination | null> {
  return new Promise((resolve) => {
    const backdrop = document.createElement("div");
    backdrop.style.position = "fixed";
    backdrop.style.inset = "0";
    backdrop.style.zIndex = "2147483647";
    backdrop.style.display = "grid";
    backdrop.style.placeItems = "center";
    backdrop.style.background = "rgba(0, 0, 0, 0.45)";

    const panel = document.createElement("form");
    panel.style.width = "min(720px, calc(100vw - 48px))";
    panel.style.padding = "20px";
    panel.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    panel.style.borderRadius = "12px";
    panel.style.background = "#202124";
    panel.style.boxShadow = "0 18px 60px rgba(0, 0, 0, 0.5)";
    panel.style.color = "#f1f3f4";
    panel.style.font = "13px system-ui, -apple-system, BlinkMacSystemFont, sans-serif";

    const title = document.createElement("div");
    title.textContent = "Save workspace as";
    title.style.fontSize = "16px";
    title.style.fontWeight = "700";
    title.style.marginBottom = "8px";

    const help = document.createElement("div");
    help.textContent = "Choose a destination folder for the current designspace/UFO source copy.";
    help.style.color = "#b8bcc2";
    help.style.marginBottom = "12px";

    const input = document.createElement("input");
    input.type = "text";
    input.value = defaultValue;
    input.placeholder = "/path/to/exported/font-source";
    input.style.boxSizing = "border-box";
    input.style.width = "100%";
    input.style.padding = "10px 12px";
    input.style.border = "1px solid rgba(255, 255, 255, 0.22)";
    input.style.borderRadius = "8px";
    input.style.background = "#111315";
    input.style.color = "#ffffff";
    input.style.font = "13px ui-monospace, SFMono-Regular, Menlo, monospace";
    input.style.outline = "none";

    const relinkRow = document.createElement("label");
    relinkRow.style.display = "flex";
    relinkRow.style.alignItems = "center";
    relinkRow.style.gap = "8px";
    relinkRow.style.marginTop = "12px";
    relinkRow.style.color = "#d7dadf";

    const relink = document.createElement("input");
    relink.type = "checkbox";
    relink.checked = true;
    relinkRow.append(relink, document.createTextNode("Save future edits back to this folder"));

    const pickerActions = document.createElement("div");
    pickerActions.style.display = "flex";
    pickerActions.style.justifyContent = "flex-start";
    pickerActions.style.marginTop = "10px";

    const folderPicker = document.createElement("button");
    folderPicker.type = "button";
    folderPicker.textContent = "Choose Folder...";
    folderPicker.style.padding = "8px 12px";
    folderPicker.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    folderPicker.style.borderRadius = "8px";
    folderPicker.style.background = "#2a2d31";
    folderPicker.style.color = "#f1f3f4";
    pickerActions.append(folderPicker);

    const actions = document.createElement("div");
    actions.style.display = "flex";
    actions.style.justifyContent = "flex-end";
    actions.style.gap = "8px";
    actions.style.marginTop = "14px";

    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.textContent = "Cancel";
    cancel.style.padding = "8px 14px";
    cancel.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    cancel.style.borderRadius = "8px";
    cancel.style.background = "#2a2d31";
    cancel.style.color = "#f1f3f4";

    const submit = document.createElement("button");
    submit.type = "submit";
    submit.textContent = "Save As";
    submit.style.padding = "8px 14px";
    submit.style.border = "1px solid #18b86f";
    submit.style.borderRadius = "8px";
    submit.style.background = "#121212";
    submit.style.color = "#18b86f";
    submit.style.fontWeight = "700";

    actions.append(cancel, submit);
    panel.append(title, help, input, pickerActions, relinkRow, actions);
    backdrop.append(panel);
    document.body.append(backdrop);

    const close = (value: SaveAsDestination | null) => {
      backdrop.remove();
      resolve(value);
    };

    cancel.addEventListener("click", () => close(null));
    backdrop.addEventListener("click", (event) => {
      if (event.target === backdrop) close(null);
    });
    folderPicker.addEventListener("click", async () => {
      folderPicker.disabled = true;
      try {
        const path = await chooseDestinationFolder();
        if (path) input.value = path;
      } catch (error) {
        window.alert(`Runebender destination picker failed: ${error}`);
      } finally {
        folderPicker.disabled = false;
        input.focus();
      }
    });
    panel.addEventListener("submit", (event) => {
      event.preventDefault();
      const destination = input.value.trim();
      close(destination ? { destination, relink: relink.checked } : null);
    });
    panel.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close(null);
      }
    });

    input.focus();
    input.select();
  });
}

async function onSaveAs() {
  if (!currentFontPath.value) return;
  try {
    if (hasDirtyChanges.value) {
      const saved = await onSave();
      if (!saved) {
        status.value = "save as cancelled; resolve unsaved changes first";
        return;
      }
    }
    const chosen = await requestSaveAsDestination("");
    if (!chosen) return;
    status.value = "saving workspace copy…";
    const { response, data } = await runebenderHost.saveWorkspaceAs({
      slot: currentFontPath.value,
      destination: chosen.destination,
      relink: chosen.relink,
    });
    if (!response.ok) {
      throw new Error(data.error || `${response.status} ${response.statusText}`);
    }
    sourceSaveLabel.value = data.linked_source
      ? `Editing ${data.origin_source || data.origin_root || "source"}`
      : `Exported ${data.destination || chosen.destination}`;
    lastSavedDisplay.value = formatLastSavedDisplay();
    status.value = data.linked_source
      ? `saved as editable source at ${data.destination || chosen.destination}`
      : `saved copy to ${data.destination || chosen.destination}`;
  } catch (error) {
    console.warn("save as failed:", error);
    status.value = `save as failed: ${error}`;
  }
}

async function invalidateCompiledWorkspacePath(path: string | null): Promise<void> {
  if (!path || !currentFontPath.value) return;
  try {
    await runebenderHost.invalidateWorkspacePath(path);
  } catch (error) {
    console.warn("workspace invalidation failed:", error);
  }
}

async function recordWorkspaceWriteResult(res: Response): Promise<void> {
  if (!res.ok) return;
  const data = (await res.clone().json().catch(() => null)) as {
    saved_to_source?: boolean;
  } | null;
  if (data?.saved_to_source) {
    mirroredSaveWrites.value += 1;
  }
}

function filenameFromPath(path: string | null, fallback: string): string {
  if (!path) return fallback;
  return path.split("/").filter(Boolean).pop() ?? fallback;
}

function saveWarningSuffix(groupsSaved: boolean, kerningSaved: boolean, designspaceSaved: boolean): string {
  const missing = [];
  if (!groupsSaved) missing.push("groups");
  if (!kerningSaved) missing.push("kerning");
  if (!designspaceSaved) missing.push("designspace");
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
    const res = await runebenderHost.writeWorkspaceFile(data.groupsPath, text);
    await recordWorkspaceWriteResult(res);
    return res.ok;
  }

  if (data.groupsFileHandle) {
    const writable = await data.groupsFileHandle.createWritable();
    await writable.write(new TextEncoder().encode(text));
    await writable.close();
    await invalidateCompiledWorkspacePath(data.groupsPath);
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

async function persistDirtyDesignspace(): Promise<boolean> {
  if (!designspaceDirty.value) {
    return true;
  }
  if (!designspaceText.value || !designspacePath.value) {
    return false;
  }

  if (currentFontPath.value) {
    const res = await runebenderHost.writeWorkspaceFile(
      designspacePath.value,
      designspaceText.value,
    );
    await recordWorkspaceWriteResult(res);
    if (!res.ok) return false;
    designspaceDirty.value = false;
    return true;
  }

  if (designspaceFileHandle.value) {
    const writable = await designspaceFileHandle.value.createWritable();
    await writable.write(new TextEncoder().encode(designspaceText.value));
    await writable.close();
    await invalidateCompiledWorkspacePath(designspacePath.value);
    designspaceDirty.value = false;
    return true;
  }

  const saved = await exportTextFile(
    designspaceText.value,
    filenameFromPath(designspacePath.value, "font.designspace"),
    "Designspace",
    [".designspace"],
  );
  if (saved) designspaceDirty.value = false;
  return saved;
}

async function persistKerningData(data: MasterData, masterName = activeMasterName.value): Promise<boolean> {
  if (!dirtyKerningMasters.value.has(masterName)) {
    return true;
  }

  const text = serializeKerningPlist(data.kerning);

  if (currentFontPath.value && data.kerningPath) {
    const res = await runebenderHost.writeWorkspaceFile(data.kerningPath, text);
    await recordWorkspaceWriteResult(res);
    return res.ok;
  }

  if (data.kerningFileHandle) {
    const writable = await data.kerningFileHandle.createWritable();
    await writable.write(new TextEncoder().encode(text));
    await writable.close();
    await invalidateCompiledWorkspacePath(data.kerningPath);
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

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function syncCurrentGlyphBytesFromEditor(
  options: {
    skipUnchanged?: boolean;
    preserveMetadata?: boolean;
    refreshGridSvg?: boolean;
    refreshCompatibility?: boolean;
    notifyMasterData?: boolean;
    syncComfy?: boolean;
    syncTextPreview?: boolean;
    syncComponentCache?: boolean;
  } = {},
): boolean {
  if (!editor || !currentGlyph.value) return false;
  const data = activeMasterData.value;
  const originalBytes = data?.glyphBytes.get(currentGlyph.value);
  if (!data || !originalBytes) return false;

  try {
    const markColor = data.glyphMarkColors.get(currentGlyph.value) ?? "";
    const bytes = editor.currentGlyphGlif(originalBytes, markColor);
    if (options.skipUnchanged && bytesEqual(bytes, originalBytes)) {
      editorGlyphNeedsSync = false;
      return false;
    }
    const previousMetadata = data.glyphMetadata.get(currentGlyph.value);
    const previousGroups = data.glyphKerningGroups.get(currentGlyph.value);
    const info = options.preserveMetadata && previousMetadata
      ? {
          name: previousMetadata.name,
          unicode: previousMetadata.unicode,
          unicodes: [...previousMetadata.unicodes],
          markColor: data.glyphMarkColors.get(currentGlyph.value) ?? null,
          leftKerningGroup: previousGroups?.left ?? previousMetadata.leftKerningGroup ?? null,
          rightKerningGroup: previousGroups?.right ?? previousMetadata.rightKerningGroup ?? null,
          width: previousMetadata.width,
          contours: previousMetadata.contours,
        }
      : parseGlyphInfo(bytes);
    const metadata = {
      name: currentGlyph.value,
      width: info.width,
      contours: info.contours,
      unicode: info.unicode,
      unicodes: info.unicodes,
    };
    setGlyphBytes(data, currentGlyph.value, bytes, {
      syncComponentCache: options.syncComponentCache,
    });
    data.glyphMetadata.set(currentGlyph.value, metadata);
    setGlyphKerningGroupsFromInfo(data, currentGlyph.value, info);
    if (info.unicode) {
      data.glyphUnicodes.set(currentGlyph.value, info.unicode);
    } else {
      data.glyphUnicodes.delete(currentGlyph.value);
    }
    if (options.refreshGridSvg !== false) {
      refreshGridGlyphSvg(data, currentGlyph.value, bytes);
    }
    setRefNumber(currentWidth, info.width);
    setRefNumber(currentContours, info.contours);
    refreshSidebearingsFromEditor();
    if (options.notifyMasterData !== false) {
      masterDataMap.value = new Map(masterDataMap.value);
    }
    if (options.refreshCompatibility !== false) {
      updateCompatibilityErrors();
    }
    if (options.syncTextPreview !== false && hasTextBufferSession.value) {
      syncCurrentTextSorts(metadata);
      syncTextKerningModelToEditor();
      bumpTextPreviewRevision();
    }
    if (options.syncComfy !== false) {
      queueComfyStateSync();
    }
    editorGlyphNeedsSync = false;
    return true;
  } catch (e) {
    console.warn("serializing current glyph failed:", e);
    return false;
  }
}

function markGlyphDirty(glyphName: string, masterName = activeMasterName.value) {
  if (!glyphName || !masterName) return;
  if (dirtyGlyphsByMaster.value.get(masterName)?.has(glyphName)) return;
  const next = new Map(dirtyGlyphsByMaster.value);
  const glyphs = new Set(next.get(masterName) ?? []);
  glyphs.add(glyphName);
  next.set(masterName, glyphs);
  dirtyGlyphsByMaster.value = next;
}

function markKerningDirty() {
  if (!activeMasterName.value) return;
  if (dirtyKerningMasters.value.has(activeMasterName.value)) return;
  const next = new Set(dirtyKerningMasters.value);
  next.add(activeMasterName.value);
  dirtyKerningMasters.value = next;
}

function markGroupsDirty() {
  if (!activeMasterName.value) return;
  if (dirtyGroupsMasters.value.has(activeMasterName.value)) return;
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

function onDesignspaceTextInput(event: Event) {
  const value = (event.target as HTMLTextAreaElement).value;
  if (value === designspaceText.value) return;
  designspaceText.value = value;
  designspaceDirty.value = true;
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
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
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
  requestRender({ refreshDerivedState: false, refreshBackgroundImageFrame: true });
}

function arrowNudgeDelta(key: string): [number, number] | null {
  return key === "ArrowLeft"
    ? [-1, 0]
    : key === "ArrowRight"
      ? [1, 0]
      : key === "ArrowUp"
        ? [0, 1]
        : key === "ArrowDown"
          ? [0, -1]
          : null;
}

function handleArrowNudgeKey(e: KeyboardEvent, meta: boolean): boolean {
  if (textModeActive.value) return false;
  const nudge = arrowNudgeDelta(e.key);
  if (!nudge) return false;
  if (nudgeSelectedBackgroundImage(nudge[0], nudge[1])) {
    return true;
  }
  return (
    selectionCount.value > 0 &&
    applyEditorNudge(nudge[0], nudge[1], e.shiftKey, meta, e.altKey)
  );
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

  // Esc returns to the grid view from the editor, except in active Text mode.
  // Xilem keeps Text editing active here; Enter is the close-editor key
  // outside Text mode, while Enter inserts line breaks inside Text mode.
  if (e.key === "Escape" && (backgroundImageContextMenu.value || contourContextMenu.value)) {
    e.preventDefault();
    dismissBackgroundImageContextMenu();
    dismissContourContextMenu();
    return;
  }
  if (e.key === "Escape" && viewMode.value === "editor") {
    e.preventDefault();
    if (textModeActive.value) {
      return;
    }
    backToGrid();
    return;
  }

  // Undo/redo only apply in the editor.
  if (viewMode.value !== "editor") return;

  const meta = e.metaKey || e.ctrlKey;
  if (handleArrowNudgeKey(e, meta)) {
    e.preventDefault();
    return;
  }

  if ((activeTool.value === "Shapes" || activeTool.value === "Knife") && e.key === "Shift") {
    const changed =
      activeTool.value === "Shapes"
        ? editor.setShapeShiftLocked(true)
        : editor.setKnifeShiftLocked(true);
    if (changed) {
      requestRender();
    }
  }
  if (e.key === "Enter" && !textModeActive.value) {
    e.preventDefault();
    backToGrid();
    return;
  }

  if (e.key === "Tab") {
    e.preventDefault();
    if (editor.cycleSelectedPoint(e.shiftKey)) {
      refreshSelectionState();
      requestRender();
    }
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
    !textModeActive.value &&
    temporaryPreviewReturnTool.value === null
  ) {
    e.preventDefault();
    if (activeTool.value !== "Preview") {
      temporaryPreviewReturnTool.value = activeTool.value;
      onToolSelect("Preview");
    }
    return;
  }

  if (meta && e.key.toLowerCase() === "z") {
    e.preventDefault();
    applyEditorHistoryChange(() => (e.shiftKey ? editor.redo() : editor.undo()));
  } else if (meta && !e.shiftKey && e.key.toLowerCase() === "i") {
    e.preventDefault();
    backgroundImageInput.value?.click();
  } else if (
    meta &&
    !e.shiftKey &&
    e.key.toLowerCase() === "l"
  ) {
    if (toggleBackgroundImageLock()) {
      e.preventDefault();
    }
  } else if (meta && e.key.toLowerCase() === "t") {
    e.preventDefault();
    void traceBackgroundImageToGlyph(false);
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "y"
  ) {
    if (reportUnavailableBackgroundTrace("quiver")) {
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
    e.key.toLowerCase() === "h"
  ) {
    if (applyEditorMutation(() => editor.convertHyperToCubic())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "r" &&
    !textModeActive.value
  ) {
    if (applyEditorMutation(() => editor.rotateSelectionClockwise())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.shiftKey &&
    e.key.toLowerCase() === "l" &&
    !textModeActive.value
  ) {
    if (applyEditorMutation(() => editor.rotateSelectionCounterClockwise())) {
      e.preventDefault();
    }
  } else if (
    meta &&
    e.key.toLowerCase() === "d" &&
    !textModeActive.value
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
    !textModeActive.value
  ) {
    if (applyEditorMutation(() => editor.unionSelection())) {
      e.preventDefault();
    }
  } else if (meta && handleZoomShortcut(e.key)) {
    e.preventDefault();
  } else if ((e.key === "Backspace" || e.key === "Delete") && !textModeActive.value) {
    if (deleteSelectedBackgroundImage()) {
      e.preventDefault();
    } else if (selectionCount.value > 0 && applySelectionEdit("delete")) {
      e.preventDefault();
    }
  } else if (
    !meta &&
    !e.shiftKey &&
    e.key.toLowerCase() === "t" &&
    !textModeActive.value &&
    selectionCount.value > 0
  ) {
    if (applySelectionEdit("toggle-point")) {
      e.preventDefault();
    }
  } else if (!meta && e.shiftKey && e.key.toLowerCase() === "h" && !textModeActive.value) {
    if (applyEditorMutation(() => editor.flipSelectionHorizontal())) {
      e.preventDefault();
    }
  } else if (!meta && e.shiftKey && e.key.toLowerCase() === "v" && !textModeActive.value) {
    if (applyEditorMutation(() => editor.flipSelectionVertical())) {
      e.preventDefault();
    }
  } else if (!meta && !e.shiftKey && e.key.toLowerCase() === "r" && !textModeActive.value) {
    if (applyEditorMutation(() => editor.reverseContours())) {
      e.preventDefault();
    }
  } else if (!meta && !e.shiftKey && !textModeActive.value) {
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
              : null;
    if (tool) {
      e.preventDefault();
      onToolSelect(tool);
    }
  }

  if (handleTextToolKey(e)) {
    e.preventDefault();
    return;
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (!editor || viewMode.value !== "editor" || eventTargetAcceptsText(e)) {
    return;
  }
  if (e.key === "Alt" && selectIdleHoverActive) {
    selectIdleHoverActive = false;
    if (editor.clearSegmentHover()) {
      requestRender({ refreshDerivedState: false });
    }
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
  if (textModeActive.value) {
    return;
  }
  if (arrowNudgeDelta(e.key)) {
    e.preventDefault();
    if (nudgePreviewActive || editorGlyphNeedsSync) {
      scheduleDeferredGlyphSyncCommitAfterPaint();
    }
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

function onWindowBlur() {
  if (nudgePreviewActive || editorGlyphNeedsSync) {
    flushDeferredGlyphSync();
  }
}

function ownsGlobalKeyboardEvent(e: KeyboardEvent): boolean {
  if (!editor || eventTargetAcceptsText(e)) return false;
  return viewMode.value === "editor" || viewMode.value === "grid";
}

function onGlobalKeyDownCapture(e: KeyboardEvent) {
  if (!ownsGlobalKeyboardEvent(e)) return;
  onKeyDown(e);
  e.stopImmediatePropagation();
}

function onGlobalKeyUpCapture(e: KeyboardEvent) {
  if (!ownsGlobalKeyboardEvent(e)) return;
  onKeyUp(e);
  e.stopImmediatePropagation();
}

onBeforeUnmount(() => {
  if (raf !== null) {
    cancelAnimationFrame(raf);
    raf = null;
  }
  if (pendingGridSelectionRaf !== null) {
    cancelAnimationFrame(pendingGridSelectionRaf);
    pendingGridSelectionRaf = null;
  }
  if (pendingGridScrollRaf !== null) {
    cancelAnimationFrame(pendingGridScrollRaf);
    pendingGridScrollRaf = null;
  }
  flushDeferredGlyphSync();
  cancelPostPaintNudgeSelectionState();
  if (comfySyncTimer !== null) {
    clearTimeout(comfySyncTimer);
    comfySyncTimer = null;
  }
  if (clipboardNoticeTimer !== null) {
    window.clearTimeout(clipboardNoticeTimer);
    clipboardNoticeTimer = null;
  }
  resizeObserver?.disconnect();
  themeObserver?.disconnect();
  stopBottomPreviewResize();
  clearBackgroundImage();
  window.removeEventListener("keydown", onGlobalKeyDownCapture, { capture: true });
  window.removeEventListener("keyup", onGlobalKeyUpCapture, { capture: true });
  window.removeEventListener("blur", onWindowBlur);
  window.removeEventListener("pointerdown", onWindowPointerDown);
  window.removeEventListener("dragenter", onWindowDragOver, { capture: true });
  window.removeEventListener("dragover", onWindowDragOver, { capture: true });
  window.removeEventListener("drop", onWindowDrop, { capture: true });
  editor?.free();
  editor = null;
});
</script>

<template>
  <div class="runebender-host" :class="{ 'is-editor-mode': viewMode === 'editor' }">
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
      :source-label="sourceSaveLabel"
      :masters="masters"
      :active-master="activeMasterIndex"
      :master-previews="masterPreviewSvgs"
      :save-enabled="glyphNames.length > 0"
      :save-as-enabled="!!currentFontPath && glyphNames.length > 0"
      :close-enabled="!!props.onCloseRequested"
      @select-master="onSelectMaster"
      @save="onSave"
      @save-as="onSaveAs"
      @close="props.onCloseRequested?.()"
    />

    <!-- Content row: left rail switches based on view mode
         (glyph navigation in grid, tool palette in editor). The
         stage holds the canvas + grid. Right sidebar shows glyph
         info whenever a font is loaded. -->
    <div class="content">
      <div v-if="glyphNames.length > 0 && viewMode === 'grid'" class="left-col">
          <CategorySidebar
            v-model:search-query="sidebarSearchQuery"
            v-model:search-mode="sidebarSearchMode"
            v-model:search-match-case="sidebarSearchMatchCase"
            v-model:search-regex="sidebarSearchRegex"
            v-model:sort-mode="glyphSortMode"
            :selected="selectedSidebarFilter"
            :counts="categoryCounts"
            :total-count="glyphNames.length"
            :selected-text-glyph-count="selectedGridTextGlyphCount"
            :category-groups="CATEGORY_GROUPS"
            :language-groups="SIDEBAR_LANGUAGE_GROUPS"
            :filters="SIDEBAR_FILTERS"
            @select="onSelectSidebarFilter"
            @copy-selected-text="copySelectedGridGlyphText"
          />
      </div>

      <!-- Stage = canvas + grid stacked on the same area. Canvas
           stays in the DOM (visibility-hidden in grid mode) so the
           WebGPU surface stays bound. -->
      <div
        ref="stage"
        class="stage"
        :class="{ 'editor-bottom-preview-visible': editorBottomPreviewVisible }"
        :style="editorBottomPreviewStyle"
      >
        <canvas
          ref="canvas"
          class="runebender-canvas"
          :class="{
            'is-hidden': viewMode !== 'editor' && glyphNames.length > 0,
            'text-buffer-visible': viewMode === 'editor' && textBufferPreviewVisible,
          }"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerCancel"
          @dblclick="onCanvasDoubleClick"
          @wheel.prevent="onWheel"
          @contextmenu.prevent="onCanvasContextMenu"
          @dragover="onDragOver"
          @drop="onDrop"
        />

        <div
          ref="gridView"
          v-if="viewMode === 'grid' && glyphNames.length > 0"
          class="grid-view"
          :style="gridStyle"
          @dragover="onDragOver"
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
            @dblclick="openGridSelectionInEditor(item.name)"
          />
        </div>

        <div v-if="viewMode === 'editor'" class="editor-top-overlay">
          <div class="editor-tools-cluster">
            <EditModeToolbar
              :active="activeTool"
              @select="onToolSelect"
            />
            <div
              v-if="compatErrors.length"
              class="compat-badge"
              role="status"
              aria-live="polite"
            >
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

          <TopBar
            v-if="glyphNames.length > 0"
            class="editor-status-topbar"
            :font-label="fontLabel"
            :unsaved="hasDirtyChanges"
            :last-saved="lastSavedDisplay"
            :source-label="sourceSaveLabel"
            file-only
          />

          <div class="workspace-overlay">
            <MasterToolbar
              v-if="masters.length > 1"
              :masters="masters"
              :active-master="activeMasterIndex"
              :previews="masterPreviewSvgs"
              @select-master="onSelectMaster"
            />
            <WorkspaceToolbar @glyph-grid="backToGrid" />
            <SystemToolbar
              :save-enabled="glyphNames.length > 0"
              :save-as-enabled="!!currentFontPath && glyphNames.length > 0"
              :close-enabled="!!props.onCloseRequested"
              @save="onSave"
              @save-as="onSaveAs"
              @close="props.onCloseRequested?.()"
            />
          </div>
        </div>

        <!-- Welcome / file-picker panel. Only show when there's
             genuinely no font to open: no glyphs loaded AND the host
             hasn't passed a font path. When the host (e.g. ComfyUI)
             supplied a path, the editor is in the brief loading
             window before glyphs populate, and a momentary flash of
             "Drop a .ufo folder" would be confusing. -->
        <WelcomePanel
          v-if="glyphNames.length === 0 && !currentFontPath"
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
            class="background-image-menu-item"
            role="menuitem"
            @click="applyBackgroundImageContextMenuAction"
          >
            <span class="background-image-menu-icon" aria-hidden="true">
              <GeneratedIcon name="select" :size="16" />
            </span>
            <span class="background-image-menu-copy">
              <span>{{ backgroundImageContextMenu.locked ? "Unlock Image" : "Lock Image" }}</span>
              <small>
                {{ backgroundImageContextMenu.locked ? "Enable move and resize" : "Keep placement fixed" }}
              </small>
            </span>
          </button>
          <button
            type="button"
            class="background-image-menu-item primary"
            role="menuitem"
            @pointerdown.stop
            @pointerup.prevent.stop="traceBackgroundImageFromMenu('pointer')"
            @click.prevent.stop="traceBackgroundImageFromMenu('click')"
          >
            <span class="background-image-menu-icon" aria-hidden="true">
              <GeneratedIcon name="hyperpen" :size="16" />
            </span>
            <span class="background-image-menu-copy">
              <span>Trace Image</span>
              <small>Insert outline into this glyph</small>
            </span>
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
            v-if="contourContextMenu.pathIndex !== null"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('reverse')"
          >
            Reverse Contour
          </button>
          <button
            v-if="contourContextMenu.canRoundCorners"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('round-corners')"
          >
            Round Corners
          </button>
          <button
            v-if="contourContextMenu.canMoveUp"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('move-up')"
          >
            Move Contour Up ({{ contourContextMenu.pathIndex }} -> {{ (contourContextMenu.pathIndex ?? 0) - 1 }})
          </button>
          <button
            v-if="contourContextMenu.canMoveDown"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('move-down')"
          >
            Move Contour Down ({{ contourContextMenu.pathIndex }} -> {{ (contourContextMenu.pathIndex ?? 0) + 1 }})
          </button>
          <button
            v-if="contourContextMenu.canAddAnchor"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('add-anchor')"
          >
            Add Anchor
          </button>
          <button
            v-if="contourContextMenu.canEditAnchor"
            type="button"
            role="menuitem"
            @click="applyContourContextMenuAction('delete-anchor')"
          >
            Delete Anchor
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
        </template>

        <div
          v-if="viewMode === 'editor' && editorPanelsVisible && currentGlyph && activeTool !== 'Text'"
          class="glyph-preview-overlay"
          aria-label="Active glyph preview"
        >
          <span
            v-if="activeGlyphPreviewSvg"
            class="glyph-preview-shape"
            v-html="activeGlyphPreviewSvg"
          />
        </div>

        <div
          v-if="viewMode === 'editor' && editorPanelsVisible && activeGlyphPanelVisible"
          class="active-glyph-overlay"
          :class="{ 'text-mode': activeTool === 'Text' }"
          aria-label="Active glyph metrics"
        >
          <div class="active-glyph-header">
            <label class="metric-field glyph-name-field">
              <span>Name</span>
              <input
                type="text"
                :value="currentGlyph"
                aria-label="Glyph name"
                @change="onActiveGlyphNameChange"
                @keydown.enter.prevent="onActiveGlyphNameChange"
              />
            </label>
            <label class="metric-field width-field">
              <span>Width</span>
              <input
                type="number"
                :value="Math.round(currentWidth)"
                aria-label="Advance width"
                @change="onActiveGlyphWidthChange"
                @keydown.enter.prevent="onActiveGlyphWidthChange"
              />
            </label>
            <label class="metric-field unicode-field">
              <span>Unicode</span>
              <input
                type="text"
                :value="activeGlyphUnicode ?? ''"
                aria-label="Unicode"
                placeholder="None"
                @change="onActiveGlyphUnicodeChange"
                @keydown.enter.prevent="onActiveGlyphUnicodeChange"
              />
            </label>
          </div>

          <div class="metrics-strip">
            <div class="metrics-half">
              <label class="metric-field group-field">
                <span>Left Group</span>
                <input
                  type="text"
                  :value="activeGlyphKerningGroups?.left ?? ''"
                  aria-label="Left kerning group"
                  placeholder="None"
                  @change="updateGlyphKerningGroup('left', ($event.target as HTMLInputElement).value)"
                  @keydown.enter.prevent="updateGlyphKerningGroup('left', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="metric-field metric-compact">
                <span>LSB</span>
                <input
                  type="number"
                  :value="Math.round(currentLeftSidebearing)"
                  aria-label="Left sidebearing"
                  @change="onActiveGlyphSidebearingChange('left', $event)"
                  @keydown.enter.prevent="onActiveGlyphSidebearingChange('left', $event)"
                />
              </label>
              <label class="metric-field kern-field">
                <span>Left Kern</span>
                <input
                  type="number"
                  :value="activeLeftKern ?? ''"
                  aria-label="Left kern"
                  placeholder="Auto"
                  :disabled="!canEditActiveLeftKern"
                  @change="updateActiveTextKern('left', ($event.target as HTMLInputElement).value)"
                  @keydown.enter.prevent="updateActiveTextKern('left', ($event.target as HTMLInputElement).value)"
                />
              </label>
            </div>
            <div class="metrics-half">
              <label class="metric-field kern-field">
                <span>Right Kern</span>
                <input
                  type="number"
                  :value="activeRightKern ?? ''"
                  aria-label="Right kern"
                  placeholder="Auto"
                  :disabled="!canEditActiveRightKern"
                  @change="updateActiveTextKern('right', ($event.target as HTMLInputElement).value)"
                  @keydown.enter.prevent="updateActiveTextKern('right', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="metric-field metric-compact">
                <span>RSB</span>
                <input
                  type="number"
                  :value="Math.round(currentRightSidebearing)"
                  aria-label="Right sidebearing"
                  @change="onActiveGlyphSidebearingChange('right', $event)"
                  @keydown.enter.prevent="onActiveGlyphSidebearingChange('right', $event)"
                />
              </label>
              <label class="metric-field group-field">
                <span>Right Group</span>
                <input
                  type="text"
                  :value="activeGlyphKerningGroups?.right ?? ''"
                  aria-label="Right kerning group"
                  placeholder="None"
                  @change="updateGlyphKerningGroup('right', ($event.target as HTMLInputElement).value)"
                  @keydown.enter.prevent="updateGlyphKerningGroup('right', ($event.target as HTMLInputElement).value)"
                />
              </label>
            </div>
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

        <AnchorPanel
          v-if="viewMode === 'editor' && editorPanelsVisible && selectedAnchor"
          class="anchor-overlay"
          :value="selectedAnchor"
          @change-anchor="onAnchorChange"
        />

        <TransformPanel
          v-if="viewMode === 'editor' && editorPanelsVisible"
          class="transform-overlay"
          :bounds="selectedBounds"
          :contour-count="currentContours"
          @transform="onTransform"
        />

        <HelperPanel
          v-if="viewMode === 'editor' && editorPanelsVisible"
          class="helper-overlay"
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
          v-if="editorBottomPreviewVisible"
          class="editor-bottom-preview-panel"
          :class="{ 'text-preview-panel': textBufferPreviewVisible }"
          :aria-label="textBufferPreviewVisible ? 'Text preview' : 'Active glyph filled preview'"
        >
          <div
            class="editor-bottom-preview-resizer"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize active glyph preview"
            @pointerdown="onBottomPreviewResizePointerDown"
            @pointermove="onBottomPreviewResizePointerMove"
            @pointerup="onBottomPreviewResizePointerUp"
            @pointercancel="onBottomPreviewResizePointerUp"
            @mousedown="onBottomPreviewResizeMouseDown"
          />
          <div
            v-if="textBufferPreviewVisible"
            class="text-preview-surface"
            aria-hidden="true"
          >
            <div
              v-if="textBufferPreviewSvg"
              class="text-preview-glyphs"
              v-html="textBufferPreviewSvg"
            />
          </div>
          <span
            v-else
            class="editor-bottom-preview-glyph"
            v-html="activeGlyphPreviewSvg"
          />
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
        <MarkColorPanel
          :active="selectedGlyph ? glyphMarkColors.get(selectedGlyph) : ''"
          :enabled="!!selectedGlyph"
          :can-apply-all-masters="masters.length > 1"
          v-model:apply-all-masters="markColorApplyAllMasters"
          @set="setMarkOnSelected"
        />
        <!-- Designspace XML panel hidden for now — not needed yet. The
             state and save plumbing (designspaceText, onDesignspaceTextInput,
             persistDesignspace) are left intact so it can be re-enabled
             by restoring this <section>. -->
      </div>
    </div>
  </div>
</template>

<style scoped>
/*
 * Color palette — the editor inherits from ComfyUI's CSS variables
 * so it tracks the user's active ComfyUI theme (dark/light/custom).
 * Each fallback hex preserves the runebender-xilem reference theme,
 * so the editor still looks coherent on a ComfyUI build that doesn't
 * ship one of these variables, or when running outside ComfyUI.
 *
 * Original xilem palette (preserved as fallbacks below):
 *   APP_BACKGROUND       #101010 (BASE_A)
 *   PANEL_BACKGROUND     #1C1C1C
 *   PANEL_OUTLINE        #606060 (BASE_F)
 *   PRIMARY_UI_TEXT      #909090 (BASE_I)
 *   SECONDARY_UI_TEXT    #707070 (BASE_G)
 *   GRID_CELL_TEXT       #808080 (BASE_H)
 *   GRID_GLYPH_COLOR     #a0a0a0 (BASE_J)
 *   ACCENT (METRICS_GUIDE / SELECTED_OUTLINE)  #18B86F
 *   SELECTION_RECT_STROKE / TOOL_PREVIEW       #ff980f
 *
 * ComfyUI v1 exposes both generic colors (--bg-color, --fg-color,
 * --border-color, --content-bg, etc.) and PrimeVue tokens (--p-*).
 * We map them onto the existing --rb-* convention so individual
 * component stylesheets don't have to know about either upstream
 * namespace.
 */

.runebender-host {
  /* Surfaces */
  --rb-app-background:     var(--rb-canvas-background, #0c0c0c);
  --rb-panel-background:   #121212;
  --rb-grid-cell-background: var(--rb-panel-background, #121212);
  --rb-grid-cell-hover-background: #1d1d1d;
  --rb-button-background:  #1d1d1d;
  --rb-control-background: var(--comfy-input-bg, #111315);

  /* Borders / outlines */
  --rb-panel-outline:      #404040;
  --rb-stroke-width:       1.5px;
  --rb-panel-radius:       12px;
  --rb-button-radius:      8px;

  /* Text */
  --rb-primary-text:       #909090;
  --rb-secondary-text:     #808080;
  --rb-muted-text:         #808080;
  --rb-subdued-text:       color-mix(in srgb, var(--rb-primary-text) 35%, transparent);
  --rb-overlay-text:       var(--rb-primary-text);
  --rb-glyph-preview:      var(--rb-muted-text);

  /* Accents / state */
  --rb-accent:                     #18b86f;
  --rb-warning:                    #ffdc32;
  --rb-danger:                     #ff4a3d;
  --rb-danger-text:                #ff4a3d;
  --rb-mark-hover-ring:            color-mix(in srgb, var(--rb-accent) 55%, transparent);
  --rb-mark-selected-ring:         var(--rb-accent);
  --rb-background-image-selection: var(--rb-accent);

  /* Canvas theme bridge. Vue resolves these variables and passes the
   * resulting RGBA values into the Rust renderer, keeping the WebGPU
   * scene aligned with the surrounding ComfyUI chrome. */
  --rb-canvas-background:         #0c0c0c;
  --rb-canvas-path-stroke:        #b0b0b0;
  --rb-canvas-selection:          #ff980f;
  --rb-canvas-component:          #6699cc;
  --rb-canvas-component-selected: #88bbff;
  --rb-canvas-point-smooth-inner: #181818;
  --rb-canvas-point-smooth-outer: #18b86f;
  --rb-canvas-point-corner-inner: #181818;
  --rb-canvas-point-corner-outer: #ff980f;
  --rb-canvas-point-offcurve-inner: #181818;
  --rb-canvas-point-offcurve-outer: #8c6cff;
  --rb-canvas-point-hyper-inner: #181818;
  --rb-canvas-point-hyper-outer: #8c6cff;
  --rb-canvas-start-node:         #ff980f;
  --rb-canvas-text-cursor:        #ff980f;
  --rb-canvas-kern-active:        #456fff;
  --rb-canvas-kern-previous:      #ff980f;
  --rb-canvas-text-preview-fill:  #808080;
  --rb-editor-edge-inset:         8px;
  --rb-editor-bottom-preview-height: clamp(112px, 15%, 180px);

  width: 100%;
  height: 100%;
  background: var(--rb-app-background);
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-sizing: border-box;
}

.runebender-host.is-editor-mode {
  padding: 0;
  gap: 0;
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
  /* No flex gap: it would add space on the sidebar side of the grid's
   * scrollbar but not the grid-content side, making the scrollbar look
   * off-center in the gutter. Instead the left-col carries its own
   * right margin, and the stage↔sidebar boundary is gutter-free so the
   * grid scrollbar track is the only thing between them (symmetric
   * margins around the thumb). */
  gap: 0;
}

/* Left column: glyph navigation. */
.left-col {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  width: 286px;
  margin-right: 0;
}
.left-col > :deep(.category-sidebar) {
  flex: 1;
  min-height: 0;
}

/* Right column: info on top, anatomy fills remaining height. */
.right-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
  width: 228px;
}
.right-col > :deep(.info-sidebar) {
  width: auto;
  flex-shrink: 0;
}
.right-col > :deep(.mark-color-panel) {
  flex-shrink: 0;
}

.designspace-panel {
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 10px;
  min-height: 220px;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.designspace-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.designspace-title {
  color: var(--rb-primary-text, #909090);
  font: 13px ui-sans-serif, system-ui, sans-serif;
}

.designspace-status {
  color: var(--rb-muted-text, #808080);
  font: 12px ui-sans-serif, system-ui, sans-serif;
}

.designspace-path,
.designspace-summary {
  color: var(--rb-secondary-text, #707070);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.designspace-editor {
  flex: 1;
  min-height: 140px;
  width: 100%;
  resize: none;
  box-sizing: border-box;
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: 4px;
  background: var(--rb-app-background, #101010);
  color: var(--rb-primary-text, #909090);
  padding: 8px;
  font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  line-height: 1.35;
  outline: none;
}

.designspace-editor:focus {
  border-color: var(--rb-accent, #18b86f);
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
  right: var(--rb-editor-edge-inset, 8px);
  bottom: var(--rb-editor-edge-inset, 8px);
  z-index: 3;
}

.anchor-overlay {
  position: absolute;
  right: var(--rb-editor-edge-inset, 8px);
  bottom: calc(var(--rb-editor-edge-inset, 8px) + 92px);
  z-index: 3;
}

.stage.editor-bottom-preview-visible .coordinate-overlay,
.stage.editor-bottom-preview-visible .glyph-preview-overlay,
.stage.editor-bottom-preview-visible .active-glyph-overlay {
  bottom: calc(var(--rb-editor-bottom-preview-height) + var(--rb-editor-edge-inset, 8px));
}

.stage.editor-bottom-preview-visible .anchor-overlay {
  bottom: calc(var(--rb-editor-bottom-preview-height) + var(--rb-editor-edge-inset, 8px) + 92px);
}

.transform-overlay {
  position: absolute;
  right: var(--rb-editor-edge-inset, 8px);
  top: calc(var(--rb-editor-edge-inset, 8px) + 70px);
  bottom: calc(var(--rb-editor-edge-inset, 8px) + 92px);
  height: max-content;
  margin-block: auto;
  z-index: 3;
}

.stage.editor-bottom-preview-visible .transform-overlay {
  bottom: calc(var(--rb-editor-bottom-preview-height) + var(--rb-editor-edge-inset, 8px) + 92px);
}

.helper-overlay {
  position: absolute;
  left: var(--rb-editor-edge-inset, 8px);
  top: calc(var(--rb-editor-edge-inset, 8px) + 70px);
  bottom: calc(var(--rb-editor-edge-inset, 8px) + 92px);
  height: max-content;
  margin-block: auto;
  z-index: 3;
}

.stage.editor-bottom-preview-visible .helper-overlay {
  bottom: calc(var(--rb-editor-bottom-preview-height) + var(--rb-editor-edge-inset, 8px) + 92px);
}

.editor-top-overlay {
  position: absolute;
  left: var(--rb-editor-edge-inset, 8px);
  top: var(--rb-editor-edge-inset, 8px);
  right: var(--rb-editor-edge-inset, 8px);
  z-index: 4;
  display: flex;
  align-items: flex-start;
  gap: 6px;
  pointer-events: none;
}

.editor-top-overlay > * {
  pointer-events: auto;
}

.editor-tools-cluster {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  flex: 0 0 auto;
  width: 440px;
  max-width: calc(100vw - 16px);
}

.editor-status-topbar {
  flex: 1 1 auto;
  min-width: 180px;
}

.workspace-overlay {
  display: flex;
  flex: 0 0 auto;
  gap: 6px;
  align-items: flex-start;
}

.background-image {
  position: absolute;
  z-index: 1;
  user-select: none;
  touch-action: none;
  cursor: move;
  border: var(--rb-stroke-width, 1px) solid transparent;
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
  border-color: var(--rb-background-image-selection, #456fff);
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
  border: var(--rb-stroke-width, 1px) solid var(--rb-background-image-selection, #456fff);
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
  padding: 6px;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}

.background-image-menu {
  min-width: 230px;
  padding: 7px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.035), rgba(255, 255, 255, 0)),
    var(--rb-panel-background, #1c1c1c);
  box-shadow:
    0 14px 32px rgba(0, 0, 0, 0.46),
    inset 0 1px 0 rgba(255, 255, 255, 0.05);
}

.context-menu {
  min-width: 140px;
}

.background-image-menu button,
.context-menu button {
  appearance: none;
  width: 100%;
  color: var(--rb-overlay-text, #f0f0f0);
  background: transparent;
  border: 0;
  border-radius: 4px;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  text-align: left;
  cursor: pointer;
}

.context-menu button {
  height: 28px;
  padding: 0 10px;
}

.background-image-menu button {
  min-height: 48px;
  padding: 7px 9px;
}

.background-image-menu button:focus,
.context-menu button:focus {
  outline: none;
}

.background-image-menu button:hover,
.context-menu button:hover {
  background: var(--rb-control-background, #303030);
}

.background-image-menu button:focus-visible,
.context-menu button:focus-visible {
  outline: var(--rb-stroke-width, 1px) solid var(--rb-accent, #66ee88);
  outline-offset: 1px;
}

.background-image-menu button:active,
.context-menu button:active {
  background: color-mix(in srgb, var(--rb-control-background, #303030) 76%, var(--rb-accent, #66ee88));
}

.background-image-menu-item {
  display: grid;
  grid-template-columns: 28px 1fr;
  align-items: center;
  gap: 9px;
  transition:
    background-color 80ms ease,
    color 80ms ease,
    border-color 80ms ease;
}

.background-image-menu-item + .background-image-menu-item {
  margin-top: 3px;
}

.background-image-menu-item.primary {
  color: var(--rb-accent, #66ee88);
}

.background-image-menu-item:hover {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--rb-accent, #66ee88) 14%, transparent), transparent 78%),
    var(--rb-control-background, #303030);
}

.background-image-menu-item.primary:hover {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--rb-accent, #66ee88) 22%, transparent), transparent 78%),
    var(--rb-control-background, #303030);
}

.background-image-menu-item:active {
  transform: translateY(1px);
}

.background-image-menu-icon {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  color: var(--rb-primary-text, #b0b0b0);
  background: var(--rb-button-background, #181818);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: 5px;
}

.background-image-menu-item.primary .background-image-menu-icon {
  color: var(--rb-accent, #66ee88);
  border-color: color-mix(in srgb, var(--rb-accent, #66ee88) 58%, var(--rb-panel-outline, #606060));
  background: color-mix(in srgb, var(--rb-accent, #66ee88) 12%, var(--rb-button-background, #181818));
}

.background-image-menu-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
  line-height: 1.1;
}

.background-image-menu-copy span {
  color: var(--rb-overlay-text, #f0f0f0);
  font-size: 12px;
  font-weight: 650;
}

.background-image-menu-item.primary .background-image-menu-copy span {
  color: var(--rb-accent, #66ee88);
}

.background-image-menu-copy small {
  color: var(--rb-primary-text, #909090);
  font-size: 11px;
  font-weight: 500;
}

.compat-marker {
  position: absolute;
  z-index: 5;
  width: 16px;
  height: 16px;
  margin: -8px 0 0 -8px;
  border: 2px solid var(--rb-danger, #ff4a3d);
  border-radius: 50%;
  box-sizing: border-box;
  pointer-events: none;
}

.compat-badge {
  box-sizing: border-box;
  width: 100%;
  max-height: 160px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  background: color-mix(in srgb, var(--rb-panel-background, #1c1c1c) 94%, transparent);
  border: var(--rb-stroke-width, 1px) solid var(--rb-danger, #ff4a3d);
  border-radius: var(--rb-panel-radius, 12px);
  color: var(--rb-overlay-text, #f0f0f0);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  pointer-events: none;
}
.compat-badge strong {
  color: var(--rb-danger-text, #ff4a3d);
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
  bottom: var(--rb-editor-edge-inset, 8px);
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  pointer-events: auto;
}

.glyph-preview-overlay {
  left: var(--rb-editor-edge-inset, 8px);
  width: 235px;
  height: 86px;
  padding: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.glyph-preview-shape {
  width: auto;
  height: 100%;
  /* Core palette yellow: the bottom-left preview reads as the current glyph
     highlight rather than another gray panel. */
  color: var(--rb-warning, #ffdc32);
  display: flex;
  align-items: center;
  justify-content: center;
}
.glyph-preview-shape :deep(svg) {
  display: block;
  width: auto;
  height: 100%;
  max-width: calc(235px - 24px);
  max-height: 100%;
}

.active-glyph-overlay {
  left: 50%;
  width: min(860px, calc(100% - 32px));
  padding: 8px;
  transform: translateX(-50%);
  display: grid;
  gap: 6px;
}
.active-glyph-overlay.text-mode {
  bottom: calc(var(--rb-editor-bottom-preview-height) + 18px);
}

.active-glyph-header,
.metrics-strip {
  display: grid;
  gap: 6px;
}

.active-glyph-header {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.metrics-strip {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.metrics-half {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
}

.metric-field {
  box-sizing: border-box;
  min-width: 0;
  display: grid;
  grid-template-columns: 70px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px;
  background: var(--rb-app-background, #101010);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
}

.metric-field span {
  color: var(--rb-muted-text, #808080);
  font: 10px ui-sans-serif, system-ui, sans-serif;
  letter-spacing: 0;
  text-transform: uppercase;
  white-space: nowrap;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.metric-field input {
  appearance: textfield;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  min-width: 0;
  margin: 0;
  padding: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: transparent;
  border: 0;
  color: var(--rb-primary-text, #909090);
  font: 13px ui-monospace, monospace;
  text-align: right;
  outline: none;
}

.metric-field input::-webkit-outer-spin-button,
.metric-field input::-webkit-inner-spin-button {
  appearance: none;
  margin: 0;
}

.glyph-name-field input {
  color: var(--rb-accent, #18b86f);
  font-weight: 700;
}

.glyph-name-field {
  grid-column: span 2;
  grid-template-columns: 48px minmax(0, 1fr);
}

.unicode-field input {
  text-align: right;
  font-size: 12px;
}

.width-field {
  grid-template-columns: 44px minmax(0, 1fr);
}

.metric-field:focus-within {
  border-color: var(--rb-accent, #18b86f);
}

.metric-field input::placeholder {
  color: var(--rb-subdued-text, #505050);
}

.group-field input {
  color: var(--rb-secondary-text, #707070);
}

.group-field input:placeholder-shown {
  color: var(--rb-subdued-text, #505050);
}

.metric-field input:disabled {
  color: var(--rb-subdued-text, #505050);
  opacity: 1;
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
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-accent, #18b86f);
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
  border: var(--rb-stroke-width, 1px) solid var(--rb-accent, #18b86f);
  border-radius: 6px;
  color: var(--rb-overlay-text, #f0f0f0);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  pointer-events: none;
}

.editor-bottom-preview-panel {
  position: absolute;
  z-index: 2;
  left: 0;
  right: 0;
  bottom: 0;
  height: var(--rb-editor-bottom-preview-height);
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px var(--rb-editor-edge-inset, 8px) 14px;
  overflow: hidden;
  background: var(--rb-panel-background, #1c1c1c);
  pointer-events: auto;
}

.editor-bottom-preview-panel.text-preview-panel {
  padding-left: 0;
  padding-right: 0;
}

.editor-bottom-preview-resizer {
  position: absolute;
  z-index: 1;
  left: 0;
  right: 0;
  top: -6px;
  height: 14px;
  cursor: ns-resize;
  pointer-events: auto;
}

.editor-bottom-preview-resizer::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: 6px;
  height: var(--rb-stroke-width, 1px);
  background: var(--rb-panel-outline, #606060);
}

.editor-bottom-preview-resizer:hover::after {
  background: var(--rb-accent, #18b86f);
}

.editor-bottom-preview-glyph {
  width: 100%;
  height: 100%;
  color: var(--rb-warning, #ffdc32);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.editor-bottom-preview-glyph :deep(svg) {
  display: block;
  width: auto;
  height: auto;
  max-width: min(520px, 80%);
  max-height: 100%;
}

.text-preview-surface {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  pointer-events: none;
}

.text-preview-glyphs {
  width: 100%;
  height: 100%;
  color: var(--rb-warning, #ffdc32);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.text-preview-glyphs :deep(svg) {
  width: 100%;
  height: 100%;
  max-width: 100%;
  flex: 0 1 100%;
  display: block;
}
.text-preview-glyphs :deep(path) {
  fill: currentColor !important;
  stroke: none !important;
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
  bottom: var(--rb-editor-bottom-preview-height);
  height: calc(100% - var(--rb-editor-bottom-preview-height));
}
.runebender-canvas.is-hidden {
  visibility: hidden;
  pointer-events: none;
}
/* ----- Grid ----- */
/* BENTO_GAP = 6px from xilem's views/glyph_grid/mod.rs */
.grid-view {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  box-sizing: border-box;
  display: grid;
  gap: 6px;
  scroll-snap-type: y mandatory;
  background: var(--rb-canvas-background, #0c0c0c);
}

/* ----- Scrollbars -----
 * WebKit (the editor runs in a Chrome app window) draws a chunky grey
 * track by default, which shows up as ugly background strips next to
 * the glyph grid and the designspace panel. Strip the track, narrow
 * the bar, and color the thumb from the palette. :deep() so the rule
 * reaches scroll areas inside child components (sidebars, panels) too.
 * Firefox uses the inherited scrollbar-width / scrollbar-color in the
 * Firefox-only feature query below. */
.runebender-host :deep(::-webkit-scrollbar) {
  width: 6px;
  height: 6px;
}
.runebender-host :deep(::-webkit-scrollbar-track) {
  background: transparent;
}
.runebender-host :deep(::-webkit-scrollbar-thumb) {
  /* Reserve exactly one BENTO_GAP for the scrollbar, then inset the
   * visible thumb so it does not touch either neighboring panel edge. */
  background: rgba(96, 96, 96, 0.36);
  border-radius: 999px;
  border: 2.25px solid transparent;
  background-clip: padding-box;
  min-height: 32px;
}
.runebender-host :deep(::-webkit-scrollbar-thumb:hover) {
  background: rgba(144, 144, 144, 0.41);
  border: 2.25px solid transparent;
  background-clip: padding-box;
}
.runebender-host :deep(::-webkit-scrollbar-corner) {
  background: transparent;
}

@supports (-moz-appearance: none) {
  .runebender-host {
    scrollbar-width: thin;
    scrollbar-color: rgba(96, 96, 96, 0.36) transparent;
  }
}
</style>
