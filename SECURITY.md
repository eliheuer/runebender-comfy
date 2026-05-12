# Supply-chain checks

Two independent layers protect the dependency graph: one for npm, one
for cargo. Run both before publishing or merging dependency bumps.

## npm side — pnpm cooldown (proactive)

`~/.npmrc` and `web/.npmrc` set:

```
minimum-release-age=10080
```

10080 minutes = 7 days. pnpm refuses to install any package version
younger than the cooldown. This blocks malicious releases at *install*
time — they never reach `node_modules`.

To bypass for a specific package (only after auditing it), add to the
`minimum-release-age-exclude` list in `.npmrc`.

## cargo side, layer 1 — RustSec via cargo-deny (reactive)

`rust-core/deny.toml` configures four checks:

| Check | Catches |
|---|---|
| `advisories` | Known-vulnerable + yanked crates (RustSec database) |
| `licenses` | License policy (GPL-3.0-compatible only) |
| `bans` | Duplicate-version trees, banned crates |
| `sources` | Restricts to crates.io + `linebender/spline` git URL |

Install once:

```sh
cargo install cargo-deny --locked
```

Run from `rust-core/`:

```sh
cargo deny check
```

This is **reactive**: RustSec finds bad crates *after* they're
published. For proactive cooldown like the npm side, see layer 2.

## cargo side, layer 2 — `check-crate-age` (proactive)

Cargo has no install-time hook, so the cooldown runs as an audit step
*after* dependencies resolve. Same 7-day threshold as the npm rule.

Build once:

```sh
cargo build --release --manifest-path tools/check-crate-age/Cargo.toml
```

Run against the lockfile:

```sh
./tools/check-crate-age/target/release/check-crate-age \
    rust-core/Cargo.lock
```

Exit codes:

- `0` — every crates.io-sourced crate is at least 7 days old
- `1` — one or more crates are too fresh (lists them)
- `2` — bad CLI args / unreadable lockfile / network failure

Bypass a specific crate (e.g. after auditing wasm-bindgen):

```sh
check-crate-age rust-core/Cargo.lock --exclude wasm-bindgen,js-sys
```

Override the cooldown window:

```sh
check-crate-age rust-core/Cargo.lock --days 14
```

## Running everything at once

```sh
./scripts/audit.sh
```

…runs both `cargo deny check` and `check-crate-age`. CI gate or
pre-merge ritual.
