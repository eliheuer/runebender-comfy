# Code-Quality & Anti-Slop Cleanup — pre-Linebender-post review

Date: 2026-05-28. Audience: the Linebender community (kurbo/vello authors),
who are skeptical of AI-authored code and will actively hunt for "AI slop."
This document is the synthesis of a four-part audit (Python, Web TS/Vue,
Rust, tests+docs+hygiene) plus a prioritized checklist at the end.

## Overall verdict

The code is **better than typical AI output and reads as human-maintained in
most places**: idiomatic Rust (`cargo fmt` clean, near-clippy-clean), fully
typed Vue with model teardown, type-hinted Python with actionable errors, no
emoji or marketing prose, a real supply-chain policy. A reviewer who reads the
*code* will mostly find nits.

The problem is what a reviewer sees **before** the code: the repo ships its
AI-process scaffolding in plain sight, and two test/versioning artifacts are
textbook "the agent grepped its own diff." Those are what get a project
dismissed. The cleanup is high-leverage and mostly localized.

## The five things a skeptic notices first (fix these or the rest doesn't matter)

1. **`.agents/` is committed — ~140 KB of AI planning docs.** `UI_PARITY_PLAN.md`
   is 109 KB / 2,238 lines; files carry `agent: Codex (eli@desktop)` frontmatter
   and "claim file" coordination prose. This is the single most damaging
   artifact — it reads as "AI agents run this repo" before a line of Rust is
   read. **[needs-decision]**
2. **`tests/test_web_bundle.py` — 65 tests, 372 `assertIn` calls** that read
   `.vue/.ts/.rs` source as text and assert exact code substrings exist
   (`assertIn("if (raf !== null) return;", body)`). It tests *that specific
   lines were written*, not behavior. The canonical AI-test smell: "the agent
   couldn't run the frontend, so it grepped its own output." **[needs-decision]**
3. **The `rb-bundle-2026-05-28-...-NN` fingerprint** — a hand-bumped date+counter
   string, asserted in both `extension.ts:14` and the test (so they tautologically
   agree), surfaced in a user-facing status banner. Ad-hoc debugging scaffolding.
   **[needs-decision]**
4. **`extension.ts` console scaffolding** — a global `console.*` monkey-patch
   ("log mirror") + ~10 unconditional `console.info` diagnostics + a
   `logNodeSockets` socket-dumper + visible status/error panes. Diagnostic
   cruft shipped to users. **[safe to remove]**
5. **Hardcoded personal paths** (`/Users/eli/...`, `~/Work/comfy/...`) in README,
   docs, generated TS, script docstrings — and one is *enforced by a test*
   assertion. Screams "dumped from one machine." **[safe to genericize]**

Also a top-visibility Rust tell: **`fn _suppress(_: Glyph) {}` with
`#[allow(dead_code)]`** (`editor.rs:2571`) — a no-op to silence an unused-import
lint. A human fixes the import; this is a classic LLM workaround. **[safe to fix]**

---

## Findings by area

### Python (`nodes/`, `scripts/`) — solid; cleanup is ~4 clusters
- **`nodes/font.py`**: three near-duplicate aiohttp routes (`link_source`,
  `import_source_path`, tail of `import_font`) ~90% identical; the
  `try: compile_slot … except: print("eager compile failed")` block appears 4×
  verbatim. Extract a shared `_create_and_compile_slot` + response builder. (MED)
- **`nodes/font.py`**: `preview_workspace_slot` is instrumented with `_preview_log`
  at nearly every branch — leftover debug trace. Collapse to entry/exit + errors. (MED)
- **Silent `except Exception: pass`** with no logging: `font_preview.py:98,102,504,
  516,543`, `font_specimen.py:101`, `workspace.py:750,903`. Narrow the exception
  types and log the ones that hide real failures (drawbot→skia fallback, glyphs
  build). (MED)
- **Dead code**: `workspace.py:909 _copy_tree_contents` (no callers);
  `compile_font.py:40` assigns `compiled` then ignores it. (MED)
- **`font_specimen.py:154 tempfile.mktemp`** — deprecated/insecure + brittle
  `_1.png` guessing. Use `NamedTemporaryFile`/`TemporaryDirectory` (designbot.py
  already does). (MED)
