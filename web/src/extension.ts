// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";
import { createApp, ref } from "vue";

import { runebenderHostKey, type WorkspaceChoice } from "./host/runebenderHost";
import { comfyHost } from "./hosts/comfy/comfyHost";
import Runebender from "./Runebender.vue";

const RUNEBENDER_BUNDLE_FINGERPRINT = "rb-bundle-2026-05-28-codemirror-84";

// Mirror our own console output to the ComfyUI terminal through the
// injected Comfy host. Filters to messages prefixed with
// "[runebender" so we don't push unrelated browser noise through.
// Installed BEFORE the first console.info so the "loaded ..." line
// shows up in the terminal too.
function installRunebenderLogMirror() {
  const original = {
    log: console.log.bind(console),
    info: console.info.bind(console),
    warn: console.warn.bind(console),
    error: console.error.bind(console),
  };
  const isRunebenderMessage = (args: any[]) => {
    if (!args.length) return false;
    const first = args[0];
    return typeof first === "string" && first.includes("[runebender");
  };
  const formatArgs = (args: any[]) =>
    args
      .map((a) => {
        if (typeof a === "string") return a;
        try {
          return JSON.stringify(a);
        } catch {
          return String(a);
        }
      })
      .join(" ");
  const forward = (level: string, args: any[]) => {
    try {
      comfyHost.log?.(level, formatArgs(args));
    } catch {}
  };
  for (const level of ["log", "info", "warn", "error"] as const) {
    (console as any)[level] = (...args: any[]) => {
      original[level](...args);
      if (isRunebenderMessage(args)) forward(level, args);
    };
  }
}
installRunebenderLogMirror();

console.info(`[runebender-comfy] loaded ${RUNEBENDER_BUNDLE_FINGERPRINT}`);

// ComfyUI auto-loads .js from WEB_DIRECTORY but not sibling .css. Vite's
// library build emits the Vue scoped styles to a separate style.css, so
// without this injection the editor renders unstyled (file pickers as
// raw inputs, panels as plain text lists, no canvas background).
function injectRunebenderStyles() {
  const STYLE_ID = "runebender-comfy-styles";
  if (document.getElementById(STYLE_ID)) return;
  try {
    const cssUrl = new URL("./style.css", import.meta.url).href;
    const link = document.createElement("link");
    link.id = STYLE_ID;
    link.rel = "stylesheet";
    link.href = cssUrl;
    document.head.appendChild(link);
    console.info(`[runebender-comfy] injected styles from ${cssUrl}`);
  } catch (error) {
    console.warn("[runebender-comfy] could not inject style.css:", error);
  }
}
injectRunebenderStyles();

declare const window: any;

function looksLikeFontSourcePath(value: string): boolean {
  const trimmed = value.trim();
  return (
    trimmed.startsWith("/") ||
    trimmed.startsWith("~") ||
    trimmed.startsWith("./") ||
    trimmed.startsWith("../") ||
    /\.(designspace|ufo|glyphs|glyphspackage)(?:\/)?$/i.test(trimmed)
  );
}

async function chooseSourcePath(): Promise<string | null> {
  if (typeof window.electronAPI?.showDirectoryPicker === "function") {
    const path = await window.electronAPI.showDirectoryPicker();
    return String(path ?? "").trim() || null;
  }

  const data = await comfyHost.chooseSource();
  if (data.cancelled) return null;
  return String(data.path ?? "").trim() || null;
}

