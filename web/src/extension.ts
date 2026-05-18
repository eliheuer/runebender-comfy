// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";
import { createApp, ref } from "vue";

import Runebender from "./Runebender.vue";

const RUNEBENDER_BUNDLE_FINGERPRINT = "rb-bundle-2026-05-18-close-in-toolbar";
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

function createHiddenFileInput(
  onChange: () => Promise<void>,
  options: { directory?: boolean } = {},
) {
  const input = document.createElement("input");
  input.type = "file";
  input.multiple = true;
  input.accept = ".designspace,.ufo,.glyphs,.glyphspackage";
  if (options.directory) {
    input.webkitdirectory = true;
  }
  input.style.display = "none";

  input.addEventListener("change", async () => {
    if (!input.files || input.files.length === 0) return;
    try {
      await onChange();
    } catch (error) {
      console.warn("font import failed:", error);
    } finally {
      input.value = "";
    }
  });

  return input;
}

function resolveNodeFontValue(node: any): string {
  const inputIndex = node.inputs?.findIndex((slot: any) => slot.name === "font") ?? -1;
  if (inputIndex >= 0) {
    const upstream = node.getInputNode?.(inputIndex);
    if (upstream) {
      for (const candidate of ["workspace", "font", "source_path"]) {
        const widget = upstream.widgets?.find((w: any) => w.name === candidate);
        const value = String(widget?.value ?? "").trim();
        if (value) return value;
      }
    }
  }

  const directWidget = node.widgets?.find((w: any) => w.name === "font");
  return String(directWidget?.value ?? "").trim();
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
  const pick = (selectors: string[], dimension: "h" | "w"): number => {
    for (const sel of selectors) {
      const el = document.querySelector(sel) as HTMLElement | null;
      if (!el) continue;
      const rect = el.getBoundingClientRect();
      const value = dimension === "h" ? rect.height : rect.width;
      if (value > 0) return Math.round(value);
    }
    return 0;
  };
  return {
    top: pick([".comfyui-body-top", ".comfy-menu", "header.app-header"], "h") || 40,
    left: pick([".comfyui-body-left", ".side-bar-panel.left"], "w") || 56,
    right: pick([".comfyui-body-right", ".side-bar-panel.right"], "w") || 0,
    bottom: pick([".comfyui-body-bottom"], "h") || 0,
  };
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
    // Subtle border + shadow so the editor reads as a distinct surface
    // sitting on top of ComfyUI rather than replacing it.
    "border:1px solid #2a2a2a",
    "border-radius:6px",
    "box-shadow:0 8px 32px rgba(0,0,0,0.5)",
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

      const specimenImage = new Image();
      let specimenStatus = "No workspace selected";
      let specimenReady = false;
      specimenImage.onload = () => {
        specimenReady = true;
        this.setDirtyCanvas(true, true);
      };
      specimenImage.onerror = () => {
        specimenReady = false;
        this.setDirtyCanvas(true, true);
      };

      const currentSourceValue = () => {
        const upstream = resolveNodeFontValue(this);
        if (upstream) return upstream;
        return String(sourceWidget?.value ?? "").trim();
      };

      const syncPreview = () => {
        const value = currentSourceValue();
        if (!value) {
          specimenStatus = "No workspace selected";
          specimenReady = false;
          specimenImage.removeAttribute("src");
          this.setDirtyCanvas(true, true);
          return;
        }
        specimenStatus = value.startsWith("workspace://")
          ? value.slice("workspace://".length)
          : value;
        specimenReady = false;
        specimenImage.src = `/runebender/workspace/${encodeURIComponent(value)}/preview?text=Aa&width=480&height=160&t=${Date.now()}`;
        this.setDirtyCanvas(true, true);
      };

      const refreshChoices = async () => {
        try {
          const response = await fetch("/runebender/workspaces");
          if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
          const data = (await response.json()) as { slots?: string[] };
          const slots = Array.from(new Set(["demo", ...(data.slots ?? [])]));
          workspaceSelect.options.values = slots;
          const current = String(sourceWidget?.value ?? "demo");
          const next = slots.includes(current) ? current : "demo";
          workspaceSelect.value = next;
          if (sourceWidget) {
            sourceWidget.value = next;
            sourceWidget.callback?.(next);
          }
          syncPreview();
        } catch (error) {
          console.warn("workspace list failed:", error);
        }
      };

      const folderInput = createHiddenFileInput(async () => {
        const files = Array.from(folderInput.files ?? []);
        if (files.length === 0) return;
        const body = new FormData();
        for (const file of files) {
          const relPath = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
          body.append("file", file, relPath);
        }
        body.append("source_kind", String(sourceKindWidget?.value ?? "auto"));
        body.append("workspace_name", String(workspaceNameWidget?.value ?? ""));
        const response = await fetch("/runebender/import_font", {
          method: "POST",
          body,
        });
        if (!response.ok) {
          throw new Error(`${response.status} ${response.statusText}`);
        }
        const data = (await response.json()) as { slot?: string };
        const slot = String(data.slot ?? "");
        if (sourceWidget && slot) {
          sourceWidget.value = slot;
          sourceWidget.callback?.(slot);
        }
        await refreshChoices();
        workspaceSelect.value = slot || workspaceSelect.value;
        syncPreview();
        this.setDirtyCanvas(true, true);
      }, { directory: true });

      const fileInput = createHiddenFileInput(async () => {
        const files = Array.from(fileInput.files ?? []);
        if (files.length === 0) return;
        const body = new FormData();
        for (const file of files) {
          body.append("file", file, file.name);
        }
        body.append("source_kind", String(sourceKindWidget?.value ?? "auto"));
        body.append("workspace_name", String(workspaceNameWidget?.value ?? ""));
        const response = await fetch("/runebender/import_font", {
          method: "POST",
          body,
        });
        if (!response.ok) {
          throw new Error(`${response.status} ${response.statusText}`);
        }
        const data = (await response.json()) as { slot?: string };
        const slot = String(data.slot ?? "");
        if (sourceWidget && slot) {
          sourceWidget.value = slot;
          sourceWidget.callback?.(slot);
        }
        await refreshChoices();
        workspaceSelect.value = slot || workspaceSelect.value;
        syncPreview();
        this.setDirtyCanvas(true, true);
      });

      const importFolderButton = this.addWidget("button", "Import Folder...", null, () => {
        folderInput.click();
      }, {});
      importFolderButton.serialize = false;

      const importFileButton = this.addWidget("button", "Import File...", null, () => {
        fileInput.click();
      }, {});
      importFileButton.serialize = false;

      const refreshButton = this.addWidget("button", "Refresh Workspaces", null, () => {
        void refreshChoices();
      }, {});
      refreshButton.serialize = false;

      const workspaceSelect = this.addWidget("combo", "workspace", "demo", (value: any) => {
        if (sourceWidget) {
          sourceWidget.value = String(value ?? "");
          sourceWidget.callback?.(sourceWidget.value);
        }
        syncPreview();
      }, {
        values: ["demo"],
      });
      workspaceSelect.serialize = false;

      const editButton = this.addWidget("button", "Edit", null, () => {
        try {
          const currentFont = currentSourceValue();
          if (!currentFont) {
            alert("Runebender: pick a workspace first (the Edit button needs a loaded font).");
            return;
          }
          const fontPath = ref(currentFont);
          openRunebenderOverlay({
            nodeId: String(this.id),
            fontPathRef: fontPath,
            onGlyphDataChange: (value: string) => {
              this.setDirtyCanvas(true, true);
              this.properties.glyph_data = value;
            },
          });
        } catch (err) {
          const message = err instanceof Error
            ? `${err.name}: ${err.message}\n\n${err.stack ?? ""}`
            : String(err);
          alert(`Runebender Edit click threw:\n\n${message}`);
          console.error("[runebender-comfy] Edit click threw:", err);
        }
      }, {});
      editButton.serialize = false;

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
          syncPreview();
        };
        hideWidget(sourceWidget);
      }
      hideWidget(sourceKindWidget);
      hideWidget(workspaceNameWidget);

      const [oldWidth, oldHeight] = this.size;
      this.setSize([Math.max(oldWidth, 360), Math.max(oldHeight, 220)]);

      void refreshChoices();
      syncPreview();

      const origDrawBackground = this.onDrawBackground;
      this.onDrawBackground = function (ctx: CanvasRenderingContext2D) {
        origDrawBackground?.apply(this, arguments as any);
        const pad = 8;
        const widgets = this.widgets ?? [];
        const lastWidget = widgets[widgets.length - 1];
        const imgY = lastWidget?.last_y != null ? lastWidget.last_y + 26 : widgets.length * 24 + 10;
        const availW = this.size[0] - pad * 2;
        const availH = this.size[1] - imgY - pad;
        if (availW < 20 || availH < 20) return;

        ctx.save();
        ctx.beginPath();
        ctx.roundRect(pad, imgY, availW, availH, 4);
        ctx.fillStyle = "#111";
        ctx.fill();
        ctx.strokeStyle = "#444";
        ctx.stroke();
        ctx.beginPath();
        ctx.rect(pad, imgY, availW, availH);
        ctx.clip();

        if (specimenReady && specimenImage.complete && specimenImage.naturalWidth > 0) {
          const scale = Math.min(
            availW / specimenImage.naturalWidth,
            availH / specimenImage.naturalHeight,
          );
          const drawW = specimenImage.naturalWidth * scale;
          const drawH = specimenImage.naturalHeight * scale;
          const x = pad + (availW - drawW) / 2;
          const y = imgY + (availH - drawH) / 2;
          ctx.drawImage(specimenImage, x, y, drawW, drawH);
        } else {
          ctx.fillStyle = "#bbb";
          ctx.font = "12px ui-sans-serif, system-ui, sans-serif";
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(specimenStatus, pad + availW / 2, imgY + availH / 2);
        }
        ctx.restore();
      };
    };
  },
});
