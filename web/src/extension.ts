// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";
import { createApp, ref } from "vue";

import { runebenderHostKey } from "./host/runebenderHost";
import { comfyHost } from "./hosts/comfy/comfyHost";
import Runebender from "./Runebender.vue";

const RUNEBENDER_BUNDLE_FINGERPRINT = "rb-bundle-2026-05-24-sort-dblclick-81";

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

async function chooseSourcePath(mode: "file" | "folder"): Promise<string | null> {
  if (mode === "folder" && typeof window.electronAPI?.showDirectoryPicker === "function") {
    const path = await window.electronAPI.showDirectoryPicker();
    return String(path ?? "").trim() || null;
  }

  const data = await comfyHost.chooseSource(mode);
  if (data.cancelled) return null;
  return String(data.path ?? "").trim() || null;
}

function requestSourcePath(defaultValue: string): Promise<string | null> {
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
    title.textContent = "Link source path";
    title.style.fontSize = "16px";
    title.style.fontWeight = "700";
    title.style.marginBottom = "8px";

    const help = document.createElement("div");
    help.textContent = "Enter a .designspace, .ufo, or folder path. Saves will mirror supported edits back to this source.";
    help.style.color = "#b8bcc2";
    help.style.marginBottom = "12px";

    const input = document.createElement("input");
    input.type = "text";
    input.value = defaultValue;
    input.placeholder = "/path/to/font/VirtuaGrotesk.designspace";
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

    const filePicker = document.createElement("button");
    filePicker.type = "button";
    filePicker.textContent = "Choose File...";
    filePicker.style.padding = "8px 12px";
    filePicker.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    filePicker.style.borderRadius = "8px";
    filePicker.style.background = "#2a2d31";
    filePicker.style.color = "#f1f3f4";

    const folderPicker = document.createElement("button");
    folderPicker.type = "button";
    folderPicker.textContent = "Choose Folder...";
    folderPicker.style.padding = "8px 12px";
    folderPicker.style.border = "1px solid rgba(255, 255, 255, 0.18)";
    folderPicker.style.borderRadius = "8px";
    folderPicker.style.background = "#2a2d31";
    folderPicker.style.color = "#f1f3f4";

    pickerActions.append(filePicker, folderPicker);

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
    submit.textContent = "Link";
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
    const browse = async (mode: "file" | "folder") => {
      filePicker.disabled = true;
      folderPicker.disabled = true;
      try {
        const path = await chooseSourcePath(mode);
        if (path) input.value = path;
      } catch (error) {
        alert(`Runebender source picker failed: ${error}`);
        console.error("[runebender-comfy] source picker failed:", error);
      } finally {
        filePicker.disabled = false;
        folderPicker.disabled = false;
        input.focus();
      }
    };
    filePicker.addEventListener("click", () => {
      void browse("file");
    });
    folderPicker.addEventListener("click", () => {
      void browse("folder");
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

function installGlobalLinkTrace() {
  if (window.__runebenderGlobalLinkTraceInstalled) return;
  window.__runebenderGlobalLinkTraceInstalled = true;

  const LG = window.LiteGraph;
  if (!LG) {
    console.warn("[runebender-comfy] LiteGraph global missing — global trace skipped");
    return;
  }

  const nodeProto = LG.LGraphNode?.prototype;
  const graphProto = LG.LGraph?.prototype;

  if (nodeProto?.disconnectInput) {
    const orig = nodeProto.disconnectInput;
    nodeProto.disconnectInput = function (slot: any, ...args: any[]) {
      const slotIndex = typeof slot === "string" ? this.findInputSlot?.(slot) : slot;
      const input = this.inputs?.[slotIndex];
      if (input?.name === "font" || slot === "font") {
        console.warn("[runebender-comfy] PROTO disconnectInput on font slot", {
          nodeId: this.id,
          nodeTitle: this.title,
          comfyClass: this.comfyClass,
          slot,
          slotIndex,
          link: input?.link ?? null,
          stack: new Error().stack,
        });
      }
      return orig.apply(this, [slot, ...args]);
    };
  }

  if (nodeProto?.disconnectOutput) {
    const orig = nodeProto.disconnectOutput;
    nodeProto.disconnectOutput = function (slot: any, ...args: any[]) {
      const slotIndex = typeof slot === "string" ? this.findOutputSlot?.(slot) : slot;
      const output = this.outputs?.[slotIndex];
      if (output?.name === "font" || slot === "font") {
        console.warn("[runebender-comfy] PROTO disconnectOutput on font slot", {
          nodeId: this.id,
          nodeTitle: this.title,
          comfyClass: this.comfyClass,
          slot,
          slotIndex,
          links: output?.links ? [...output.links] : [],
          stack: new Error().stack,
        });
      }
      return orig.apply(this, [slot, ...args]);
    };
  }

  if (graphProto?.removeLink) {
    const orig = graphProto.removeLink;
    graphProto.removeLink = function (link_id: any) {
      const link = this.links?.[link_id];
      const originNode = link ? this.getNodeById?.(link.origin_id) : null;
      const targetNode = link ? this.getNodeById?.(link.target_id) : null;
      const isFontLink =
        link?.type === "FONT" ||
        originNode?.outputs?.[link?.origin_slot]?.name === "font" ||
        targetNode?.inputs?.[link?.target_slot]?.name === "font";
      if (isFontLink) {
        console.warn("[runebender-comfy] GRAPH removeLink on FONT link", {
          link_id,
          link,
          origin: originNode ? { id: originNode.id, comfyClass: originNode.comfyClass } : null,
          target: targetNode ? { id: targetNode.id, comfyClass: targetNode.comfyClass } : null,
          stack: new Error().stack,
        });
      }
      return orig.apply(this, [link_id]);
    };
  }

  console.info("[runebender-comfy] global FONT link trace installed");
}

function installSlotLinkSensor(node: any, slotKind: "input" | "output", label: string) {
  const slots = slotKind === "input" ? node.inputs : node.outputs;
  if (!Array.isArray(slots)) return;
  for (const slot of slots) {
    if (slot?.name !== "font") continue;
    if (slot.__runebenderLinkSensorInstalled) continue;
    slot.__runebenderLinkSensorInstalled = true;
    const linkKey = slotKind === "input" ? "link" : "links";
    let stored = slot[linkKey];
    Object.defineProperty(slot, linkKey, {
      configurable: true,
      enumerable: true,
      get() {
        return stored;
      },
      set(value) {
        const before = stored;
        stored = value;
        const cleared =
          slotKind === "input"
            ? before != null && value == null
            : (Array.isArray(before) && before.length > 0) &&
              (!Array.isArray(value) || value.length === 0);
        if (cleared) {
          console.warn(`[runebender-comfy] ${label} ${slotKind} font slot ${linkKey} cleared`, {
            nodeId: node.id,
            before,
            value,
            stack: new Error().stack,
          });
        }
      },
    });
  }
}

installGlobalLinkTrace();

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

function traceFontInputDisconnects(node: any, label: string) {
  if (node.__runebenderFontInputTraceInstalled) return;
  if (typeof node.disconnectInput !== "function") return;
  node.__runebenderFontInputTraceInstalled = true;
  const originalDisconnectInput = node.disconnectInput;
  node.disconnectInput = function (slot: any, ...args: any[]) {
    const slotIndex = typeof slot === "string" ? this.findInputSlot?.(slot) : slot;
    const input = this.inputs?.[slotIndex];
    if (slot === "font" || input?.name === "font") {
      console.warn(`[runebender-comfy] ${label} font input disconnect requested`, {
        slot,
        slotIndex,
        link: input?.link ?? null,
        stack: new Error().stack,
      });
      logNodeSockets(this, `${label} before font input disconnect`);
    }
    return originalDisconnectInput.call(this, slot, ...args);
  };
}

function traceFontOutputDisconnects(node: any, label: string) {
  if (node.__runebenderFontOutputTraceInstalled) return;
  if (typeof node.disconnectOutput !== "function") return;
  node.__runebenderFontOutputTraceInstalled = true;
  const originalDisconnectOutput = node.disconnectOutput;
  node.disconnectOutput = function (slot: any, ...args: any[]) {
    const slotIndex = typeof slot === "string" ? this.findOutputSlot?.(slot) : slot;
    const output = this.outputs?.[slotIndex];
    if (slot === "font" || output?.name === "font") {
      console.warn(`[runebender-comfy] ${label} font output disconnect requested`, {
        slot,
        slotIndex,
        links: output?.links ?? [],
        stack: new Error().stack,
      });
      logNodeSockets(this, `${label} before font output disconnect`);
    }
    return originalDisconnectOutput.call(this, slot, ...args);
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
}) {
  closeActiveOverlay();

  const insets = measureComfyChromeInsets();

  const root = document.createElement("div");
  root.setAttribute("data-runebender-overlay", options.nodeId);
  root.style.cssText = [
    "position:fixed",
    `top:${insets.top}px`,
    `left:${insets.left}px`,
    `right:${insets.right}px`,
    `bottom:${insets.bottom}px`,
    "z-index:9999",
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

  // Forward-declared so the Vue app can call it via onCloseRequested.
  let cleanup: () => void = () => {};

  let app: ReturnType<typeof createApp> | null = null;
  try {
    app = createApp(Runebender, {
      nodeId: options.nodeId,
      fontPathRef: options.fontPathRef,
      onGlyphDataChange: options.onGlyphDataChange,
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
    document.removeEventListener("keydown", onKeyDown, true);
    try {
      app?.unmount();
    } catch (err) {
      console.warn("[runebender-comfy] unmount threw:", err);
    }
    root.remove();
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
      traceFontInputDisconnects(this, "Runebender node");
      traceFontOutputDisconnects(this, "Runebender node");
      installSlotLinkSensor(this, "input", "Runebender node");
      installSlotLinkSensor(this, "output", "Runebender node");
      this.properties ??= {};
      this.properties.glyph_data ??= "";

      const sourceWidget = this.widgets?.find((w: any) => w.name === "source_path");
      const sourceKindWidget = this.widgets?.find((w: any) => w.name === "source_kind");
      const workspaceNameWidget = this.widgets?.find((w: any) => w.name === "workspace_name");

      let workspaceSelect: any = null;

      const visibleSourceValue = () => String(workspaceSelect?.value ?? "").trim();
      const storedSourceValue = () => String(sourceWidget?.value ?? "").trim();
      const localSourceValue = () => {
        const visible = visibleSourceValue();
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

      const setWorkspaceChoices = (slots: string[], pinnedValue?: string) => {
        if (!workspaceSelect) return;
        const pinned = String(pinnedValue ?? "").trim();
        workspaceSelect.options.values = Array.from(new Set(
          ["demo", ...(pinned ? [pinned] : []), ...slots.filter(Boolean)],
        ));
      };

      const setSourceValue = (value: string) => {
        const source = String(value ?? "").trim() || "demo";
        setWorkspaceChoices(
          Array.isArray(workspaceSelect?.options?.values)
            ? workspaceSelect.options.values.map((entry: unknown) => String(entry))
            : [],
          source,
        );
        if (workspaceSelect) {
          workspaceSelect.value = source;
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
      const syncPreview = () => {
        const value = currentSourceValue();
        if (!value) {
          lastPreviewSlot = "";
          previewImg.removeAttribute("src");
          return;
        }
        if (value === lastPreviewSlot && previewImg.src) return;
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
        const values = Array.isArray(workspaceSelect.options?.values)
          ? workspaceSelect.options.values.map((entry: unknown) => String(entry))
          : [];
        if (values.includes(value)) return value;
        if (value === "demo" || value === "ufo/designspace") return value;
        if (!looksLikeFontSourcePath(value)) return value;
        return (await linkSourcePath(value)) || value;
      };

      // Widget order, matching node-based editors (Fusion/Nuke/Houdini):
      //   Font Source (combo)         — the loaded font, visible at a glance
      //   Import Font Source (button) — bring a new font into the workspace
      //   Edit Font Source (button)   — open the full-screen editor
      //   <img> specimen preview      — DOM widget, v1-native
      workspaceSelect = this.addWidget("combo", "Font Source", "demo", (value: any) => {
        setSourceValue(String(value ?? ""));
        syncPreview();
      }, {
        values: ["demo"],
      });
      workspaceSelect.serialize = true;

      const importButton = this.addWidget("button", "Import Font Source", null, () => {
        void linkSourcePath().catch((error) => {
          alert(`Runebender source link failed: ${error}`);
          console.error("[runebender-comfy] source link failed:", error);
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
            workspaceSelect.value = source;
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