function requestSourcePath(defaultValue: string): Promise<string | null> {
  return new Promise((resolve) => {
    const initialValue = ["demo", "ufo/designspace"].includes(defaultValue.trim().toLowerCase())
      ? ""
      : defaultValue;
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
    title.textContent = "Open font source";
    title.style.fontSize = "16px";
    title.style.fontWeight = "700";
    title.style.marginBottom = "8px";

    const help = document.createElement("div");
    help.textContent = "Choose the font source you want Runebender to edit. Saves are written back to this source.";
    help.style.color = "#b8bcc2";
    help.style.marginBottom = "12px";

    const input = document.createElement("input");
    input.type = "text";
    input.value = initialValue;
    input.placeholder = "/path/to/font.designspace";
    input.style.boxSizing = "border-box";
    input.style.width = "100%";
    input.style.padding = "10px 12px";
    input.style.border = "1px solid rgba(255, 255, 255, 0.22)";
    input.style.borderRadius = "8px";
    input.style.background = "#111315";
    input.style.color = "#ffffff";
    input.style.font = "13px ui-monospace, SFMono-Regular, Menlo, monospace";
    input.style.outline = "none";

    const pickerActions = document.createElement("div");
    pickerActions.style.display = "flex";
    pickerActions.style.justifyContent = "flex-start";
    pickerActions.style.gap = "8px";
    pickerActions.style.marginTop = "10px";

    const sourcePicker = document.createElement("button");
    sourcePicker.type = "button";
    sourcePicker.textContent = "Choose Source...";
    sourcePicker.style.padding = "8px 12px";
    sourcePicker.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    sourcePicker.style.borderRadius = "8px";
    sourcePicker.style.background = "#2a2d31";
    sourcePicker.style.color = "#f1f3f4";

    pickerActions.append(sourcePicker);

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
    submit.textContent = "Open for Editing";
    submit.style.padding = "8px 14px";
    submit.style.border = "1px solid #66ee88";
    submit.style.borderRadius = "8px";
    submit.style.background = "#1f6f3d";
    submit.style.color = "#ffffff";
    submit.style.fontWeight = "700";

    actions.append(cancel, submit);
    panel.append(title, help, input, pickerActions, actions);
    backdrop.append(panel);
    document.body.append(backdrop);

    const close = (value: string | null) => {
      backdrop.remove();
      resolve(value);
    };

    cancel.addEventListener("click", () => close(null));
    backdrop.addEventListener("click", (event) => {
      if (event.target === backdrop) close(null);
    });
    const browse = async () => {
      sourcePicker.disabled = true;
      try {
        const path = await chooseSourcePath();
        if (path) input.value = path;
      } catch (error) {
        alert(`Runebender source picker failed: ${error}`);
        console.error("[runebender-comfy] source picker failed:", error);
      } finally {
        sourcePicker.disabled = false;
        input.focus();
      }
    };
    sourcePicker.addEventListener("click", () => {
      void browse();
    });
    panel.addEventListener("submit", (event) => {
      event.preventDefault();
      close(input.value.trim() || null);
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

function logNodeSockets(node: any, label: string) {
  const payload = {
    id: node.id,
    title: node.title,
    inputs: (node.inputs ?? []).map((slot: any) => ({
      name: slot.name,
      type: slot.type,
      link: slot.link ?? null,
      hasWidget: !!slot.widget,
    })),
    outputs: (node.outputs ?? []).map((slot: any) => ({
      name: slot.name,
      type: slot.type,
      links: slot.links ?? [],
    })),
  };
  console.info(`[runebender-comfy] ${label} ${JSON.stringify(payload)}`);
}

function resolveConnectedFontValue(node: any): string {
  const inputIndex = node.inputs?.findIndex((slot: any) => slot.name === "font") ?? -1;
  if (inputIndex >= 0) {
    const input = node.inputs?.[inputIndex];
    if (input?.link == null) return "";
    const upstream = node.getInputNode?.(inputIndex);
    if (upstream) {
      for (const candidate of ["source", "workspace", "font", "source_path"]) {
        const widget = upstream.widgets?.find((w: any) => w.name === candidate);
        const value = String(widget?.value ?? "").trim();
        if (value) return value;
      }
    }
  }
  return "";
}

async function fetchDrawBotPresetSource(name: string): Promise<string | null> {
  try {
    return await comfyHost.drawBotPresetSource(name);
  } catch (error) {
    console.warn("[runebender-comfy] drawbot preset fetch failed:", error);
    return null;
  }
}

function setWidgetStringValue(widget: any, value: string) {
  widget.value = value;
  if (widget.inputEl) widget.inputEl.value = value;
  // Keep the CodeMirror view (if mounted) in sync with programmatic changes
  // such as preset loads — but only when it differs, so we don't reset the
  // cursor on every keystroke-driven round trip.
  const cm = widget.__runebenderCM;
  if (cm && cm.getValue() !== value) cm.setValue(value);
}

// CodeMirror 5 (vendored under web/public/vendor/codemirror, copied into
// dist/ by Vite) gives the DrawBot script widget Python syntax highlighting
// and line numbers, matching the old comfyfont DrawBot editor. Loaded once,
// lazily, via <script>/<link> tags resolved relative to this bundle's URL
// (same trick as injectRunebenderStyles).
let codeMirrorReady: Promise<void> | null = null;
function loadCodeMirror(): Promise<void> {
  if (codeMirrorReady) return codeMirrorReady;
  codeMirrorReady = new Promise<void>((resolve) => {
    const base = new URL("./vendor/codemirror/", import.meta.url).href;
    for (const css of ["codemirror.css", "theme-preschool.css"]) {
      const href = base + css;
      if (!document.querySelector(`link[href="${href}"]`)) {
        const link = Object.assign(document.createElement("link"), { rel: "stylesheet", href });
        document.head.appendChild(link);
      }
    }
    const loadScript = (src: string, cb: () => void) => {
      if (document.querySelector(`script[src="${src}"]`)) { cb(); return; }
      const s = document.createElement("script");
      s.src = src;
      s.onload = cb;
      s.onerror = () => { console.error("[runebender-comfy] failed to load", src); resolve(); };
      document.head.appendChild(s);
    };
    loadScript(base + "codemirror.js", () => loadScript(base + "python.js", () => resolve()));
  });
  return codeMirrorReady;
}

// The script widget's underlying <textarea>: widget.inputEl (deprecated) on
// older ComfyUI, widget.element on newer builds.
function scriptTextareaEl(widget: any): HTMLTextAreaElement | undefined {
  if (widget.inputEl instanceof HTMLTextAreaElement) return widget.inputEl;
  const el = widget.element;
  if (el instanceof HTMLTextAreaElement) return el;
  return el?.querySelector?.("textarea") ?? undefined;
}

// Mount a CodeMirror editor over the (now-hidden) script textarea. Strategy
// ported from comfyfont: a position:fixed overlay whose rect is synced to the
// textarea's getBoundingClientRect via rAF, so it follows ComfyUI's canvas
// pan/zoom/resize without reaching into litegraph's coordinate system. The
// textarea stays in layout (opacity 0, pointer-events none) so its value
// plumbing — and getBoundingClientRect — keep working.
async function mountScriptEditor(widget: any, textarea: HTMLTextAreaElement) {
  if (widget.__runebenderCM) return;
  await loadCodeMirror();
  const CM = (window as any).CodeMirror;
  if (!CM) {
    console.error("[runebender-comfy] CodeMirror unavailable");
    return;
  }
  if (widget.__runebenderCM) return; // another mount won the race during await

  const wrapper = document.createElement("div");
  Object.assign(wrapper.style, {
    position: "fixed",
    zIndex: "1000",
    overflow: "hidden",
    boxSizing: "border-box",
  });
  document.body.appendChild(wrapper);

  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";

  const editor = CM(wrapper, {
    value: String(widget.value ?? ""),
    mode: "python",
    theme: "preschool",
    lineNumbers: true,
    fixedGutter: true,
    indentUnit: 4,
    tabSize: 4,
    indentWithTabs: false,
    lineWrapping: false,
    scrollbarStyle: "null",
    extraKeys: {
      Tab: (cm: any) => cm.execCommand("indentMore"),
      "Shift-Tab": (cm: any) => cm.execCommand("indentLess"),
    },
  });
  widget.__runebenderCM = editor;

  let lastW = 0;
  let lastH = 0;
  const syncLoop = () => {
    if (!textarea.isConnected) {
      wrapper.remove();
      widget.__runebenderCM = null;
      return;
    }
    const rect = textarea.getBoundingClientRect();
    Object.assign(wrapper.style, {
      top: `${rect.top}px`,
      left: `${rect.left}px`,
      width: `${rect.width}px`,
      height: `${rect.height}px`,
    });
    if (rect.width > 0 && (rect.width !== lastW || rect.height !== lastH)) {
      lastW = rect.width;
      lastH = rect.height;
      editor.setSize(rect.width, rect.height);
      editor.refresh();
    }
    requestAnimationFrame(syncLoop);
  };
  requestAnimationFrame(syncLoop);

  editor.on("change", () => {
    const src = editor.getValue();
    widget.value = src;
    textarea.value = src;
  });
}

function attachFontSpecimenPresetSync(node: any) {
  if (node.comfyClass === "ComfyFontDrawBot") {
    node.title = "DrawBot Skia";
    node.constructor.title = "DrawBot Skia";
    node.constructor.category = "Runebender / Font";
  }
  const presetWidget = node.widgets?.find((widget: any) => widget.name === "preset");
  const scriptWidget = node.widgets?.find((widget: any) =>
    widget.name === "custom_script" || widget.name === "script_override"
  );
  if (!presetWidget || !scriptWidget) return;

  const pollForTextarea = window.setInterval(() => {
    const textarea = scriptTextareaEl(scriptWidget);
    if (!textarea) return;
    window.clearInterval(pollForTextarea);
    void mountScriptEditor(scriptWidget, textarea);
  }, 100);

  const presetValues = presetWidget.options?.values ?? [];
  const normalizePreset = () => {
    if (!presetValues.length) return;
    if (presetValues.includes(presetWidget.value)) return;
    const match = presetValues.find((value: string) => value.toLowerCase() === String(presetWidget.value).toLowerCase());
    if (match) presetWidget.value = match;
  };

  const loadPresetIntoScript = async (value: string, force = false) => {
    normalizePreset();
    if (!force && String(scriptWidget.value ?? "").trim()) return;
    const source = await fetchDrawBotPresetSource(value);
    if (source != null) setWidgetStringValue(scriptWidget, source);
  };

  normalizePreset();
  void loadPresetIntoScript(String(presetWidget.value ?? ""));

  const originalCallback = presetWidget.callback;
  presetWidget.callback = function (value: string, ...args: any[]) {
    originalCallback?.call(this, value, ...args);
    void loadPresetIntoScript(String(value ?? ""), true);
  };

  const originalConfigure = node.onConfigure;
  node.onConfigure = function (info: any) {
    originalConfigure?.call(this, info);
    normalizePreset();
    void loadPresetIntoScript(String(presetWidget.value ?? ""));
  };
}

type OverlaySession = {
  root: HTMLDivElement;
  cleanup: () => void;
};

let activeOverlay: OverlaySession | null = null;

function closeActiveOverlay() {
  if (!activeOverlay) return;
  activeOverlay.cleanup();
}

const RUNEBENDER_OVERLAY_CLASS = "runebender-overlay-open";
const RUNEBENDER_OVERLAY_STYLE_ID = "runebender-comfy-overlay-chrome-style";

function ensureRunebenderOverlayChromeStyle() {
  if (document.getElementById(RUNEBENDER_OVERLAY_STYLE_ID)) return;
  const style = document.createElement("style");
  style.id = RUNEBENDER_OVERLAY_STYLE_ID;
  style.textContent = `
body.${RUNEBENDER_OVERLAY_CLASS} #graph-canvas-container .ml-1.flex.flex-col.gap-1.pt-1,
body.${RUNEBENDER_OVERLAY_CLASS} #graph-canvas-container .graph-canvas-panel > *,
body.${RUNEBENDER_OVERLAY_CLASS} #graph-canvas-container .selection-toolbox {
  display: none !important;
  pointer-events: none !important;
}
`;
  document.head.appendChild(style);
}

// Measure ComfyUI's persistent chrome (top tabs, left/right rails) so the
// editor overlay can sit inside it rather than covering it. Falls back to
// conservative defaults if the selectors change in a future ComfyUI build.
function measureComfyChromeInsets(): {
  top: number;
  left: number;
  right: number;
  bottom: number;
} {
  // Use the chrome elements' actual EDGE positions, not their sizes.
  // The overlay is positioned by distance-from-viewport-edge, so what
  // matters is where each chrome region ends, not how tall/wide it is.
  // (Using height assumed the top chrome starts at y=0, which adds a
  // few px of extra top margin if it doesn't.)
  // The visible ComfyUI chrome that the editor must sit below/right of:
  //   top  = .workflow-tabs-container (the workflow tab strip)
  //   left = .side-tool-bar-container (the icon rail)
  // The #comfyui-body-* elements are zero-size grid markers, and
  // #graph-canvas is full-viewport (the rails float over it), so
  // neither is usable for insets.
  //
  // Each measurement is gated on a sane range so a surprising rect can
  // never blow the layout up the way the full-viewport graph-canvas
  // anchor did — out-of-range values fall back to the known-good
  // constants (which keep every panel visible, just a slightly large
  // top margin).
  const DEFAULT_TOP = 40;
  const DEFAULT_LEFT = 56;
  const measureEdge = (
    selector: string,
    edge: "bottom" | "right",
    fallback: number,
    maxSane: number,
  ): number => {
    const el = document.querySelector(selector) as HTMLElement | null;
    if (!el) return fallback;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return fallback;
    const value = edge === "bottom" ? r.bottom : r.right;
    if (value <= 0 || value > maxSane) return fallback;
    return Math.round(value);
  };

  const insets = {
    top: measureEdge(".workflow-tabs-container", "bottom", DEFAULT_TOP, 200),
    left: measureEdge(".side-tool-bar-container", "right", DEFAULT_LEFT, 300),
    right: 0,
    bottom: 0,
  };
  console.info("[runebender-comfy] chrome insets", JSON.stringify(insets));
  return insets;
}

function openRunebenderOverlay(options: {
  nodeId: string;
  fontPathRef: { value: string };
  onGlyphDataChange: (value: string) => void;
  onWorkspaceSaved?: () => void;
}) {
  closeActiveOverlay();

  const insets = measureComfyChromeInsets();
  ensureRunebenderOverlayChromeStyle();
  document.body.classList.add(RUNEBENDER_OVERLAY_CLASS);

  const root = document.createElement("div");
  root.setAttribute("data-runebender-overlay", options.nodeId);
  root.style.cssText = [
    "position:fixed",
    `top:${insets.top}px`,
    `left:${insets.left}px`,
    `right:${insets.right}px`,
    `bottom:${insets.bottom}px`,
    // Stay above the graph canvas, but below ComfyUI's global side-panel
    // flyouts and theme menus.
    "z-index:20",
    "display:block",
    "padding:0",
    "margin:0",
    "box-sizing:border-box",
    "min-width:0",
    "min-height:0",
    // Solid background so the overlay is obviously open even if Vue or
    // the WASM editor never paints a frame. Previously this was
    // transparent, which made an unmounted overlay look indistinguishable
    // from the editor never opening at all.
    "background:#181818",
    // Flush plain rectangle — no border / radius / shadow. The earlier
    // "floating panel" framing produced a double line under ComfyUI's
    // tab bar (ComfyUI's bottom border + our top border) and rounded
    // top corners. The editor should read as a plain background filling
    // the area under the chrome.
    "border:none",
    "border-radius:0",
    "box-shadow:none",
    "overflow:hidden",
  ].join(";");

  // The Close button now lives inside the editor's own SystemToolbar
  // (top-right, next to Save) via the onCloseRequested prop passed to
  // the Vue app. Keep a tiny diagnostic status banner up top until the
  // editor is mounted, then hide it.

  const statusBanner = document.createElement("div");
  statusBanner.style.cssText = [
    "position:absolute",
    "left:8px",
    "top:8px",
    "right:8px",
    "z-index:2",
    "padding:6px 12px",
    "border-radius:6px",
    "background:#222",
    "color:#ddd",
    "font:12px ui-sans-serif, system-ui, sans-serif",
    "pointer-events:none",
  ].join(";");
  statusBanner.textContent = `Runebender (${RUNEBENDER_BUNDLE_FINGERPRINT}) — initializing editor…`;

  const errorPane = document.createElement("pre");
  errorPane.style.cssText = [
    "position:absolute",
    "left:20px",
    "right:20px",
    "top:56px",
    "bottom:20px",
    "z-index:2",
    "margin:0",
    "padding:16px",
    "border:2px solid #d33",
    "border-radius:8px",
    "background:#2a0808",
    "color:#fdd",
    "font:13px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace",
    "white-space:pre-wrap",
    "overflow:auto",
    "display:none",
  ].join(";");

  const showError = (label: string, err: unknown) => {
    const message = err instanceof Error
      ? `${err.name}: ${err.message}\n\n${err.stack ?? ""}`
      : String(err);
    errorPane.textContent = `Runebender failed: ${label}\n\n${message}`;
    errorPane.style.display = "block";
    statusBanner.textContent = `Runebender — error: ${label}`;
    statusBanner.style.background = "#502020";
    statusBanner.style.color = "#fff";
    console.error(`[runebender-comfy] ${label}:`, err);
  };

  const mount = document.createElement("div");
  mount.style.cssText = [
    "position:absolute",
    "inset:0",
    "z-index:1",
    "overflow:hidden",
  ].join(";");

  root.append(mount, statusBanner, errorPane);
  document.body.appendChild(root);

  // Anything thrown by Vue setup or async WASM init while the overlay
  // is open should land on the visible error pane so users don't need
  // DevTools to diagnose.
  const onWindowError = (event: ErrorEvent) => {
    if (errorPane.style.display !== "none") return;
    showError("uncaught error", event.error ?? event.message);
  };
  const onUnhandledRejection = (event: PromiseRejectionEvent) => {
    if (errorPane.style.display !== "none") return;
    showError("unhandled rejection", event.reason);
  };
  window.addEventListener("error", onWindowError);
  window.addEventListener("unhandledrejection", onUnhandledRejection);

  const onDocumentPointerDown = (event: PointerEvent) => {
    const target = event.target as Element | null;
    if (!target?.closest(".workflow-tabs-container")) return;
    console.info("[runebender-comfy] closing editor for workflow tab switch");
    cleanup();
  };
  document.addEventListener("pointerdown", onDocumentPointerDown, true);

  // Forward-declared so the Vue app can call it via onCloseRequested.
  let cleanup: () => void = () => {};

  let app: ReturnType<typeof createApp> | null = null;
  try {
    app = createApp(Runebender, {
      nodeId: options.nodeId,
      fontPathRef: options.fontPathRef,
      onGlyphDataChange: options.onGlyphDataChange,
      onWorkspaceSaved: options.onWorkspaceSaved,
      onCloseRequested: () => cleanup(),
    });
    app.provide(runebenderHostKey, comfyHost);
    app.config.errorHandler = (err, _instance, info) => {
      showError(`Vue error (${info})`, err);
    };
    app.mount(mount);
    // Editor mounted cleanly — hide the diagnostic banner so the
    // editor's own toolbar is the only chrome on screen.
    statusBanner.style.display = "none";
  } catch (err) {
    showError("createApp/mount threw synchronously", err);
  }

  cleanup = () => {
    window.removeEventListener("error", onWindowError);
    window.removeEventListener("unhandledrejection", onUnhandledRejection);
    document.removeEventListener("pointerdown", onDocumentPointerDown, true);
    document.removeEventListener("keydown", onKeyDown, true);
    try {
      app?.unmount();
    } catch (err) {
      console.warn("[runebender-comfy] unmount threw:", err);
    }
    root.remove();
    document.body.classList.remove(RUNEBENDER_OVERLAY_CLASS);
    if (activeOverlay?.root === root) {
      activeOverlay = null;
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      cleanup();
    }
  };

  document.addEventListener("keydown", onKeyDown, true);

  activeOverlay = { root, cleanup };
}

app.registerExtension({
  name: "runebender-comfy.Runebender",
  nodeCreated(node: any) {
    if (node.comfyClass === "FontSpecimen" || node.comfyClass === "ComfyFontDrawBot") {
      attachFontSpecimenPresetSync(node);
    }
  },
  async beforeRegisterNodeDef(nodeType: any, nodeData: any) {
    if (nodeData.name !== "Runebender") return;

    // No on-node specimen preview. ComfyUI v1's frontend does not invoke
    // onDrawBackground on custom nodes (verified by installing comfyfont
    // side-by-side: its specimen also does not render in this build).
    // The full-screen Edit overlay renders the actual font, which is
    // where any preview work belongs.

    const onCreated = nodeType.prototype.onNodeCreated;
    nodeType.prototype.onNodeCreated = function () {
      onCreated?.apply(this, arguments);
      console.log(`[runebender-comfy] ${RUNEBENDER_BUNDLE_FINGERPRINT} Runebender node active`);
      logNodeSockets(this, "Runebender node sockets");
      this.properties ??= {};
      this.properties.glyph_data ??= "";

      const sourceWidget = this.widgets?.find((w: any) => w.name === "source_path");
      const sourceKindWidget = this.widgets?.find((w: any) => w.name === "source_kind");
      const workspaceNameWidget = this.widgets?.find((w: any) => w.name === "workspace_name");

      let workspaceSelect: any = null;

      const visibleSourceValue = () => String(workspaceSelect?.value ?? "").trim();
      const storedSourceValue = () => String(sourceWidget?.value ?? "").trim();
      const localSourceValue = () => {
        const visible = normalizeSourceValue(visibleSourceValue());
        const stored = storedSourceValue();
        if (stored && stored !== "demo") return stored;
        if (visible && visible !== "demo") return visible;
        return stored || visible || "demo";
      };

      const currentSourceValue = () => {
        const upstream = resolveConnectedFontValue(this);
        if (upstream) return upstream;
        return localSourceValue();
      };

      let workspaceChoiceBySlot = new Map<string, WorkspaceChoice>();
      let workspaceSlotByLabel = new Map<string, string>();

      const normalizeSourceValue = (value: string) => {
        const source = String(value ?? "").trim() || "demo";
        return workspaceSlotByLabel.get(source) || source;
      };

      const displayLabelForSource = (source: string) => {
        return workspaceChoiceBySlot.get(source)?.label || source;
      };

      const setWorkspaceChoices = (choices: WorkspaceChoice[], pinnedValue?: string) => {
        if (!workspaceSelect) return;
        const pinned = String(pinnedValue ?? "").trim();
        workspaceChoiceBySlot = new Map([["demo", { slot: "demo", label: "demo" }]]);
        for (const choice of choices) {
          if (!choice.slot) continue;
          workspaceChoiceBySlot.set(choice.slot, {
            slot: choice.slot,
            label: choice.label || choice.slot,
            origin_source: choice.origin_source,
          });
        }
        if (pinned && !workspaceChoiceBySlot.has(pinned)) {
          workspaceChoiceBySlot.set(pinned, { slot: pinned, label: pinned });
        }
        workspaceSlotByLabel = new Map(
          Array.from(workspaceChoiceBySlot.values()).map((choice) => [choice.label, choice.slot]),
        );
        workspaceSelect.options.values = Array.from(
          new Set(Array.from(workspaceChoiceBySlot.values()).map((choice) => choice.label)),
        );
      };

      const setSourceValue = (value: string) => {
        const source = normalizeSourceValue(value);
        setWorkspaceChoices(
          Array.from(workspaceChoiceBySlot.values()),
          source,
        );
        if (workspaceSelect) {
          workspaceSelect.value = displayLabelForSource(source);
        }
        if (sourceWidget && String(sourceWidget.value ?? "") !== source) {
          sourceWidget.value = source;
        }
      };

      // Node-body specimen preview, mounted via ComfyUI v1's
      // addDOMWidget API as a plain <img>. The browser handles
      // loading, decoding, error events, retry on src reassignment —
      // none of the canvas-paint / setDirtyCanvas plumbing we wasted
      // effort on, because v1 doesn't invoke onDrawBackground on
      // custom nodes (verified with comfyfont side-by-side).
      const previewImg = document.createElement("img");
      previewImg.alt = "Font specimen";
      previewImg.style.cssText = [
        "display:block",
        "width:100%",
        // 1:1 aspect ratio so the displayed box matches the 1024x1024
        // backing image — no letterboxing, no wasted space.
        "aspect-ratio:1 / 1",
        "object-fit:contain",
        "background:#111",
        "border:1px solid #2a2a2a",
        "border-radius:6px",
        "padding:4px",
        "box-sizing:border-box",
        // Display-only specimen. Without this the <img> captures pointer
        // events over the canvas it overlays, swallowing wire drags (and
        // node grabs) that cross it — so no link can be dragged to/from a
        // socket beneath or beside it. Make it transparent to the mouse.
        "pointer-events:none",
      ].join(";");

      // Latin specimen sent as a single string. The backend picks the
      // line count (1..8) that maximizes per-glyph scale on the canvas,
      // so the glyphs fill the preview box instead of being pinned by
      // a 26-letter line. TODO: language-specific default character
      // sets for Arabic, Hebrew, CJK, etc. once Latin lands cleanly.
      const SPECIMEN_TEXT =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ" +
        "abcdefghijklmnopqrstuvwxyz" +
        "0123456789";
      // Square 1024x1024 backing image. The browser downsamples to the
      // displayed widget size; the square aspect ratio matches what
      // node-based editor previews typically use and gives the
      // auto-wrap algorithm balanced width vs height to work with.
      const SPECIMEN_WIDTH = 1024;
      const SPECIMEN_HEIGHT = 1024;
      previewImg.addEventListener("error", () => {
        console.warn("[runebender-comfy] preview <img> failed", previewImg.src);
      });
      previewImg.addEventListener("load", () => {
        console.info("[runebender-comfy] preview <img> loaded", JSON.stringify({
          src: previewImg.src,
          width: previewImg.naturalWidth,
          height: previewImg.naturalHeight,
        }));
      });

      let lastPreviewSlot = "";
      const syncPreview = (force = false) => {
        const value = currentSourceValue();
        if (!value) {
          lastPreviewSlot = "";
          previewImg.removeAttribute("src");
          return;
        }
        if (!force && value === lastPreviewSlot && previewImg.src) return;
        lastPreviewSlot = value;
        const params = new URLSearchParams({
          text: SPECIMEN_TEXT,
          width: String(SPECIMEN_WIDTH),
          height: String(SPECIMEN_HEIGHT),
          // Cache-bust so the image refreshes after edits.
          t: String(Date.now()),
        });
        previewImg.src = comfyHost.workspacePreviewUrl(value, params);
        console.info("[runebender-comfy] preview request", JSON.stringify({
          value,
          visible: visibleSourceValue(),
          stored: storedSourceValue(),
          upstream: resolveConnectedFontValue(this),
          src: previewImg.src,
        }));
      };

      const refreshChoices = async () => {
        try {
          const current = localSourceValue();
          setWorkspaceChoices(await comfyHost.listWorkspaceSlots(), current);
          setSourceValue(current || "demo");
          syncPreview();
        } catch (error) {
          console.warn("workspace list failed:", error);
        }
      };

      const linkSourcePath = async (initialPath?: string) => {
        const sourcePath = initialPath ?? await requestSourcePath(String(sourceWidget?.value ?? ""));
        if (!sourcePath) return;
        const { response, data } = await comfyHost.linkSource({
          sourcePath,
          sourceKind: String(sourceKindWidget?.value ?? "auto"),
          workspaceName: String(workspaceNameWidget?.value ?? ""),
        });
        if (!response.ok) {
          const routeHint = response.status === 404 || response.status === 405
            ? " The browser has the new Runebender bundle, but ComfyUI has not registered the linked-source backend route. Fully restart ComfyUI, then hard-refresh the browser."
            : "";
          throw new Error(data.error || `${response.status} ${response.statusText}.${routeHint}`);
        }
        const slot = String(data.slot ?? "");
        if (slot) {
          setSourceValue(slot);
        }
        await refreshChoices();
        if (slot) setSourceValue(slot);
        syncPreview();
        this.setDirtyCanvas(true, true);
        return slot;
      };

      const ensureEditableWorkspace = async (value: string) => {
        if (!value || value.startsWith("workspace://")) return value;
        const values = Array.from(workspaceChoiceBySlot.keys());
        if (values.includes(value)) return value;
        if (value === "demo" || value === "ufo/designspace") return value;
        if (!looksLikeFontSourcePath(value)) return value;
        return (await linkSourcePath(value)) || value;
      };

      // Widget order, matching node-based editors (Fusion/Nuke/Houdini):
      //   Font Source (combo)         — the loaded font, visible at a glance
      //   Open Font Source (button)   — choose a disk source for edit/save-back
      //   Edit Font Source (button)   — open the full-screen editor
      //   <img> specimen preview      — DOM widget, v1-native
      workspaceSelect = this.addWidget("combo", "Font Source", "demo", (value: any) => {
        setSourceValue(String(value ?? ""));
        syncPreview();
      }, {
        values: ["demo"],
      });
      workspaceSelect.serialize = true;

      const importButton = this.addWidget("button", "Open Font Source", null, () => {
        void linkSourcePath().catch((error) => {
          alert(`Runebender source open failed: ${error}`);
          console.error("[runebender-comfy] source open failed:", error);
        });
      }, {});
      importButton.serialize = false;

      const editButton = this.addWidget("button", "Edit Font Source", null, () => {
        void (async () => {
          const currentFont = currentSourceValue();
          if (!currentFont) {
            alert("Runebender: pick a font first (the Edit button needs a loaded font).");
            return;
          }
          const editableFont = await ensureEditableWorkspace(currentFont);
          const fontPath = ref(editableFont);
          openRunebenderOverlay({
            nodeId: String(this.id),
            fontPathRef: fontPath,
            onGlyphDataChange: (value: string) => {
              this.setDirtyCanvas(true, true);
              this.properties.glyph_data = value;
            },
            onWorkspaceSaved: () => {
              syncPreview(true);
              this.setDirtyCanvas(true, true);
            },
          });
        })().catch((err) => {
          const message = err instanceof Error
            ? `${err.name}: ${err.message}\n\n${err.stack ?? ""}`
            : String(err);
          alert(`Runebender Edit click threw:\n\n${message}`);
          console.error("[runebender-comfy] Edit click threw:", err);
        });
      }, {});
      editButton.serialize = false;

      // Mount the <img> as a DOM widget — ComfyUI v1's actually-supported
      // way to put HTML inside a node body. Falls back to a plain
      // canvas widget if addDOMWidget isn't available on this build.
      if (typeof this.addDOMWidget === "function") {
        this.addDOMWidget("preview", "image", previewImg, {
          serialize: false,
          hideOnZoom: false,
        });
      } else {
        console.warn("[runebender-comfy] addDOMWidget not available; specimen preview disabled");
      }

      const origConfigure = this.onConfigure;
      this.onConfigure = function (info: unknown) {
        origConfigure?.call(this, info);
        const restored = localSourceValue();
        console.info("[runebender-comfy] source restore", JSON.stringify({
          restored,
          visible: visibleSourceValue(),
          stored: storedSourceValue(),
        }));
        if (restored) setSourceValue(restored);
        syncPreview();
      };

      const hideWidget = (widget: any) => {
        if (!widget) return;
        widget.hidden = true;
        widget.type = "hidden";
        widget.computeSize = () => [0, -4];
        widget.draw = () => {};
      };

      if (sourceWidget) {
        const origCallback = sourceWidget.callback;
        sourceWidget.callback = (...args: any[]) => {
          origCallback?.(...args);
          const source = String(sourceWidget.value ?? "").trim();
          if (source && workspaceSelect && visibleSourceValue() !== source) {
            workspaceSelect.value = displayLabelForSource(source);
          }
          syncPreview();
        };
        hideWidget(sourceWidget);
      }
      hideWidget(sourceKindWidget);
      hideWidget(workspaceNameWidget);

      const [oldWidth, oldHeight] = this.size;
      this.setSize([Math.max(oldWidth, 360), Math.max(oldHeight, 320)]);

      void refreshChoices();
      syncPreview();
    };
  },
});
