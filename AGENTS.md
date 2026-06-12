# AGENTS.md

Context for AI coding agents working on `runebender-comfy`. Evergreen
info only — architecture, build, conventions, load-bearing gotchas.
Task-specific plans (current phase work, in-flight refactors) live
under `.agents/`. New agents: read this top-to-bottom before touching
code, then check `.agents/` for active plans.

## What this is

A ComfyUI custom-node package that ships two nodes:

- **Runebender** — full-screen glyph editor widget, Vue host + Rust
  core compiled to WASM, rendered with Vello + Kurbo. Ported from
  [`runebender-xilem`](https://github.com/eliheuer/runebender-xilem).
- **DesignBot** — Rust DrawBot/Processing-style 2D graphics node.
  Currently a stub. Backed by
  [`designbot`](https://github.com/eliheuer/designbot).

Both nodes are GPL-3.0 because ComfyUI is GPL-3.0. Inbound sources
(`runebender-xilem` Apache-2.0, `runebender-core` Apache-2.0, Vello /
Kurbo MIT-or-Apache) are all GPL-3.0-compatible.

## Repo layout

```
.agents/              task-specific plans (UI_PARITY_PLAN.md etc.)
  REPO_MAP.md         task routing: which files to read for each area
  active/             in-flight task claims — one md per running task
  active/_template.md template for new claims
docs/architecture/    durable architecture decisions and boundaries
nodes/                ComfyUI Python nodes (runebender.py, designbot.py)
scripts/audit.sh      runs cargo-deny + check-crate-age (against the
                      runebender-web dependency's Rust crate)
scripts/agent-doctor.sh preflight checks for new agent sessions
tools/check-crate-age/  Rust binary querying crates.io for npm-style age cooldown
web/                  ComfyUI extension build (thin since 2026-06-12)
  src/extension.ts    ComfyUI widget registration (the only comfy entry)
  src/hosts/comfy/    RunebenderHost implementation over the workspace API
  package.json        depends on runebender-web (file:../../runebender-web) —
                      the editor itself (Runebender.vue, components, Rust
                      wasm core) lives in that repo now
  pnpm-workspace.yaml pnpm 10 config — see "pnpm 10 gotchas" below
SECURITY.md           supply-chain policy
```

## Architecture

```
┌──────────────────────────────────────────┐
│ ComfyUI graph (Python)                   │
│   nodes/runebender.py   nodes/designbot.py
└─────────────┬────────────────────────────┘
              │ custom widget registration (WEB_DIRECTORY)
┌─────────────▼────────────────────────────┐
│ ComfyUI frontend (Vue 3)                 │
│   web/src/extension.ts + hosts/comfy/    │
└─────────────┬────────────────────────────┘
              │ imports (file: dep)
┌─────────────▼────────────────────────────┐
│ runebender-web (separate repo)           │
│   Runebender.vue + core/ wasm            │
│   (Vello + Kurbo, NO Xilem)              │
└──────────────────────────────────────────┘
```

The Rust core deliberately drops Xilem's view tree / Masonry widgets —
only the rendering layer (Kurbo + Peniko + Vello) survives. Vue mouse
and keyboard handlers call into plain WASM methods; there's no
`app_logic()` rebuild-the-tree loop. Practical consequence: you cannot
lift xilem modules whole — you pull editing/model code out from
underneath its event loop.

WebGPU-only (Chrome 113+, Edge, Safari TP). No CPU fallback shipped.
Vello supports `use_cpu: true` if reach is ever needed.

Durable architecture decisions live in
`docs/architecture/decisions.md`. Use `.agents/REPO_MAP.md` to find
the files and sibling-repo references for a specific task area.

## Sister repos and cross-repo dependencies

All assumed to be siblings under `~/GH/repos/`:

| Repo | License | Role |
|---|---|---|
| `runebender-xilem` | Apache-2.0 | Native editor, canonical UI/UX reference. Active development. |
| `runebender-core` | Apache-2.0 | Shared editing/model crate. Local `path = "../runebender-core"` dep. |
| `runebender-comfy` | GPL-3.0 | **This repo.** |
| `comfy-img2bez-node` | GPL-3.0 | Sibling ComfyUI node, used as scaffolding template. |
| `designbot` | — | Future DesignBot node source. `nodes/designbot.py` is a stub. |
| `ComfyUI_Lite_frontend` | — | Frontend fork; theme adapter target. |
| `virtua-grotesk` | — | Test font (2 masters: Regular + Bold). |

Fresh-clone needs both `runebender-comfy` and `runebender-core` checked
out as siblings (or switch to a git dep before public publish).

All three Runebender repos (`-comfy`, `-xilem`, `-core`) carry an
`AGENTS.md` with the same multi-agent coordination protocol — see
"Multi-agent coordination" below. Each repo's `.agents/active/` is
its own coordination space; cross-repo work files the claim in the
primary repo and lists the cross-repo paths in `touches:`.

### `runebender-core` — what's actually shared

Only the 5 kurbo-free modules: `selection.rs`, `undo.rs`,
`edit_types.rs`, `entity_id.rs`, `kerning.rs`, plus
`unicode-general-category`-based glyph categorization. ~600 lines,
~22 tests. `path/`, `viewport`, `hit_test`, `mouse`, `workspace`,
`glyph_renderer` are NOT shared because they touch kurbo types — see
next section.

## ⚠ Load-bearing gotcha: the kurbo version split

- `runebender-xilem` is pinned to **kurbo 0.12** (via masonry 0.4).
- `runebender-comfy` is on **kurbo 0.13** (forced by peniko 0.5 /
  vello 0.8).
- The `spline` crate uses kurbo 0.9 internally; conversion happens at
  the boundary.

Sharing any kurbo-using crate between xilem and comfy currently
produces ~289 errors of `masonry::kurbo::X is not kurbo::X`. Switching
xilem to masonry-2 is a multi-week project.

**Do not naively bump xilem's kurbo. Do not naively downgrade comfy's
kurbo.** The path forward (when picked up) is boundary conversion at
the vello edge — the same trick `spline` already uses.

## Build and dev commands

The editor itself (UI, Rust core, wasm) is developed in the sibling
**runebender-web** repo — `cd ../runebender-web && pnpm dev` is the
main loop, and its AGENTS.md covers it. This repo only wires the
editor into ComfyUI.

```sh
# Build the ComfyUI extension bundle (dist/runebender-comfy.js):
cd web
pnpm install        # links runebender-web from ../../runebender-web
pnpm build

# After changing the editor in ../runebender-web, just rebuild here —
# the file: dependency picks the sibling checkout up live. If the
# editor wasm changed, run `pnpm wasm` over there first (its wasm/
# output is committed in that repo).

# ⚠ The bundle-size test (tests/test_web_bundle.py) guards against a
# debug wasm landing in dist/ — keep runebender-web wasm/ a release
# build (`pnpm wasm`, never `pnpm wasm:debug`, before committing).

# Supply-chain audit (audits the runebender-web crate via node_modules):
./scripts/audit.sh
```

### Stale-state cheat sheet (these all bit during development)

1. After any Rust change → run `pnpm wasm`, **restart `pnpm dev`**
   (Ctrl+C and relaunch), then **Cmd+Shift+R** in the browser. The
   dev server caches the wasm shim in memory.
2. If hard refresh isn't enough → `rm -rf web/node_modules/.vite`
   (Vite's pre-bundle cache).
3. `web/public/wasm/` must **NOT** exist. Vite's `public/` directory
   shadows the canonical `web/wasm/`. The wasm output dir was moved
   out of `public/` deliberately — if it reappears, delete it.
4. When normal hard-refresh fails: DevTools → right-click reload →
   "Empty Cache and Hard Reload".

## Conventions and policies

### Supply chain (load-bearing — security policy)

- **7-day npm cooldown** enforced via `minimum-release-age=10080` in
  `~/.npmrc` and `web/.npmrc`. Always assume active. `--force` and
  `yes |` do NOT bypass it — it's resolution-level.
- **Rust side** has no built-in age filter. Use the binary at
  `tools/check-crate-age/` (queries crates.io), plus `cargo-deny`
  via the runebender-web crate `core/deny.toml`. `scripts/audit.sh` runs both.
- Note: `cargo-deny` does **not** have age filtering despite older
  docs suggesting otherwise. That's why `check-crate-age` exists.
- If adding any dep, run the relevant age check before merging.

### Git workflow

- **Commit locally as you work, push only when a phase is coherent.**
  Don't push every commit. Squash iteration commits before pushing.
- Don't squash commits that have already been pushed.

### Multi-agent coordination

Multiple agents (Claude Code, Codex, Hermes, future others) may be
working in this repo concurrently — possibly across machines. The
protocol uses git as the lowest-common-denominator coordination
channel and lives in `.agents/active/`. Agent-name-agnostic: any
agent that can read AGENTS.md can participate.

**Before starting any non-trivial task:**

1. **Run `scripts/agent-doctor.sh`.** It checks the expected sibling
   repos, active claims, dirty state, and stale wasm output traps.
2. **Pull `main` and skim `.agents/active/*.md`.** Each file is a
   claim by an agent currently working on something. If your task
   overlaps an existing claim's `touches:` list, pick a different
   slice or check with the human.
3. **Write your own claim file** to `.agents/active/<slug>.md` using
   `.agents/active/_template.md`. `<slug>` is short kebab-case
   (`wire-coordinate-panel`, `theme-json-skeleton`). One file per
   concurrent task.
4. **Commit and push the claim immediately to `main`.** This is an explicit
   exception to the "push at milestones" rule above — the claim is
   coordination state, not feature work, and is useless if other
   agents can't see it. Use a one-line commit like `claim: <slug>`.
   Do this in the main checkout before creating the feature worktree;
   feature branches are not a reliable coordination channel.
5. **Work in a git worktree, not the main checkout:**
   ```sh
   git fetch origin
   git worktree add ~/Temp/worktrees/runebender-comfy-<slug> \
     -b agent/<slug> origin/main
   ```
   Worktrees isolate `web/wasm/` rebuilds, lockfile churn, dev-server
   ports, and `node_modules` state. `~/Temp/` is user-policy for
   scratch dirs.
6. **Bump `last_touched:`** in the claim file when you resume after
   an idle stretch (hour+). Cheap signal that the claim is alive.
7. **Delete the claim file** when you finish, hand off, or abandon.
   Commit + push the deletion. A claim with `last_touched:` older
   than ~24h is considered stale — **don't silently reclaim**, ping
   the human first.

When the feature work merges, the worktree can be removed:
`git worktree remove ~/Temp/worktrees/runebender-comfy-<slug>`.

**Cross-repo work:** if your task spans this repo plus
`runebender-core` or `runebender-xilem`, file the claim in your
primary repo and list cross-repo paths in `touches:` (e.g.,
`../runebender-core/src/selection.rs`). Skim the other repos'
`.agents/active/` too.

Long-lived multi-session plans (Phase docs, design notes) still live
at `.agents/<NAME>.md`, not under `active/`. `active/` is only for
in-flight claims.

### UI parity with xilem

The product north star is "xilem and comfy feel like the same tool."
Concretely:

- **Each xilem `src/components/*.rs` gets a 1:1 Vue sibling** with
  the same filename stem in runebender-web `src/components/`. Makes
  "look at what xilem changed and mirror it" trivially obvious.
- **Mirror xilem's palette byte-for-byte** so files round-trip.
  Background `#101010`, panels `#1C1C1C`, green accent `#66EE88`,
  `BENTO_GAP = 6px`, stroke widths `1.5×`. Reference values are in
  runebender-web `core/src/renderer.rs` and `src/Runebender.vue` `<style>`.
- **Glyph mark colors are semantic, not cosmetic** — the user's local
  AI workflows depend on the specific color slots. Don't tweak
  `MARK_RED / ORANGE / YELLOW / GREEN / BLUE / PURPLE / PINK`.
- **Grid interaction:** single-click selects, double-click opens the
  editor (supports batch-tagging workflow). Don't revert to
  single-click-opens.
- **No floating chrome.** Help overlays, banners, "Open UFO"
  buttons, full-screen buttons, status pills, and drop hints have
  all been stripped. Drag-drop + Esc + keyboard shortcuts.

### Working style

- Default to terse. State results, not narration.
- Confirm before destructive ops. Don't silently `rm` shared dirs.
- If a command needs elevated permissions, **print the command and
  let the user run it** rather than retrying with broader perms.
- Pre-flight questions (multiple choice) for non-trivial decisions
  are appreciated; don't just guess.

## Other gotchas worth knowing

### UFO / glyph loading

- **`norad` works in WASM** with the `uuid` `js` feature enabled.
  Use `norad::Glyph::parse_raw(xml: &[u8])` for in-memory `.glif`
  parsing. `Font::load(path)` does **not** work in the browser.
- **`.ufo` is greyed out in macOS file pickers** if FontLab or Glyphs
  is installed (the system treats it as a bundle). The web platform
  can't override this — `<input webkitdirectory>` and
  `showDirectoryPicker()` both hit the same wall. **Drag-and-drop
  is the primary UX path.** Don't try to "fix" the picker.
- **Drop listeners must be on `window`**, not the canvas. In grid mode
  the canvas has `visibility: hidden` + `pointer-events: none`, so
  canvas-level handlers don't fire and the browser falls back to its
  default file:// open-in-tab behavior.
- `AtomicU64` compiles fine on `wasm32-unknown-unknown` (single-
  threaded; `std::sync::atomic` emulates). Ignore older "shopping
  list" notes that flagged it as a problem.
- The `spline` crate (Linebender git dep, hyperbezier) is WASM-clean.

### pnpm 10 (consumed huge chunks of time)

- pnpm 10 refuses install scripts by default. `esbuild` needs its
  install script to fetch its native binary.
- Config key is **`allowBuilds` in `pnpm-workspace.yaml`**, not
  `onlyBuiltDependencies` in `package.json` (older home; pnpm 10 may
  not respect it).
- `pnpm approve-builds` writes a placeholder string
  `"set this to true or false"` in the YAML — **this is not garbage**;
  pnpm regenerates it on every install until you replace it with a
  real bool. If you see that string, fix it to `true`/`false`, don't
  delete the file.
- `verify-deps-before-run=false` and `runDepsStatusCheck` are
  **separate**. Both can re-wipe `node_modules` and undo manual
  `pnpm rebuild esbuild` fixes.
- Force-fix one package: `pnpm rebuild esbuild`.

### Rendering correctness

- **Fill all contours as one `BezPath`**, not per-contour. The
  `NonZero` winding rule then treats counter contours (inside of D,
  O, etc.) as holes. Per-contour filling fills the counter solid.
- **Handle lines** iterate **on-curves** and connect each to its
  immediate prev/next neighbor with wrap. Iterating off-curves and
  searching for the "nearest on-curve" leaps across the glyph and
  draws diagonals. Mirror xilem's traversal exactly.
- Point colors dispatch on `PointType`: smooth on-curve = blue circle,
  corner on-curve = green square, off-curve = purple circle, selected
  = yellow inner + orange outline. Strokes `1.5×`.

## Outstanding work / known TODOs

Snapshot as of 2026-05-14. Always re-confirm against `git log` and
`.agents/UI_PARITY_PLAN.md` before quoting.

- **Save is not wired.** All edits in-memory only. `TODO` comment
  marks the spot. Deferred.
- **ComfyUI integration (Wave 4) unstarted.** Python node + widget
  registration. Bundle is currently ~7.4MB (wasm inlined as base64).
  Optimize later.
- **Theme JSON system unbuilt.** Plan: live JSON in `runebender-core`
  consumed by both xilem (→ Rust consts) and comfy (→ CSS vars +
  WASM values). Phase 3 of that plan is the ComfyUI palette
  adapter (`comfy_base.bg-color → BG_CANVAS`).
- **Info sidebar Width + Contours** show `—` for selected-but-not-
  opened glyphs. Needs a wasm helper to parse-without-loading.
- **`groups.plist` parsing not done.** Kerning Groups sidebar shows
  `(empty)`.
- **Glyph anatomy panel is a stub** — should later show xilem's
  outline-+-points-+-handles "x-ray" view.
- **Edit-mode toolbar (Phase 8 partial):** 8 tool buttons exist;
  only Select is wired up. Other tools are visual stubs.
- **Coordinate / Transform panels** stubbed in last work. Coordinate
  wired to a real getter, Transform is pure stub.
- **9 fresh transitive deps** flagged by the age checker
  (wasm-bindgen 0.2.121 ecosystem, hashbrown, quick-xml). Decision
  pending: wait / pin `=` versions / audit-and-exclude.

See `.agents/UI_PARITY_PLAN.md` for the phased UI parity roadmap
(Phases 1–6 mostly done, 7 in progress, 8 partial).
