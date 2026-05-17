// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";
import { createApp, ref } from "vue";

import Runebender from "./Runebender.vue";

const RUNEBENDER_BUNDLE_FINGERPRINT = "rb-bundle-2026-05-16";
console.info(`[runebender-comfy] loaded ${RUNEBENDER_BUNDLE_FINGERPRINT}`);

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
  const directWidget = node.widgets?.find((w: any) => w.name === "font");
  const directValue = String(directWidget?.value ?? "").trim();
  if (directValue) return directValue;

  const input = node.inputs?.find((slot: any) => slot.name === "font");
  const linkId = input?.link;
  const graph = node.graph;
  if (linkId == null || !graph) return directValue;

  const link = graph.getLink(linkId);
  const origin = link ? graph.getNodeById(link.origin_id) : null;
  if (!origin) return directValue;

  const originWidgets = origin.widgets ?? [];
  for (const candidate of ["workspace", "font", "source_path"]) {
    const widget = originWidgets.find((w: any) => w.name === candidate);
    const value = String(widget?.value ?? "").trim();
    if (value) return value;
  }

  return directValue;
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
    "display:flex",
    "flex-direction:column",
    "gap:8px",
    "padding:8px",
    "box-sizing:border-box",
    "min-width:0",
    "min-height:0",
    "background:#0f0f0f",
    "color:#dcdcdc",
  ].join(";");

  const chrome = document.createElement("div");
  chrome.style.cssText = [
    "display:flex",
    "justify-content:flex-end",
    "height:44px",
    "min-height:44px",
    "pointer-events:none",
  ].join(";");

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.textContent = "Close Editor";
  closeButton.style.cssText = [
    "pointer-events:auto",
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
    "flex:1",
    "min-width:0",
    "min-height:0",
    "width:100%",
    "height:100%",
    "overflow:hidden",
  ].join(";");

  chrome.appendChild(closeButton);
  root.append(chrome, mount);
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
    if (nodeData.name === "Font") {
      const onCreated = nodeType.prototype.onNodeCreated;
      nodeType.prototype.onNodeCreated = function () {
        onCreated?.apply(this, arguments);
        console.log(`[runebender-comfy] ${RUNEBENDER_BUNDLE_FINGERPRINT} Font node active`);

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

        const syncPreview = () => {
          const value = String(sourceWidget?.value ?? "").trim();
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

        const importFolderButton = this.addWidget("button", "Import Folder...", "runebender-import", () => {
          folderInput.click();
        }, {});
        importFolderButton.serialize = false;
        importFolderButton.label = `Import Folder... (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

        const importFileButton = this.addWidget("button", "Import File...", "runebender-import-file", () => {
          fileInput.click();
        }, {});
        importFileButton.serialize = false;
        importFileButton.label = `Import File... (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

        const refreshButton = this.addWidget("button", "Refresh Workspaces", "runebender-refresh", () => {
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
        this.setSize([Math.max(oldWidth, 420), Math.max(oldHeight, 300)]);

        const origWorkspaceCallback = workspaceSelect.callback;
        workspaceSelect.callback = (value: any) => {
          origWorkspaceCallback?.(value);
          this.setDirtyCanvas(true, true);
        };

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
    }

    if (nodeData.name !== "Runebender") return;

    const onCreated = nodeType.prototype.onNodeCreated;
    nodeType.prototype.onNodeCreated = function () {
      onCreated?.apply(this, arguments);
      console.log(`[runebender-comfy] ${RUNEBENDER_BUNDLE_FINGERPRINT} Runebender node active`);
      const fontPath = ref("");
      const fontWidget = this.widgets?.find((w: any) => w.name === "font");
      if (fontWidget) {
        fontWidget.hidden = true;
        fontWidget.computeSize = () => [0, -4];
      }
      const glyphDataWidget = this.addWidget("STRING", "glyph_data", "", () => {}, {
        multiline: true,
      });
      glyphDataWidget.hidden = true;
      glyphDataWidget.computeSize = () => [0, -4];

      const editButton = this.addWidget("button", "Edit", "runebender-edit", () => {
        const currentFont = resolveNodeFontValue(this);
        if (!currentFont) {
          console.warn("runebender editor requested without a loaded font");
          return;
        }
        fontPath.value = currentFont;
        openRunebenderOverlay({
          nodeId: String(this.id),
          fontPathRef: fontPath,
          onGlyphDataChange: (value: string) => {
            this.setDirtyCanvas(true, true);
            glyphDataWidget.value = value;
          },
        });
      }, {});
      editButton.serialize = false;
      editButton.label = `Edit (${RUNEBENDER_BUNDLE_FINGERPRINT})`;

      const [oldWidth, oldHeight] = this.size;
      this.setSize([Math.max(oldWidth, 420), Math.max(oldHeight, 280)]);

      const syncPreview = () => {
        const value = resolveNodeFontValue(this);
        fontPath.value = value;
        if (!value) {
          editButton.disabled = true;
          return;
        }
        editButton.disabled = false;
      };

      const syncFontSelection = () => {
        const value = resolveNodeFontValue(this);
        fontPath.value = value;
        if (fontWidget) {
          fontWidget.value = value;
        }
        syncPreview();
      };

      if (fontWidget) {
        const origCallback = fontWidget.callback;
        fontWidget.callback = (...args: any[]) => {
          origCallback?.(...args);
          syncFontSelection();
        };
      }

      const origConfigure = this.onConfigure;
      this.onConfigure = function (info: any) {
        origConfigure?.call(this, info);
        syncFontSelection();
      };

      const origConnectionsChange = this.onConnectionsChange;
      this.onConnectionsChange = function (...args: any[]) {
        origConnectionsChange?.apply(this, args);
        syncFontSelection();
      };

      requestAnimationFrame(syncFontSelection);
    };
  },
});
