from __future__ import annotations

import asyncio
import json
import os
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

from nodes import font_preview, workspace

ROOT = Path(__file__).resolve().parents[1]


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
from nodes import font_preview
from nodes.font_preview import FontPreview
from nodes.font_specimen import FontSpecimen, load_presets
from nodes.fork_font import ForkFont
from nodes.apply_glyph_candidates import ApplyGlyphCandidates
from nodes.glyph_candidate_builder import GlyphCandidateBuilder, arabic_glyph_filter, rgba_matches
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

    def test_glyph_candidate_builder_declares_font_output_and_report(self) -> None:
        self.assertEqual(GlyphCandidateBuilder.CATEGORY, "Runebender / Font")
        self.assertEqual(GlyphCandidateBuilder.RETURN_TYPES, ("FONT", "STRING"))
        input_types = GlyphCandidateBuilder.INPUT_TYPES()
        self.assertIn("font", input_types["required"])
        self.assertIn("donor_path", input_types["required"])
        self.assertIn("glyphs", input_types["optional"])

    def test_apply_glyph_candidates_declares_review_apply_inputs(self) -> None:
        self.assertEqual(ApplyGlyphCandidates.CATEGORY, "Runebender / Font")
        self.assertEqual(ApplyGlyphCandidates.RETURN_TYPES, ("FONT", "STRING"))
        input_types = ApplyGlyphCandidates.INPUT_TYPES()
        self.assertIn("candidate_font", input_types["required"])
        self.assertIn("target_font", input_types["required"])
        self.assertIn("glyphs", input_types["optional"])
        self.assertIn("clear_mark_color", input_types["optional"])
        self.assertIn("write_linked_source", input_types["optional"])

    def test_glyph_candidate_builder_color_and_arabic_filters(self) -> None:
        self.assertTrue(rgba_matches("1,0.251,0.251,1", (1.0, 0.3, 0.3, 1.0)))
        self.assertFalse(rgba_matches("0.267,0.733,0.267,1", (1.0, 0.3, 0.3, 1.0)))
        self.assertEqual(
            arabic_glyph_filter(["A", "seen-ar", "dottedCircle", "zeroFarsi-ar"]),
            ["seen-ar", "dottedCircle", "zeroFarsi-ar"],
        )

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

    def test_compile_slot_preserves_linked_source_manifest_provenance(self) -> None:
        source_root = Path(self.tmp.name) / "source"
        glyphs_dir = source_root / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_root / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        designspace = source_root / "Demo.designspace"
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
        slot = workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)

        def fake_run(cmd, check, cwd, **kwargs):
            self.assertTrue(check)
            self.assertTrue(kwargs["capture_output"])
            self.assertTrue(kwargs["text"])
            out_index = cmd.index("--output-file") + 1
            out_path = Path(cmd[out_index])
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(b"dummy font")
            return mock.Mock(returncode=0)

        with mock.patch.object(workspace.shutil, "which", return_value="/usr/bin/fontc"), \
             mock.patch.object(workspace.subprocess, "run", side_effect=fake_run):
            workspace.compile_slot(slot, force=True)

        manifest = json.loads((workspace.FONTS_DIR / slot / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest["origin_mode"], "linked")
        self.assertEqual(Path(manifest["origin_root"]).resolve(), source_root.resolve())
        self.assertEqual(Path(manifest["origin_source"]).resolve(), designspace.resolve())
        self.assertEqual(manifest["compile_backend"], "fontc")

        result = workspace.write_workspace_text_file_with_result(
            "linked-demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\"><advance width=\"620\"/></glyph>",
        )
        self.assertIsNotNone(result.source_path)
        self.assertEqual(result.source_path.resolve(), (source_root / "Demo.ufo" / "glyphs" / "A_.glif").resolve())
        self.assertIn('width="620"', (source_root / "Demo.ufo" / "glyphs" / "A_.glif").read_text(encoding="utf-8"))

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

    def test_designspace_import_copies_nested_referenced_ufo_sources(self) -> None:
        source_dir = Path(self.tmp.name) / "nested-sources"
        glyphs_dir = source_dir / "masters" / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (source_dir / "masters" / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "contents.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        designspace = source_dir / "Demo.designspace"
        designspace.write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5\">
  <sources>
    <source name=\"Regular\" filename=\"masters/Demo.ufo\"/>
  </sources>
</designspace>
""",
            encoding="utf-8",
        )

        slot = workspace.create_slot_from_path(str(designspace), "nested-designspace-import")
        slot_dir = workspace.FONTS_DIR / slot

        self.assertTrue((slot_dir / "Demo.designspace").exists())
        self.assertTrue((slot_dir / "masters" / "Demo.ufo" / "glyphs" / "A_.glif").exists())

    def test_export_glyphspackage_preserves_nested_designspace_source_paths(self) -> None:
        slot_dir = self._make_slot("nested-demo")
        masters_dir = slot_dir / "masters"
        masters_dir.mkdir()
        shutil.move(str(slot_dir / "Demo.ufo"), str(masters_dir / "Demo.ufo"))
        (slot_dir / "Demo.designspace").write_text(
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5\">
  <sources>
    <source name=\"Regular\" filename=\"masters/Demo.ufo\"/>
  </sources>
</designspace>
""",
            encoding="utf-8",
        )
        slot_info = workspace.slot_from_name("nested-demo")
        self.assertIsNotNone(slot_info)

        package_dir = workspace.export_glyphspackage(slot_dir, slot_info)  # type: ignore[arg-type]

        self.assertTrue((package_dir / "sources" / "Demo.designspace").exists())
        self.assertTrue((package_dir / "sources" / "masters" / "Demo.ufo" / "glyphs" / "A_.glif").exists())
        config_text = (package_dir / "sources" / "config.yaml").read_text(encoding="utf-8")
        self.assertIn("- sources/Demo.designspace", config_text)
        self.assertIn("- sources/masters/Demo.ufo", config_text)

    def test_preview_ufo_picker_finds_nested_designspace_masters(self) -> None:
        slot_dir = self._make_slot("nested-preview")
        masters_dir = slot_dir / "masters"
        masters_dir.mkdir()
        shutil.move(str(slot_dir / "Demo.ufo"), str(masters_dir / "Demo.ufo"))

        self.assertEqual(
            font_preview._pick_preview_ufo(slot_dir),  # type: ignore[attr-defined]
            masters_dir / "Demo.ufo",
        )

    def test_linked_designspace_write_mirrors_back_to_original_source(self) -> None:
        source_dir = Path(self.tmp.name) / "linked-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text("<glyph name=\"A\"><advance width=\"600\"/></glyph>", encoding="utf-8")
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

        slot = workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)
        slot_dir = workspace.FONTS_DIR / slot
        workspace_glif = slot_dir / "Demo.ufo" / "glyphs" / "A_.glif"

        result = workspace.write_workspace_text_file_with_result(
            "linked-demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\"><advance width=\"720\"/></glyph>",
        )
        designspace_result = workspace.write_workspace_text_file_with_result(
            "linked-demo/Demo.designspace",
            """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5\">
  <sources>
    <source name=\"Regular\" filename=\"Demo.ufo\"/>
  </sources>
  <axes>
    <axis tag=\"wght\" name=\"Weight\" minimum=\"400\" maximum=\"700\" default=\"400\"/>
  </axes>
</designspace>
""",
        )

        manifest = json.loads((slot_dir / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest["origin_mode"], "linked")
        self.assertEqual(Path(manifest["origin_root"]).resolve(), source_dir.resolve())
        source_info = workspace.source_info_for_slot("linked-demo")
        self.assertTrue(source_info.linked)
        self.assertEqual(source_info.origin_root, source_dir.resolve())
        self.assertEqual(source_info.origin_source, designspace.resolve())
        self.assertEqual(result.workspace_path.resolve(), workspace_glif.resolve())
        self.assertEqual(result.source_path.resolve(), original_glif.resolve())  # type: ignore[union-attr]
        self.assertEqual(designspace_result.source_path.resolve(), designspace.resolve())  # type: ignore[union-attr]
        self.assertIn('width="720"', workspace_glif.read_text(encoding="utf-8"))
        self.assertIn('width="720"', original_glif.read_text(encoding="utf-8"))
        self.assertIn('tag="wght"', designspace.read_text(encoding="utf-8"))

    def test_reopening_same_linked_source_reuses_workspace_slot(self) -> None:
        source_dir = Path(self.tmp.name) / "linked-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        first = workspace.create_slot_from_path(str(designspace), linked=True)
        second = workspace.create_slot_from_path(str(designspace), linked=True)

        self.assertEqual(first, "demo")
        self.assertEqual(second, first)
        self.assertEqual(workspace.list_slots(), [first])
        self.assertEqual(workspace.source_display_label(first), "Demo.designspace")
        self.assertEqual(
            workspace.list_workspace_choices(),
            [
                {
                    "slot": "demo",
                    "label": "Demo.designspace",
                    "origin_source": str(designspace.resolve()),
                }
            ],
        )

    def test_linked_source_refresh_reloads_external_disk_edits(self) -> None:
        source_dir = Path(self.tmp.name) / "linked-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text(
            "<glyph name=\"A\"><advance width=\"600\"/></glyph>",
            encoding="utf-8",
        )
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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
        slot = workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)
        workspace_glif = workspace.FONTS_DIR / slot / "Demo.ufo" / "glyphs" / "A_.glif"
        manifest_path = workspace.FONTS_DIR / slot / workspace.MANIFEST_NAME
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        cached_snapshot = int(manifest["origin_snapshot_mtime_ns"])

        original_glif.write_text(
            "<glyph name=\"A\"><advance width=\"700\"/></glyph>",
            encoding="utf-8",
        )
        os.utime(original_glif, ns=(cached_snapshot + 1_000_000_000, cached_snapshot + 1_000_000_000))

        self.assertTrue(workspace.refresh_linked_slot_from_source_if_newer(slot))

        self.assertIn('width="700"', workspace_glif.read_text(encoding="utf-8"))
        refreshed_manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(refreshed_manifest["origin_mode"], "linked")
        self.assertEqual(Path(refreshed_manifest["origin_source"]).resolve(), designspace.resolve())
        self.assertGreater(int(refreshed_manifest["origin_snapshot_mtime_ns"]), cached_snapshot)

    def test_text_writes_normalize_browser_crlf_to_existing_lf_style(self) -> None:
        source_dir = Path(self.tmp.name) / "linked-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>\n", encoding="utf-8")
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text("<glyph name=\"A\">\n  <advance width=\"600\"/>\n</glyph>\n", encoding="utf-8")
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
        workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)

        workspace.write_workspace_text_file_with_result(
            "linked-demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\">\r\n  <advance width=\"720\"/>\r\n</glyph>\r\n",
        )

        self.assertNotIn(b"\r\n", (workspace.FONTS_DIR / "linked-demo" / "Demo.ufo" / "glyphs" / "A_.glif").read_bytes())
        self.assertNotIn(b"\r\n", original_glif.read_bytes())
        self.assertIn(b'width="720"', original_glif.read_bytes())

    def test_export_slot_to_directory_copies_source_and_can_relink(self) -> None:
        self._make_slot("managed-demo")
        destination = Path(self.tmp.name) / "exported-source"

        result = workspace.export_slot_to_directory(
            "managed-demo",
            str(destination),
            relink=True,
        )

        exported_designspace = destination / "Demo.designspace"
        exported_glif = destination / "Demo.ufo" / "glyphs" / "A_.glif"
        self.assertEqual(result.destination, destination.resolve())
        self.assertTrue(exported_designspace.exists())
        self.assertTrue(exported_glif.exists())
        self.assertTrue(result.linked)
        self.assertEqual(result.origin_root, destination.resolve())
        self.assertEqual(result.origin_source, exported_designspace.resolve())

        write_result = workspace.write_workspace_text_file_with_result(
            "managed-demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\"><advance width=\"755\"/></glyph>",
        )

        self.assertEqual(write_result.source_path, exported_glif.resolve())
        self.assertIn('width="755"', exported_glif.read_text(encoding="utf-8"))

    def test_export_slot_to_directory_without_relink_leaves_original_source_unlinked(self) -> None:
        self._make_slot("managed-demo")
        destination = Path(self.tmp.name) / "exported-copy"

        result = workspace.export_slot_to_directory(
            "managed-demo",
            str(destination),
            relink=False,
        )

        self.assertFalse(result.linked)
        self.assertIsNone(result.origin_source)
        self.assertTrue((destination / "Demo.designspace").exists())
        self.assertTrue((destination / "Demo.ufo" / "glyphs" / "A_.glif").exists())
        write_result = workspace.write_workspace_text_file_with_result(
            "managed-demo/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\"><advance width=\"755\"/></glyph>",
        )
        self.assertIsNone(write_result.source_path)
        self.assertNotIn('width="755"', (destination / "Demo.ufo" / "glyphs" / "A_.glif").read_text(encoding="utf-8"))

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

    def test_runebender_source_path_links_disk_source_for_save_back(self) -> None:
        source_dir = Path(self.tmp.name) / "linked-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text("<glyph name=\"A\"><advance width=\"600\"/></glyph>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        font, glyph_svg = Runebender().run(
            source_path=str(designspace),
            unique_id="missing",
        )

        self.assertEqual(font, "demo")
        self.assertEqual(glyph_svg, "")
        manifest = json.loads((workspace.FONTS_DIR / font / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertEqual(manifest["origin_mode"], "linked")
        self.assertEqual(Path(manifest["origin_root"]).resolve(), source_dir.resolve())

        result = workspace.write_workspace_text_file_with_result(
            f"{font}/Demo.ufo/glyphs/A_.glif",
            "<glyph name=\"A\"><advance width=\"720\"/></glyph>",
        )

        self.assertEqual(result.source_path.resolve(), original_glif.resolve())  # type: ignore[union-attr]
        self.assertIn('width="720"', original_glif.read_text(encoding="utf-8"))

        reopened_font, _reopened_svg = Runebender().run(
            source_path=str(designspace),
            unique_id="missing-again",
        )
        self.assertEqual(reopened_font, font)
        self.assertEqual(workspace.list_slots(), [font])

    def test_runebender_demo_source_stays_managed_copy(self) -> None:
        font, _glyph_svg = Runebender().run(
            source_path="demo",
            unique_id="missing",
        )

        manifest = json.loads((workspace.FONTS_DIR / font / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))
        self.assertNotIn("origin_mode", manifest)


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

    def test_runebender_source_path_defaults_to_placeholder_not_demo_text(self) -> None:
        input_types = Runebender.INPUT_TYPES()

        source_options = input_types["required"]["source_path"][1]
        self.assertEqual(source_options["default"], "")
        self.assertEqual(source_options["placeholder"], "/path/to/font.designspace")

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

    def test_link_source_route_creates_linked_workspace_slot(self) -> None:
        from nodes.font import link_source

        source_dir = Path(self.tmp.name) / "linked"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        class _Request:
            async def post(self):
                return {
                    "source_path": str(designspace),
                    "workspace_name": "linked-demo",
                    "source_kind": "auto",
                }

        payload = asyncio.run(link_source(_Request()))
        manifest = json.loads((workspace.FONTS_DIR / "linked-demo" / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["slot"], "linked-demo")
        self.assertEqual(payload["label"], "Demo.designspace")
        self.assertEqual(Path(payload["origin_source"]).resolve(), designspace.resolve())
        self.assertEqual(manifest["origin_mode"], "linked")
        self.assertEqual(Path(manifest["origin_root"]).resolve(), source_dir.resolve())
        self.assertTrue((workspace.FONTS_DIR / "linked-demo" / "Demo.ufo" / "glyphs" / "A_.glif").exists())

    def test_link_source_route_uses_designspace_extension_over_stale_kind(self) -> None:
        from nodes.font import link_source

        source_dir = Path(self.tmp.name) / "linked-stale-kind"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        class _Request:
            async def post(self):
                return {
                    "source_path": str(designspace),
                    "workspace_name": "linked-stale-kind",
                    "source_kind": "glyphs",
                }

        payload = asyncio.run(link_source(_Request()))
        manifest = json.loads(
            (workspace.FONTS_DIR / "linked-stale-kind" / workspace.MANIFEST_NAME).read_text(encoding="utf-8")
        )

        self.assertTrue(payload["success"])
        self.assertEqual(payload["source_kind"], "ufo/designspace")
        self.assertEqual(manifest["source_kind"], "ufo/designspace")
        self.assertEqual(manifest["origin_kind"], "ufo/designspace")

    def test_workspaces_route_reports_display_labels(self) -> None:
        from nodes.font import list_workspaces

        source_dir = Path(self.tmp.name) / "linked"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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
        workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)

        payload = asyncio.run(list_workspaces(types.SimpleNamespace()))

        self.assertIn({"slot": "demo", "label": "demo", "origin_source": ""}, payload["choices"])
        self.assertIn(
            {
                "slot": "linked-demo",
                "label": "Demo.designspace",
                "origin_source": str(designspace.resolve()),
            },
            payload["choices"],
        )

    def test_clear_workspaces_route_removes_cached_sources_and_returns_demo_choice(self) -> None:
        from nodes.font import clear_workspaces, list_workspaces

        for slot in ("demo", "old-one", "old-two"):
            slot_dir = workspace.FONTS_DIR / slot
            slot_dir.mkdir(parents=True)
            (slot_dir / "Demo.designspace").write_text(
                '<?xml version="1.0" encoding="UTF-8"?><designspace format="5"><sources/></designspace>',
                encoding="utf-8",
            )

        payload = asyncio.run(clear_workspaces(types.SimpleNamespace()))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["deleted"], ["demo", "old-one", "old-two"])
        self.assertFalse((workspace.FONTS_DIR / "demo").exists())
        self.assertFalse((workspace.FONTS_DIR / "old-one").exists())
        self.assertFalse((workspace.FONTS_DIR / "old-two").exists())

        choices = asyncio.run(list_workspaces(types.SimpleNamespace()))
        self.assertEqual(choices["choices"], [{"slot": "demo", "label": "demo", "origin_source": ""}])

    def test_import_source_path_route_creates_managed_copy_slot(self) -> None:
        from nodes.font import import_source_path

        source_dir = Path(self.tmp.name) / "import-copy"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        class _Request:
            async def post(self):
                return {
                    "source_path": str(designspace),
                    "workspace_name": "managed-copy",
                    "source_kind": "auto",
                }

        payload = asyncio.run(import_source_path(_Request()))
        manifest = json.loads((workspace.FONTS_DIR / "managed-copy" / workspace.MANIFEST_NAME).read_text(encoding="utf-8"))

        self.assertTrue(payload["success"])
        self.assertEqual(payload["slot"], "managed-copy")
        self.assertFalse(payload["linked_source"])
        self.assertNotIn("origin_mode", manifest)
        self.assertTrue((workspace.FONTS_DIR / "managed-copy" / "Demo.ufo" / "glyphs" / "A_.glif").exists())

    def test_imported_copy_write_does_not_modify_original_disk_source(self) -> None:
        from nodes.font import import_font
        from nodes.runebender import write_workspace_file

        source_dir = Path(self.tmp.name) / "original-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
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
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text("<glyph name=\"A\"><advance width=\"600\"/></glyph>", encoding="utf-8")

        class _UploadedFile:
            def __init__(self, filename: str, content: bytes):
                self.filename = filename
                self.file = types.SimpleNamespace(read=lambda: content)

        files = [
            _UploadedFile("project/Demo.designspace", designspace.read_bytes()),
            _UploadedFile("project/Demo.ufo/metainfo.plist", (source_dir / "Demo.ufo" / "metainfo.plist").read_bytes()),
            _UploadedFile("project/Demo.ufo/glyphs/A_.glif", original_glif.read_bytes()),
        ]
        import_payload = asyncio.run(import_font(self._request_with_files(
            files,
            {"workspace_name": "imported-copy", "source_kind": "auto"},
        )))
        self.assertEqual(import_payload["slot"], "imported-copy")

        class _WriteRequest:
            async def post(self):
                return {
                    "path": "imported-copy/Demo.ufo/glyphs/A_.glif",
                    "text": "<glyph name=\"A\"><advance width=\"720\"/></glyph>",
                }

        write_payload = asyncio.run(write_workspace_file(_WriteRequest()))

        self.assertFalse(write_payload["saved_to_source"])
        self.assertIn('width="720"', (workspace.FONTS_DIR / "imported-copy" / "Demo.ufo" / "glyphs" / "A_.glif").read_text(encoding="utf-8"))
        self.assertIn('width="600"', original_glif.read_text(encoding="utf-8"))

    def test_imported_copy_save_as_relink_then_writes_to_exported_folder(self) -> None:
        from nodes.font import import_font
        from nodes.runebender import save_workspace_as, write_workspace_file

        source_dir = Path(self.tmp.name) / "original-source"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
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
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text("<glyph name=\"A\"><advance width=\"600\"/></glyph>", encoding="utf-8")

        class _UploadedFile:
            def __init__(self, filename: str, content: bytes):
                self.filename = filename
                self.file = types.SimpleNamespace(read=lambda: content)

        files = [
            _UploadedFile("project/Demo.designspace", designspace.read_bytes()),
            _UploadedFile("project/Demo.ufo/metainfo.plist", (source_dir / "Demo.ufo" / "metainfo.plist").read_bytes()),
            _UploadedFile("project/Demo.ufo/glyphs/A_.glif", original_glif.read_bytes()),
        ]
        import_payload = asyncio.run(import_font(self._request_with_files(
            files,
            {"workspace_name": "imported-save-as", "source_kind": "auto"},
        )))
        self.assertEqual(import_payload["slot"], "imported-save-as")

        destination = Path(self.tmp.name) / "exported-source"

        class _SaveAsRequest:
            async def post(self):
                return {
                    "slot": "imported-save-as",
                    "destination": str(destination),
                    "relink": "true",
                }

        save_as_payload = asyncio.run(save_workspace_as(_SaveAsRequest()))
        self.assertTrue(save_as_payload["linked_source"])
        self.assertEqual(Path(save_as_payload["origin_root"]).resolve(), destination.resolve())

        class _WriteRequest:
            async def post(self):
                return {
                    "path": "imported-save-as/Demo.ufo/glyphs/A_.glif",
                    "text": "<glyph name=\"A\"><advance width=\"730\"/></glyph>",
                }

        write_payload = asyncio.run(write_workspace_file(_WriteRequest()))

        exported_glif = destination / "Demo.ufo" / "glyphs" / "A_.glif"
        self.assertTrue(write_payload["saved_to_source"])
        self.assertEqual(Path(write_payload["source_path"]).resolve(), exported_glif.resolve())
        self.assertIn('width="730"', exported_glif.read_text(encoding="utf-8"))
        self.assertIn('width="600"', original_glif.read_text(encoding="utf-8"))


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

    def test_workspace_route_reports_linked_source_metadata(self) -> None:
        from nodes.runebender import get_workspace_slot

        source_dir = Path(self.tmp.name) / "linked"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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
        workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)

        class _Request:
            match_info = {"slot": "linked-demo"}

        payload = asyncio.run(get_workspace_slot(_Request()))

        self.assertEqual(payload["slot"], "linked-demo")
        self.assertTrue(payload["linked_source"])
        self.assertEqual(Path(payload["origin_root"]).resolve(), source_dir.resolve())
        self.assertEqual(Path(payload["origin_source"]).resolve(), designspace.resolve())
        self.assertFalse(payload["refreshed_from_source"])

    def test_workspace_route_refreshes_newer_linked_source_before_loading(self) -> None:
        from nodes.runebender import get_workspace_slot

        source_dir = Path(self.tmp.name) / "linked-refresh"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text(
            "<glyph name=\"A\"><advance width=\"600\"/></glyph>",
            encoding="utf-8",
        )
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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
        workspace.create_slot_from_path(str(designspace), "linked-demo", linked=True)
        manifest_path = workspace.FONTS_DIR / "linked-demo" / workspace.MANIFEST_NAME
        cached_snapshot = int(json.loads(manifest_path.read_text(encoding="utf-8"))["origin_snapshot_mtime_ns"])
        original_glif.write_text(
            "<glyph name=\"A\"><advance width=\"710\"/></glyph>",
            encoding="utf-8",
        )
        os.utime(original_glif, ns=(cached_snapshot + 1_000_000_000, cached_snapshot + 1_000_000_000))

        class _Request:
            match_info = {"slot": "linked-demo"}

        payload = asyncio.run(get_workspace_slot(_Request()))

        self.assertTrue(payload["refreshed_from_source"])
        glif_files = [entry for entry in payload["files"] if entry["path"].endswith("A_.glif")]
        self.assertEqual(len(glif_files), 1)
        self.assertIn('width="710"', glif_files[0]["text"])

    def test_linked_workspace_routes_round_trip_glif_and_designspace_to_source(self) -> None:
        from nodes.font import link_source
        from nodes.runebender import get_workspace_slot, write_workspace_file

        source_dir = Path(self.tmp.name) / "linked"
        glyphs_dir = source_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        original_glif = glyphs_dir / "A_.glif"
        original_glif.write_text(
            "<glyph name=\"A\"><advance width=\"600\"/></glyph>",
            encoding="utf-8",
        )
        (source_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
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

        class _LinkRequest:
            async def post(self):
                return {
                    "source_path": str(designspace),
                    "workspace_name": "linked-demo",
                    "source_kind": "auto",
                }

        link_payload = asyncio.run(link_source(_LinkRequest()))
        self.assertEqual(link_payload["slot"], "linked-demo")

        class _WorkspaceRequest:
            match_info = {"slot": "linked-demo"}

        workspace_payload = asyncio.run(get_workspace_slot(_WorkspaceRequest()))
        exported_paths = {entry["path"] for entry in workspace_payload["files"]}
        self.assertTrue(workspace_payload["linked_source"])
        self.assertIn("Demo.designspace", exported_paths)
        self.assertIn("Demo.ufo/glyphs/A_.glif", exported_paths)

        class _WriteGlifRequest:
            async def post(self):
                return {
                    "path": "linked-demo/Demo.ufo/glyphs/A_.glif",
                    "text": "<glyph name=\"A\"><advance width=\"720\"/></glyph>",
                }

        glif_payload = asyncio.run(write_workspace_file(_WriteGlifRequest()))
        self.assertTrue(glif_payload["success"])
        self.assertTrue(glif_payload["saved_to_source"])
        self.assertEqual(Path(glif_payload["source_path"]).resolve(), original_glif.resolve())
        self.assertIn('width="720"', original_glif.read_text(encoding="utf-8"))

        class _WriteDesignspaceRequest:
            async def post(self):
                return {
                    "path": "linked-demo/Demo.designspace",
                    "text": """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<designspace format=\"5\">
  <sources>
    <source name=\"Regular\" filename=\"Demo.ufo\"/>
  </sources>
  <axes>
    <axis tag=\"wght\" name=\"Weight\" minimum=\"400\" maximum=\"700\" default=\"400\"/>
  </axes>
</designspace>
""",
                }

        designspace_payload = asyncio.run(write_workspace_file(_WriteDesignspaceRequest()))
        self.assertTrue(designspace_payload["success"])
        self.assertTrue(designspace_payload["saved_to_source"])
        self.assertEqual(Path(designspace_payload["source_path"]).resolve(), designspace.resolve())
        self.assertIn('tag="wght"', designspace.read_text(encoding="utf-8"))

    def test_macos_source_picker_route_returns_chosen_source_path(self) -> None:
        from nodes.font import choose_source

        class _Request:
            async def post(self):
                return {"mode": "source"}

        completed = mock.Mock(returncode=0, stdout="/tmp/Demo.designspace\n", stderr="")
        with mock.patch("nodes.font.sys.platform", "darwin"), \
             mock.patch("nodes.font.subprocess.run", return_value=completed) as run:
            payload = asyncio.run(choose_source(_Request()))

        self.assertEqual(payload["path"], "/tmp/Demo.designspace")
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0][:2], ["osascript", "-e"])
        self.assertIn("choose file", run.call_args.args[0][2])
        self.assertIn("font source", run.call_args.args[0][2])

    def test_macos_source_picker_route_returns_chosen_folder_path(self) -> None:
        from nodes.font import choose_source

        class _Request:
            async def post(self):
                return {"mode": "folder"}

        completed = mock.Mock(returncode=0, stdout="/tmp/Demo.ufo/\n", stderr="")
        with mock.patch("nodes.font.sys.platform", "darwin"), \
             mock.patch("nodes.font.subprocess.run", return_value=completed) as run:
            payload = asyncio.run(choose_source(_Request()))

        self.assertEqual(payload["path"], "/tmp/Demo.ufo/")
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0][:2], ["osascript", "-e"])
        self.assertIn("choose folder", run.call_args.args[0][2])

    def test_macos_source_picker_route_reports_cancelled_selection(self) -> None:
        from nodes.font import choose_source

        class _Request:
            async def post(self):
                return {}

        completed = mock.Mock(returncode=1, stdout="", stderr="User canceled. (-128)")
        with mock.patch("nodes.font.sys.platform", "darwin"), \
             mock.patch("nodes.font.subprocess.run", return_value=completed):
            payload = asyncio.run(choose_source(_Request()))

        self.assertFalse(payload["success"])
        self.assertTrue(payload["cancelled"])
        self.assertEqual(payload["path"], "")

    def test_source_picker_route_rejects_non_macos_backend(self) -> None:
        from nodes.font import choose_source

        class _Request:
            async def post(self):
                return {}

        with mock.patch("nodes.font.sys.platform", "linux"), self.assertRaises(Exception):
            asyncio.run(choose_source(_Request()))

    def test_save_as_route_exports_workspace_and_relinks_destination(self) -> None:
        from nodes.runebender import save_workspace_as, write_workspace_file

        slot_dir = workspace.FONTS_DIR / "managed-demo"
        glyphs_dir = slot_dir / "Demo.ufo" / "glyphs"
        glyphs_dir.mkdir(parents=True)
        (slot_dir / "Demo.designspace").write_text("<designspace/>", encoding="utf-8")
        (slot_dir / "Demo.ufo" / "metainfo.plist").write_text("<plist/>", encoding="utf-8")
        (glyphs_dir / "A_.glif").write_text("<glyph name=\"A\"/>", encoding="utf-8")
        workspace._write_manifest(slot_dir, {"source_kind": "ufo/designspace"})  # type: ignore[attr-defined]
        destination = Path(self.tmp.name) / "saved-as"

        class _SaveAsRequest:
            async def post(self):
                return {
                    "slot": "managed-demo",
                    "destination": str(destination),
                    "relink": "true",
                }

        payload = asyncio.run(save_workspace_as(_SaveAsRequest()))

        self.assertTrue(payload["success"])
        self.assertTrue(payload["linked_source"])
        self.assertEqual(Path(payload["destination"]).resolve(), destination.resolve())
        self.assertTrue((destination / "Demo.designspace").exists())
        self.assertTrue((destination / "Demo.ufo" / "glyphs" / "A_.glif").exists())

        class _WriteRequest:
            async def post(self):
                return {
                    "path": "managed-demo/Demo.ufo/glyphs/A_.glif",
                    "text": "<glyph name=\"A\"><advance width=\"777\"/></glyph>",
                }

        write_payload = asyncio.run(write_workspace_file(_WriteRequest()))
        self.assertTrue(write_payload["saved_to_source"])
        self.assertEqual(
            Path(write_payload["source_path"]).resolve(),
            (destination / "Demo.ufo" / "glyphs" / "A_.glif").resolve(),
        )
        self.assertIn('width="777"', (destination / "Demo.ufo" / "glyphs" / "A_.glif").read_text(encoding="utf-8"))

    def test_linked_virtua_sample_round_trip_uses_real_designspace_layout(self) -> None:
        from nodes.font import link_source
        from nodes.runebender import get_workspace_slot, write_workspace_file

        source_dir = Path(self.tmp.name) / "virtua-linked"
        shutil.copytree(ROOT / "samples" / "virtua-grotesk", source_dir)
        designspace = source_dir / "VirtuaGrotesk.designspace"
        regular_k = source_dir / "VirtuaGrotesk-Regular.ufo" / "glyphs" / "k.glif"
        original_designspace = designspace.read_text(encoding="utf-8")
        original_k = regular_k.read_text(encoding="utf-8")

        class _LinkRequest:
            async def post(self):
                return {
                    "source_path": str(designspace),
                    "workspace_name": "virtua-linked",
                    "source_kind": "auto",
                }

        link_payload = asyncio.run(link_source(_LinkRequest()))
        self.assertEqual(link_payload["slot"], "virtua-linked")

        class _WorkspaceRequest:
            match_info = {"slot": "virtua-linked"}

        workspace_payload = asyncio.run(get_workspace_slot(_WorkspaceRequest()))
        exported_paths = {entry["path"] for entry in workspace_payload["files"]}
        self.assertTrue(workspace_payload["linked_source"])
        self.assertIn("VirtuaGrotesk.designspace", exported_paths)
        self.assertIn("VirtuaGrotesk-Regular.ufo/glyphs/k.glif", exported_paths)
        self.assertIn("VirtuaGrotesk-Bold.ufo/glyphs/k.glif", exported_paths)

        class _WriteGlifRequest:
            async def post(self):
                return {
                    "path": "virtua-linked/VirtuaGrotesk-Regular.ufo/glyphs/k.glif",
                    "text": original_k.replace('width="540"', 'width="541"', 1),
                }

        glif_payload = asyncio.run(write_workspace_file(_WriteGlifRequest()))
        self.assertTrue(glif_payload["saved_to_source"])
        self.assertEqual(Path(glif_payload["source_path"]).resolve(), regular_k.resolve())
        self.assertIn('width="541"', regular_k.read_text(encoding="utf-8"))

        class _WriteDesignspaceRequest:
            async def post(self):
                return {
                    "path": "virtua-linked/VirtuaGrotesk.designspace",
                    "text": original_designspace.replace(
                        '<axes>',
                        '<axes>\n    <axis tag="TEST" name="Test Axis" minimum="0" maximum="1" default="0"/>',
                        1,
                    ),
                }

        designspace_payload = asyncio.run(write_workspace_file(_WriteDesignspaceRequest()))
        self.assertTrue(designspace_payload["saved_to_source"])
        self.assertEqual(Path(designspace_payload["source_path"]).resolve(), designspace.resolve())
        self.assertIn('tag="TEST"', designspace.read_text(encoding="utf-8"))


