#!/usr/bin/env bash
# Runs both supply-chain checks for the Rust side. See SECURITY.md for
# the npm-side equivalent (pnpm's minimum-release-age, enforced at
# install time).
#
# Exit codes:
#   0 — all checks passed
#   1 — at least one check failed (look at the output to see which)
#   2 — environment problem (cargo-deny not installed, etc.)

set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# The Rust editor crate lives in the runebender-web package (file: dep).
EDITOR_CRATE="$ROOT/web/node_modules/runebender-web/core"
if [[ ! -d "$EDITOR_CRATE" ]]; then
    echo "runebender-web dependency not installed; run: cd web && pnpm install" >&2
    exit 2
fi

failures=0

echo "=== cargo deny check (RustSec + licenses + sources) ==="
if ! command -v cargo-deny >/dev/null 2>&1; then
    echo "cargo-deny not installed. Install with:" >&2
    echo "    cargo install cargo-deny --locked" >&2
    exit 2
fi
( cd "$EDITOR_CRATE" && cargo deny check ) || failures=$((failures + 1))

echo
echo "=== check-crate-age (7-day cooldown) ==="
AGE_BIN="$ROOT/tools/check-crate-age/target/release/check-crate-age"
if [[ ! -x "$AGE_BIN" ]]; then
    echo "building check-crate-age..." >&2
    ( cd "$ROOT/tools/check-crate-age" && cargo build --release ) || exit 2
fi
"$AGE_BIN" "$EDITOR_CRATE/Cargo.lock" "$@" || failures=$((failures + 1))

echo
if [[ $failures -eq 0 ]]; then
    echo "✓ all supply-chain checks passed"
    exit 0
else
    echo "✗ $failures check(s) failed"
    exit 1
fi
