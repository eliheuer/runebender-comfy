#!/usr/bin/env python3
"""Check live Runebender tracing readiness.

This checker avoids paid Cloud execution. It verifies local prerequisites,
ComfyUI node registration, and whether Cloud credentials are present.
"""

from __future__ import annotations

import argparse
import ast
from dataclasses import dataclass
import json
import os
from pathlib import Path
import shutil
import sys
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMFY_URL = "http://127.0.0.1:8188"

REQUIRED_TRACE_NODES = [
    "Runebender",
    "BuildGlyphTraceRequest",
    "TraceToCandidate",
    "TraceWithQuiverAI",
    "TraceWithComfyCloudQuiverAI",
    "TraceLocalMaskToCandidate",
    "ScoreCandidate",
]


@dataclass(frozen=True)
class LiveCheck:
    name: str
    status: str
    detail: str


def resolve_local_trace_tool(
    *,
    env: dict[str, str] | None = None,
    home: Path | None = None,
    which=shutil.which,
) -> LiveCheck:
    env = env if env is not None else os.environ
    home = home if home is not None else Path.home()
    for env_name in ("RUNEBENDER_TRACE_TOOL", "RUNEBENDER_IMG2BEZ"):
        configured = env.get(env_name, "").strip()
        if not configured:
            continue
        configured_path = Path(configured).expanduser()
        if configured_path.exists():
            return LiveCheck("local tracer", "PASS", f"{env_name} -> {configured_path}")
        found = which(configured)
        if found:
            return LiveCheck("local tracer", "PASS", f"{env_name} -> {found}")
        return LiveCheck("local tracer", "FAIL", f"{env_name} is set but not executable: {configured}")

    path_tool = which("img2bez")
    if path_tool:
        return LiveCheck("local tracer", "PASS", f"img2bez on PATH -> {path_tool}")

    cargo_tool = home / ".cargo" / "bin" / "img2bez"
    if cargo_tool.exists():
        return LiveCheck("local tracer", "PASS", f"cargo install -> {cargo_tool}")

    sibling = home / "GH" / "repos" / "img2bez"
    release_bin = sibling / "target" / "release" / "img2bez"
    if release_bin.exists():
        return LiveCheck("local tracer", "PASS", f"sibling release build -> {release_bin}")
    if (sibling / "Cargo.toml").exists():
        return LiveCheck("local tracer", "PASS", f"sibling cargo manifest -> {sibling / 'Cargo.toml'}")

    return LiveCheck(
        "local tracer",
        "FAIL",
        "set RUNEBENDER_TRACE_TOOL, set RUNEBENDER_IMG2BEZ, install img2bez, or build ~/GH/repos/img2bez",
    )


def check_cloud_key(env: dict[str, str] | None = None) -> LiveCheck:
    env = env if env is not None else os.environ
    if env.get("COMFY_CLOUD_API_KEY", "").strip():
        return LiveCheck("Comfy Cloud API key", "PASS", "COMFY_CLOUD_API_KEY is set")
    return LiveCheck(
        "Comfy Cloud API key",
        "FAIL",
        "COMFY_CLOUD_API_KEY is not set; live Quiver Cloud execution remains pending",
    )


def check_registered_nodes(object_info: dict) -> LiveCheck:
    missing = [node for node in REQUIRED_TRACE_NODES if node not in object_info]
    if missing:
        return LiveCheck("ComfyUI node registration", "FAIL", "missing nodes: " + ", ".join(missing))
    return LiveCheck(
        "ComfyUI node registration",
        "PASS",
        "registered nodes: " + ", ".join(REQUIRED_TRACE_NODES),
    )


def check_custom_node_install(comfy_root: str, *, repo_root: Path = ROOT) -> LiveCheck:
    root = Path(comfy_root).expanduser()
    if not comfy_root.strip():
        return LiveCheck(
            "ComfyUI custom node install",
            "INFO",
            "not checked; pass --comfy-root or set COMFYUI_ROOT",
        )
    custom_node = root / "custom_nodes" / "runebender-comfy"
    if not custom_node.exists():
        return LiveCheck(
            "ComfyUI custom node install",
            "FAIL",
            f"{custom_node} does not exist",
        )
    try:
        resolved = custom_node.resolve()
        expected = repo_root.resolve()
    except OSError as exc:
        return LiveCheck("ComfyUI custom node install", "FAIL", f"could not resolve symlink: {exc}")
    if resolved == expected:
        kind = "symlink" if custom_node.is_symlink() else "directory"
        return LiveCheck(
            "ComfyUI custom node install",
            "PASS",
            f"{kind} points at this checkout: {custom_node} -> {resolved}",
        )
    return LiveCheck(
        "ComfyUI custom node install",
        "FAIL",
        f"{custom_node} points at {resolved}, expected {expected}",
    )


