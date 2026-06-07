import type {
  ClearWorkspaceSlotsResult,
  ChooseSourceResult,
  LinkSourceResult,
  RunebenderHost,
  SaveWorkspaceAsResult,
  TraceBackgroundCandidateResult,
  TraceBackgroundGlyphResult,
  WorkspaceSlotPayload,
} from "../../host/runebenderHost";

export const comfyHost: RunebenderHost = {
  log(level, message) {
    const body = new FormData();
    body.append("level", level);
    body.append("message", message);
    void fetch("/runebender/log", { method: "POST", body, keepalive: true }).catch(() => {});
  },

  async publishState(payload) {
    const body = new FormData();
    body.append("node_id", payload.nodeId);
    body.append("font", payload.font);
    body.append("glyph_data", payload.glyphData);
    await fetch("/runebender/set_state", {
      method: "POST",
      body,
    });
  },

  async loadWorkspaceSlot(slot) {
    const res = await fetch(`/runebender/workspace/${encodeURIComponent(slot)}`);
    if (!res.ok) return null;
    return (await res.json()) as WorkspaceSlotPayload;
  },

  async listWorkspaceSlots() {
    const response = await fetch("/runebender/workspaces");
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
    const data = (await response.json()) as {
      slots?: string[];
      choices?: Array<{ slot?: string; label?: string; origin_source?: string }>;
    };
    if (Array.isArray(data.choices)) {
      return data.choices
        .map((choice) => ({
          slot: String(choice.slot ?? "").trim(),
          label: String(choice.label ?? choice.slot ?? "").trim(),
          origin_source: String(choice.origin_source ?? "").trim(),
        }))
        .filter((choice) => choice.slot);
    }
    return (data.slots ?? []).map((slot) => ({ slot, label: slot }));
  },

  async clearWorkspaceSlots() {
    const response = await fetch("/runebender/workspaces/clear", { method: "POST" });
    const data = (await response.json().catch(() => ({}))) as ClearWorkspaceSlotsResult;
    if (!response.ok) {
      throw new Error(data.error || `${response.status} ${response.statusText}`);
    }
    return data;
  },

  workspacePreviewUrl(slot, params) {
    return `/runebender/workspace/${encodeURIComponent(slot)}/preview?${params.toString()}`;
  },

  async drawBotPresetSource(name) {
    const response = await fetch(`/runebender/drawbot_preset?name=${encodeURIComponent(name)}`);
    return response.ok ? await response.text() : null;
  },

  async writeWorkspaceFile(path, text) {
    const body = new FormData();
    body.append("path", path);
    body.append("text", text);
    return fetch("/runebender/workspace/write", {
      method: "POST",
      body,
    });
  },

  async chooseSource(mode) {
    const body = new FormData();
    if (mode) body.append("mode", mode);
    const response = await fetch("/runebender/choose_source", {
      method: "POST",
      body,
    });
    const data = (await response.json().catch(() => ({}))) as ChooseSourceResult;
    if (!response.ok) {
      throw new Error(data.error || `${response.status} ${response.statusText}`);
    }
    return data;
  },

  async linkSource(args) {
    const body = new FormData();
    body.append("source_path", args.sourcePath);
    body.append("source_kind", args.sourceKind);
    body.append("workspace_name", args.workspaceName);
    const response = await fetch("/runebender/link_source", {
      method: "POST",
      body,
    });
    const data = (await response.json().catch(() => ({}))) as LinkSourceResult;
    return { response, data };
  },

  async saveWorkspaceAs(args) {
    const body = new FormData();
    body.append("slot", args.slot);
    body.append("destination", args.destination);
    body.append("relink", args.relink ? "true" : "false");
    const response = await fetch("/runebender/workspace/save_as", {
      method: "POST",
      body,
    });
    const data = (await response.json().catch(() => ({}))) as SaveWorkspaceAsResult;
    return { response, data };
  },

  async traceBackgroundGlyph(args) {
    const body = new FormData();
    body.append("slot", args.slot);
    body.append("master", args.master);
    body.append("glyph", args.glyph);
    body.append("image", args.image);
    if (args.unicode) body.append("unicode", args.unicode);
    body.append("width", String(args.width));
    body.append("target_height", String(args.targetHeight));
    body.append("x_offset", String(args.xOffset));
    body.append("y_offset", String(args.yOffset));
    if (args.imageWidth !== undefined) body.append("image_width", String(args.imageWidth));
    if (args.imageHeight !== undefined) body.append("image_height", String(args.imageHeight));
    if (args.designX !== undefined) body.append("design_x", String(args.designX));
    if (args.designY !== undefined) body.append("design_y", String(args.designY));
    if (args.designScaleX !== undefined) body.append("design_scale_x", String(args.designScaleX));
    if (args.designScaleY !== undefined) body.append("design_scale_y", String(args.designScaleY));
    body.append("grid", String(args.grid ?? 2));
    body.append("accuracy", String(args.accuracy ?? 4));
    body.append("smooth", String(args.smooth ?? 1));
    body.append("alphamax", String(args.alphamax ?? 0.35));
    if (args.globalFit) body.append("globalFit", "true");
    if (args.invert) body.append("invert", "true");
    if (args.threshold !== undefined && args.threshold !== null) {
      body.append("threshold", String(args.threshold));
    }
    const response = await fetch("/runebender/workspace/trace_background", {
      method: "POST",
      body,
    });
    const data = (await response.json().catch(() => ({}))) as TraceBackgroundGlyphResult;
    return { response, data };
  },

  async traceBackgroundCandidate(args) {
    const body = new FormData();
    body.append("slot", args.slot);
    body.append("master", args.master);
    body.append("glyph", args.glyph);
    body.append("image", args.image);
    if (args.candidateName) body.append("candidate_name", args.candidateName);
    if (args.unicode) body.append("unicode", args.unicode);
    body.append("width", String(args.width));
    body.append("target_height", String(args.targetHeight));
    body.append("x_offset", String(args.xOffset));
    body.append("y_offset", String(args.yOffset));
    if (args.imageWidth !== undefined) body.append("image_width", String(args.imageWidth));
    if (args.imageHeight !== undefined) body.append("image_height", String(args.imageHeight));
    if (args.designX !== undefined) body.append("design_x", String(args.designX));
    if (args.designY !== undefined) body.append("design_y", String(args.designY));
    if (args.designScaleX !== undefined) body.append("design_scale_x", String(args.designScaleX));
    if (args.designScaleY !== undefined) body.append("design_scale_y", String(args.designScaleY));
    if (args.unitsPerEm !== undefined) body.append("units_per_em", String(args.unitsPerEm));
    if (args.ascender !== undefined) body.append("ascender", String(args.ascender));
    if (args.descender !== undefined) body.append("descender", String(args.descender));
    body.append("grid", String(args.grid ?? 2));
    body.append("accuracy", String(args.accuracy ?? 4));
    body.append("smooth", String(args.smooth ?? 1));
    body.append("alphamax", String(args.alphamax ?? 0.35));
    if (args.globalFit) body.append("globalFit", "true");
    if (args.invert) body.append("invert", "true");
    if (args.threshold !== undefined && args.threshold !== null) {
      body.append("threshold", String(args.threshold));
    }
    const response = await fetch("/runebender/workspace/trace_background_candidate", {
      method: "POST",
      body,
    });
    const data = (await response.json().catch(() => ({}))) as TraceBackgroundCandidateResult;
    return { response, data };
  },

  async invalidateWorkspacePath(path) {
    const body = new FormData();
    body.append("path", path);
    await fetch("/runebender/workspace/invalidate", {
      method: "POST",
      body,
    });
  },
};
