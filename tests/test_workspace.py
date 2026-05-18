from __future__ import annotations

import asyncio
import json
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

from nodes import workspace


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

from nodes.runebender import RUNEBENDER_STATE, Runebender
from nodes.font import Font
from nodes.compile_font import CompileFont
from nodes.font_preview import FontPreview
from nodes.font_specimen import FontSpecimen
from nodes.fork_font import ForkFont
from nodes.designbot import DesignBot, _script_for_render


class WorkspaceTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        self.tmp.cleanup()

    def _make_slot(self, slot_name: str = "demo") -> Path:
        slot_dir = workspace.FONTS_DIR / slot_name
        slot_dir.mkdir(parents=True, exist_ok=True)

        designspace = slot_dir / "Demo.designspace"
        designspace.write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5.0\">
  <sources />
</designspace>
""",
            encoding="utf-8",
        )

        ufo_dir = slot_dir / "Demo.ufo"
        glyphs_dir = ufo_dir / "glyphs"
        glyphs_dir.mkdir(parents=True, exist_ok=True)
        (ufo_dir / "metainfo.plist").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<plist version=\"1.0\"><dict/></plist>
""",
            encoding="utf-8",
        )
        (glyphs_dir / "A_.glif").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<glyph name=\"A\" format=\"2\">
  <advance width=\"600\"/>
</glyph>
""",
            encoding="utf-8",
        )
        workspace._write_manifest(slot_dir, {"source_kind": "ufo/designspace"})  # type: ignore[attr-defined]
        return slot_dir

    def test_export_glyphspackage_writes_sources_tree_and_config(self) -> None:
        slot_dir = self._make_slot()
        slot_info = workspace.slot_from_name("demo")
        self.assertIsNotNone(slot_info)

        package_dir = workspace.export_glyphspackage(slot_dir, slot_info)  # type: ignore[arg-type]

        self.assertTrue((package_dir / "sources" / "config.yaml").exists())
        self.assertTrue((package_dir / "sources" / "Demo.designspace").exists())
        self.assertTrue((package_dir / "sources" / "Demo.ufo" / "glyphs" / "A_.glif").exists())

        config_text = (package_dir / "sources" / "config.yaml").read_text(encoding="utf-8")
        self.assertIn("name: demo", config_text)
        self.assertIn("source_kind: ufo/designspace", config_text)
        self.assertIn("- sources/Demo.designspace", config_text)
        self.assertIn("- sources/Demo.ufo", config_text)

        manifest = json.loads((package_dir / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest["source_slot"], "demo")
        self.assertEqual(manifest["sources_root"], "sources")

    def test_compile_slot_records_compiled_artifact(self) -> None:
        self._make_slot()

        def fake_run(cmd, check, cwd, **kwargs):
            self.assertTrue(check)
            self.assertTrue(kwargs["capture_output"])
            self.assertTrue(kwargs["text"])
            self.assertEqual(Path(cwd).resolve(), (workspace.FONTS_DIR / "demo").resolve())
            self.assertEqual(Path(cmd[1]).name, "Demo.designspace")
            out_index = cmd.index("--output-file") + 1
            out_path = Path(cmd[out_index])
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(b"dummy font")
            return mock.Mock(returncode=0)

        with mock.patch.object(workspace.shutil, "which", return_value="/usr/bin/fontc"), \
             mock.patch.object(workspace.subprocess, "run", side_effect=fake_run):
            compiled = workspace.compile_slot("demo", force=True)

        self.assertTrue(compiled.exists())
        self.assertEqual(compiled.read_bytes(), b"dummy font")

        manifest = json.loads((workspace.FONTS_DIR / "demo" / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest["compile_backend"], "fontc")
        self.assertIn("compiled_path", manifest)
        self.assertIn("package_path", manifest)
        self.assertTrue((workspace.FONTS_DIR / "demo" / manifest["package_path"]).exists())

    def test_compile_slot_rebuilds_package_when_no_compiled_artifact(self) -> None:
        slot_dir = self._make_slot()
        slot_info = workspace.slot_from_name("demo")
        self.assertIsNotNone(slot_info)
        package_dir = workspace.export_glyphspackage(slot_dir, slot_info)  # type: ignore[arg-type]
        packaged_glif = package_dir / "sources" / "Demo.ufo" / "glyphs" / "A_.glif"
        self.assertIn('width="600"', packaged_glif.read_text(encoding="utf-8"))

        (slot_dir / "Demo.ufo" / "glyphs" / "A_.glif").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<glyph name=\"A\" format=\"2\">
  <advance width=\"720\"/>
</glyph>
""",
            encoding="utf-8",
        )

        def fake_run(cmd, check, cwd, **kwargs):
            self.assertTrue(check)
            self.assertTrue(kwargs["capture_output"])
            self.assertTrue(kwargs["text"])
            self.assertEqual(Path(cmd[1]).name, "Demo.designspace")
            self.assertIn('width="720"', packaged_glif.read_text(encoding="utf-8"))
            out_index = cmd.index("--output-file") + 1
            out_path = Path(cmd[out_index])
            out_path.write_bytes(b"fresh font")
            return mock.Mock(returncode=0)

        with mock.patch.object(workspace.shutil, "which", return_value="/usr/bin/fontc"), \
             mock.patch.object(workspace.subprocess, "run", side_effect=fake_run):
            workspace.compile_slot("demo")

    def test_compile_slot_surfaces_fontc_diagnostics(self) -> None:
        self._make_slot()

        def fake_run(cmd, check, cwd, **kwargs):
            raise subprocess.CalledProcessError(
                returncode=1,
                cmd=cmd,
                stderr="source validation failed",
                output="fontc details",
            )

        with mock.patch.object(workspace.shutil, "which", return_value="/usr/bin/fontc"), \
             mock.patch.object(workspace.subprocess, "run", side_effect=fake_run):
            with self.assertRaisesRegex(
                RuntimeError,
                "fontc failed for workspace slot 'demo':\\nsource validation failed\\nfontc details",
            ):
                workspace.compile_slot("demo", force=True)

    def test_designspace_import_copies_referenced_ufo_sources(self) -> None:
        source_dir = Path(self.tmp.name) / "sources"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "contents.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        designspace = source_dir / "Demo.designspace"
        designspace.write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5\">
  <sources>
    <source name=\"Regular\" filename=\"Demo.ufo\"/>
  </sources>