- **Hardcoded venv path** in `scripts/{bootstrap_icons_ufo,build_toolbar_icons,
  seed_extra_toolbar_icons}.py` docstrings. Genericize. (MED)
- **Over-narrated comments/docstrings**: `font_preview.py:21-25`,
  `runebender.py:74-82`, `font.py:36-39`. Trim to one line. (LOW)
- **`font.py:394-395`** recomputes a membership test it just stored. (LOW)
- Good: `from __future__ import annotations` throughout, lazy heavy imports with
  actionable `ImportError`s, careful path-escape validation in `workspace.py`,
  `sync-comfy-theme.py` as the model for *intentional* documented error handling.

### Web TS/Vue (`web/src/`) — slop concentrated in `extension.ts`
- **`extension.ts:21-56`**: `installRunebenderLogMirror` global console
  monkey-patch — invasive, drop it. (HIGH)
- **`extension.ts`**: 26 `console.*`, ~10 unconditional `console.info` dumps
  ("chrome insets", "preview request", "source restore", node-socket JSON).
  Keep only catch-path `warn`/`error`. (HIGH)
- **`extension.ts:241-258`** `logNodeSockets` — pure diagnostic, delete. (MED)
- **`extension.ts:14`** fingerprint constant + banner usage — remove/replace
  with real version or build hash; drop from user banner. (MED)
- **`extension.ts`**: ~30 `any`, `declare const window: any` (`:81`) de-types the
  whole module → narrow to a `Window` augmentation + minimal litegraph interfaces. (MED)
- **Giant inline `cssText` arrays** (overlay/banner/error-pane/preview-img +
  the imperative `requestSourcePath` modal with ~40 `el.style.x=` and magic
  z-index `2147483647`). Move to the stylesheet with classnames. (MED)
- **`onNodeCreated` god function** (~310 lines, ~15 nested closures). Extract the
  source-value state machine + preview-img setup. (MED, follow-up)
- **Changelog-narration comments** ("Previously this was…", "The earlier framing…").
  Trim; keep the ones documenting real ComfyUI quirks. (LOW-MED)
- **No `tsconfig.json` and no ESLint/Prettier** in `web/` — for a public TS
  project this is itself a tell, and it's why the `any`s aren't being caught.
  Add `tsconfig` (`strict:true`) + lint config. (MED) **[needs-decision on scope]**
- Good: `Runebender.vue` is strong — zero `any`, exemplary `onBeforeUnmount`
  teardown, real `parseCssColor`, coalesced rAFs. The host-adapter abstraction
  is clean. `mountScriptEditor`'s rAF loop is correct and its comments earn
  their place. (Vue file size is a noted follow-up, not a blocker.)

### Rust (`rust-core/`) — genuinely good; nits + dead-code triage
- **`editor.rs:2571 fn _suppress` + `#[allow(dead_code)]`** — delete, fix the
  underlying import. (HIGH visibility)
- **11 `#[allow(dead_code)]`** across `path/`, `editing/mouse.rs`,
  `model/workspace.rs`, `path/quadrant.rs` — several whole structs/impls are dead
  (ported from xilem). Triage: remove stale allows, delete truly-dead code, wire
  up anything intended. (MED)
- **`renderer.rs:9` / `wasm_api.rs:5`** redundant inner `#![cfg(target_arch=
  "wasm32")]` (already gated in lib.rs). Clippy `duplicated attribute`. (LOW)
- **`workspace.rs:113,121`** `partial_cmp(b).unwrap()` panics on NaN from
  malformed UFO — use `total_cmp` or delete the (dead) block. (MED)
- **WASM-target clippy** (invisible to default clippy): useless `.into()`
  (`renderer.rs:1337`), collapsible `if` (`1020,1026`), too-many-args
  (`689,1082`), unnecessary `if let` (`wasm_api.rs:361`), needless borrow
  (`text.rs:422`). (MED)
- **`HyperPath` has `len()` but no `is_empty()`** (`hyper.rs:72`). (MED)
- **`glif_metadata_from_norad` vs `glyph_metadata_from_norad`** (`wasm_api.rs:137,160`)
  one-char-apart names — rename. (MED)
