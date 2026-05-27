import type {
  ChooseSourceResult,
  LinkSourceResult,
  RunebenderHost,
  SaveWorkspaceAsResult,
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

  workspacePreviewUrl(slot, params) {
    return `/runebender/workspace/${encodeURIComponent(slot)}/preview?${params.toString()}`;
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

  async invalidateWorkspacePath(path) {
    const body = new FormData();
    body.append("path", path);
    await fetch("/runebender/workspace/invalidate", {
      method: "POST",
      body,
    });
  },
};