class FontPreviewNodeTests(unittest.TestCase):
    def test_font_preview_module_imports_without_image_stack(self) -> None:
        input_types = FontPreview.INPUT_TYPES()

        self.assertEqual(input_types["required"]["font"], ("FONT",))
        self.assertEqual(FontPreview.RETURN_TYPES, ("IMAGE",))

    def test_glif_source_preview_parser_extracts_unicode_outline_and_width(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            glif = Path(tmp) / "A_.glif"
            glif.write_text(
                """<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<glyph name=\"A\" format=\"2\">
  <unicode hex=\"0041\"/>
  <advance width=\"600\"/>
  <outline>
    <contour>
      <point x=\"0\" y=\"0\" type=\"move\"/>
      <point x=\"300\" y=\"700\" type=\"line\"/>
      <point x=\"600\" y=\"0\" type=\"line\"/>
    </contour>
  </outline>
</glyph>
""",
                encoding="utf-8",
            )

            parsed = font_preview._parse_glif(glif)  # type: ignore[attr-defined]

        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["advance"], 600)
        self.assertIn("0041", parsed["unicodes"])
        self.assertEqual(len(parsed["polygons"]), 1)

    def test_font_preview_renders_from_workspace_source_without_compiled_font(self) -> None:
        class _FakeImg:
            def convert(self, _mode):
                return self

        class _FakeImageModule:
            @staticmethod
            def open(_bio):
                return _FakeImg()

        with mock.patch("nodes.font_preview.compiled_path", side_effect=FileNotFoundError), \
             mock.patch("nodes.font_preview.resolve_slot", return_value=Path("/tmp/font-slot")), \
             mock.patch("nodes.font_preview.render_workspace_preview_png", return_value=b"png"), \
             mock.patch("nodes.font_preview._image_stack", return_value=(object(), _FakeImageModule, object(), object(), object())), \
             mock.patch("nodes.font_preview._pil_to_tensor", return_value="tensor"):
            image, = FontPreview().run("demo", "Aa", 256, 128)

        self.assertEqual(image, "tensor")


