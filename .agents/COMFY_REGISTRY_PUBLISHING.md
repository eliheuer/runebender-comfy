# Comfy Registry Publishing Plan

Durable notes for getting `runebender-comfy` ready for an official Comfy
Registry publish. Keep this focused on repeatable publish mechanics and
low-risk readiness work; product/UI launch plans belong in separate `.agents`
files.

## Current target

Publish `runebender-comfy` as a normal ComfyUI custom node package first. Treat
Comfy Cloud and QuiverAI support as follow-on integration tracks unless Comfy
provides a direct pre-release/private custom-node test path.

The base node must install and run without paid Partner Nodes. QuiverAI support
should be optional: example workflows and helper scripts can depend on Quiver,
but core Runebender should remain usable without it.

## Upstream references

Verified against the current public Comfy docs on 2026-05-30:

- Registry publishing: https://docs.comfy.org/registry/publishing
- Registry standards: https://docs.comfy.org/registry/standards
- `pyproject.toml` spec: https://docs.comfy.org/registry/specifications
- Custom Node CI/CD: https://docs.comfy.org/registry/cicd
- Registry site: https://registry.comfy.org/

## Required publish metadata

Root `pyproject.toml` exists with:

- `[project].name`: immutable registry node id. Candidate:
  `runebender-comfy` unless the registry rejects it.
- `[project].version`: semantic version. Start below `1.0.0` while this is
  still experimental. Current value: `0.1.0`.
- `[project].description`: short user-facing description.
- `[project].license`: point at the repo license file.
- `[project].requires-python`: match Comfy-supported Python versions.
- `[project].dynamic = ["dependencies"]` plus
  `[tool.setuptools.dynamic].dependencies = { file = ["requirements.txt"] }`.
- `[project.urls].Repository`: public GitHub URL.
- `[project.urls].Documentation`: README or docs URL.
- `[project.urls]."Bug Tracker"`: GitHub issues URL.
- `[tool.comfy].PublisherId`: currently `eliheuer`. Verify this publisher id
  exists before publishing. This id is globally unique and effectively
  permanent.
- `[tool.comfy].DisplayName`: `Runebender`.
- `[tool.comfy].Icon`: public square image URL, max 400x400 in the
  `pyproject.toml` spec. The publishing guide currently shows a looser
  800x400 example; for Runebender, use a square icon and keep it within the
  stricter spec limit.
- `[tool.comfy].requires-comfyui`: set once the minimum tested ComfyUI version
  is known.

Do not publish with placeholder publisher, repository, icon, or minimum
ComfyUI version values.

Note: Comfy marks `Icon` and `requires-comfyui` as optional metadata, but this
repo treats them as required for an official Runebender publish candidate. That
is why `scripts/check_comfy_publish_ready.py --strict` blocks on them.

## Package contents

`.comfyignore` exists. The registry packages tracked files by default, so this
file explicitly excludes development-only material that is useful in git but
should not ship to users.

Likely excludes:

- `.agents/`
- `.github/` if CI is not needed in the package archive
- `docs/architecture/` if it is agent/internal only
- `tests/`
- `rust-core/target/`
- `web/node_modules/`
- `web/wasm/`
- `web/assets/test-fonts/`
- local build scratch and generated cache directories

Do not exclude runtime files needed by Comfy:

- `__init__.py`
- `nodes/`
- `web/dist/` or whatever built frontend directory Comfy loads
- `requirements.txt`
- runtime docs/examples/workflows intended for users

If the built frontend directory is gitignored but must ship, add it to
`[tool.comfy].includes`.

## Registry icon

There is not yet a finished Comfy Registry icon asset in this repo. The
toolbar icon source at `assets/runebender-icons.ufo` is for in-app toolbar
glyphs only; do not treat it as the package icon without a separate design
pass.

Recommended next step:

1. Add a square registry icon under `docs/registry/` or another tracked public
   docs path.
2. Keep the source editable, ideally SVG plus any generated PNG export.
3. After the file is pushed to GitHub, set `[tool.comfy].Icon` to the public
   raw GitHub URL.
4. Keep `scripts/check_comfy_publish_ready.py --strict` failing until this URL
   exists and has been verified.

