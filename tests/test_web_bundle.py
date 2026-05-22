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
        # New widget labels: combo first showing what's loaded, then
        # import + edit buttons. Matches the comfyfont-style UX the
        # user requested.
        self.assertIn("Font Source", bundle)
        self.assertIn("Import Font Source", bundle)
        self.assertIn("Edit Font Source", bundle)
        self.assertNotIn("Open Font Source", bundle)
        self.assertNotIn("Import Copy Folder...", bundle)
        self.assertNotIn("Link Source Path...", bundle)
        self.assertNotIn("Import Copy File...", bundle)
        self.assertNotIn("Refresh Workspaces", bundle)
        self.assertIn("Close editor", bundle)
        self.assertIn("font input disconnect requested", bundle)
        self.assertIn("runebender/link_source", bundle)
        self.assertIn("workspace/invalidate", bundle)
        self.assertIn("rb-bundle-2026-05-21-grid-uniform-scale-27", bundle)
        # Grid thumbnail SVGs must come from one batched WASM call
        # (glifMapToSvgs) not 600+ per-glyph crossings.
        self.assertIn("glifMapToSvgs", bundle)
        # Console mirror posts [runebender...] messages to /runebender/log
        # so they show up in the ComfyUI terminal too.
        self.assertIn("/runebender/log", bundle)
        # Latin specimen: uppercase + lowercase + numerals as one
        # string. Backend auto-wraps to maximize per-glyph scale.
        self.assertIn("ABCDEFGHIJKLMNOPQRSTUVWXYZ", bundle)
        self.assertIn("abcdefghijklmnopqrstuvwxyz", bundle)
        self.assertIn("0123456789", bundle)
        # Specimen preview must mount as a DOM widget (<img> via
        # addDOMWidget), v1's actually-supported way to put HTML
        # inside a node body. Canvas-paint approaches do NOT work on
        # custom nodes in v1.
        self.assertIn("addDOMWidget", bundle)
        self.assertIn("Font specimen", bundle)
        self.assertIn("onConfigure", bundle)
        self.assertIn("preview request", bundle)
        self.assertIn("visible:", bundle)
        self.assertIn("stored:", bundle)
        self.assertIn("upstream:", bundle)
        self.assertIn("Link source path", bundle)
        self.assertIn("Save workspace as", bundle)
        self.assertIn("Managed copy (workspace cache)", bundle)
        self.assertIn("runebender/workspace/save_as", bundle)
        self.assertIn("Choose File...", bundle)
        self.assertIn("Choose Folder...", bundle)
        self.assertIn("runebender/choose_source", bundle)
        self.assertIn("showDirectoryPicker", bundle)
        self.assertIn("Link source path", bundle)
        self.assertIn("Save workspace as", bundle)
        self.assertIn("Managed copy (workspace cache)", bundle)
        self.assertIn("runebender/workspace/save_as", bundle)
        self.assertIn("Choose File...", bundle)
        self.assertIn("Choose Folder...", bundle)
        self.assertIn("runebender/choose_source", bundle)
        self.assertIn("showDirectoryPicker", bundle)
        self.assertNotIn("window.prompt", bundle)
        self.assertIn("Designspace", bundle)
        self.assertIn("Fully restart ComfyUI", bundle)
        self.assertIn("console.info(`[runebender-comfy] loaded", bundle)
        self.assertIn("JSON.stringify(", bundle)
        self.assertIn("glyph_data", bundle)
        self.assertNotIn("onConnectionsChange", bundle)
        self.assertNotIn("process.env.NODE_ENV", bundle)

        dist_files = sorted(p.name for p in (ROOT / "web" / "dist").iterdir() if p.is_file())
        self.assertEqual(dist_files, ["runebender-comfy.js", "style.css"])

    def test_linked_source_smoke_workflow_loads_runebender(self) -> None:
        workflow_path = ROOT / "example_workflows" / "runebender-linked-source-smoke.json"
        workflow = json.loads(workflow_path.read_text(encoding="utf-8"))

        self.assertEqual(workflow["id"], "runebender-linked-source-smoke")
        self.assertEqual(len(workflow["nodes"]), 1)
        node = workflow["nodes"][0]
        self.assertEqual(node["type"], "Runebender")
        self.assertEqual(node["widgets_values"], ["demo", "auto", ""])
        self.assertEqual([output["type"] for output in node["outputs"]], ["FONT", "STRING"])


if __name__ == "__main__":
    unittest.main()
