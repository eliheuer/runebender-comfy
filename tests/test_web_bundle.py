from __future__ import annotations

import ast
import importlib.util
import json
import sys
import types
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


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
        self.assertIn("candidate_name", bundle)
        self.assertIn("/runebender/workspaces/clear", bundle)
        self.assertIn("All masters", bundle)

        # The DrawBot script editor loads CodeMirror from vendored assets that
        # Vite copies from web/public/ into dist/vendor/.
        self.assertIn("vendor/codemirror/", bundle)
        self.assertTrue((dist / "vendor" / "codemirror" / "codemirror.js").is_file())

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


if __name__ == "__main__":
    unittest.main()