</designspace>
""",
            encoding="utf-8",
        )

        slot = workspace.create_slot_from_path(str(designspace), "designspace-import")
        slot_dir = workspace.FONTS_DIR / slot

        self.assertTrue((slot_dir / "Demo.designspace").exists())
        self.assertTrue((slot_dir / "Demo.ufo" / "glyphs" / "A_.glif").exists())

    def test_locate_source_root_finds_nested_designspace(self) -> None:
        project_root = Path(self.tmp.name) / "project"
        nested = project_root / "nested"
        nested.mkdir(parents=True)
        designspace = nested / "Demo.designspace"
        designspace.write_text("<designspace/>", encoding="utf-8")

        found = workspace.locate_source_root(project_root)

        self.assertIsNotNone(found)
        self.assertEqual(found.resolve(), designspace.resolve())

    def test_locate_source_root_accepts_ufo_directory_root(self) -> None:
        ufo_root = Path(self.tmp.name) / "Demo.ufo"
        (ufo_root / "glyphs").mkdir(parents=True)

        found = workspace.locate_source_root(ufo_root)

        self.assertIsNotNone(found)
        self.assertEqual(found.resolve(), ufo_root.resolve())

    def test_glyphs_import_normalizes_to_ufo_designspace_when_glyphslib_exists(self) -> None:
        source = Path(self.tmp.name) / "Demo.glyphs"
        source.write_text("glyphs source", encoding="utf-8")

        def fake_build_masters(_source: str, build_dir: str):
            build_root = Path(build_dir)
            designspace = build_root / "Demo.designspace"
            designspace.write_text("<designspace/>", encoding="utf-8")
            glyphs_dir = build_root / "Demo.ufo" / "glyphs"
            glyphs_dir.mkdir(parents=True)
            (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
            return (None, str(designspace))

        fake_glyphslib = types.SimpleNamespace(build_masters=fake_build_masters)
        with mock.patch.dict(sys.modules, {"glyphsLib": fake_glyphslib}):
            slot = workspace.create_slot_from_path(str(source), "glyphs-demo", source_kind="glyphs")

        slot_dir = workspace.FONTS_DIR / slot
        manifest = json.loads((slot_dir / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        slot_info = workspace.slot_from_name(slot)

        self.assertEqual(manifest["source_kind"], "ufo/designspace")
        self.assertTrue((slot_dir / "Demo.glyphs").exists())
        self.assertTrue((slot_dir / "Demo.designspace").exists())
        self.assertTrue((slot_dir / "Demo.ufo" / "glyphs" / "A_.glif").exists())
        self.assertIsNotNone(slot_info)
        self.assertEqual(slot_info.source_path.name, "Demo.designspace")  # type: ignore[union-attr]

    def test_glyphspackage_source_exports_inner_sources_without_colliding(self) -> None:
        package = Path(self.tmp.name) / "Demo.glyphspackage"
        package_sources = package / "sources"
        glyphs_dir = package_sources / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (package_sources / "Demo.designspace").write_text("<designspace/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")

        slot = workspace.create_slot_from_path(str(package), "demo", source_kind="glyphspackage")
        slot_dir = workspace.FONTS_DIR / slot
        slot_info = workspace.slot_from_name(slot)
        self.assertIsNotNone(slot_info)

        exported = workspace.export_glyphspackage(slot_dir, slot_info)  # type: ignore[arg-type]

        self.assertEqual(exported.name, "demo.fontc.glyphspackage")
        self.assertNotEqual(exported.resolve(), (slot_dir / "Demo.glyphspackage").resolve())
        self.assertTrue((slot_dir / "Demo.glyphspackage").exists())
        self.assertTrue((exported / "sources" / "Demo.designspace").exists())
        self.assertTrue((exported / "sources" / "Demo.ufo" / "glyphs" / "A_.glif").exists())
        config_text = (exported / "sources" / "config.yaml").read_text(encoding="utf-8")
        self.assertIn("source_kind: glyphspackage", config_text)
        self.assertIn("- sources/Demo.designspace", config_text)
        self.assertIn("- sources/Demo.ufo", config_text)

    def test_export_slot_text_files_skips_generated_glyphspackage(self) -> None:
        slot_dir = self._make_slot()
        package_dir = slot_dir / "demo.glyphspackage"
        generated_glif = package_dir / "sources" / "Demo.ufo" / "glyphs" / "A_.glif"
        generated_glif.parent.mkdir(parents=True, exist_ok=True)
        generated_glif.write_text("<glyph name=\"Generated\" format=\"2\"/>", encoding="utf-8")
        (package_dir / workspace.MANIFEST_NAME).write_text("{}", encoding="utf-8")
        workspace._write_manifest(  # type: ignore[attr-defined]
            slot_dir,
            {
                "source_kind": "ufo/designspace",
                "package_path": "demo.glyphspackage",
            },
        )

        paths = {entry["path"] for entry in workspace.export_slot_text_files("demo")}

        self.assertIn("Demo.designspace", paths)
        self.assertIn("Demo.ufo/glyphs/A_.glif", paths)
        self.assertNotIn("demo.glyphspackage/sources/Demo.ufo/glyphs/A_.glif", paths)
        self.assertNotIn("demo.glyphspackage/workspace.json", paths)

    def test_write_workspace_text_file_targets_font_slot_and_invalidates_compile(self) -> None:
        slot_dir = self._make_slot()
        compiled = slot_dir / "demo.ttf"
        compiled.write_bytes(b"old compiled")
        package = slot_dir / "demo.glyphspackage"
        package.mkdir()
        workspace._write_manifest(  # type: ignore[attr-defined]
            slot_dir,
            {
                "source_kind": "ufo/designspace",
                "compiled_path": "demo.ttf",
                "compile_backend": "fontc",
                "package_path": "demo.glyphspackage",
            },
        )

        written = workspace.write_workspace_text_file(
            "demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\" format=\"2\"/>",
        )

        self.assertEqual(
            written.resolve(),
            (slot_dir / "Demo.ufo" / "glyphs" / "A_.glif").resolve(),
        )
        self.assertEqual(written.read_text(encoding="utf-8"), "<glyph name=\"A\" format=\"2\"/>")
        self.assertFalse(compiled.exists())
        self.assertFalse(package.exists())
        manifest = json.loads((slot_dir / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest, {"source_kind": "ufo/designspace"})

    def test_write_workspace_text_file_rejects_parent_traversal(self) -> None:
        self._make_slot()

        with self.assertRaises(ValueError):
            workspace.write_workspace_text_file("demo/../escape.glif", "<glyph/>")

    def test_fork_font_generates_unique_default_name(self) -> None:
        self._make_slot("demo")

        first, = ForkFont().run("demo", "")
        second, = ForkFont().run("demo", "")

        self.assertEqual(first, "demo-fork")
        self.assertEqual(second, "demo-fork-002")
        self.assertTrue((workspace.FONTS_DIR / first / "Demo.designspace").exists())
        self.assertTrue((workspace.FONTS_DIR / second / "Demo.designspace").exists())


class RunebenderNodeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        RUNEBENDER_STATE.clear()
        self.tmp.cleanup()

    def test_runebender_live_state_overrides_everything(self) -> None:
        RUNEBENDER_STATE["42"] = {
            "font": "edited-slot",
            "glyph_data": "<svg><path d='M0 0Z'/></svg>",
        }

        font, glyph_svg = Runebender().run(
            source_path="demo",
            font="wired-slot",
            unique_id="42",
        )

        self.assertEqual(font, "edited-slot")
        self.assertEqual(glyph_svg, "<svg><path d='M0 0Z'/></svg>")

    def test_runebender_uses_wired_font_when_no_live_state(self) -> None:
        font, glyph_svg = Runebender().run(
            source_path="demo",
            font="wired-slot",
            unique_id="missing",
        )

        self.assertEqual(font, "wired-slot")
        self.assertEqual(glyph_svg, "")

    def test_runebender_falls_back_to_existing_workspace_slot(self) -> None:
        slot_dir = workspace.FONTS_DIR / "imported-demo"
        slot_dir.mkdir(parents=True, exist_ok=True)
        (slot_dir / "Demo.designspace").write_text("<designspace/>", encoding="utf-8")
        (slot_dir / workspace.MANIFEST_NAME).write_text(
            json.dumps({"source_kind": "ufo/designspace"}) + "\n",
            encoding="utf-8",
        )

        font, glyph_svg = Runebender().run(
            source_path="imported-demo",
            unique_id="missing",
        )

        self.assertEqual(font, "imported-demo")
        self.assertEqual(glyph_svg, "")


class FontNodeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        self.tmp.cleanup()

    def test_source_kind_is_constrained_to_supported_options(self) -> None:
        input_types = Font.INPUT_TYPES()

        self.assertEqual(input_types["required"]["source_path"][0], "STRING")
        self.assertEqual(
            input_types["optional"]["source_kind"],
            (
                ("auto", "ufo/designspace", "glyphs", "glyphspackage"),
                {
                    "default": "auto",
                    "tooltip": "Auto-detect from the file extension unless you need to override it.",
                    "advanced": True,
                },
            ),
        )
        self.assertEqual(
            input_types["optional"]["workspace_name"],
            (
                "STRING",
                {
                    "multiline": False,
                    "default": "",
                    "tooltip": "Leave blank to auto-name the workspace from the source file.",
                    "advanced": True,
                },
            ),
        )

    def test_loaded_workspace_slot_is_returned_without_reimporting(self) -> None:
        slot_dir = workspace.FONTS_DIR / "imported-demo"
        slot_dir.mkdir(parents=True, exist_ok=True)
        (slot_dir / "Demo.designspace").write_text("<designspace/>", encoding="utf-8")
        (slot_dir / workspace.MANIFEST_NAME).write_text(
            json.dumps({"source_kind": "ufo/designspace"}) + "\n",
            encoding="utf-8",
        )

        slot, = Font().run("imported-demo", "auto", "")

        self.assertEqual(slot, "imported-demo")


class FontImportRouteTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        self.tmp.cleanup()

    @staticmethod
    def _request_with_files(files, data=None):
        payload = dict(data or {})

        class _PostData(dict):
            def getall(self, key):
                value = self.get(key, [])
                return value if isinstance(value, list) else [value]

        class _Request:
            async def post(self):
                form = _PostData(payload)
                form["file"] = files
                return form

        return _Request()

    def test_import_font_route_accepts_nested_folder_upload(self) -> None:
        from nodes.font import import_font

        class _UploadedFile:
            def __init__(self, filename: str, content: bytes):
                self.filename = filename
                self.file = types.SimpleNamespace(read=lambda: content)

        files = [
            _UploadedFile(
                "project/Demo.designspace",
                b'<?xml version="1.0" encoding="UTF-8"?><designspace format="5"><sources/></designspace>',
            ),
            _UploadedFile(
                "project/Demo.ufo/glyphs/A_.glif",
                b'<?xml version="1.0" encoding="UTF-8"?><glyph name="A" format="2"><advance width="600"/></glyph>',
            ),
        ]
        request = self._request_with_files(files, {"workspace_name": "demo-import", "source_kind": "auto"})

        payload = asyncio.run(import_font(request))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["slot"], "demo-import")
        self.assertEqual(payload["source_root"], "project/Demo.designspace")
        self.assertTrue((workspace.FONTS_DIR / "demo-import" / "Demo.designspace").exists())
        self.assertTrue((workspace.FONTS_DIR / "demo-import" / "Demo.ufo" / "glyphs" / "A_.glif").exists())

    def test_import_font_route_accepts_single_file_upload(self) -> None:
        from nodes.font import import_font

        class _UploadedFile:
            def __init__(self, filename: str, content: bytes):
                self.filename = filename
                self.file = types.SimpleNamespace(read=lambda: content)

        files = [
            _UploadedFile(
                "Demo.designspace",
                b'<?xml version="1.0" encoding="UTF-8"?><designspace format="5"><sources/></designspace>',
            ),
        ]
        request = self._request_with_files(files, {"workspace_name": "", "source_kind": "auto"})
        payload = asyncio.run(import_font(request))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["source_root"], "Demo.designspace")
        self.assertTrue((workspace.FONTS_DIR / "Demo" / "Demo.designspace").exists())


class WorkspaceInvalidateRouteTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        self.tmp.cleanup()

    def test_invalidate_workspace_route_removes_compiled_artifact(self) -> None:
        from nodes.runebender import invalidate_workspace_file

        slot_dir = workspace.FONTS_DIR / "demo"
        glyphs_dir = slot_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True, exist_ok=True)
        (slot_dir / "Demo.designspace").write_text("<designspace/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        compiled = slot_dir / "demo.ttf"
        compiled.write_bytes(b"compiled")
        package = slot_dir / "demo.glyphspackage"
        package.mkdir()
        workspace._write_manifest(  # type: ignore[attr-defined]
            slot_dir,
            {
                "source_kind": "ufo/designspace",
                "compiled_path": "demo.ttf",
                "compile_backend": "fontc",
                "package_path": "demo.glyphspackage",
            },
        )

        class _Request:
            async def post(self):
                return {"path": "demo/Demo.ufo/glyphs/A_.glif"}

        payload = asyncio.run(invalidate_workspace_file(_Request()))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["path"], "demo/Demo.ufo/glyphs/A_.glif")
        self.assertFalse(compiled.exists())
        self.assertFalse(package.exists())
        manifest = json.loads((slot_dir / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest, {"source_kind": "ufo/designspace"})


class FontPreviewNodeTests(unittest.TestCase):
    def test_font_preview_module_imports_without_image_stack(self) -> None:
        input_types = FontPreview.INPUT_TYPES()

        self.assertEqual(input_types["required"]["font"], ("FONT",))
        self.assertEqual(FontPreview.RETURN_TYPES, ("IMAGE",))


class FontSpecimenNodeTests(unittest.TestCase):
    def test_font_specimen_exposes_scriptable_image_and_mask_outputs(self) -> None:
        input_types = FontSpecimen.INPUT_TYPES()

        self.assertEqual(input_types["required"]["font"], ("FONT",))
        self.assertEqual(FontSpecimen.RETURN_TYPES, ("IMAGE", "MASK"))
        self.assertIn("custom_script", input_types["optional"])

    def test_font_specimen_renders_image_and_mask_tensors(self) -> None:
        class _FakeImg:
            def convert(self, _mode):
                return self

            def getchannel(self, _name):
                return self

        class _FakeDraw:
            def textbbox(self, _xy, _text, font=None):
                return (0, 0, 64, 32)

            def text(self, *_args, **_kwargs):
                return None

            def multiline_text(self, *_args, **_kwargs):
                return None

            def rectangle(self, *_args, **_kwargs):
                return None

        class _FakeImageModule:
            @staticmethod
            def new(_mode, _size, _color):
                return _FakeImg()

            @staticmethod
            def alpha_composite(_base, _overlay):
                return _FakeImg()

        class _FakeImageDrawModule:
            @staticmethod
            def Draw(_img):
                return _FakeDraw()

        class _FakeImageFontModule:
            @staticmethod
            def truetype(_path, size=None):
                return object()

            @staticmethod
            def load_default():
                return object()

        class _FakeTorch:
            @staticmethod
            def from_numpy(_arr):
                return "tensor"

        with mock.patch("nodes.font_specimen.compiled_path", return_value=Path("/tmp/font.ttf")), \
             mock.patch("nodes.font_specimen._image_stack", return_value=(object(), _FakeImageModule, _FakeImageDrawModule, _FakeImageFontModule, _FakeTorch)), \
             mock.patch("nodes.font_specimen._image_to_tensor", return_value="tensor"), \
             mock.patch("nodes.font_specimen._mask_to_tensor", return_value="tensor"):
            image, mask = FontSpecimen().run("demo", "glyph", "A", 256, 128)

        self.assertEqual(image, "tensor")
        self.assertEqual(mask, "tensor")


class LocalWorkflowSmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_workspace_dir = workspace.WORKSPACE_DIR
        self._old_fonts_dir = workspace.FONTS_DIR
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        workspace.WORKSPACE_DIR = root / "workspace"
        workspace.FONTS_DIR = workspace.WORKSPACE_DIR / "fonts"

    def tearDown(self) -> None:
        workspace.WORKSPACE_DIR = self._old_workspace_dir
        workspace.FONTS_DIR = self._old_fonts_dir
        RUNEBENDER_STATE.clear()
        self.tmp.cleanup()

    def _make_source(self) -> Path:
        source_dir = Path(self.tmp.name) / "source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True, exist_ok=True)
        (source_dir / "Demo.designspace").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5.0\">
  <sources>
    <source name=\"Regular\" filename=\"Demo.ufo\"/>
  </sources>
</designspace>
""",
            encoding="utf-8",
        )
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<glyph name=\"A\" format=\"2\">
  <advance width=\"600\"/>
