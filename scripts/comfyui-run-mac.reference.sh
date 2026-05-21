#!/usr/bin/env bash
#
# REFERENCE COPY — not run from this repo.
#
# This is a copy of run-mac.sh as it lives in the ComfyUI repo
# (~/Work/comfy/repos/ComfyUI/run-mac.sh) on the dev machine. It is
# kept here only so the launch wiring isn't lost, since the real
# run-mac.sh stays uncommitted in the ComfyUI repo.
#
# The relevant addition for runebender-comfy is the theme-sync step
# near the bottom, which runs scripts/sync-comfy-theme.py before
# ComfyUI starts so themes/runebender-dark.json is copied into
# ComfyUI's settings on every launch. The RUNEBENDER_THEME_SYNC path
# is machine-specific; adjust it if the runebender-comfy checkout
# moves.
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="$ROOT/.venv"
URL="http://127.0.0.1:8188"

# Keep the Runebender Dark dev palette in sync with the runebender-comfy
# repo on every launch, so editing themes/runebender-dark.json there
# and restarting is all it takes to update the ComfyUI theme menu.
RUNEBENDER_THEME_SYNC="/Users/eli/GH/repos/runebender-comfy/scripts/sync-comfy-theme.py"

if [[ ! -d "$VENV" ]]; then
  echo "Missing virtualenv: $VENV" >&2
  exit 1
fi

if [[ -z "${VIRTUAL_ENV:-}" ]] || [[ "$(cd "$VIRTUAL_ENV" && pwd)" != "$(cd "$VENV" && pwd)" ]]; then
  # shellcheck disable=SC1090
  source "$VENV/bin/activate"
fi

if ! command -v open >/dev/null 2>&1; then
  echo "macOS 'open' command not found" >&2
  exit 1
fi

echo "Starting ComfyUI from: $ROOT"
echo "Python: $(command -v python)"
echo "Browser: Chrome app window at $URL"

cleanup() {
  true
}
trap cleanup EXIT INT TERM

if [[ -f "$RUNEBENDER_THEME_SYNC" ]]; then
  python3 "$RUNEBENDER_THEME_SYNC" "$ROOT/user/default/comfy.settings.json" || true
fi

(sleep 5; open -na "Google Chrome" --args --app="$URL") &
cd "$ROOT"
python main.py --preview-method auto
