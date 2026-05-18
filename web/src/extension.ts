// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";
import { createApp, ref } from "vue";

import Runebender from "./Runebender.vue";

const RUNEBENDER_BUNDLE_FINGERPRINT = "rb-bundle-2026-05-17-runebender-merge";
console.info(`[runebender-comfy] loaded ${RUNEBENDER_BUNDLE_FINGERPRINT}`);

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

function openRunebenderOverlay(options: {
  nodeId: string;
  fontPathRef: { value: string };
  onGlyphDataChange: (value: string) => void;
}) {
  closeActiveOverlay();

  const root = document.createElement("div");
  root.setAttribute("data-runebender-overlay", options.nodeId);
  root.style.cssText = [
    "position:fixed",
    "inset:0",
    "z-index:9999",
    "display:block",
    "padding:0",
    "margin:0",
    "box-sizing:border-box",
    "min-width:0",
    "min-height:0",
    "background:transparent",
  ].join(";");

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.textContent = "Close Editor";
  closeButton.style.cssText = [
    "position:absolute",
    "right:8px",
    "top:8px",
    "z-index:1",
    "height:32px",
    "padding:0 12px",
    "border-radius:6px",
    "border:1px solid var(--input-border,#555)",
    "background:var(--button-bg,#1f1f1f)",
    "color:var(--fg,#eee)",
    "font:12px ui-sans-serif, system-ui, sans-serif",
    "cursor:pointer",
  ].join(";");

  const mount = document.createElement("div");
  mount.style.cssText = [
    "position:relative",
    "width:100%",
    "height:100%",
    "min-width:0",
    "min-height:0",
    "overflow:hidden",
  ].join(";");

  root.append(closeButton, mount);
  document.body.appendChild(root);

  const app = createApp(Runebender, {
    nodeId: options.nodeId,
    fontPathRef: options.fontPathRef,
    onGlyphDataChange: options.onGlyphDataChange,
  });
  app.mount(mount);

  const cleanup = () => {
    document.removeEventListener("keydown", onKeyDown, true);
    app.unmount();
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

  closeButton.addEventListener("click", cleanup);
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
      importFolderButton.label = `Import Folder... (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

      const importFileButton = this.addWidget("button", "Import File...", null, () => {
        fileInput.click();
      }, {});
      importFileButton.serialize = false;
      importFileButton.label = `Import File... (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

      const refreshButton = this.addWidget("button", "Refresh Workspaces", null, () => {
        void refreshChoices();
      }, {});
      refreshButton.serialize = false;
      refreshButton.label = `Refresh Workspaces (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

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
        const currentFont = currentSourceValue();
        if (!currentFont) {
          console.warn("runebender editor requested without a loaded font");
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
      }, {});
      editButton.serialize = false;
      editButton.label = `Edit (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

      if (sourceWidget) {
        sourceWidget.hidden = true;
        sourceWidget.computeSize = () => [0, -4];
        const origCallback = sourceWidget.callback;
        sourceWidget.callback = (...args: any[]) => {
          origCallback?.(...args);
          syncPreview();
        };
      }
      if (sourceKindWidget) {
        sourceKindWidget.hidden = true;
        sourceKindWidget.computeSize = () => [0, -4];
      }
      if (workspaceNameWidget) {
        workspaceNameWidget.hidden = true;
        workspaceNameWidget.computeSize = () => [0, -4];
      }

      const [oldWidth, oldHeight] = this.size;
      this.setSize([Math.max(oldWidth, 420), Math.max(oldHeight, 320)]);

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
