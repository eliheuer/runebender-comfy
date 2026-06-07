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

export type ClearWorkspaceSlotsResult = {
  deleted?: string[];
  choices?: WorkspaceChoice[];
  error?: string;
};

export type RunebenderStatePayload = {
  nodeId: string;
  font: string;
  glyphData: string;
};

export type TraceBackgroundGlyphArgs = {
  slot: string;
  master: string;
  glyph: string;
  image: File;
  unicode?: string;
  width: number;
  targetHeight: number;
  xOffset: number;
  yOffset: number;
  imageWidth?: number;
  imageHeight?: number;
  designX?: number;
  designY?: number;
  designScaleX?: number;
  designScaleY?: number;
  grid?: number;
  accuracy?: number;
  smooth?: number;
  alphamax?: number;
  invert?: boolean;
  threshold?: number | null;
};

export type TraceBackgroundCandidateArgs = TraceBackgroundGlyphArgs & {
  candidateName?: string;
  unitsPerEm?: number;
  ascender?: number;
  descender?: number;
};

export type TraceBackgroundGlyphResult = {
  success?: boolean;
  glyph?: string;
  glif?: string;
  source_ufo?: string;
  command?: string[];
  error?: string;
};

export type TraceBackgroundCandidateResult = {
  success?: boolean;
  candidate_slot?: string;
  trace_request?: string;
  request_id?: string;
  glyph?: string;
  master?: string;
  report?: Record<string, unknown>;
  error?: string;
};

export type RunebenderHost = {
  log?(level: string, message: string): void;
  publishState(payload: RunebenderStatePayload): Promise<void>;
  loadWorkspaceSlot(slot: string): Promise<WorkspaceSlotPayload | null>;
  listWorkspaceSlots(): Promise<WorkspaceChoice[]>;
  clearWorkspaceSlots(): Promise<ClearWorkspaceSlotsResult>;
  workspacePreviewUrl(slot: string, params: URLSearchParams): string;
  drawBotPresetSource(name: string): Promise<string | null>;
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
  traceBackgroundGlyph(args: TraceBackgroundGlyphArgs): Promise<{
    response: Response;
    data: TraceBackgroundGlyphResult;
  }>;
  traceBackgroundCandidate(args: TraceBackgroundCandidateArgs): Promise<{
    response: Response;
    data: TraceBackgroundCandidateResult;
  }>;
  invalidateWorkspacePath(path: string): Promise<void>;
};

export const runebenderHostKey: InjectionKey<RunebenderHost> = Symbol("runebender-host");