- **`#[inline(always)]` on a trivial ctor** (`hyper.rs:183`) → `#[inline]`. (LOW)
- **`.to_vec()`-to-iterate** churn (`editor.rs:1034,1079,1113,1318,2218`,
  `renderer.rs:774`) and **manual `for el {push(*el)}`** vs `.extend()`
  (`renderer.rs:436,1412`, `editor.rs:2543`) — inconsistent with idiomatic uses
  elsewhere in the same files. (LOW)
- **`//` banner module docs where `//!` is expected** (`lib.rs`, `editor.rs`,
  `tool.rs`, `renderer.rs`, `wasm_api.rs`, `model/mod.rs`, `editing/mod.rs`) —
  inconsistent with the files that correctly use `//!`. (MED)
- **`thiserror` likely unused** (no `#[derive(Error)]`); if so drop it. `1.0`
  pin is also dated vs `2.0`. (LOW)
- Good: textbook FFI error handling (`Result<_, JsValue>`, `?`, contextual
  `map_err` — no `unwrap` on parse paths), structured `CompatError`, why-not-what
  comments, idiomatic let-else throughout, `cargo fmt` clean.

### Tests / Docs / Hygiene
- **`tests/test_web_bundle.py`** — see tell #2. `[needs-decision]`
- **`tests/test_workspace.py`** — *good*, keep and showcase: real behavior
  (glyphspackage export, mocked `fontc` with arg assertions, import/round-trip,
  path-traversal rejection, CRLF). This is the model the frontend "tests" should
  follow.
- **`.agents/` committed** — see tell #1. Plus references to scrub in
  `AGENTS.md` (multi-agent coordination section ~195-249), `CLAUDE.md`,
  `docs/architecture/decisions.md` ("Multi-Agent Context"). `[needs-decision]`
- **README** — strong/honest overall; genericize `/Users/eli/GF/...` paths
  (lines 66,67,179); consider moving the long theming/palette table to `docs/`.
- **`AGENTS.md`** — useful engineering content (kurbo split, pnpm pain); the
  multi-agent protocol is the tell; drop the `⚠` U+26A0 glyph (line 112). `[needs-decision]`
- **Personal-path leaks** (cross-cutting): README, `docs/architecture/decisions.md:75`,
  `web/src/gfSidebarData.generated.ts:2`, `tests/test_web_bundle.py:104` (enforced!),
  3 script docstrings, `scripts/comfyui-run-mac.reference.sh`,
  `scripts/generate-gf-sidebar-data.mjs`. `[safe to genericize]`
- **`requirements.txt`** pins `drawbot-skia` to a personal fork — document *why*
  or upstream. (MED) `[needs-decision]`
- Good: `.gitignore` complete; `git ls-files` clean (no `__pycache__`/`target`/
  `dist`/`.DS_Store`); full LICENSE; substantive SECURITY.md (a *positive*
  credibility signal — surface it).
- **`tools/check-crate-age/target/`** build artifacts committed — gitignore. (LOW)

---

## Cleanup checklist

Legend: `[ ]` todo · `[x]` done · `[~]` decided: no action · `[S]` safe
mechanical · `[D]` needs a decision from Eli first.

**Decisions (2026-05-28):** keep `.agents/` public · keep `AGENTS.md`/`CLAUDE.md`
as-is (openly AI-transparent — a deliberate stance for a feedback post) · trim
`test_web_bundle.py` to real invariants · proceed on safe items.

**Note on the test trim:** the ~60 deleted `test_web_bundle.py` tests were
source-substring guards for xilem-parity *behaviors* (kern-zero storage, text-mode
key order, etc.). They're slop by Linebender standards, but they were the only
regression net for those behaviors — there's no frontend test runner. Gap is now
open; a Vitest/Playwright harness is the proper replacement (follow-up).

### Tier 1 — the "first five minutes" tells (highest leverage)
- [~] **[D]** `.agents/`: decided — keep public.
- [~] **[D]** `AGENTS.md`/`CLAUDE.md`: decided — keep as-is.
- [x] **[D]** `tests/test_web_bundle.py`: trimmed 65→4 real-invariant tests;
      dropped the source-greps, the `rb-bundle` fingerprint assertion, and the
      README-path-enforcing assertion.