The current toolbar UFO can influence the visual language, but the registry
icon should read clearly at small sizes and should not depend on the incomplete
toolbar redraw work currently in progress.

## Standards checklist

Before the first publish candidate:

- No runtime `pip install` or dependency installation via `subprocess`.
- No `eval` or unreviewed `exec`.
  - Current review item: `nodes/font_specimen.py` intentionally calls
    `exec(script, namespace)` for user-authored local DrawBot scripts. The
    node description, custom-script tooltip, and README now warn that custom
    scripts execute locally and should be trusted. The current Comfy Registry
    standards prohibit `exec`, so this is now a strict publish blocker until
    the DrawBot scripting path is removed, sandboxed, or split out of the
    publishable node.
- No obfuscated/minified Python source.
- Optional dependencies and optional Partner Node workflows display clear
  warnings when used.
- README explains install, launch, local font source workflow, save behavior,
  and known limitations.
- Example workflows demonstrate any optional dependency requirements.
- License file is present and matches `pyproject.toml`.
- ComfyUI can import the package cleanly after a fresh clone/install.

## CI and publish path

Use two separate automation tracks:

1. CI readiness:
   - run Python tests
   - run frontend bundle test
   - run Rust tests
   - build the frontend bundle
   - optionally run a minimal Comfy workflow via Comfy-Action
   - run `python scripts/check_comfy_publish_ready.py` to fail on repo-local
     packaging errors while still reporting external blockers

2. Publishing:
   - create a Registry publisher account
   - create a Registry publishing API key
   - add `REGISTRY_ACCESS_TOKEN` to GitHub Actions secrets
   - add the official `Comfy-Org/publish-node-action` workflow
   - publish only from an explicit release/version bump, not every normal push

Use manual `comfy node publish` only for local dry runs or one-off releases.

For release gating, run:

```sh
python scripts/check_comfy_publish_ready.py --strict
```

Strict mode exits nonzero while any known publish blockers remain. Non-strict
mode is suitable for CI because it catches local regressions without failing
just because account-only values or larger review items still need human
confirmation. As of this pass, strict mode should report the registry icon,
minimum ComfyUI version, publisher id, and DrawBot `exec` blockers.

The checker currently validates these repo-local facts before it reports
external blockers:

- `pyproject.toml` exists and has the expected package metadata, dynamic
  dependency source, HTTPS project URLs, SemVer version, and Comfy metadata.
- `.comfyignore` keeps runtime files in the package and excludes development
  files, tests, agent notes, source-only assets, and build inputs.
- `__init__.py` sets `WEB_DIRECTORY = "./web/dist"`.
- The package imports under a minimal Comfy-like route stub and registers the
  expected node class/display mappings.
- `requirements.txt` uses the `eliheuer/drawbot-skia` fork, avoids editable or
  local path dependencies, and uses HTTPS for git dependencies.
- The intentional `FontSpecimen` DrawBot `exec` path is documented and reported
  as a blocker for registry submission.

## QuiverAI support direction

QuiverAI should be integrated as a supported optional workflow, not as a hard
dependency.

Best first bridge:

- Generate SVG candidates with Quiver Text-to-SVG or Image-to-SVG.
- Import chosen SVGs into `assets/runebender-icons.ufo` with a repo script.
- Rebuild generated toolbar assets with `./rebuild-icons.sh`.

The importer should:

- accept only simple filled SVG paths at first
- reject strokes, masks, filters, gradients, and unsupported transforms with a
  clear error
- normalize to the 768-wide icon cell used by the toolbar UFO
- write one named `.glif`
- avoid generating round stroke-expansion artifacts

This keeps Runebender as the source-of-truth editor while letting Quiver
accelerate candidate generation.

## Low-hanging fruit goal

For the first publish-readiness pass, stay small:

1. Add `pyproject.toml` with conservative metadata and TODOs only where the
   value truly needs the user's registry account. Done.
2. Add `.comfyignore`. Done.
3. Add a short README publishing/install section if missing. Done.
4. Add a basic CI workflow that runs existing tests/builds. Done.
5. Document what remains blocked on registry account, icon URL, final tested
   ComfyUI minimum version, and the DrawBot `exec` path. Done in README and
   this file.

Do not attempt a real registry publish until those blockers are resolved.

