# Runebender Source Workflow Plan

This note captures the intended product shape for loading, editing, and
saving font sources in Runebender-Comfy. It is meant to stay actionable
across agent sessions.

## Product Model

Runebender-Comfy should support both safe Comfy-managed font workspaces
and real source-linked font development.

### 1. Linked Source

Use this when the user wants to work on a real font project on disk, for
example `~/GH/repos/virtua-grotesk/VirtuaGrotesk.designspace`.

- The user chooses a `.designspace`, `.ufo`, `.glyphs`, `.glyphspackage`,
  or containing folder.
- Runebender stores source provenance in the workspace manifest.
- Runebender builds or refreshes a local workspace cache from that source.
- `Save` writes into the workspace cache and mirrors supported edits back
  to the original disk source.
- This is the primary workflow for source-controlled font development.

### 2. Imported Workspace / Comfy Asset

Use this when the user wants a safe managed copy inside ComfyUI.

- The user uploads, drops, or imports a designspace/UFO tree into ComfyUI.
- Runebender copies it into `workspace/fonts/<slot>/` as a managed
  workspace.
- `Save` writes only to the managed copy.
- The workspace manifest records `source_kind`, but does not record
  `origin_mode: linked`.
- The original disk folder is not edited, because browser uploads do not
  expose or preserve the original absolute path.
- This is useful for experiments, sharing workflows, and avoiding accidental
  edits to source repos.

### 3. Save As / Export Back To Disk

Use this when the user wants to turn an imported or edited workspace into
an editable disk project.

- The user chooses a destination folder.
- Runebender writes the current designspace/UFO package there.
- After a successful export, Runebender can optionally convert the workspace
  into a linked source backed by that new destination.
- This is the path that lets an imported Comfy-managed asset become a real
  disk project again.

## Why Both Are Needed

ComfyUI's normal file picker/upload path returns browser `File` objects,
not absolute local filesystem paths. That is correct browser behavior, but
it means an uploaded font source cannot automatically save back to the
original folder it came from.

Linked source mode solves real disk editing by explicitly capturing a local
path through a native/local picker or text path entry. Imported workspace
mode solves safe Comfy-managed editing. Save As bridges the two.

## Intended UX

- The graph node should start simple: `Open Source...`, source chooser, and
  `Edit`.
- `Open Source...`: edit a real source folder on disk.
- `Save`: write to the current backing store.
- `Save As...`: write the current workspace to a chosen disk folder.
- After `Save As...`, offer to treat the destination as the new linked source.
- Imported/managed-copy workflows stay supported in the backend and tests, but
  should not be front-and-center on the node until the user actually needs that
  complexity.

## Actionable Todo List

- [x] Implement linked source manifests and safe save-back mapping.
- [x] Verify designspace edits can save back to the linked disk source.
- [x] Verify glyph width edits can save back to linked UFO GLIF files.
- [x] Replace unsupported `window.prompt()` source path entry with an in-page
      dialog.
- [x] Decide final picker strategy for `Link Source...`:
      Comfy Desktop `showDirectoryPicker()` where available, plus local
      backend macOS picker fallback for browser/app-window mode.
- [x] Polish `Link Source...` UI copy and failure states.
- [x] Add or refine tests for source picker cancellation and non-macOS
      fallback behavior.
- [x] Define the exact managed workspace / asset import contract:
      where copied sources live, how they are named, and what metadata is
      stored.
- [x] Make imported workspaces clearly show that saves affect the managed
      copy, not the original disk folder.
- [x] Implement `Save As...` for writing the current workspace to a chosen
      disk destination.
- [x] After `Save As...`, add a user-visible option to relink the workspace
      to the exported destination.
- [x] Add round-trip tests for imported workspace -> `Save As...` -> linked
      source save-back.
- [x] Manual test in ComfyUI:
      link `~/GH/repos/virtua-grotesk`, edit a glyph, save, and confirm git
      diff appears in the original repo.
- [x] Manual test in ComfyUI:
      import a copy, edit it, save, and confirm the original repo is unchanged.
- [x] Manual test in ComfyUI:
      import a copy, `Save As...` to a temp folder, relink, edit again, and
      confirm changes land in the exported folder.

## Verification Notes

- 2026-05-19: automated verification passed with
  `python3 -m unittest tests.test_workspace tests.test_web_bundle`.
- 2026-05-19: frontend bundle rebuilt with `pnpm build`; live bundle
  fingerprint for this phase is `rb-bundle-2026-05-19-source-workflows-9`.
