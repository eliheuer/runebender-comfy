#!/usr/bin/env python3
"""Check low-friction Comfy Registry publish readiness.

This is intentionally conservative: it distinguishes repo-local errors
from external blockers that cannot be proven from a checkout, such as a
registered publisher account or a public icon URL.
"""

from __future__ import annotations

import argparse
import ast
import fnmatch
import importlib.util
from pathlib import Path
import re
import sys
import types

try:
    import tomllib
except ImportError:  # pragma: no cover - CI uses Python 3.11+
    tomllib = None  # type: ignore[assignment]


ROOT = Path(__file__).resolve().parents[1]

NODE_ID_RE = re.compile(r"^[A-Za-z][A-Za-z0-9]*(?:[._-][A-Za-z0-9]+|[A-Za-z0-9])*$")
SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+$")

RUNTIME_FILES = [
    "__init__.py",
    "nodes/runebender.py",
    "nodes/font.py",
    "nodes/workspace.py",
    "nodes/drawbot_presets/01_specimen.py",
    "nodes/drawbot_presets/02_waterfall.py",
    "nodes/drawbot_presets/03_glyph.py",
    "nodes/drawbot_presets/04_pangram.py",
    "nodes/drawbot_presets/05_custom.py",
    "nodes/drawbot_presets/README.md",
    "nodes/drawbot_presets/helpers.py",
    "requirements.txt",
    "README.md",
    "LICENSE",
    "docs/workflows/local-font-workflow.md",
    "example_workflows/runebender-linked-source-smoke.json",
    "web/dist/runebender-comfy.js",
    "web/dist/style.css",
]

DEV_ONLY_FILES = [
    ".agents/COMFY_REGISTRY_PUBLISHING.md",
    ".github/workflows/ci.yml",
    "AGENTS.md",
    "CLAUDE.md",
    "rebuild-icons.sh",
    "assets/runebender-icons.ufo/fontinfo.plist",
    "rust-core/src/lib.rs",
    "scripts/check_comfy_publish_ready.py",
    "tests/test_web_bundle.py",
    "tools/check-crate-age/Cargo.toml",
    "web/src/Runebender.vue",
    "web/wasm/runebender_comfy_core.js",
    "web/node_modules/.modules.yaml",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit nonzero when known external publish blockers remain",
    )
    args = parser.parse_args()

    hard_errors: list[str] = []
    blockers: list[str] = []

    metadata = _load_pyproject(hard_errors)
    if metadata is not None:
        _check_pyproject(metadata, hard_errors, blockers)
    _check_comfyignore(hard_errors)
    _check_entrypoint(hard_errors)
    _check_node_registration(hard_errors)
    _check_requirements(hard_errors)
    _check_drawbot_exec_review(blockers)

    if hard_errors:
        print("Comfy publish readiness: ERROR")
        for item in hard_errors:
            print(f"error: {item}")
        if blockers:
            print("\nKnown publish blockers:")
            for item in blockers:
                print(f"blocker: {item}")
        return 2

    if blockers:
        print("Comfy publish readiness: NOT READY")
        for item in blockers:
            print(f"blocker: {item}")
        return 1 if args.strict else 0

    print("Comfy publish readiness: READY")
    return 0


def _load_pyproject(hard_errors: list[str]) -> dict | None:
    if tomllib is None:
        hard_errors.append("Python 3.11+ tomllib is required to parse pyproject.toml")
        return None
    path = ROOT / "pyproject.toml"
    if not path.is_file():
        hard_errors.append("pyproject.toml is missing")
        return None
    return tomllib.loads(path.read_text(encoding="utf-8"))


def _check_pyproject(metadata: dict, hard_errors: list[str], blockers: list[str]) -> None:
    project = metadata.get("project", {})
    comfy = metadata.get("tool", {}).get("comfy", {})
    dynamic = metadata.get("tool", {}).get("setuptools", {}).get("dynamic", {})

    _require_equal(project.get("name"), "runebender-comfy", "project.name", hard_errors)
    _require_equal(project.get("license"), {"file": "LICENSE"}, "project.license", hard_errors)
    _require_equal(project.get("dynamic"), ["dependencies"], "project.dynamic", hard_errors)
    _require_equal(
        dynamic.get("dependencies"),
        {"file": ["requirements.txt"]},
        "tool.setuptools.dynamic.dependencies",
        hard_errors,
    )
    _require_equal(comfy.get("PublisherId"), "eliheuer", "tool.comfy.PublisherId", hard_errors)
    _require_equal(comfy.get("DisplayName"), "Runebender", "tool.comfy.DisplayName", hard_errors)
    if not NODE_ID_RE.fullmatch(str(project.get("name", ""))):
        hard_errors.append("project.name must be a valid Comfy node id")
    if not SEMVER_RE.fullmatch(str(project.get("version", ""))):
        hard_errors.append("project.version must be X.Y.Z semantic version")
    if not project.get("description"):
        hard_errors.append("project.description is required")
    if not project.get("requires-python"):
        hard_errors.append("project.requires-python is required")
    if "web/dist" not in comfy.get("includes", []):
        hard_errors.append("tool.comfy.includes must include web/dist")

    for key in ("Repository", "Documentation", "Bug Tracker"):
        value = project.get("urls", {}).get(key)
        if not value:
            hard_errors.append(f"project.urls.{key} is required")
        elif not str(value).startswith("https://"):
            hard_errors.append(f"project.urls.{key} must be an https URL")

    if not comfy.get("Icon"):
        blockers.append("tool.comfy.Icon needs a public square icon URL")
    if not comfy.get("requires-comfyui"):
        blockers.append("tool.comfy.requires-comfyui needs a tested minimum ComfyUI version")
    blockers.append("verify the eliheuer PublisherId exists in the Comfy Registry account")


