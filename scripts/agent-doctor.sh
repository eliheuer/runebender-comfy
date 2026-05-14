#!/usr/bin/env bash
# Preflight for agent sessions. This is intentionally lightweight:
# it checks coordination files, sibling repo layout, dirty state, and
# known stale-output traps without building or touching dependencies.

set -uo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
PARENT="$(dirname "$ROOT")"

failures=0
warnings=0

section() {
    printf '\n== %s ==\n' "$1"
}

ok() {
    printf 'ok: %s\n' "$1"
}

warn() {
    warnings=$((warnings + 1))
    printf 'warn: %s\n' "$1"
}

fail() {
    failures=$((failures + 1))
    printf 'fail: %s\n' "$1"
}

section "repo"
if git rev-parse --show-toplevel >/dev/null 2>&1; then
    TOP="$(git rev-parse --show-toplevel)"
    if [[ "$TOP" == "$ROOT" ]]; then
        ok "running from repo root"
    else
        fail "script root $ROOT does not match git root $TOP"
    fi
else
    fail "not inside a git checkout"
fi

BRANCH="$(git branch --show-current 2>/dev/null || true)"
if [[ -n "$BRANCH" ]]; then
    ok "branch: $BRANCH"
else
    warn "detached HEAD or branch unavailable"
fi

if [[ -f AGENTS.md && -f .agents/REPO_MAP.md ]]; then
    ok "agent docs present"
else
    fail "missing AGENTS.md or .agents/REPO_MAP.md"
fi

section "dirty state"
STATUS="$(git status --short)"
if [[ -z "$STATUS" ]]; then
    ok "worktree clean"
else
    warn "worktree has local changes"
    printf '%s\n' "$STATUS" | sed 's/^/  /'
fi

section "active claims"
claim_count=0
if compgen -G ".agents/active/*.md" >/dev/null; then
    for claim in .agents/active/*.md; do
        [[ "$(basename "$claim")" == "_template.md" ]] && continue
        claim_count=$((claim_count + 1))
        printf 'claim: %s\n' "$claim"
        grep -E '^(slug|agent|branch|last_touched):' "$claim" | sed 's/^/  /' || true
    done
fi
if [[ $claim_count -eq 0 ]]; then
    ok "no active claims"
fi

section "sibling repos"
for repo in runebender-core runebender-xilem; do
    if [[ -d "$PARENT/$repo/.git" || -f "$PARENT/$repo/.git" ]]; then
        ok "../$repo exists"
        if [[ -f "$PARENT/$repo/AGENTS.md" ]]; then
            ok "../$repo/AGENTS.md exists"
        else
            warn "../$repo missing AGENTS.md"
        fi
    else
        fail "../$repo is missing; path dependencies and references expect it"
    fi
done

section "wasm output"
if [[ -d web/public/wasm ]]; then
    fail "web/public/wasm exists and can shadow current wasm-pack output"
else
    ok "web/public/wasm absent"
fi

if [[ -d web/wasm ]]; then
    ok "web/wasm exists"
else
    warn "web/wasm missing; run 'cd web && pnpm wasm' before frontend dev"
fi

section "tooling"
for tool in cargo pnpm wasm-pack node; do
    if command -v "$tool" >/dev/null 2>&1; then
        ok "$tool available"
    else
        warn "$tool not found on PATH"
    fi
done

if [[ -f web/.npmrc ]] && grep -q '^minimum-release-age=10080' web/.npmrc; then
    ok "pnpm 7-day cooldown configured"
else
    warn "web/.npmrc missing minimum-release-age=10080"
fi

if [[ -f web/pnpm-workspace.yaml ]] && grep -q 'esbuild: true' web/pnpm-workspace.yaml; then
    ok "pnpm esbuild build approval configured"
else
    warn "web/pnpm-workspace.yaml missing esbuild build approval"
fi

section "next docs"
printf 'read: AGENTS.md\n'
printf 'read: .agents/REPO_MAP.md\n'
printf 'read: docs/architecture/decisions.md\n'
printf 'read: .agents/UI_PARITY_PLAN.md for UI parity work\n'

section "summary"
if [[ $failures -eq 0 ]]; then
    printf 'ok: preflight passed with %s warning(s)\n' "$warnings"
    exit 0
else
    printf 'fail: preflight found %s failure(s) and %s warning(s)\n' "$failures" "$warnings"
    exit 1
fi