def check_local_node_mappings(*, repo_root: Path = ROOT) -> LiveCheck:
    init_path = repo_root / "__init__.py"
    if not init_path.is_file():
        return LiveCheck("local node mappings", "FAIL", f"{init_path} does not exist")
    try:
        tree = ast.parse(init_path.read_text(encoding="utf-8"))
    except (OSError, SyntaxError) as exc:
        return LiveCheck("local node mappings", "FAIL", f"could not parse {init_path}: {exc}")
    mapping_keys: set[str] = set()
    for node in tree.body:
        if not isinstance(node, ast.Assign):
            continue
        if not any(isinstance(target, ast.Name) and target.id == "NODE_CLASS_MAPPINGS" for target in node.targets):
            continue
        if not isinstance(node.value, ast.Dict):
            return LiveCheck("local node mappings", "FAIL", "NODE_CLASS_MAPPINGS is not a literal dict")
        for key in node.value.keys:
            if isinstance(key, ast.Constant) and isinstance(key.value, str):
                mapping_keys.add(key.value)
    missing = [node for node in REQUIRED_TRACE_NODES if node not in mapping_keys]
    if missing:
        return LiveCheck("local node mappings", "FAIL", "checkout missing nodes: " + ", ".join(missing))
    return LiveCheck(
        "local node mappings",
        "PASS",
        "checkout declares nodes: " + ", ".join(REQUIRED_TRACE_NODES),
    )


def load_json_url(base_url: str, path: str, *, timeout_seconds: float) -> dict:
    url = base_url.rstrip("/") + path
    request = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        data = response.read().decode("utf-8")
    parsed = json.loads(data)
    if not isinstance(parsed, dict):
        raise ValueError(f"{url} did not return a JSON object")
    return parsed


def collect_checks(
    comfy_url: str,
    *,
    timeout_seconds: float,
    comfy_root: str = "",
    local_only: bool = False,
) -> list[LiveCheck]:
    checks = [
        resolve_local_trace_tool(),
        check_custom_node_install(comfy_root or os.environ.get("COMFYUI_ROOT", "")),
        check_local_node_mappings(),
    ]
    if not local_only:
        checks.insert(1, check_cloud_key())

    try:
        load_json_url(comfy_url, "/system_stats", timeout_seconds=timeout_seconds)
    except (OSError, urllib.error.URLError, ValueError, json.JSONDecodeError) as exc:
        checks.append(LiveCheck("ComfyUI host", "FAIL", f"{comfy_url} is not reachable: {exc}"))
        checks.append(
            LiveCheck(
                "ComfyUI node registration",
                "FAIL",
                "skipped because /object_info cannot be checked without a running host",
            )
        )
        return checks

    checks.append(LiveCheck("ComfyUI host", "PASS", f"{comfy_url} responded to /system_stats"))

    try:
        object_info = load_json_url(comfy_url, "/object_info", timeout_seconds=timeout_seconds)
    except (OSError, urllib.error.URLError, ValueError, json.JSONDecodeError) as exc:
        checks.append(LiveCheck("ComfyUI node registration", "FAIL", f"/object_info failed: {exc}"))
    else:
        checks.append(check_registered_nodes(object_info))
    return checks


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--comfy-url", default=DEFAULT_COMFY_URL)
    parser.add_argument(
        "--comfy-root",
        default=os.environ.get("COMFYUI_ROOT", ""),
        help="ComfyUI checkout root; used to verify custom_nodes/runebender-comfy points at this checkout",
    )
    parser.add_argument("--timeout-seconds", type=float, default=2.0)
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit nonzero when any live readiness check fails",
    )
    parser.add_argument(
        "--local-only",
        action="store_true",
        help="skip Comfy Cloud API key checks and validate only local tracing readiness",
    )
    args = parser.parse_args()

    checks = collect_checks(
        args.comfy_url,
        timeout_seconds=max(0.1, args.timeout_seconds),
        comfy_root=args.comfy_root,
        local_only=args.local_only,
    )
    failures = [check for check in checks if check.status == "FAIL"]
    print("Runebender tracing live readiness: " + ("READY" if not failures else "NOT READY"))
    for check in checks:
        print(f"{check.status.lower()}: {check.name}: {check.detail}")
    return 1 if failures and args.strict else 0


if __name__ == "__main__":
    sys.exit(main())