- [ ] **[D]** `rb-bundle-…-NN` fingerprint (still in `extension.ts:14` + banner):
      replace with `package.json` version (Vite `define`) or drop from the
      user-facing banner. (Kept for now — useful cache-bust signal mid-iteration.)
- [ ] **[S]** Genericize all `/Users/eli` / `~/Work` paths in tracked files
      (README, decisions.md, generated TS header, 3 script docstrings, the 2 other
      scripts). Use env vars / `~`-relative / `$COMFY` placeholders.

### Tier 2 — `extension.ts` cleanup (safe)
- [ ] **[D]** `installRunebenderLogMirror` (global console patch): it's the
      browser→ComfyUI-terminal log feature (paired with the `/runebender/log`
      Python route), not pure cruft — but monkey-patching global `console` is the
      flagged tell. Decide: keep the feature, or drop it + the route.
- [x] **[S]** Delete `logNodeSockets` + its call site.
- [x] **[S]** Remove unconditional `console.info` diagnostics (preview request,
      source restore, chrome insets, node-active, img-loaded, injected-styles,
      tab-switch); kept catch-path `warn`/`error` + the single "loaded" line.
- [ ] **[S]** Trim the changelog-narration comments ("Previously…/The earlier…").
- [ ] **[D]** Replace `declare const window: any` with a narrowed `Window`
      augmentation; add minimal litegraph `node`/`widget` types. (bundle with
      tsconfig decision)
- [ ] **[D]** Move inline `cssText` blocks → stylesheet classnames (overlay,
      banner, error-pane, preview-img, source modal).
- [ ] **[D]** Add `web/tsconfig.json` (`strict:true`) + ESLint/Prettier.

### Tier 3 — Rust (safe nits + dead-code triage)
- [x] **[S]** Delete `fn _suppress`; dropped `self` from the workspace import
      (the aliases are what's used). `cargo fmt`/check/test/wasm-check all clean.
- [x] **[S]** Remove redundant inner `#![cfg(target_arch="wasm32")]`
      (`renderer.rs`, `wasm_api.rs`) — gating lives in lib.rs.
- [ ] **[S]** Fix WASM-target clippy: useless `.into()`, collapsible `if`,
      unnecessary `if let`, needless borrow; add `HyperPath::is_empty`.
- [ ] **[S]** `workspace.rs:113,121` NaN-safe compare (or delete dead block).
- [ ] **[S]** `#[inline(always)]`→`#[inline]`; `.to_vec()`→`.iter()`;
      `for{push(*el)}`→`.extend()`.
- [ ] **[S]** `//`→`//!` module docs across the flagged files.
- [ ] **[S]** Rename `glif_/glyph_metadata_from_norad` pair.
- [ ] **[D]** Triage the 11 `#[allow(dead_code)]`: delete dead, remove stale
      allows, wire up intended. (needs Eli — some may be deliberate xilem-parity
      placeholders)
- [ ] **[S]** Drop `thiserror` if unused (verify with `cargo +nightly udeps`).

### Tier 4 — Python (safe)
- [ ] **[S]** Extract shared `_create_and_compile_slot` + response builder in
      `font.py`; collapse the 3 duplicate routes and 4× compile block.
- [ ] **[S]** Collapse `preview_workspace_slot` debug-trace logging.
- [ ] **[S]** Narrow + log the silent `except Exception: pass` cluster.
- [ ] **[S]** Delete `_copy_tree_contents`; drop unused `compiled` in compile_font.py.
- [ ] **[S]** `font_specimen.py` → `NamedTemporaryFile`/`TemporaryDirectory`.
- [ ] **[S]** Trim over-narrated docstrings/comments.

### Tier 5 — hygiene / follow-ups
- [ ] **[S]** Gitignore `tools/check-crate-age/target/`; untrack it.
- [ ] **[D]** Document or upstream the `drawbot-skia` fork in `requirements.txt`.
- [ ] **[D]** Move README theming/palette table to `docs/`.
- [ ] **[D]** (Follow-up) Decompose `onNodeCreated` / extract `Runebender.vue`
      composables — larger refactor, not a posting blocker.
</content>
