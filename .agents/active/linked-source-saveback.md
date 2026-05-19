---
slug: linked-source-saveback
agent: Codex
branch: main
worktree: /Users/eli/GH/repos/runebender-comfy
started: 2026-05-18
last_touched: 2026-05-18
touches:
  - nodes/workspace.py
  - nodes/runebender.py
  - nodes/font.py
  - web/src/Runebender.vue
  - web/src/extension.ts
  - tests/test_workspace.py
  - tests/test_web_bundle.py
---

## Goal

Make Runebender-Comfy able to load a designspace plus associated UFOs
from disk, edit workspace source files, and save those changes back to
the original disk source when explicitly opened as a linked source.

## Status

- [ ] Add source provenance to workspace manifests.
- [ ] Add safe save-back mapping from workspace-relative files to the
      original source root.
- [ ] Wire editor save status so users can tell whether writes landed
      in the workspace cache or the disk source.
- [ ] Cover the linked designspace/UFO save-back path with tests.

## Notes

Existing saves write into `workspace/fonts/<slot>/` via
`/runebender/workspace/write`. The missing product boundary is
remembering the original designspace/UFO root and mirroring changed
files back there safely.