class FontSpecimenNodeTests(unittest.TestCase):
    def test_font_specimen_exposes_scriptable_image_and_mask_outputs(self) -> None:
        input_types = FontSpecimen.INPUT_TYPES()

        self.assertEqual(input_types["required"]["font"][0], "FONT")
        self.assertTrue(input_types["required"]["font"][1]["forceInput"])
        self.assertIn("Specimen", input_types["required"]["preset"][0])
        self.assertEqual(FontSpecimen.RETURN_TYPES, ("IMAGE", "MASK"))
        self.assertIn("custom_script", input_types["optional"])
        self.assertIn("DrawBot Python", input_types["optional"]["custom_script"][1]["tooltip"])
        self.assertIn("trusted", input_types["optional"]["custom_script"][1]["tooltip"].lower())
        self.assertIn("execute locally", input_types["optional"]["custom_script"][1]["tooltip"])

    def test_font_specimen_loads_drawbot_presets_from_disk(self) -> None:
        presets = load_presets()

        self.assertIn("Specimen", presets)
        self.assertIn("Waterfall", presets)
        self.assertIn("font_path", presets["Custom"])
        self.assertNotIn("helpers", presets)

    def test_font_specimen_uses_runebender_drawbot_skia_fork(self) -> None:
        requirements = (ROOT / "requirements.txt").read_text(encoding="utf-8")

        self.assertIn("drawbot-skia @ git+https://github.com/eliheuer/drawbot-skia.git", requirements)

    def test_font_specimen_renders_image_and_mask_tensors(self) -> None:
        class _FakeImg:
            def convert(self, _mode):
                return self

            def getchannel(self, _name):
                return self

        with mock.patch("nodes.font_specimen.compiled_path", return_value=Path("/tmp/font.ttf")), \
             mock.patch("nodes.font_specimen._render_drawbot", return_value=_FakeImg()), \
             mock.patch("nodes.font_specimen._image_to_tensor", return_value="tensor"), \
             mock.patch("nodes.font_specimen._mask_to_tensor", return_value="tensor"):
            # run(font, preset, width, height, custom_script="") — input_text
            # widget was removed; presets fall back to their own defaults.
            image, mask = FontSpecimen().run("demo", "glyph", 256, 128)

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
