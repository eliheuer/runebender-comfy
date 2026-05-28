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
    def test_text_edit_wrappers_do_not_force_full_reshaping(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        for name in [
            "insertTextGlyphByName",
            "insertInactiveTextGlyphByName",
            "insertTextLineBreak",
            "deleteTextBeforeCursor",
            "deleteTextAfterCursor",
        ]:
            start = source.index(f"function {name}(")
            next_function = source.find("\nfunction ", start + 1)
            body = source[start:next_function if next_function >= 0 else len(source)]
            self.assertNotIn("reshapeTextBuffer();", body, name)

    def test_editor_save_refreshes_comfy_node_specimen_preview(self) -> None:
        editor_source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        extension_source = (ROOT / "web" / "src" / "extension.ts").read_text(encoding="utf-8")

        save_start = editor_source.index("async function onSave()")
        save_end = editor_source.index("\nasync function persistGlyphData", save_start)
        save_body = editor_source[save_start:save_end]
        self.assertIn("props.onWorkspaceSaved?.();", save_body)
        self.assertLess(save_body.index("queueComfyStateSync(true);"), save_body.index("props.onWorkspaceSaved?.();"))

        self.assertIn("onWorkspaceSaved?: () => void;", editor_source)
        self.assertIn("onWorkspaceSaved?: () => void;", extension_source)
        self.assertIn("onWorkspaceSaved: options.onWorkspaceSaved", extension_source)
        self.assertIn("const syncPreview = (force = false)", extension_source)
        self.assertIn("if (!force && value === lastPreviewSlot && previewImg.src) return;", extension_source)
        self.assertIn("onWorkspaceSaved: () =>", extension_source)
        self.assertIn("syncPreview(true);", extension_source)

    def test_workflow_tab_switch_closes_global_editor_overlay(self) -> None:
        extension_source = (ROOT / "web" / "src" / "extension.ts").read_text(encoding="utf-8")
        self.assertIn("const onDocumentPointerDown = (event: PointerEvent)", extension_source)
        self.assertIn('target?.closest(".workflow-tabs-container")', extension_source)
        self.assertIn("closing editor for workflow tab switch", extension_source)
        self.assertIn('document.addEventListener("pointerdown", onDocumentPointerDown, true);', extension_source)
        self.assertIn('document.removeEventListener("pointerdown", onDocumentPointerDown, true);', extension_source)

    def test_grid_keyboard_selection_paints_before_reactive_sidebar_work(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        nav_start = source.index("function navigateGridSelection")
        nav_end = source.index("\nfunction copyGridGlyph", nav_start)
        nav_body = source[nav_start:nav_end]
        self.assertIn("pendingGridSelectionName || selectedGlyph.value", nav_body)
        self.assertIn("setPrimarySelectedGlyphFromKeyboard(names[nextIndex], nextIndex);", nav_body)

        fast_start = source.index("function setPrimarySelectedGlyphFromKeyboard")
        fast_end = source.index("\nfunction selectGlyph", fast_start)
        fast_body = source[fast_start:fast_end]
        self.assertLess(
            fast_body.index("applyImmediateGridSelection(previousIndex, index);"),
            fast_body.index("requestAnimationFrame(() => {"),
        )
        self.assertIn("classList.add(\"selected\")", source)
        self.assertIn("classList.remove(\"selected\")", source)
        self.assertIn("let pendingGridScrollRaf", source)
        self.assertIn("if (pendingGridScrollRaf !== null) return;", source)
        self.assertIn("cancelAnimationFrame(pendingGridSelectionRaf);", source)

    def test_editor_render_requests_coalesce_without_postponing_frame(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function requestRender(")
        end = source.index("\nasync function loadDevTestFont", start)
        body = source[start:end]

        self.assertIn("if (raf !== null) return;", body)
        self.assertIn("rafNeedsDerivedState ||= options.refreshDerivedState !== false;", body)
        self.assertIn("const refreshDerivedState = rafNeedsDerivedState;", body)
        self.assertIn("raf = null;", body)
        self.assertIn("rafNeedsDerivedState = false;", body)
        self.assertIn("editor?.render();", body)
        self.assertIn("if (refreshDerivedState)", body)
        self.assertNotIn("cancelAnimationFrame(raf);", body)

    def test_glyph_sidebar_uses_generated_google_fonts_data(self) -> None:
        sidebar_source = (ROOT / "web" / "src" / "glyphSidebarData.ts").read_text(encoding="utf-8")
        generated_source = (ROOT / "web" / "src" / "gfSidebarData.generated.ts").read_text(encoding="utf-8")
        package_source = (ROOT / "web" / "package.json").read_text(encoding="utf-8")
        readme_source = (ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn('import { GF_GLYPHSETS } from "./gfSidebarData.generated";', sidebar_source)
        self.assertIn("glyphNames?: readonly string[];", sidebar_source)
        self.assertIn('"id": "GF_Latin_Core"', generated_source)
        self.assertIn('"id": "GF_Arabic_Core"', generated_source)
        self.assertIn('"gf-sidebar": "cd .. && node scripts/generate-gf-sidebar-data.mjs"', package_source)
        self.assertIn("/Users/eli/GF/repos/glyphsets", readme_source)
        self.assertIn("https://github.com/googlefonts/glyphsets", readme_source)

    def test_empty_render_overlays_do_not_churn_reactive_state(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        compat_start = source.index("function refreshCompatibilityMarkers()")
        compat_end = source.index("\nfunction refreshMeasureState", compat_start)
        compat_body = source[compat_start:compat_end]
        self.assertIn("compatMarkers.value.length > 0", compat_body)

        measure_start = source.index("function refreshMeasureState()")
        measure_end = source.index("\nfunction measureLabelsFromInfo", measure_start)
        measure_body = source[measure_start:measure_end]
        self.assertIn('activeTool.value !== "Measure"', measure_body)
        self.assertIn("if (measureInfo.value)", measure_body)
        self.assertIn("measureInfo.value = undefined;", measure_body)
        self.assertLess(
            measure_body.index('activeTool.value !== "Measure"'),
            measure_body.index("editor.measureInfo()"),
        )

    def test_selection_state_refresh_avoids_identical_reactive_assignments(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("function sameSelectionBounds(", source)
        self.assertIn("function setSelectionState(", source)

        refresh_start = source.index("function refreshSelectionState()")
        refresh_end = source.index("\nfunction sameSelectionBounds", refresh_start)
        refresh_body = source[refresh_start:refresh_end]
        self.assertIn("setSelectionState(", refresh_body)
        self.assertNotIn("selectedBounds.value =", refresh_body)

        setter_start = source.index("function setSelectionState(")
        setter_end = source.index("\nfunction updateCompatibilityErrors", setter_start)
        setter_body = source[setter_start:setter_end]
        self.assertIn("selectionCount.value !== nextSelectionCount", setter_body)
        self.assertIn("selectedContourCount.value !== nextSelectedContourCount", setter_body)
        self.assertIn("!sameSelectionBounds(selectedBounds.value, nextBounds)", setter_body)

    def test_pointer_hover_avoids_work_for_tools_without_hover_state(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onPointerMove(")
        end = source.index("\nfunction onPointerUp", start)
        body = source[start:end]

        self.assertIn("const pointerActive = e.buttons !== 0;", body)
        self.assertIn("if (pointerActive) {\n    refreshMeasureState();\n  }", body)
        self.assertIn('activeTool.value === "Text" && pointerActive', body)
        self.assertIn('activeTool.value === "Measure"', body)
        self.assertLess(body.index("if (\n    !pointerActive"), body.index("editor.pointerMove("))
        self.assertLess(body.index("if (\n    !pointerActive"), body.index("requestRender();"))

    def test_arrow_nudge_uses_fast_render_path_and_defers_heavy_sync(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("function applyEditorNudge(", source)
        nudge_start = source.index("function applyEditorNudge(")
        nudge_end = source.index("\nfunction applyEditorHistoryChange", nudge_start)
        nudge_body = source[nudge_start:nudge_end]
        self.assertIn("editor.nudgeSelection(dx, dy, shift, ctrl, independent)", nudge_body)
        self.assertIn("requestRender({ refreshDerivedState: false });", nudge_body)
        self.assertIn("scheduleDeferredGlyphSync(glyphName, masterName);", nudge_body)
        self.assertNotIn("syncCurrentGlyphBytesFromEditor();", nudge_body)
        self.assertNotIn("refreshSelectionState();", nudge_body)
        self.assertIn("finishNudgeSelection(): void;", source)
        self.assertIn("editor?.finishNudgeSelection();", source)

        key_start = source.index("function onKeyDown(")
        key_end = source.index("\nfunction onKeyUp", key_start)
        key_body = source[key_start:key_end]
        self.assertIn("applyEditorNudge(nudge[0], nudge[1], e.shiftKey, meta, e.altKey)", key_body)
        self.assertNotIn("editor.nudgeSelection(", key_body)

    def test_wasm_nudge_reuses_one_undo_snapshot_per_burst(self) -> None:
        source = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("pending_nudge_snapshot: Option<EditorState>", source)
        self.assertIn("fn commit_pending_nudge_snapshot(&mut self)", source)
        self.assertIn("pub fn finish_nudge_selection(&mut self)", source)

        start = source.index("pub fn nudge_selection(")
        end = source.index("\n    #[wasm_bindgen(js_name = finishNudgeSelection)]", start)
        body = source[start:end]
        self.assertIn("if self.state.selection.is_empty()", body)
        self.assertIn("if self.pending_nudge_snapshot.is_none()", body)
        self.assertIn("self.pending_nudge_snapshot = Some(self.state.clone());", body)
        self.assertNotIn("self.undo.add_undo_group", body)

    def test_master_switch_refreshes_text_sort_metrics_from_active_master(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        self.assertIn("function syncTextSortMetricsToActiveMaster()", source)
        sync_start = source.index("function syncTextSortMetricsToActiveMaster()")
        sync_end = source.index("\nfunction onActiveGlyphNameChange", sync_start)
        sync_body = source[sync_start:sync_end]
        self.assertIn("glyphMetadataMap.value.get(sort.glyphName)", sync_body)
        self.assertIn("metadata.width", sync_body)
        self.assertIn("editor.updateTextGlyph(", sync_body)
        self.assertIn("refreshTextStateFromEditor();", sync_body)

        master_start = source.index("function activateMaster(name: string)")
        master_end = source.index("\nfunction onSelectMaster", master_start)
        master_body = source[master_start:master_end]
        self.assertIn("syncTextSortMetricsToActiveMaster();", master_body)
        self.assertLess(
            master_body.index("syncTextSortMetricsToActiveMaster();"),
            master_body.index("const glyphToReload ="),
        )

    def test_text_metrics_and_kerning_edits_do_not_force_arabic_reshaping(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        for name in [
            "onActiveGlyphWidthChange",
            "onActiveGlyphSidebearingChange",
            "updateActiveTextKern",
            "updateGlyphKerningGroup",
        ]:
            start = source.index(f"function {name}(")
            next_function = source.find("\nfunction ", start + 1)
            body = source[start:next_function if next_function >= 0 else len(source)]
            self.assertIn("syncTextKerningModelToEditor();", body, name)
            self.assertIn("queueComfyStateSync();", body, name)
            self.assertNotIn("reshapeTextBuffer();", body, name)

    def test_text_kerning_field_edits_request_canvas_render_like_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        for name in ["updateActiveTextKern", "updateGlyphKerningGroup"]:
            start = source.index(f"function {name}(")
            next_function = source.find("\nfunction ", start + 1)
            body = source[start:next_function if next_function >= 0 else len(source)]
            self.assertIn("syncTextKerningModelToEditor();", body, name)
            self.assertIn("requestRender();", body, name)
            self.assertLess(
                body.index("syncTextKerningModelToEditor();"),
                body.index("requestRender();"),
                name,
            )

    def test_text_kern_zero_is_stored_like_xilem_not_deleted(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function updateActiveTextKern")
        end = source.index("\nfunction updateGlyphKerningGroup", start)
        body = source[start:end]

        self.assertIn('if (!value || value === "-")', body)
        self.assertIn("pairs.delete(pair[1]);", body)
        self.assertIn("pairs.set(pair[1], kernValue);", body)
        self.assertNotIn("Math.abs(kernValue) < Number.EPSILON", body)

    def test_text_kern_pair_delete_uses_xilem_raw_field_value(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function updateActiveTextKern")
        end = source.index("\nfunction updateGlyphKerningGroup", start)
        body = source[start:end]

        self.assertIn('if (!value || value === "-")', body)
        self.assertIn("if (value.trim() !== value) return;", body)
        self.assertIn("const kernValue = Number(value);", body)
        self.assertNotIn("const trimmed = value.trim();", body)
        self.assertNotIn("Number(trimmed)", body)

    def test_active_glyph_width_uses_xilem_raw_f64_parse_shape(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onActiveGlyphWidthChange")
        end = source.index("\nfunction refreshSidebearingsFromEditor", start)
        body = source[start:end]

        self.assertIn('const value = input?.value ?? "";', body)
        self.assertIn("const width = Number(value);", body)
        self.assertIn("!value || value.trim() !== value || !Number.isFinite(width)", body)
        self.assertNotIn("Number(input?.value)", body)

    def test_active_glyph_group_fields_show_full_xilem_group_names(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("<span>Left Group</span>")
        end = source.index("<span>Right Group</span>", start)
        left_field = source[start:end]
        right_field = source[end:source.index("</label>", end)]

        self.assertIn(":value=\"activeGlyphKerningGroups?.left ?? ''\"", left_field)
        self.assertIn(":value=\"activeGlyphKerningGroups?.right ?? ''\"", right_field)
        self.assertNotIn("displayKerningGroup(activeGlyphKerningGroups?.left", left_field)
        self.assertNotIn("displayKerningGroup(activeGlyphKerningGroups?.right", right_field)

    def test_active_glyph_group_edits_use_xilem_glif_lib_storage(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("glifWithKerningGroup", source)
        start = source.index("function updateGlyphKerningGroup")
        end = source.index("\nfunction syncTextKerningModelToEditor", start)
        body = source[start:end]

        self.assertIn("glifWithKerningGroup(bytes, side, nextGroup)", body)
        self.assertIn("data.glyphBytes.set(glyphName, nextBytes);", body)
        self.assertIn("markGlyphDirty(glyphName);", body)
        self.assertNotIn("normalizeKerningGroup", body)
        self.assertNotIn("markGroupsDirty();", body)
        self.assertNotIn("data.groups.set", body)
        self.assertNotIn("data.groups.delete", body)

    def test_text_kerning_model_includes_xilem_glif_lib_groups(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function textKerningGroupsForEditor")
        end = source.index("\nfunction textGlyphNameAt", start)
        body = source[start:end]

        self.assertIn("const merged = new Map<string, string[]>();", body)
        self.assertIn("for (const [groupName, members] of groups.value)", body)
        self.assertIn("for (const [glyphName, glyphGroups] of glyphKerningGroups.value)", body)
        self.assertIn("for (const groupName of [glyphGroups.left, glyphGroups.right])", body)
        self.assertIn("merged.set(groupName, [...members, glyphName]);", body)

        sync_start = source.index("function syncTextKerningModelToEditor")
        sync_end = source.index("\nfunction syncTextKerningModelFromEditor", sync_start)
        sync_body = source[sync_start:sync_end]
        self.assertIn("stringArrayMapToRecord(textKerningGroupsForEditor())", sync_body)
        self.assertIn('leftGroups: textKerningGroupHintsForEditor("left")', sync_body)
        self.assertIn('rightGroups: textKerningGroupHintsForEditor("right")', sync_body)
        self.assertNotIn("stringArrayMapToRecord(groups.value)", sync_body)

    def test_glif_lib_kerning_groups_are_read_by_norad_metadata_like_xilem(self) -> None:
        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("struct GlifMetadata", wasm)
        self.assertIn('.get("public.kern1")', wasm)
        self.assertIn('.get("public.kern2")', wasm)
        self.assertIn("let metadata = glif_metadata_from_norad(&glyph);", wasm)

        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function parseGlyphInfo")
        end = source.index("\nfunction parseGroupsPlist", start)
        body = source[start:end]
        self.assertIn("leftKerningGroup: metadata.leftKerningGroup ?? null", body)
        self.assertIn("rightKerningGroup: metadata.rightKerningGroup ?? null", body)
        self.assertNotIn("public\\.kern1", body)
        self.assertNotIn("public\\.kern2", body)

    def test_serialized_active_glyph_refreshes_glif_lib_group_cache(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        helper_start = source.index("function setGlyphKerningGroupsFromInfo")
        helper_end = source.index("\nfunction textKerningGroupsForEditor", helper_start)
        helper_body = source[helper_start:helper_end]
        self.assertIn("...plistKerningGroupsForGlyph(data.groups, glyphName)", helper_body)
        self.assertIn("info.leftKerningGroup ? { left: info.leftKerningGroup } : {}", helper_body)
        self.assertIn("info.rightKerningGroup ? { right: info.rightKerningGroup } : {}", helper_body)
        self.assertIn("data.glyphKerningGroups.set(glyphName, nextGroups);", helper_body)
        self.assertIn("data.glyphKerningGroups.delete(glyphName);", helper_body)

    def test_active_text_kern_lookup_uses_xilem_group_membership_without_prefix_filter(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function kerningGroupsForGlyph")
        end = source.index("\nfunction lookupKerningValue", start)
        body = source[start:end]

        self.assertIn("const groupMap = textKerningGroupsForEditor();", body)
        self.assertIn("if (hint && groupMap.get(hint)?.includes(glyphName))", body)
        self.assertIn("groupNames.push(hint);", body)
        self.assertIn("members.includes(glyphName)", body)
        self.assertNotIn("startsWith", body)
        self.assertNotIn("public.kern1", body)
        self.assertNotIn("public.kern2", body)

        lookup_start = source.index("function lookupKerningValue")
        lookup_end = source.index("\nfunction activeTextKernPair", lookup_start)
        lookup_body = source[lookup_start:lookup_end]
        self.assertIn(
            "const leftGroups = kerningGroupsForGlyph(left, glyphKerningGroups.value.get(left)?.right);",
            lookup_body,
        )
        self.assertIn(
            "const rightGroups = kerningGroupsForGlyph(right, glyphKerningGroups.value.get(right)?.left);",
            lookup_body,
        )

        sync_start = source.index("function syncCurrentGlyphBytesFromEditor")
        sync_end = source.index("\nfunction markGlyphDirty", sync_start)
        sync_body = source[sync_start:sync_end]
        self.assertIn("setGlyphKerningGroupsFromInfo(data, currentGlyph.value, info);", sync_body)

    def test_active_glyph_edit_updates_only_active_text_sort_like_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function syncCurrentTextSorts")
        end = source.index("\nfunction syncTextSortMetricsToActiveMaster", start)
        body = source[start:end]

        self.assertIn("activeTextSortIndex.value === null", body)
        self.assertIn("const activeSort = textBuffer.value[activeTextSortIndex.value];", body)
        self.assertIn('activeSort?.kind !== "glyph" || activeSort.glyphName !== currentGlyph.value', body)
        self.assertIn("editor.updateTextGlyph(\n      activeTextSortIndex.value,", body)
        self.assertNotIn("syncTextSortsForGlyph(currentGlyph.value, currentGlyph.value, metadata)", body)
        self.assertNotIn("for (let index = 0; index < textBuffer.value.length; index++)", body)

    def test_text_direction_select_refreshes_snapshot_without_reshaping(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onTextDirectionSelect")
        end = source.index("\nfunction textPreviewLineHeight", start)
        body = source[start:end]
        self.assertIn("editor?.setTextDirection(direction);", body)
        self.assertIn("refreshTextStateFromEditor();", body)
        self.assertIn("requestRender();", body)
        self.assertNotIn("reshapeTextBuffer", body)

    def test_text_line_height_matches_xilem_rendered_metric_box(self) -> None:
        editor = (ROOT / "rust-core" / "src" / "editor.rs").read_text(encoding="utf-8")
        start = editor.index("pub fn text_line_height")
        end = editor.index("\n    pub fn text_metric_bounds", start)
        body = editor[start:end]
        self.assertIn("let (ascender, descender) = self.text_metric_bounds();", body)
        self.assertIn("ascender - descender", body)
        self.assertNotIn("units_per_em - descender", body)

        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertNotIn("js_name = textLineHeight", wasm)

        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function textPreviewLineHeight")
        end = source.index("\nfunction refreshTextStateFromEditor", start)
        body = source[start:end]
        self.assertIn("editor.metricBounds()", body)
        self.assertIn("ascender - descender", body)
        self.assertNotIn("textLineHeight", body)

    def test_text_width_fallbacks_match_xilem_shaper_default(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("advanceWidth: sort.advanceWidth ?? 500", source)
        self.assertIn("advanceWidth: metadata?.width ?? 500", source)
        self.assertIn("currentWidth.value > 0 ? currentWidth.value : 500", source)
        self.assertNotIn("advanceWidth: sort.advanceWidth ?? 600", source)
        self.assertNotIn("advanceWidth: metadata?.width ?? 600", source)

    def test_text_key_input_uses_unicode_scalars_not_utf16_length(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("function singleInputCharacter", source)
        self.assertIn("Array.from(key)", source)
        self.assertIn("NON_TEXT_KEY_VALUES", source)
        self.assertIn("if (NON_TEXT_KEY_VALUES.has(key)) return null;", source)
        self.assertIn("if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return null;", source)
        self.assertIn("if (/^[A-Z][A-Za-z0-9]*$/.test(key)) return null;", source)
        self.assertIn("return chars[0];", source)
        self.assertNotIn("e.key.length === 1", source)
        self.assertNotIn("/^[A-Za-z][A-Za-z0-9]*$", source)

    def test_text_key_input_rejects_browser_named_keys_like_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function singleInputCharacter")
        end = source.index("\nfunction handleTextToolKey", start)
        body = source[start:end]
        self.assertIn("if (chars.length === 1) return chars[0];", body)
        self.assertIn("if (NON_TEXT_KEY_VALUES.has(key)) return null;", body)
        self.assertIn("if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return null;", body)
        self.assertIn("if (/^[A-Z][A-Za-z0-9]*$/.test(key)) return null;", body)
        self.assertIn('"ArrowUp"', source)
        self.assertIn('"Tab"', source)
        self.assertIn('"PageDown"', source)

    def test_text_key_input_only_handles_inserted_characters(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function handleTextToolKey")
        end = source.index("\nfunction handleResize", start)
        body = source[start:end]
        self.assertIn('return insertTextCharacter(char) || char === " ";', body)
        self.assertNotIn("insertTextCharacter(char);\n          return true;", body)

    def test_text_mode_consumes_plain_space_even_without_space_glyph(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function handleTextToolKey")
        end = source.index("\nfunction handleResize", start)
        body = source[start:end]
        self.assertIn("if (commandModified) return false;", body)
        self.assertIn('return insertTextCharacter(char) || char === " ";', body)
        self.assertNotIn('return insertTextCharacter(char) || char === e.key;', body)

    def test_text_mode_consumes_delete_keys_at_buffer_edges(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function handleTextToolKey")
        end = source.index("\nfunction handleResize", start)
        body = source[start:end]
        self.assertIn('case "Backspace":', body)
        self.assertIn("if (!deleteTextBeforeCursor()) requestRender();\n      return true;", body)
        self.assertIn('case "Delete":', body)
        self.assertIn("if (!deleteTextAfterCursor()) requestRender();\n      return true;", body)
        self.assertNotIn("return deleteTextBeforeCursor();", body)
        self.assertNotIn("return deleteTextAfterCursor();", body)

        before_start = source.index("function deleteTextBeforeCursor")
        before_end = source.index("\nfunction deleteTextAfterCursor", before_start)
        before_body = source[before_start:before_end]
        self.assertIn("const changed = editor?.deleteTextBeforeCursor() ?? false;", before_body)
        self.assertNotIn("textCursor.value <= 0", before_body)

        after_start = source.index("function deleteTextAfterCursor")
        after_end = source.index("\nfunction moveTextCursorVisual", after_start)
        after_body = source[after_start:after_end]
        self.assertIn("const changed = editor?.deleteTextAfterCursor() ?? false;", after_body)
        self.assertNotIn("textCursor.value >= textBuffer.value.length", after_body)

    def test_text_command_modifiers_only_skip_printable_input(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function handleTextToolKey")
        end = source.index("\nfunction handleResize", start)
        body = source[start:end]
        self.assertIn("if (!textModeActive.value)", body)
        self.assertIn("const commandModified = e.metaKey || e.ctrlKey;", body)
        self.assertIn("if (commandModified) return false;", body)
        self.assertNotIn('activeTool.value !== "Text" || e.metaKey || e.ctrlKey', body)

    def test_text_mode_blocks_fallback_tool_switching(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onKeyDown")
        end = source.index("\nfunction onKeyUp", start)
        body = source[start:end]
        self.assertIn("const textModeActive = computed", source)
        self.assertIn('activeTool.value === "Text" && hasTextBufferSession.value', source)
        self.assertIn("!meta && !e.shiftKey && !textModeActive.value", body)

    def test_bare_t_does_not_switch_to_text_tool_matching_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("const tool =")
        end = source.index("if (tool)", start)
        tool_switch = source[start:end]
        self.assertIn('key === "v"', tool_switch)
        self.assertIn('key === "p"', tool_switch)
        self.assertIn('key === "h"', tool_switch)
        self.assertIn('key === "k"', tool_switch)
        self.assertNotIn('"Text"', tool_switch)

    def test_text_mode_allows_xilem_command_shortcuts(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onKeyDown")
        end = source.index("\nfunction onKeyUp", start)
        body = source[start:end]
        for needle in [
            'e.key.toLowerCase() === "l"',
            'e.key.toLowerCase() === "t"',
            'e.key.toLowerCase() === "y"',
            'e.key.toLowerCase() === "h"',
        ]:
            shortcut_start = body.index(needle)
            branch_end = body.find("  } else if", shortcut_start + 1)
            branch = body[shortcut_start:branch_end if branch_end >= 0 else len(body)]
            self.assertNotIn("!textModeActive.value", branch, needle)

    def test_ctrl_space_preview_shortcut_runs_before_text_input_like_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onKeyDown")
        end = source.index("\nfunction onKeyUp", start)
        body = source[start:end]

        ctrl_space = body.index('if (e.ctrlKey && !e.metaKey && e.key === " ")')
        text_input = body.index("if (handleTextToolKey(e))")
        branch_end = body.find("\n\n  if (", ctrl_space + 1)
        branch = body[ctrl_space:branch_end if branch_end >= 0 else text_input]

        self.assertLess(ctrl_space, text_input)
        self.assertIn('onToolSelect("Preview");', branch)
        self.assertNotIn("!textModeActive.value", branch)

    def test_redo_shortcuts_match_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onKeyDown")
        end = source.index("\nfunction onKeyUp", start)
        body = source[start:end]
        self.assertIn("e.shiftKey ? editor.redo() : editor.undo()", body)
        self.assertNotIn('e.key.toLowerCase() === "y") {\n    e.preventDefault();\n    applyEditorHistoryChange(() => editor.redo())', body)
        self.assertIn('e.key.toLowerCase() === "y"', body)
        self.assertIn('reportUnavailableBackgroundTrace("quiver", false)', body)

    def test_canvas_double_click_matches_xilem_text_buffer_order(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onCanvasDoubleClick")
        end = source.index("\n// ---------------------------------------------------------------------", start)
        body = source[start:end]
        point_hit = body.index("editor.togglePointTypeAt")
        component_hit = body.index("editor.componentBaseAt")
        activate_sort = body.index("editor.activateTextSortAt")
        self.assertLess(point_hit, component_hit)
        self.assertLess(component_hit, activate_sort)
        before_point = body[:point_hit]
        self.assertNotIn('activeTool.value === "Select"', before_point)
        self.assertIn("hasTextBufferSession.value", body)
        self.assertIn("insertInactiveTextGlyphByName(baseName)", body)
        self.assertIn("editor.clearComponentSelection()", body)
        self.assertIn("refreshSelectionState()", body)
        self.assertNotIn("openGlyph(baseName)", body)
        after_component = body[component_hit:]
        self.assertNotIn('activeTool.value === "Select"', after_component)
        self.assertNotIn('activeTool.value === "Text"', after_component)

        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("js_name = clearComponentSelection", wasm)
        self.assertIn("self.state.clear_component_selection();", wasm)

    def test_text_mode_keyboard_matches_xilem_arrow_scope(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function handleTextToolKey")
        end = source.index("\nfunction handleResize", start)
        body = source[start:end]
        self.assertIn('case "ArrowLeft":', body)
        self.assertIn('case "ArrowRight":', body)
        self.assertNotIn('case "ArrowUp":', body)
        self.assertNotIn('case "ArrowDown":', body)
        self.assertNotIn('case "Home":', body)
        self.assertNotIn('case "End":', body)

        keydown_start = source.index("function onKeyDown")
        keydown_end = source.index("\nfunction onKeyUp", keydown_start)
        keydown_body = source[keydown_start:keydown_end]
        nudge_start = keydown_body.index("const nudge =")
        nudge_body = keydown_body[nudge_start:]
        self.assertNotIn("!textModeActive.value", nudge_body)

        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("setGlyphGlifWithComponentsPreserveHistory", wasm)
        self.assertIn("set_glyph_from_norad_preserving_history", wasm)
        self.assertNotIn("setTextCursor", source)
        self.assertNotIn("set_text_cursor", wasm)
        self.assertNotIn("textBufferLen", wasm)
        self.assertNotIn("textCursor", wasm)
        self.assertNotIn("textActiveSort", source)
        self.assertNotIn("textActiveSort", wasm)
        self.assertNotIn("activateTextSort(index", source)
        self.assertNotIn("activate_text_sort(&mut self, index", wasm)
        self.assertNotIn("moveTextCursorVisualUp", wasm)
        self.assertNotIn("moveTextCursorVisualDown", wasm)
        self.assertNotIn("moveTextCursorLineStart", wasm)
        self.assertNotIn("moveTextCursorLineEnd", wasm)

        text_core = (ROOT / "rust-core" / "src" / "text.rs").read_text(encoding="utf-8")
        self.assertNotIn("move_cursor_visual_up", text_core)
        self.assertNotIn("move_cursor_visual_down", text_core)
        self.assertNotIn("move_cursor_line_start", text_core)
        self.assertNotIn("move_cursor_line_end", text_core)

    def test_text_mode_key_order_matches_xilem_shortcuts_then_text_then_nudge(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function onKeyDown")
        end = source.index("\nfunction onKeyUp", start)
        body = source[start:end]

        undo = body.index('if (meta && e.key.toLowerCase() === "z")')
        save = body.index('e.key.toLowerCase() === "s"')
        text = body.index("if (handleTextToolKey(e))")
        nudge = body.index("const nudge =")
        self.assertLess(undo, text)
        self.assertLess(save, text)
        self.assertLess(text, nudge)
        self.assertIn(
            '} else if ((e.key === "Backspace" || e.key === "Delete") && !textModeActive.value) {',
            body,
        )

    def test_text_preview_split_stays_visible_outside_text_tool(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn(
            "const textBufferPreviewVisible = computed(() => hasTextBufferSession.value)",
            source,
        )
        preview_start = source.index("const editorBottomPreviewVisible = computed")
        preview_end = source.index("const editorBottomPreviewStyle", preview_start)
        preview_body = source[preview_start:preview_end]
        self.assertIn("textBufferPreviewVisible.value", preview_body)
        self.assertNotIn('activeTool.value === "Text"', preview_body)
        self.assertIn(
            "'text-buffer-visible': viewMode === 'editor' && textBufferPreviewVisible",
            source,
        )
        self.assertIn(
            ':aria-label="textBufferPreviewVisible ? \'Text preview\' : \'Active glyph filled preview\'"',
            source,
        )
        self.assertIn('v-if="textBufferPreviewVisible"', source)
        self.assertNotIn('class="text-empty"', source)
        self.assertNotIn(">Text\n", source)
        self.assertNotIn("function openTextSort", source)

    def test_text_session_history_refreshes_outside_text_tool(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        start = source.index("function applyEditorHistoryChange")
        end = source.index("\nfunction copySelection", start)
        body = source[start:end]
        self.assertIn('activeTool.value === "Text" || hasTextBufferSession.value', body)
        self.assertIn("refreshTextStateFromEditor();", body)
        self.assertIn("syncCurrentGlyphBytesFromEditor({ skipUnchanged: true })", body)
        self.assertIn("if (glyphChanged) {\n    markGlyphDirty(currentGlyph.value);\n  }", body)
        self.assertNotIn('if (activeTool.value === "Text")', body)
        self.assertNotIn("syncCurrentGlyphBytesFromEditor();\n  markGlyphDirty(currentGlyph.value);", body)

    def test_empty_text_session_remains_text_active_like_xilem(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("const hasTextBufferSession = ref<boolean>(false)", source)
        self.assertIn("hasTextSession: boolean;", source)
        self.assertIn(
            'const textModeActive = computed(() => activeTool.value === "Text" && hasTextBufferSession.value)',
            source,
        )
        refresh_start = source.index("function refreshTextStateFromEditor")
        refresh_end = source.index("\nfunction insertTextCharacter", refresh_start)
        refresh_body = source[refresh_start:refresh_end]
        self.assertIn("hasTextBufferSession.value = snapshot.hasTextSession", refresh_body)
        self.assertNotIn("snapshot.sorts.length > 0", refresh_body)
        self.assertIn("hasTextBufferSession.value = true;", source)
        self.assertIn("hasTextBufferSession.value = false;", source)
        self.assertNotIn('activeTool.value === "Text" && textBuffer.value.length > 0', source)

        editor = (ROOT / "rust-core" / "src" / "editor.rs").read_text(encoding="utf-8")
        self.assertIn("pub has_text_session: bool", editor)
        self.assertIn("has_text_session: false", editor)

        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        self.assertIn("self.state.has_text_session = false;", wasm)
        self.assertIn("self.state.has_text_session = true;", wasm)
        self.assertIn("has_text_session: bool", wasm)
        self.assertIn("has_text_session: self.state.has_text_session", wasm)
        self.assertIn("let text_mode_active = self.tool.is_text() && self.state.has_text_session;", wasm)
        self.assertIn("render(&self.state, self.tool.is_preview(), text_mode_active)", wasm)

        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertIn("let has_text_session = state.has_text_session;", renderer)
        draw_start = renderer.index("fn draw_text_buffer")
        draw_end = renderer.index("\n    fn draw_text_cursor", draw_start)
        draw_body = renderer[draw_start:draw_end]
        self.assertNotIn("state.text_buffer.is_empty()", draw_body)

        sync_start = source.index("function syncCurrentGlyphBytesFromEditor")
        sync_end = source.index("\nfunction markGlyphDirty", sync_start)
        sync_body = source[sync_start:sync_end]
        self.assertIn("if (hasTextBufferSession.value)", sync_body)
        self.assertNotIn("if (textBuffer.value.length > 0)", sync_body)

    def test_text_active_sort_edit_mode_strokes_outline_without_preview_fill(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        start = renderer.index("fn draw_text_buffer")
        end = renderer.index("\n    fn draw_text_cursor", start)
        body = renderer[start:end]
        active_branch_start = body.index("if render_active_editable")
        active_branch_end = body.index('} else {\n                let Some(glyph_name)', active_branch_start)
        active_branch = body[active_branch_start:active_branch_end]
        self.assertIn("let active_outline = editable_outline_path(state);", active_branch)
        self.assertIn("self.scene.stroke", active_branch)
        self.assertIn("self.theme.path_stroke", active_branch)
        self.assertIn("self.theme.component_selected_fill", active_branch)
        self.assertIn("COMPONENT_SELECTION_STROKE_PX", active_branch)
        self.assertIn("self.theme.text_cursor", active_branch)
        self.assertNotIn("text_active_fill", renderer)

        vue = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertNotIn("textActiveFill", vue)

    def test_selected_components_get_xilem_selection_outline(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertIn("const COMPONENT_SELECTION_STROKE_PX: f64 = 2.0;", renderer)

        normal_start = renderer.index("for component in &state.component_previews")
        normal_end = renderer.index("self.draw_edit_controls(state, glyph_view);", normal_start)
        normal_branch = renderer[normal_start:normal_end]
        self.assertIn("self.theme.component_selected_fill", normal_branch)
        self.assertIn("COMPONENT_SELECTION_STROKE_PX", normal_branch)
        self.assertIn("self.theme.text_cursor", normal_branch)

    def test_text_cursor_renders_only_during_active_text_entry_like_xilem(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        start = renderer.index("fn draw_text_buffer")
        end = renderer.index("\n    fn draw_text_cursor", start)
        body = renderer[start:end]
        self.assertIn("if !preview_mode && text_mode_active {\n            self.draw_text_cursor", body)
        self.assertIn("const TEXT_CURSOR_LINE_PX: f64 = 1.5;", renderer)
        self.assertNotIn("const TEXT_CURSOR_LINE_PX: f64 = 1.0 * STROKE_SCALE;", renderer)

    def test_text_cursor_does_not_use_unwired_xilem_blink_state(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("editor?.render();", source)
        self.assertNotIn("textCursorBlinkVisible", source)
        self.assertNotIn("textCursorBlinkTimer", source)
        self.assertNotIn("resetInsertionCursorBlink", source)

        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertNotIn("text_cursor_visible", renderer)

    def test_text_sort_metrics_fall_back_to_session_bounds_like_xilem(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        start = renderer.index("fn draw_text_sort_metrics")
        end = renderer.index("\n    /// Draw thin lines", start)
        body = renderer[start:end]
        self.assertIn("let (ascender, descender) = state.text_metric_bounds();", body)
        self.assertIn("let mut ys = vec![0.0, ascender, descender];", body)
        self.assertIn("ys.extend([metrics.x_height, metrics.cap_height].into_iter().flatten());", body)
        self.assertNotIn("let Some(metrics) = state.metrics.as_ref() else", body)

    def test_text_preview_svg_inherits_panel_color(self) -> None:
        wasm = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        start = wasm.index("pub fn text_buffer_preview_svg")
        end = wasm.index("\n    #[wasm_bindgen", start + 1)
        body = wasm[start:end]
        self.assertIn('fill="currentColor"', body)
        self.assertNotIn('fill="#ffdd33"', body)
        self.assertIn("for path in paths", body)
        self.assertIn("path.to_svg()", body)
        self.assertNotIn("combined_path", body)

        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn(".text-preview-glyphs", source)
        self.assertIn("color: #ffdd33;", source)
        self.assertIn("fill: currentColor !important;", source)

    def test_text_cursor_defaults_to_xilem_selection_color(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertIn("const TEXT_CURSOR: Srgb = srgb(theme::selection::RECT_STROKE);", renderer)
        self.assertNotIn("const TEXT_CURSOR: Srgb = srgb(theme::cursor::TEXT);", renderer)

        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn(
            'textCursor: resolveHostColor("--rb-canvas-text-cursor", [0xff, 0xaa, 0x33, 0xff])',
            source,
        )
        self.assertIn("--rb-canvas-text-cursor:        #ffaa33;", source)

    def test_design_grid_levels_appear_later_to_reduce_canvas_noise(self) -> None:
        renderer = (ROOT / "rust-core" / "src" / "renderer.rs").read_text(encoding="utf-8")
        self.assertIn("const DESIGN_GRID_MID_MIN_ZOOM: f64 = 2.5;", renderer)
        self.assertIn("const DESIGN_GRID_CLOSE_MIN_ZOOM: f64 = 12.0;", renderer)
        self.assertIn("intentionally appear later to avoid filling the screen with grid", renderer)
        self.assertNotIn("const DESIGN_GRID_MID_MIN_ZOOM: f64 = 0.8;", renderer)
        self.assertNotIn("const DESIGN_GRID_CLOSE_MIN_ZOOM: f64 = 4.0;", renderer)

    def test_text_direction_toolbar_uses_xilem_shared_toolbar_constants(self) -> None:
        source = (ROOT / "web" / "src" / "components" / "TextDirectionToolbar.vue").read_text(
            encoding="utf-8"
        )
        self.assertIn('["ltr", "Left to Right"]', source)
        self.assertIn('["rtl", "Right to Left"]', source)
        self.assertLess(
            source.index('["ltr", "Left to Right"]'),
            source.index('["rtl", "Right to Left"]'),
        )
        self.assertIn('ltr: "text-ltr"', source)
        self.assertIn('rtl: "text-rtl"', source)

        start = source.index(".text-direction-toolbar {")
        end = source.index("\n}", start)
        toolbar_block = source[start:end]
        self.assertIn("border: 1.5px solid", toolbar_block)
        self.assertIn("border-radius: 8px;", toolbar_block)
        self.assertIn("padding: 8px;", toolbar_block)
        self.assertIn("gap: 6px;", toolbar_block)

        button_start = source.index(".direction-btn {")
        button_end = source.index("\n}", button_start)
        button_block = source[button_start:button_end]
        self.assertIn("width: 48px;", button_block)
        self.assertIn("height: 48px;", button_block)
        self.assertIn("border: 1.5px solid", button_block)
        self.assertIn("border-radius: 6px;", button_block)

    def test_text_preview_inventory_keeps_grid_scaled_svgs_after_edits(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("function gridGlyphSvgWithComponents", source)
        self.assertIn("glifMapToSvgs(glyphXmlMapJson(glyphBytes), unitsPerEm)", source)
        self.assertIn("glifToSvgWithComponents(bytes, glyphXmlMapJson(glyphBytes)) || glifToSvg(bytes)", source)
        self.assertNotIn("const svg = glifToSvg(bytes);", source)

        for name, next_anchor in [
            ("onActiveGlyphUnicodeChange", "\nfunction onActiveGlyphNameChange"),
            ("onActiveGlyphNameChange", "\nfunction eventTargetAcceptsText"),
            ("pasteGridGlyph", "\nfunction selectedGridGlyphNames"),
            ("setMarkOnSelected", "\nfunction backToGrid"),
            ("syncCurrentGlyphBytesFromEditor", "\nfunction markGlyphDirty"),
        ]:
            start = source.index(f"function {name}")
            end = source.index(next_anchor, start)
            body = source[start:end]
            self.assertIn("gridGlyphSvgWithComponents(", body, name)
            self.assertIn("data.glyphBytes", body, name)
            self.assertIn("data.unitsPerEm", body, name)

        paste_start = source.index("function pasteGridGlyph")
        paste_end = source.index("\nfunction selectedGridGlyphNames", paste_start)
        paste_body = source[paste_start:paste_end]
        self.assertIn("syncTextSortsForGlyph(name, name, metadata)", paste_body)
        self.assertIn("syncTextKerningModelToEditor();", paste_body)
        self.assertIn("bumpTextPreviewRevision();", paste_body)

    def test_active_text_sort_drives_editor_reload(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")

        tool_start = source.index("function onToolSelect")
        tool_end = source.index("\nfunction eventTargetAcceptsText", tool_start)
        tool_body = source[tool_start:tool_end]
        self.assertIn('activeTool.value === "Text" && activeTextSortIndex.value !== null', tool_body)
        self.assertIn('if (wasTextSession && tool !== "Text")', tool_body)
        self.assertIn("loadActiveTextSortGlyphIntoEditor();", tool_body)
        self.assertIn("setGlyphGlifWithComponentsPreserveHistory", source)
        self.assertIn("preserveHistory: true", source)

        load_start = source.index("function loadGlyphIntoEditor")
        load_end = source.index("\nfunction loadActiveTextSortGlyphIntoEditor", load_start)
        load_body = source[load_start:load_end]
        self.assertIn("if (options.preserveHistory)", load_body)
        self.assertIn("} else {\n      editor.setGlyphGlifWithComponents(bytes, glyphXmlByName);\n      coordinateQuadrant.value = \"cc\";", load_body)
        preserve_branch = load_body[
            load_body.index("if (options.preserveHistory)"):load_body.index("} else {")
        ]
        self.assertNotIn("coordinateQuadrant.value", preserve_branch)

        master_start = source.index("function activateMaster")
        master_end = source.index("\nfunction onSelectMaster", master_start)
        master_body = source[master_start:master_end]
        self.assertIn("const glyphToReload = textGlyphNameAt(activeTextSortIndex.value) ?? currentGlyph.value", master_body)
        self.assertIn("data.glyphBytes.get(glyphToReload)", master_body)
        self.assertIn("editor.setGlyphGlifWithComponentsPreserveHistory", master_body)
        self.assertNotIn("editor.setGlyphGlifWithComponents(bytes", master_body)
        self.assertNotIn('coordinateQuadrant.value = "cc"', master_body)
        self.assertIn("currentGlyph.value = glyphToReload", master_body)
        self.assertIn("selectedGlyph.value = glyphToReload", master_body)
        self.assertIn("selectedGlyphs.value = new Set([glyphToReload])", master_body)
        self.assertNotIn("data.glyphBytes.get(currentGlyph.value)", master_body)

    def test_wasm_shape_text_buffer_respects_text_direction(self) -> None:
        source = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        start = source.index("pub fn shape_text_buffer")
        next_binding = source.find("\n    #[wasm_bindgen", start + 1)
        body = source[start:next_binding if next_binding >= 0 else len(source)]
        self.assertIn("shape_arabic_if_rtl()", body)
        self.assertNotIn("shape_arabic()", body)

    def test_text_sort_activation_requires_text_session_like_xilem(self) -> None:
        source = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        start = source.index("pub fn activate_text_sort_at")
        next_binding = source.find("\n    #[wasm_bindgen", start + 1)
        body = source[start:next_binding if next_binding >= 0 else len(source)]
        self.assertIn("if !self.state.has_text_session", body)
        self.assertIn("return false;", body)
        self.assertIn(".activate_sort_at(", body)
        self.assertNotIn(".hit_test(", body)

        text = (ROOT / "rust-core" / "src" / "text.rs").read_text(encoding="utf-8")
        self.assertIn("pub struct TextSortActivation", text)
        self.assertIn("pub fn activate_sort_at", text)
        self.assertIn("hit active sort has a layout item", text)

    def test_text_manual_kerning_pointer_up_does_not_mark_unchanged_glyph_dirty(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("function bytesEqual", source)
        sync_start = source.index("function syncCurrentGlyphBytesFromEditor")
        sync_end = source.index("\nfunction markGlyphDirty", sync_start)
        sync_body = source[sync_start:sync_end]
        self.assertIn("options: { skipUnchanged?: boolean } = {}", sync_body)
        self.assertIn("if (options.skipUnchanged && bytesEqual(bytes, originalBytes))", sync_body)

        pointer_start = source.index("function onPointerUp")
        pointer_end = source.index("\nfunction onPointerCancel", pointer_start)
        pointer_body = source[pointer_start:pointer_end]
        self.assertIn("const glyphChanged = syncCurrentGlyphBytesFromEditor({ skipUnchanged: true });", pointer_body)
        self.assertIn("if (glyphChanged) {\n      markGlyphDirty(currentGlyph.value);", pointer_body)
        self.assertIn("syncTextKerningModelFromEditor(true);", pointer_body)

    def test_glyph_anatomy_preview_uses_edit_canvas_theme(self) -> None:
        source = (ROOT / "rust-core" / "src" / "wasm_api.rs").read_text(encoding="utf-8")
        start = source.index("fn anatomy_svg_from_bezpath_and_contours")
        end = source.index("\nfn build_event", start)
        anatomy = source[start:end]

        self.assertIn("var(--rb-canvas-path-stroke, #c0c0c0)", anatomy)
        self.assertIn("var(--rb-primary-text, #909090)", anatomy)
        self.assertIn("var(--rb-canvas-point-corner-inner, #181818)", anatomy)
        self.assertIn("var(--rb-canvas-point-corner-outer, #66ee88)", anatomy)
        self.assertIn("var(--rb-canvas-point-offcurve-inner, #181818)", anatomy)
        self.assertIn("var(--rb-canvas-point-offcurve-outer, #cc66ff)", anatomy)
        self.assertIn("unit_per_px = side / 300.0", anatomy)
        self.assertIn("smooth_radius = (4.5 * unit_per_px)", anatomy)
        self.assertIn("offcurve_radius = (3.0 * unit_per_px)", anatomy)
        self.assertIn("corner_half = (3.5 * unit_per_px)", anatomy)

    def test_glyph_grid_keeps_cell_outlines_inside_scrollport(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn("const gridViewportHeight = ref<number>(0);", source)
        self.assertIn("const glyphGridRowHeight = computed(() => {", source)
        self.assertIn("const targetRowHeight = 192;", source)
        self.assertIn(
            "Math.floor((height + bentoGap) / (targetRowHeight + bentoGap))",
            source,
        )
        self.assertIn("gridAutoRows: `${glyphGridRowHeight.value}px`,", source)
        self.assertIn("gridViewportHeight.value = rect?.height ?? 0;", source)

        start = source.index(".runebender-host {")
        end = source.index("\n}", start)
        host_css = source[start:end]
        self.assertNotIn("scrollbar-width", host_css)

        start = source.index(".grid-view {")
        end = source.index("\n}", start)
        grid_css = source[start:end]
        self.assertIn("scroll-snap-type: y mandatory;", grid_css)
        self.assertNotIn("grid-auto-rows: 192px;", grid_css)
        self.assertNotIn("padding-block", grid_css)

        start = source.index(".runebender-host :deep(::-webkit-scrollbar) {")
        end = source.index("\n}", start)
        scrollbar_css = source[start:end]
        self.assertIn("width: 6px;", scrollbar_css)
        self.assertIn("height: 6px;", scrollbar_css)

        start = source.index(".runebender-host :deep(::-webkit-scrollbar-thumb) {")
        end = source.index("\n}", start)
        scrollbar_thumb_css = source[start:end]
        self.assertIn("border: 1.5px solid transparent;", scrollbar_thumb_css)
        self.assertIn("background-clip: padding-box;", scrollbar_thumb_css)

        self.assertIn("@supports (-moz-appearance: none)", source)
        index_html = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
        self.assertIn("@supports (-moz-appearance: none)", index_html)
        self.assertIn("::-webkit-scrollbar { width: 6px; height: 6px;", index_html)

        cell = (ROOT / "web" / "src" / "components" / "GlyphCell.vue").read_text(
            encoding="utf-8",
        )
        start = cell.index(".cell {")
        end = cell.index("\n}", start)
        cell_css = cell[start:end]
        self.assertIn("scroll-snap-align: start;", cell_css)

    def test_runebender_vue_uses_host_service_for_comfy_routes(self) -> None:
        source = (ROOT / "web" / "src" / "Runebender.vue").read_text(encoding="utf-8")
        self.assertIn('inject(runebenderHostKey, browserHost)', source)
        self.assertNotIn("comfyHost", source)
        self.assertNotIn('fetch("/runebender/', source)
        self.assertNotIn("fetch(`/runebender/", source)

        main = (ROOT / "web" / "src" / "main.ts").read_text(encoding="utf-8")
        self.assertIn('import { browserHost } from "./hosts/browser/browserHost";', main)
        self.assertIn(".provide(runebenderHostKey, browserHost)", main)

        extension = (ROOT / "web" / "src" / "extension.ts").read_text(encoding="utf-8")
        self.assertIn('import { comfyHost } from "./hosts/comfy/comfyHost";', extension)
        self.assertIn("app.provide(runebenderHostKey, comfyHost);", extension)
        self.assertNotIn('fetch("/runebender/', extension)
        self.assertNotIn("`/runebender/", extension)

        host = (
            ROOT / "web" / "src" / "hosts" / "comfy" / "comfyHost.ts"
        ).read_text(encoding="utf-8")
        for route in [
            "/runebender/log",
            "/runebender/set_state",
            "/runebender/workspace/",
            "/runebender/workspaces",
            "/runebender/workspace/write",
            "/runebender/choose_source",
            "/runebender/link_source",
            "/runebender/workspace/save_as",
            "/runebender/workspace/invalidate",
        ]:
            self.assertIn(route, host)

        interface = (ROOT / "web" / "src" / "host" / "runebenderHost.ts").read_text(
            encoding="utf-8",
        )
        self.assertIn("export type RunebenderHost", interface)
        self.assertIn("export type WorkspaceChoice", interface)
        self.assertIn("export const runebenderHostKey", interface)
        self.assertIn("log?(level: string, message: string)", interface)
        self.assertIn("publishState(payload: RunebenderStatePayload)", interface)
        self.assertIn("loadWorkspaceSlot(slot: string)", interface)
        self.assertIn("refreshed_from_source?: boolean", interface)
        self.assertIn("listWorkspaceSlots(): Promise<WorkspaceChoice[]>", interface)
        self.assertIn("workspacePreviewUrl(slot: string, params: URLSearchParams)", interface)
        self.assertIn("writeWorkspaceFile(path: string, text: string)", interface)
        self.assertIn('chooseSource(mode?: "source" | "folder")', interface)
        self.assertIn("linkSource(args:", interface)
        self.assertIn("saveWorkspaceAs(args:", interface)
        self.assertIn("invalidateWorkspacePath(path: string)", interface)

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
        self.assertIn("Open Font Source", bundle)
        self.assertIn("Edit Font Source", bundle)
        self.assertNotIn("Import Font Source", bundle)
        self.assertNotIn("Import Copy Folder...", bundle)
        self.assertNotIn("Link Source Path...", bundle)
        self.assertNotIn("Import Copy File...", bundle)
        self.assertNotIn("Refresh Workspaces", bundle)
        self.assertIn("Close editor", bundle)
        self.assertIn("font input disconnect requested", bundle)
        self.assertIn("runebender/link_source", bundle)
        self.assertIn("workspace/invalidate", bundle)
        self.assertIn("rb-bundle-2026-05-24-sort-dblclick-81", bundle)
        # Grid thumbnail SVGs must come from one batched WASM call
        # (glifMapToSvgs) not 600+ per-glyph crossings.
        self.assertIn("glifMapToSvgs", bundle)
        self.assertIn("edited glyph grid SVG refresh failed", bundle)
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
        self.assertIn("Open font source", bundle)
        self.assertIn("Open for Editing", bundle)
        self.assertIn("Save workspace as", bundle)
        self.assertIn("Managed copy (workspace cache)", bundle)
        self.assertIn("reloaded source changes from disk", bundle)
        self.assertIn("runebender/workspace/save_as", bundle)
        self.assertIn("Choose Source...", bundle)
        self.assertNotIn("Choose File...", bundle)
        self.assertIn("runebender/choose_source", bundle)
        self.assertIn("showDirectoryPicker", bundle)
        self.assertNotIn("window.prompt", bundle)
        self.assertIn("Designspace", bundle)
        self.assertIn("Fully restart ComfyUI", bundle)
        self.assertIn("console.info(`[runebender-comfy] loaded", bundle)
        self.assertIn("z-index:20", bundle)
        self.assertIn("runebender-overlay-open", bundle)
        self.assertIn("#graph-canvas-container .selection-toolbox", bundle)
        self.assertIn("#graph-canvas-container .graph-canvas-panel > *", bundle)
        self.assertNotIn("z-index:9999", bundle)
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
        self.assertEqual(node["widgets_values"], ["", "auto", ""])
        self.assertEqual([output["type"] for output in node["outputs"]], ["FONT", "STRING"])


if __name__ == "__main__":
    unittest.main()
