import type { RunebenderHost } from "../../host/runebenderHost";

const unavailable = () =>
  new Response(JSON.stringify({ error: "No workspace host is available." }), {
    status: 501,
    statusText: "Not Implemented",
    headers: { "content-type": "application/json" },
  });

export const browserHost: RunebenderHost = {
  log() {
    // Standalone browser builds keep logs in the browser console.
  },

  async publishState() {
    // Standalone browser builds do not mirror editor state into a ComfyUI node.
  },

  async loadWorkspaceSlot() {
    return null;
  },

  async listWorkspaceSlots() {
    return [];
  },

  async clearWorkspaceSlots() {
    return { deleted: [] };
  },

  workspacePreviewUrl() {
    return "";
  },

  async drawBotPresetSource() {
    return null;
  },

  async writeWorkspaceFile() {
    return unavailable();
  },

  async chooseSource() {
    return { cancelled: true };
  },

  async linkSource() {
    return {
      response: unavailable(),
      data: { error: "No workspace host is available." },
    };
  },

  async saveWorkspaceAs() {
    return {
      response: unavailable(),
      data: { error: "No workspace host is available." },
    };
  },

  async traceBackgroundGlyph() {
    return {
      response: unavailable(),
      data: { error: "Background tracing requires the ComfyUI workspace host." },
    };
  },

  async traceBackgroundCandidate() {
    return {
      response: unavailable(),
      data: { error: "Background candidate tracing requires the ComfyUI workspace host." },
    };
  },

  async invalidateWorkspacePath() {
    // No compiled workspace cache exists in standalone browser builds.
  },
};