</glyph>
""",
            encoding="utf-8",
        )
        return source_dir / "Demo.designspace"

    def test_local_font_workflow_chain_round_trips_through_nodes(self) -> None:
        source_path = self._make_source()

        slot, glyph_svg = Runebender().run(
            source_path=str(source_path),
            workspace_name="demo",
            unique_id=None,
        )

        RUNEBENDER_STATE["7"] = {
            "font": slot,
            "glyph_data": "<svg><path d='M0 0Z'/></svg>",
        }
        font, glyph_svg = Runebender().run(
            source_path=slot,
            unique_id="7",
        )

        self.assertEqual(font, slot)
        self.assertEqual(glyph_svg, "<svg><path d='M0 0Z'/></svg>")

        def fake_run(cmd, check, cwd, **kwargs):
            self.assertTrue(check)
            self.assertTrue(kwargs["capture_output"])
            self.assertTrue(kwargs["text"])
            out_index = cmd.index("--output-file") + 1
            out_path = Path(cmd[out_index])
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(b"compiled-font")
            return mock.Mock(returncode=0)

        with mock.patch.object(workspace.shutil, "which", return_value="/usr/bin/fontc"), \
             mock.patch.object(workspace.subprocess, "run", side_effect=fake_run):
            returned_font, = CompileFont().run(font, force=True)

        self.assertEqual(returned_font, slot)
        compiled = workspace.compiled_path(slot)
        self.assertTrue(compiled.exists())

        class _FakeImg:
            def convert(self, _mode):
                return self

        class _FakeDraw:
            def textbbox(self, _xy, _text, font=None):
                return (0, 0, 96, 48)

            def text(self, _xy, _text, fill=None, font=None):
                return None

        class _FakeImageModule:
            @staticmethod
            def new(_mode, _size, _color):
                return _FakeImg()

        class _FakeImageDrawModule:
            @staticmethod
            def Draw(_img):
                return _FakeDraw()

        class _FakeImageFontModule:
            @staticmethod
            def truetype(_path, size=None):
                return object()

            @staticmethod
            def load_default():
                return object()

        class _FakeTorch:
            @staticmethod
            def from_numpy(_arr):
                return "tensor"

        class _FakeNp:
            float32 = object()

            @staticmethod
            def array(_img, dtype=None):
                return object()

        with mock.patch("nodes.font_preview._image_stack", return_value=(_FakeNp, _FakeImageModule, _FakeImageDrawModule, _FakeImageFontModule, _FakeTorch)), \
             mock.patch("nodes.font_preview._pil_to_tensor", return_value="tensor"):
            image, = FontPreview().run(slot, "Aa", 256, 128)

        self.assertEqual(image, "tensor")


class DesignBotNodeTests(unittest.TestCase):
    def test_bare_script_wrapper_uses_requested_canvas_size(self) -> None:
        script = _script_for_render(
            "ctx.rect(10.0, 20.0, 30.0, 40.0);",
            320,
            240,
            Path("/tmp/out.png"),
        )

        self.assertIn("Canvas::new(320.0, 240.0)", script)
        self.assertIn("Renderer::new(320, 240)", script)
        self.assertIn("ctx.rect(10.0, 20.0, 30.0, 40.0);", script)
        self.assertIn('renderer.render_to_png(&ctx, "/tmp/out.png").unwrap();', script)

    def test_designbot_run_invokes_cli_and_returns_tensor(self) -> None:
        calls = []

        def fake_run(cmd, check, cwd):
            self.assertTrue(check)
            calls.append((cmd, Path(cwd), Path(cmd[2]).read_text(encoding="utf-8")))
            output_path = Path(cmd[cmd.index("--output") + 1])
            output_path.write_bytes(b"png")
            return mock.Mock(returncode=0)

        with mock.patch("nodes.designbot._designbot_bin", return_value="/usr/bin/designbot"), \
             mock.patch("nodes.designbot.subprocess.run", side_effect=fake_run), \
             mock.patch("nodes.designbot._png_to_tensor", return_value="tensor"):
            image, = DesignBot().run("ctx.rect(0.0, 0.0, 10.0, 10.0);", 64, 32)

        self.assertEqual(image, "tensor")
        self.assertEqual(len(calls), 1)
        cmd, cwd, script_text = calls[0]
        self.assertEqual(cmd[:2], ["/usr/bin/designbot", "--render"])
        script_path = Path(cmd[2])
        self.assertEqual(script_path.parent, cwd)
        self.assertIn("Renderer::new(64, 32)", script_text)


if __name__ == "__main__":
    unittest.main()
