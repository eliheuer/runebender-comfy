# Editor Extraction Plan

Status: staged plan, not a package split.
Updated: 2026-05-26.

## Decision

Do not split the Vue/WASM editor into a separate repo or package for the
`runebender-dot-org` launch. The website already consumes the editor as a
static artifact built from `../runebender-comfy/web` into
`runebender-dot-org/public/cloud/editor/`.

The next architecture step is an in-place host-boundary refactor inside
`runebender-comfy`, after the website launch path is stable.

## Current website contract

- `runebender-dot-org` links `Open App` to `/cloud/editor/index.html`.
- `runebender-dot-org/scripts/build-cloud-editor.sh` builds this repo's
  `web/` app into the website's checked-in `public/cloud/editor/`
  artifact.
- Astro treats that directory as opaque static output.
- This launch path should stay intact until the website needs real
  website-specific editor behavior, such as OPFS/File System Access
  save/load or launch-state customization.

## First refactor milestone

Remove host-specific I/O from `web/src/Runebender.vue` without changing
behavior, visuals, or the Vite/WASM build loop.

Target folders:

```text
web/src/editor/        reusable editor state, canvas, grid, tools, panels
web/src/host/          small TypeScript interfaces
web/src/hosts/comfy/   ComfyUI implementation of those interfaces
web/src/extension.ts   ComfyUI registration and mounting only
```

Start smaller than this folder split if needed. The first useful seam is
host I/O, not file movement.

## Current host-specific call sites

As of this plan, direct Comfy route usage in `Runebender.vue` is:

- `/runebender/set_state`
- `/runebender/workspace/:slot`
- `/runebender/workspace/write`
- `/runebender/choose_source`
- `/runebender/workspace/save_as`
- `/runebender/workspace/invalidate`

`extension.ts` should remain Comfy-specific. It currently owns:

- console mirroring to `/runebender/log`
- Comfy source picker and widget setup
- `/runebender/workspaces`
- `/runebender/link_source`
- node preview URL generation
- Comfy widget registration/mounting

## Interface direction

Prefer small services over one speculative adapter.

```ts
interface RunebenderStorage {
  open(): Promise<LoadedWorkspace | null>;
  save(workspace: SerializableWorkspace): Promise<SaveResult>;
  saveAs?(workspace: SerializableWorkspace): Promise<SaveResult>;
}

interface RunebenderHost {
  storage: RunebenderStorage;
  log?(level: "info" | "warn" | "error", message: string): void;
  getThemeRoot?(): HTMLElement | null;
  capabilities?: {
    canPickFolder?: boolean;
    canLinkSource?: boolean;
    canSaveAs?: boolean;
  };
}
```

The first implementation can use narrower names and types that match the
current code. Do not design for Electron until a real Electron host
exists.

## Rules

- No `if (host === "comfy")` in reusable editor code.
- Capability checks are allowed.
- Keep Vue editor + WASM engine bundled together from the consumer's
  point of view until there is a real non-Vue or separately packaged
  consumer.
- Keep `assets/runebender-icons.ufo` with the editor.
- Do not introduce npm publishing, a monorepo, or a separate git repo as
  part of the website launch.

## Acceptance bar

The in-place refactor is successful when:

- `Runebender.vue` no longer directly owns Comfy HTTP route details.
- ComfyUI behavior remains visually and functionally unchanged.
- The standalone Vite app still works.
- `runebender-dot-org` can still rebuild `/cloud/editor/index.html` from
  this repo.
- A future website storage adapter would not require rewriting editor UI.
