#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="$ROOT/.venv"
PYTHON="${PYTHON:-$VENV/bin/python}"

cd "$ROOT"

if [[ ! -x "$PYTHON" ]]; then
  if [[ "$PYTHON" != "$VENV/bin/python" ]]; then
    echo "Configured PYTHON is not executable: $PYTHON"
    exit 1
  fi
  echo "==> Creating repo-local Python venv at .venv"
  python3 -m venv "$VENV"
fi

echo "==> Checking Python icon dependencies"
if ! "$PYTHON" -c 'import ufoLib2, fontTools' >/dev/null 2>&1; then
  echo "==> Installing icon build dependencies into .venv"
  "$PYTHON" -m pip install --upgrade pip
  "$PYTHON" -m pip install fonttools ufoLib2
fi

echo "==> Rebuilding generated toolbar icons from assets/runebender-icons.ufo"
"$PYTHON" scripts/assign_icon_codepoints.py
"$PYTHON" scripts/build_toolbar_icons.py

echo "==> Bumping web bundle fingerprint"
"$PYTHON" -c '
from pathlib import Path
from datetime import datetime
import re

path = Path("web/src/extension.ts")
text = path.read_text(encoding="utf-8")
stamp = datetime.now().strftime("%Y-%m-%d-icons-%H%M%S")
next_value = f"rb-bundle-{stamp}"
updated, count = re.subn(
    r"const RUNEBENDER_BUNDLE_FINGERPRINT = \"[^\"]+\";",
    f"const RUNEBENDER_BUNDLE_FINGERPRINT = \"{next_value}\";",
    text,
    count=1,
)
if count != 1:
    raise SystemExit("Could not find RUNEBENDER_BUNDLE_FINGERPRINT in web/src/extension.ts")
path.write_text(updated, encoding="utf-8")
print(next_value)
'

echo "==> Building web bundle"
(
  cd web
  COREPACK_ENABLE_AUTO_PIN=0 pnpm build
)

echo "==> Done. Restart ComfyUI and hard-refresh the browser to load the rebuilt icons."
