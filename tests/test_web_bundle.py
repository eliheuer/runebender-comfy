from __future__ import annotations

import ast
import fnmatch
import importlib.util
import json
import plistlib
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_BUNDLE_FINGERPRINT = "rb-bundle-2026-06-07-trace-apply-log"


class WebBundleTests(unittest.TestCase):
    def test_custom_node_package_registers_without_optional_preview_stack(self) -> None:
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
            "runebender_comfy_test_package",
            ROOT / "__init__.py",
            submodule_search_locations=[str(ROOT)],
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        sys.modules["runebender_comfy_test_package"] = module

        spec.loader.exec_module(module)

        self.assertEqual(module.WEB_DIRECTORY, "./web/dist")
        self.assertEqual(
            set(module.NODE_CLASS_MAPPINGS),
            {
                "CompileFont",
                "FontPreview",
                "FontSpecimen",
                "ComfyFontDrawBot",
                "ForkFont",
                "ApplyGlyphCandidates",
                "GlyphCandidateBuilder",
                "BuildGlyphTraceRequest",
                "TraceToCandidate",
                "TraceWithQuiverAI",
                "TraceWithComfyCloudQuiverAI",
                "TraceLocalMaskToCandidate",
                "ScoreCandidate",
                "Runebender",
                "DesignBot",
            },
        )
        self.assertEqual(
            set(module.NODE_DISPLAY_NAME_MAPPINGS),
            set(module.NODE_CLASS_MAPPINGS),
        )
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["FontSpecimen"], "DrawBot Skia")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["ApplyGlyphCandidates"], "Apply Glyph Candidates")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["GlyphCandidateBuilder"], "Glyph Candidate Builder")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["BuildGlyphTraceRequest"], "Build Glyph Trace Request")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["TraceToCandidate"], "Trace To Candidate")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["TraceWithQuiverAI"], "Trace With QuiverAI")
        self.assertEqual(
            module.NODE_DISPLAY_NAME_MAPPINGS["TraceWithComfyCloudQuiverAI"],
            "Trace With Comfy Cloud QuiverAI",
        )
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["TraceLocalMaskToCandidate"], "Trace Local Mask To Candidate")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["ScoreCandidate"], "Score Candidate")
        self.assertEqual(module.NODE_DISPLAY_NAME_MAPPINGS["ComfyFontDrawBot"], "DrawBot Skia (legacy)")
        self.assertTrue(module.NODE_CLASS_MAPPINGS["ComfyFontDrawBot"].DEPRECATED)
        self.assertTrue(all(cls.CATEGORY.startswith("Runebender") for cls in module.NODE_CLASS_MAPPINGS.values()))

    def test_comfy_web_directory_points_to_built_dist(self) -> None:
        init_ast = ast.parse((ROOT / "__init__.py").read_text(encoding="utf-8"))
        web_directory = None
        for node in init_ast.body:
            if (
                isinstance(node, ast.Assign)
                and any(isinstance(target, ast.Name) and target.id == "WEB_DIRECTORY" for target in node.targets)
                and isinstance(node.value, ast.Constant)
            ):
                web_directory = node.value.value

        self.assertEqual(web_directory, "./web/dist")
        dist = (ROOT / str(web_directory)).resolve()
        self.assertTrue(dist.is_dir())
        self.assertTrue((dist / "runebender-comfy.js").is_file())
        self.assertTrue((dist / "style.css").is_file())

    def test_built_bundle_registers_comfy_extension(self) -> None:
        dist = ROOT / "web" / "dist"
        bundle = (dist / "runebender-comfy.js").read_text(encoding="utf-8")

        # The bundle registers itself as a ComfyUI extension against the
        # host-provided app singleton.
        self.assertIn('from "/scripts/app.js"', bundle)
        self.assertIn("registerExtension", bundle)
        self.assertIn("runebender-comfy.Runebender", bundle)
        self.assertIn("Clear Font Sources", bundle)
        self.assertIn("Trace Image", bundle)
        self.assertIn("/runebender/workspace/trace_background", bundle)
        self.assertIn("green-reference-overlay", bundle)
        self.assertIn("Green References", bundle)
        self.assertIn("/runebender/workspaces/clear", bundle)
        self.assertIn("All masters", bundle)
        self.assertIn(EXPECTED_BUNDLE_FINGERPRINT, bundle)

        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("clearBackgroundImage();", source)
        host = (ROOT / "web" / "src" / "hosts" / "comfy" / "comfyHost.ts").read_text(encoding="utf-8")
        self.assertIn('body.append("globalFit", "true")', host)
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertNotIn("path_edit_fill", renderer)
        self.assertIn("controls.outline", renderer)
        self.assertIn("geometry.outline = Self::build_outline(path, view);", renderer)
        self.assertIn("path.append_to_bezpath(&mut outline);", renderer)
        self.assertIn("outline.apply_affine(view);", renderer)
        self.assertIn("clear_glyph_geometry_caches", renderer)
        wasm_api = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("self.renderer.clear_glyph_geometry_caches();", wasm_api)

        # The DrawBot script editor loads CodeMirror from vendored assets that
        # Vite copies from web/public/ into dist/vendor/.
        self.assertIn("vendor/codemirror/", bundle)
        self.assertTrue((dist / "vendor" / "codemirror" / "codemirror.js").is_file())
        self.assertIn("__runebenderSyncScriptValue", bundle)
        self.assertIn("setDirtyCanvas", bundle)

        source = (ROOT / "web" / "src" / "extension.ts").read_text(encoding="utf-8")
        self.assertIn("drawbotPresetLoadSerial", source)
        self.assertIn("scriptMatchesBundledPreset", source)
        self.assertIn("presetSourceCache", source)

        # Only the entry module and its stylesheet are emitted at the top level.
        top_level = sorted(p.name for p in dist.iterdir() if p.is_file())
        self.assertEqual(top_level, ["runebender-comfy.js", "style.css"])

    def test_linked_source_smoke_workflow_loads_runebender(self) -> None:
        workflow_path = ROOT / "example_workflows" / "runebender-linked-source-smoke.json"
        workflow = json.loads(workflow_path.read_text(encoding="utf-8"))

        self.assertEqual(workflow["id"], "runebender-linked-source-smoke")
        self.assertEqual(len(workflow["nodes"]), 1)
        node = workflow["nodes"][0]
        self.assertEqual(node["type"], "Runebender")
        self.assertEqual(node["widgets_values"], ["", "auto", ""])
        self.assertEqual([output["type"] for output in node["outputs"]], ["FONT", "STRING"])

    def test_registry_metadata_is_publish_candidate_shape(self) -> None:
        try:
            import tomllib
        except ImportError:
            self.skipTest("tomllib is available in Python 3.11+")

        metadata = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))

        self.assertEqual(metadata["project"]["name"], "runebender-comfy")
        self.assertEqual(metadata["project"]["version"], "0.1.0")
        self.assertEqual(metadata["project"]["description"], "Rust-powered type-design and DrawBot nodes for ComfyUI.")
        self.assertEqual(metadata["project"]["license"], {"file": "LICENSE"})
        self.assertEqual(metadata["project"]["requires-python"], ">=3.10")
        self.assertEqual(metadata["project"]["dynamic"], ["dependencies"])
        self.assertEqual(
            metadata["tool"]["setuptools"]["dynamic"]["dependencies"],
            {"file": ["requirements.txt"]},
        )
        self.assertEqual(metadata["tool"]["comfy"]["PublisherId"], "eliheuer")
        self.assertEqual(metadata["tool"]["comfy"]["DisplayName"], "Runebender")
        self.assertIn("web/dist", metadata["tool"]["comfy"]["includes"])
        self.assertEqual(
            metadata["project"]["urls"]["Repository"],
            "https://github.com/eliheuer/runebender-comfy",
        )
        self.assertTrue(metadata["project"]["urls"]["Documentation"].startswith("https://"))
        self.assertTrue(metadata["project"]["urls"]["Bug Tracker"].startswith("https://"))

    def test_comfyignore_keeps_runtime_files_and_drops_dev_files(self) -> None:
        patterns = _comfyignore_patterns()

        included = [
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
        for path in included:
            with self.subTest(path=path):
                self.assertFalse(_is_comfyignored(path, patterns))

        excluded = [
            ".agents/COMFY_REGISTRY_PUBLISHING.md",
            ".github/workflows/ci.yml",
            "AGENTS.md",
            "CLAUDE.md",
            "rebuild-icons.sh",
            "assets/runebender-icons.ufo/fontinfo.plist",
            "rust-core/src/lib.rs",
            "tests/test_web_bundle.py",
            "tools/check-crate-age/Cargo.toml",
            "web/src/Runebender.vue",
            "web/wasm/runebender_comfy_core.js",
            "web/node_modules/.modules.yaml",
        ]
        for path in excluded:
            with self.subTest(path=path):
                self.assertTrue(_is_comfyignored(path, patterns))

    def test_publish_readiness_checker_reports_known_blockers(self) -> None:
        result = subprocess.run(
            [sys.executable, "scripts/check_comfy_publish_ready.py", "--strict"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 1, result.stderr + result.stdout)
        self.assertIn("Comfy publish readiness: NOT READY", result.stdout)
        self.assertIn("tool.comfy.Icon", result.stdout)
        self.assertIn("tool.comfy.requires-comfyui", result.stdout)
        self.assertIn("PublisherId", result.stdout)
        self.assertIn("DrawBot exec", result.stdout)

    def test_tracing_live_checker_helpers_cover_local_requirements(self) -> None:
        spec = importlib.util.spec_from_file_location(
            "check_tracing_live_ready",
            ROOT / "scripts" / "check_tracing_live_ready.py",
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        sys.modules["check_tracing_live_ready"] = module
        spec.loader.exec_module(module)

        local = module.resolve_local_trace_tool(
            env={"RUNEBENDER_TRACE_TOOL": "future-tracer"},
            home=Path("/missing-home"),
            which=lambda command: "/opt/bin/future-tracer" if command == "future-tracer" else None,
        )
        self.assertEqual(local.status, "PASS")
        self.assertIn("RUNEBENDER_TRACE_TOOL", local.detail)

        cloud = module.check_cloud_key({})
        self.assertEqual(cloud.status, "FAIL")
        self.assertIn("COMFY_CLOUD_API_KEY", cloud.detail)

        install = module.check_custom_node_install("", repo_root=ROOT)
        self.assertEqual(install.status, "INFO")

        with tempfile.TemporaryDirectory() as tmp:
            comfy_root = Path(tmp) / "ComfyUI"
            custom_nodes = comfy_root / "custom_nodes"
            custom_nodes.mkdir(parents=True)
            (custom_nodes / "runebender-comfy").symlink_to(ROOT)
            installed = module.check_custom_node_install(str(comfy_root), repo_root=ROOT)
            self.assertEqual(installed.status, "PASS")
            self.assertIn("points at this checkout", installed.detail)

        mappings = module.check_local_node_mappings(repo_root=ROOT)
        self.assertEqual(mappings.status, "PASS")
        self.assertIn("TraceWithComfyCloudQuiverAI", mappings.detail)

        missing = module.check_registered_nodes({"Runebender": {}})
        self.assertEqual(missing.status, "FAIL")
        self.assertIn("TraceWithComfyCloudQuiverAI", missing.detail)

        registered = module.check_registered_nodes({node: {} for node in module.REQUIRED_TRACE_NODES})
        self.assertEqual(registered.status, "PASS")

    def test_tracing_live_checker_collects_host_failures_without_network(self) -> None:
        spec = importlib.util.spec_from_file_location(
            "check_tracing_live_ready_collect",
            ROOT / "scripts" / "check_tracing_live_ready.py",
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        sys.modules["check_tracing_live_ready_collect"] = module
        spec.loader.exec_module(module)

        with mock.patch.object(module, "resolve_local_trace_tool", return_value=module.LiveCheck("local tracer", "PASS", "ok")), \
             mock.patch.object(module, "check_cloud_key", return_value=module.LiveCheck("Comfy Cloud API key", "FAIL", "missing")), \
             mock.patch.object(module, "load_json_url", side_effect=OSError("offline")):
            checks = module.collect_checks("http://127.0.0.1:8188", timeout_seconds=0.1)

        self.assertEqual([check.status for check in checks], ["PASS", "FAIL", "INFO", "PASS", "FAIL", "FAIL"])
        self.assertIn("ComfyUI host", checks[4].name)
        self.assertIn("object_info", checks[5].detail)

        with mock.patch.object(module, "resolve_local_trace_tool", return_value=module.LiveCheck("local tracer", "PASS", "ok")), \
             mock.patch.object(module, "check_cloud_key", return_value=module.LiveCheck("Comfy Cloud API key", "FAIL", "missing")), \
             mock.patch.object(module, "load_json_url", side_effect=OSError("offline")):
            local_checks = module.collect_checks(
                "http://127.0.0.1:8188",
                timeout_seconds=0.1,
                local_only=True,
            )

        self.assertNotIn("Comfy Cloud API key", [check.name for check in local_checks])
        self.assertEqual([check.status for check in local_checks], ["PASS", "INFO", "PASS", "FAIL", "FAIL"])

    def test_requirements_use_runebender_drawbot_skia_fork(self) -> None:
        requirement_lines = [
            line.strip()
            for line in (ROOT / "requirements.txt").read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        requirements = "\n".join(requirement_lines)

        self.assertIn(
            "drawbot-skia @ git+https://github.com/eliheuer/drawbot-skia.git",
            requirements,
        )
        self.assertFalse(any(line.startswith(("-e ", "--editable")) for line in requirement_lines))
        self.assertFalse(any(line.startswith((".", "/", "~")) for line in requirement_lines))
        self.assertFalse(any(" @ file:" in line for line in requirement_lines))
        self.assertFalse(any("git+" in line and "git+https://" not in line for line in requirement_lines))

    def test_runebender_icons_have_stable_pua_codepoints(self) -> None:
        manifest = json.loads(
            (ROOT / "assets" / "runebender-icons.codepoints.json").read_text(encoding="utf-8")
        )
        glyphs_dir = ROOT / "assets" / "runebender-icons.ufo" / "glyphs"
        contents = plistlib.loads(
            (glyphs_dir / "contents.plist").read_bytes()
        )

        self.assertEqual(len(manifest), 26)
        self.assertEqual(
            manifest,
            {
                "select": "E000",
                "pen": "E001",
                "hyperpen": "E002",
                "knife": "E003",
                "measure": "E004",
                "shapes": "E005",
                "preview": "E006",
                "text": "E007",
                "shape-rectangle": "E010",
                "shape-ellipse": "E011",
                "text-ltr": "E012",
                "text-rtl": "E013",
                "flip-h": "E020",
                "flip-v": "E021",
                "rot-cw": "E022",
                "rot-ccw": "E023",
                "duplicate": "E024",
                "duplicate-repeat": "E025",
                "union": "E030",
                "subtract": "E031",
                "intersect": "E032",
                "exclude": "E033",
                "glyph-grid": "E040",
                "save": "E050",
                "save-as": "E051",
                "close": "E052",
            },
        )
        seen: set[int] = set()
        for glyph_name, value in manifest.items():
            codepoint = int(value, 16)
            with self.subTest(glyph_name=glyph_name):
                self.assertGreaterEqual(codepoint, 0xE000)
                self.assertLessEqual(codepoint, 0xF8FF)
                self.assertNotIn(codepoint, seen)
                self.assertIn(glyph_name, contents)
                glif = (glyphs_dir / contents[glyph_name]).read_text(encoding="utf-8")
                self.assertIn(f'<unicode hex="{value}"/>', glif)
                seen.add(codepoint)

    def test_glyph_grid_orders_encoded_glyphs_by_codepoint(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        sidebar = (ROOT / "web" / "src" / "components" / "CategorySidebar.vue").read_text(
            encoding="utf-8"
        )

        self.assertIn("function compareGlyphNamesByCodepoint", source)
        self.assertIn("data.glyphUnicodes", source)
        self.assertIn("aCodepoint - bCodepoint", source)
        self.assertIn('const glyphSortMode = ref<GlyphSortMode>("unicode")', source)
        self.assertIn('glyphSortMode.value === "unicode"', source)
        self.assertIn("v-model:sort-mode", source)
        self.assertIn("sortMode: GlyphSortMode", sidebar)
        self.assertIn("Glyph sort order", sidebar)
        self.assertNotIn(
            "Array.from(activeMasterData.value.glyphBytes.keys()).sort()",
            source,
        )

    def test_category_sidebar_copies_selected_glyph_text(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        sidebar = (ROOT / "web" / "src" / "components" / "CategorySidebar.vue").read_text(
            encoding="utf-8"
        )

        self.assertIn("selectedTextGlyphCount: number", sidebar)
        self.assertIn('e: "copySelectedText"', sidebar)
        self.assertIn("Copy Selection", sidebar)
        self.assertIn("function selectedGridGlyphNamesInVisibleOrder", source)
        self.assertIn("filteredGlyphNames.value.filter", source)
        self.assertIn("selectedGridGlyphTextPieces", source)
        self.assertIn("writeTextToClipboard(text)", source)
        self.assertIn("@copy-selected-text", source)

    def test_save_does_not_abort_on_noop_editor_flush(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        self.assertIn("const needsEditorFlush =", source)
        self.assertIn("flushDeferredGlyphSync();", source)
        self.assertIn("if (editorGlyphNeedsSync) {", source)
        self.assertNotIn(
            "currentGlyph.value && editor && !flushDeferredGlyphSync()",
            source,
        )

    def test_nudge_preview_defers_coordinate_panel_until_after_paint(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        self.assertIn("function schedulePostPaintNudgeSelectionState", source)
        self.assertIn("postPaintNudgeSelectionRaf = requestAnimationFrame", source)
        self.assertIn("postPaintNudgeSelectionTimer = window.setTimeout", source)
        self.assertIn("flushPostPaintNudgeSelectionState()", source)
        self.assertIn(
            "schedulePostPaintNudgeSelectionState(nudgeSelectionState, nudgePerf ?? null)",
            source,
        )

    def test_readme_documents_publish_readiness_gate(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("python3 scripts/check_comfy_publish_ready.py --strict", readme)
        self.assertIn("Strict mode intentionally fails", readme)
        self.assertIn("DrawBot scripting blocker", readme)


def _comfyignore_patterns() -> list[str]:
    return [
        line.strip()
        for line in (ROOT / ".comfyignore").read_text(encoding="utf-8").splitlines()
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


if __name__ == "__main__":
    unittest.main()