def _require_equal(actual, expected, name: str, hard_errors: list[str]) -> None:
    if actual != expected:
        hard_errors.append(f"{name} must be {expected!r}; found {actual!r}")


def _check_comfyignore(hard_errors: list[str]) -> None:
    path = ROOT / ".comfyignore"
    if not path.is_file():
        hard_errors.append(".comfyignore is missing")
        return
    patterns = _comfyignore_patterns(path)
    for rel_path in RUNTIME_FILES:
        if not (ROOT / rel_path).exists():
            hard_errors.append(f"runtime file is missing: {rel_path}")
        if _is_comfyignored(rel_path, patterns):
            hard_errors.append(f"runtime file is incorrectly excluded by .comfyignore: {rel_path}")
    for rel_path in DEV_ONLY_FILES:
        if not _is_comfyignored(rel_path, patterns):
            hard_errors.append(f"development-only file is not excluded by .comfyignore: {rel_path}")


def _comfyignore_patterns(path: Path) -> list[str]:
    return [
        line.strip()
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


def _is_comfyignored(path: str, patterns: list[str]) -> bool:
    normalized = path.strip("/")
    for pattern in patterns:
        normalized_pattern = pattern.strip("/")
        if pattern.endswith("/") and (
            normalized == normalized_pattern
            or normalized.startswith(normalized_pattern + "/")
        ):
            return True
        if fnmatch.fnmatch(normalized, normalized_pattern):
            return True
    return False


def _check_entrypoint(hard_errors: list[str]) -> None:
    init_path = ROOT / "__init__.py"
    if not init_path.is_file():
        hard_errors.append("__init__.py is missing")
        return
    tree = ast.parse(init_path.read_text(encoding="utf-8"))
    web_directory = None
    for node in tree.body:
        if (
            isinstance(node, ast.Assign)
            and any(isinstance(target, ast.Name) and target.id == "WEB_DIRECTORY" for target in node.targets)
            and isinstance(node.value, ast.Constant)
        ):
            web_directory = node.value.value
    if web_directory != "./web/dist":
        hard_errors.append(f"WEB_DIRECTORY must be './web/dist'; found {web_directory!r}")


def _check_node_registration(hard_errors: list[str]) -> None:
    class _Routes:
        def post(self, _path):
            return lambda fn: fn

        def get(self, _path):
            return lambda fn: fn

    class _PromptServer:
        instance = types.SimpleNamespace(routes=_Routes())

    sys.modules.setdefault("server", types.SimpleNamespace(PromptServer=_PromptServer))
    sys.modules.setdefault(
        "aiohttp",
        types.SimpleNamespace(
            web=types.SimpleNamespace(
                json_response=lambda payload: payload,
                HTTPBadRequest=Exception,
            ),
        ),
    )
    spec = importlib.util.spec_from_file_location(
        "runebender_comfy_publish_probe",
        ROOT / "__init__.py",
        submodule_search_locations=[str(ROOT)],
    )
    if spec is None or spec.loader is None:
        hard_errors.append("could not create import spec for __init__.py")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules["runebender_comfy_publish_probe"] = module
    try:
        spec.loader.exec_module(module)
    except Exception as exc:  # pragma: no cover - exercised by broken imports
        hard_errors.append(f"custom node package import failed: {exc}")
        return

    expected = {
        "CompileFont",
        "FontPreview",
        "FontSpecimen",
        "ComfyFontDrawBot",
        "ForkFont",
        "ApplyGlyphCandidates",
        "GlyphCandidateBuilder",
        "Runebender",
        "DesignBot",
    }
    mappings = set(getattr(module, "NODE_CLASS_MAPPINGS", {}))
    display_mappings = set(getattr(module, "NODE_DISPLAY_NAME_MAPPINGS", {}))
    if mappings != expected:
        hard_errors.append(f"NODE_CLASS_MAPPINGS mismatch: {sorted(mappings)!r}")
    if display_mappings != expected:
        hard_errors.append(f"NODE_DISPLAY_NAME_MAPPINGS mismatch: {sorted(display_mappings)!r}")
    if getattr(module, "WEB_DIRECTORY", None) != "./web/dist":
        hard_errors.append("imported WEB_DIRECTORY must be './web/dist'")


def _check_requirements(hard_errors: list[str]) -> None:
    requirement_lines = [
        line.strip()
        for line in (ROOT / "requirements.txt").read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    requirements = "\n".join(requirement_lines)
    if "drawbot-skia @ git+https://github.com/eliheuer/drawbot-skia.git" not in requirements:
        hard_errors.append("requirements.txt must use the eliheuer/drawbot-skia fork")
    for line in requirement_lines:
        if line.startswith(("-e ", "--editable")):
            hard_errors.append(f"requirements.txt must not use editable installs: {line}")
        if line.startswith((".", "/", "~")) or " @ file:" in line:
            hard_errors.append(f"requirements.txt must not use local path dependencies: {line}")
        if "git+" in line and "git+https://" not in line:
            hard_errors.append(f"requirements.txt git dependencies must use https: {line}")


def _check_drawbot_exec_review(blockers: list[str]) -> None:
    source = (ROOT / "nodes" / "font_specimen.py").read_text(encoding="utf-8")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if "exec(script, namespace)" not in source:
        return
    blockers.append("remove or sandbox FontSpecimen DrawBot exec before registry submission")
    warning_fragments = [
        "custom scripts execute locally in the ComfyUI Python process",
        "run only trusted scripts",
        "do not paste untrusted Python",
    ]
    if not all(fragment in source or fragment in readme for fragment in warning_fragments):
        blockers.append(
            "document and surface the intentional DrawBot exec path before registry submission"
        )


if __name__ == "__main__":
    sys.exit(main())
