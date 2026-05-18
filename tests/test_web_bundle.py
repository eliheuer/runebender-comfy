from __future__ import annotations

import ast
import importlib.util
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
            {"CompileFont", "FontPreview", "FontSpecimen", "ForkFont", "Runebender", "DesignBot"},
        )
        self.assertEqual(
            set(module.NODE_DISPLAY_NAME_MAPPINGS),
            set(module.NODE_CLASS_MAPPINGS),
        )
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

    def test_built_bundle_contains_comfy_extension_contract(self) -> None:
        bundle = (ROOT / "web" / "dist" / "runebender-comfy.js").read_text(encoding="utf-8")

        self.assertIn('from "/scripts/app.js"', bundle)
        self.assertIn("registerExtension", bundle)
        self.assertIn("runebender-comfy.Runebender", bundle)
        self.assertIn("Import Folder...", bundle)
        self.assertIn("Import File...", bundle)
        self.assertIn("Refresh Workspaces", bundle)
        self.assertIn("Close editor", bundle)
        self.assertIn("font input disconnect requested", bundle)
        self.assertIn("workspace/invalidate", bundle)
        self.assertIn("rb-bundle-2026-05-18-close-in-toolbar", bundle)
        self.assertIn("console.info(`[runebender-comfy] loaded", bundle)
        self.assertIn("JSON.stringify(", bundle)
        self.assertIn("glyph_data", bundle)
        self.assertNotIn("onConnectionsChange", bundle)
        self.assertNotIn("process.env.NODE_ENV", bundle)

        dist_files = sorted(p.name for p in (ROOT / "web" / "dist").iterdir() if p.is_file())
        self.assertEqual(dist_files, ["runebender-comfy.js", "style.css"])


if __name__ == "__main__":
    unittest.main()