- 2026-05-19: local `git diff --check` passed.
- 2026-05-19: `Save As...` now aborts if the preliminary save cannot
  flush dirty edits, so exported folders should not silently receive stale
  workspace files after a failed save.
- 2026-05-19: route-level tests now cover the two imported-copy manual
  scenarios: imported edits do not touch the original disk source, and
  imported copy -> `Save As...` with relink writes subsequent edits to the
  exported folder.
- 2026-05-19: restarted live ComfyUI and verified the source-workflow bundle,
  managed-copy label, and `Save workspace as` dialog were live.
- 2026-05-19: after user feedback, simplified the graph-node visible controls
  to `Open Source...`, a `source` chooser, and `Edit`. The managed-copy and
  local import routes remain available internally, but the default node no
  longer asks users to choose among import/link/copy concepts up front.
- 2026-05-19: restored the graph-node specimen preview as a real custom widget
  below the simplified controls instead of drawing it in `onDrawBackground`.
- 2026-05-19: replaced the temporary polygon source-preview fallback with a
  real UFO outline renderer using `ufoLib2` + `skia-python`, matching
  comfyfont's outline-first/nonzero-fill approach closely enough for readable
  node previews before compile.
- 2026-05-19: fixed preview source selection so the node prefers the visible
  `source` chooser unless an actual upstream FONT wire is connected. This
  prevents hidden/default `font` widget state from forcing the preview back to
  `demo`.
- 2026-05-19: tightened source/preview state restore so ComfyUI workflow
  restore cannot leave the visible `source` chooser and hidden `source_path`
  widget split. Preview now also prefers source UFO/designspace outlines over
  compiled artifacts whenever editable sources are present.
- 2026-05-19: added explicit frontend preview diagnostics for request slot,
  visible/stored source values, load success, and image failure. Use these logs
  to verify whether the graph node requests the selected source or falls back
  to `demo`.
- 2026-05-19: preview diagnostics now print JSON strings instead of collapsed
  console objects. Preview image loads are request-id guarded so stale requests
  cannot win, and image load now forces both the node and LiteGraph canvas dirty.
- 2026-05-19: made the visible `source` combo serializable. ComfyUI now saves
  and restores the user-facing source selection directly, then mirrors it into
  hidden `source_path` for Python execution.
- 2026-05-19: live ComfyUI route checks passed for all three source
  workflows. Linked `~/GH/repos/virtua-grotesk`, wrote a one-width GLIF edit
  through `/runebender/workspace/write`, observed a git diff for `k.glif`,
  then restored it through the same route; `virtua-grotesk` was clean after
  the check. Also verified imported-copy edits stay in the managed workspace,
  and imported copy -> `Save As...` with relink writes subsequent edits to the
  exported folder.
- 2026-05-19: live ComfyUI UI checks passed for linked-source designspace and
  glyph save-back. Linked `~/GH/repos/virtua-grotesk`, inserted a temporary
  designspace comment through the editor textarea, saved, observed the git
  diff, restored through the editor, and confirmed the repo was clean. Opened
  glyph `k`, changed its advance width through the active glyph metrics panel,
  saved, observed a `k.glif` git diff, restored the width through the same UI,
  and confirmed the repo was clean.
- 2026-05-19: added `POST /runebender/import_source_path` so Import Copy can
  use the same native/local picker path as Link Source while still creating a
  managed copy. Live route verification returned a managed, unlinked
  `codex-ui-import-copy` slot.
- 2026-05-19: live ComfyUI UI checks passed for imported-copy save isolation.
  Selected the real `codex-ui-import-copy` managed workspace, opened it in the
  editor, confirmed the label reads `Managed copy (workspace cache)`, changed
  glyph `k` advance width to `541`, saved, confirmed the managed workspace
  GLIF changed, and confirmed `~/GH/repos/virtua-grotesk` stayed clean.
- 2026-05-19: live ComfyUI UI checks passed for imported-copy `Save As...` with
  relink. Used the editor's `Save As` dialog to export to
  `/private/tmp/runebender-comfy-ui-save-as-1779227543042` with relink checked,
  confirmed the exported GLIF contained the prior width `541`, confirmed the
  workspace manifest now points at that temp folder, changed glyph `k` width to
  `542`, saved again, and confirmed both the workspace cache and exported GLIF
  contain `542` while `~/GH/repos/virtua-grotesk` stayed clean.
