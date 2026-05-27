import type { InjectionKey } from "vue";

export type WorkspaceFileEntry = {
  path: string;
  text: string;
};

export type WorkspaceSlotPayload = {
  slot: string;
  files: WorkspaceFileEntry[];
  linked_source?: boolean;
  origin_root?: string;
  origin_source?: string;
  refreshed_from_source?: boolean;
};

export type ChooseSourceResult = {
  path?: string;
  error?: string;
  cancelled?: boolean;
};

export type SaveWorkspaceAsResult = {
  destination?: string;
  linked_source?: boolean;
  origin_root?: string;
  origin_source?: string;
  error?: string;
};

export type WorkspaceChoice = {
  slot: string;
  label: string;
  origin_source?: string;
};

export type LinkSourceResult = {
  slot?: string;
  label?: string;
  origin_source?: string;
  error?: string;
};

export type RunebenderStatePayload = {
  nodeId: string;
  font: string;
  glyphData: string;
};

export type RunebenderHost = {
  log?(level: string, message: string): void;
  publishState(payload: RunebenderStatePayload): Promise<void>;
  loadWorkspaceSlot(slot: string): Promise<WorkspaceSlotPayload | null>;
  listWorkspaceSlots(): Promise<WorkspaceChoice[]>;
  workspacePreviewUrl(slot: string, params: URLSearchParams): string;
  writeWorkspaceFile(path: string, text: string): Promise<Response>;
  chooseSource(mode?: "source" | "folder"): Promise<ChooseSourceResult>;
  linkSource(args: {
    sourcePath: string;
    sourceKind: string;
    workspaceName: string;
  }): Promise<{ response: Response; data: LinkSourceResult }>;
  saveWorkspaceAs(args: {
    slot: string;
    destination: string;
    relink: boolean;
  }): Promise<{ response: Response; data: SaveWorkspaceAsResult }>;
  invalidateWorkspacePath(path: string): Promise<void>;
};

export const runebenderHostKey: InjectionKey<RunebenderHost> = Symbol("runebender-host");