## Pause Handoff 2026-05-30

Current state: basic publish-readiness work is in place, but the goal is paused
before a real publish candidate. Do not mark the publish goal complete until the
strict checker passes and the external registry facts have been verified.

Files added or changed for this pass:

- `pyproject.toml`: package metadata and `[tool.comfy]` basics.
- `.comfyignore`: excludes development-only files from the registry archive.
- `.github/workflows/ci.yml`: builds WASM/frontend, runs Python tests, Rust
  tests, and the publish-readiness checker.
- `scripts/check_comfy_publish_ready.py`: non-strict CI checker plus strict
  release gate.
- `tests/test_web_bundle.py`: registry metadata, `.comfyignore`, and readiness
  checker coverage.
- `tests/test_workspace.py`: covers the DrawBot custom-script warning.
- `nodes/font_specimen.py`: warns that custom DrawBot scripts execute locally
  and must be trusted.
- `README.md`: Comfy Registry publishing section and DrawBot script warning.

Worktree caveat: this checkout also contains unrelated in-progress UI/icon
changes from other Runebender work. Do not assume every dirty file belongs to
the publish-readiness pass. If committing only this publish prep, stage the
files listed above plus `.github/workflows/ci.yml`; leave the toolbar UFO,
generated toolbar icons, glyph sidebar, Rust WASM API, and visual component
changes for their own UI/icon commits.

Validation commands that passed in this state:

```sh
python3 scripts/check_comfy_publish_ready.py
python3 -m unittest tests.test_workspace tests.test_web_bundle
COREPACK_ENABLE_AUTO_PIN=0 pnpm build # from web/
cargo test # from rust-core/
```

Important nuance: strict mode intentionally exits `1` right now. That is the
expected state while registry blockers remain:

```sh
python3 scripts/check_comfy_publish_ready.py --strict
```

It should print:

- `tool.comfy.Icon needs a public square icon URL`
- `tool.comfy.requires-comfyui needs a tested minimum ComfyUI version`
- `verify the eliheuer PublisherId exists in the Comfy Registry account`
- `remove or sandbox FontSpecimen DrawBot exec before registry submission`

Known blockers before a real registry publish:

1. Confirm the Comfy Registry publisher id is really `eliheuer`.
2. Add `[tool.comfy].Icon` using a public square icon URL. There is no finished
   registry icon asset yet; do not use `assets/runebender-icons.ufo` directly as
   the package icon.
3. Test the minimum supported ComfyUI version and add
   `[tool.comfy].requires-comfyui`.
4. Remove, sandbox, or split the current DrawBot scripting `exec` path before
   registry submission. The warning remains useful for local development, but
   it is not enough for the current registry standards.

Next recommended pickup:

1. Create/add a registry icon asset and wire `[tool.comfy].Icon`.
2. Confirm or create the `eliheuer` publisher in the Comfy Registry.
3. Run against the lowest ComfyUI version we want to support, then set
   `[tool.comfy].requires-comfyui`.
4. Run `python3 scripts/check_comfy_publish_ready.py --strict`; once it exits
   `0`, add the official publish workflow using
   `Comfy-Org/publish-node-action`.

Suggested resume order:

1. Re-run `python3 scripts/check_comfy_publish_ready.py --strict` first and
   compare the output to the blocker list above. If new `error:` lines appear,
   fix those repo-local regressions before touching account or release work.
2. Decide the DrawBot publishing strategy:
   - remove `FontSpecimen`/`DrawBot Skia` from the publishable package,
   - replace custom scripts with preset-only rendering, or
   - move arbitrary DrawBot scripting into a separate non-registry/local node.
3. Add the registry icon asset and public URL after the icon design is approved.
4. Verify the publisher id and minimum ComfyUI version.
5. Re-run the full release gate:
   ```sh
   python3 scripts/check_comfy_publish_ready.py --strict
   python3 -m unittest tests.test_workspace tests.test_web_bundle
   COREPACK_ENABLE_AUTO_PIN=0 pnpm build # from web/
   cargo test # from rust-core/
   ```
6. Only after all of that passes, add the real publish workflow and configure
   `REGISTRY_ACCESS_TOKEN` in GitHub Actions.
