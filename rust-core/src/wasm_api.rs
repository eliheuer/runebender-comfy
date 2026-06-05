// wasm-bindgen surface — the JS/Vue-facing API. Holds the editor
// state, the mouse state machine, the active tool, the renderer, and
// the undo stack. (wasm32 gating lives in lib.rs.)

use kurbo::{Affine, BezPath, Line, Point, Rect, Shape, Vec2};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use runebender_core::{GlyphCategory, GlyphMetadata, mark_color};

use crate::editing::{Modifiers, Mouse, MouseButton, MouseEvent, UndoState};
use crate::editor::{
    AnchorPoint, ComponentPreview, EditorState, KnifePreview, MeasurePreview, NudgeSelectionResult,
    PenPreview, SegmentHoverPreview, ShapePreview, norad_glyph_to_bezpath,
};
use crate::model::workspace::{
    Contour as WsContour, ContourPoint as WsContourPoint, PointType as WsPointType,
};
use crate::path::Quadrant;
use crate::renderer::Renderer;
use crate::text::{TextDirection, TextGlyphInventory, TextKerningModel, TextSortKind};
use crate::tool::{ActiveTool, ShapeKind};

type GlifXmlMap = HashMap<String, String>;
type CompatMasterGlyphMap = HashMap<String, Option<String>>;

fn text_codepoint_from_wasm(codepoint: u32) -> Option<char> {
    if codepoint == 0 {
        None
    } else {
        char::from_u32(codepoint)
    }
}

fn glyph_bytes_fingerprint(bytes: &[u8]) -> GlyphBytesFingerprint {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    GlyphBytesFingerprint {
        len: bytes.len(),
        hash,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextBufferSnapshot {
    has_text_session: bool,
    cursor: usize,
    active_sort: Option<usize>,
    direction: &'static str,
    sorts: Vec<TextSortSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextLayoutSnapshot {
    cursor_x: f64,
    cursor_y: f64,
    items: Vec<TextLayoutItemSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextLayoutItemSnapshot {
    index: usize,
    x: f64,
    y: f64,
    advance_width: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextBufferStateSnapshot {
    buffer: TextBufferSnapshot,
    layout: TextLayoutSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextSortSnapshot {
    kind: &'static str,
    glyph_name: Option<String>,
    char: Option<String>,
    codepoint: Option<u32>,
    advance_width: Option<f64>,
    active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GlyphBytesFingerprint {
    len: usize,
    hash: u64,
}

#[derive(Clone)]
struct SourceGlyphCache {
    fingerprint: Option<GlyphBytesFingerprint>,
    glyph: norad::Glyph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnchorSnapshot {
    name: Option<String>,
    x: f64,
    y: f64,
}

/// Compare an active `.glif` against the same glyph in other masters.
///
/// `master_glyph_xml_by_name` is JSON shaped as
/// `{ "Bold": "<glyph .../>", "Condensed": null }`; `null` reports a
/// missing glyph for that master. The return value is a JSON array of
/// structured compatibility errors.
#[wasm_bindgen(js_name = glifCompatibility)]
pub fn glif_compatibility(
    active_bytes: &[u8],
    glyph_name: &str,
    master_glyph_xml_by_name: &str,
) -> Result<String, JsValue> {
    let reference = norad::Glyph::parse_raw(active_bytes)
        .map_err(|e| JsValue::from_str(&format!("parse active .glif: {e}")))?;
    let master_xml: CompatMasterGlyphMap = serde_json::from_str(master_glyph_xml_by_name)
        .map_err(|e| JsValue::from_str(&format!("parse compat master map: {e}")))?;
    let masters = master_xml
        .into_iter()
        .map(|(master_name, xml)| {
            let glyph = xml
                .filter(|xml| !xml.trim().is_empty())
                .map(|xml| {
                    norad::Glyph::parse_raw(xml.as_bytes())
                        .map_err(|e| JsValue::from_str(&format!("parse {master_name} .glif: {e}")))
                })
                .transpose()?;
            Ok((master_name, glyph))
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    let errors = crate::editing::compat::check_compat(glyph_name, &reference, &masters);
    serde_json::to_string(&errors)
        .map_err(|e| JsValue::from_str(&format!("serialize compat errors: {e}")))
}

/// Map a Unicode codepoint to the matching `GlyphCategory`, returned
/// as its `display_name` ("Letter", "Number", …). Uses the same
/// mapping as runebender-xilem (both go through
/// `runebender_core::GlyphCategory`).
///
/// Returns `"Other"` for codepoints outside the BMP-safe `char`
/// range — the JS side defaults to that anyway for glyphs without
/// a `<unicode>` element.
#[wasm_bindgen(js_name = glyphCategoryForCodepoint)]
pub fn glyph_category_for_codepoint(cp: u32) -> String {
    let cat = char::from_u32(cp)
        .map(GlyphCategory::from_codepoint)
        .unwrap_or(GlyphCategory::Other);
    cat.display_name().to_string()
}

/// Parse a .glif file's bytes and return lightweight metadata as
/// JSON. This lets the grid/info sidebar inspect selected glyphs
/// without loading them into the editor or disturbing undo state.
#[wasm_bindgen(js_name = glifMetadata)]
pub fn glif_metadata(bytes: &[u8]) -> Result<String, JsValue> {
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let metadata = glif_metadata_from_norad(&glyph);
    serde_json::to_string(&metadata)
        .map_err(|e| JsValue::from_str(&format!("serialize metadata: {e}")))
}

fn glyph_metadata_from_norad(glyph: &norad::Glyph) -> GlyphMetadata {
    let unicodes = glyph
        .codepoints
        .iter()
        .map(|c| format!("{:04X}", c as u32))
        .collect();
    GlyphMetadata::new(
        glyph.name().to_string(),
        glyph.width,
        glyph.contours.len(),
        unicodes,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GlifMetadata {
    #[serde(flatten)]
    glyph: GlyphMetadata,
    left_kerning_group: Option<String>,
    right_kerning_group: Option<String>,
}

fn glif_metadata_from_norad(glyph: &norad::Glyph) -> GlifMetadata {
    GlifMetadata {
        glyph: glyph_metadata_from_norad(glyph),
        left_kerning_group: glyph
            .lib
            .get("public.kern1")
            .and_then(|value| value.as_string())
            .map(ToString::to_string),
        right_kerning_group: glyph
            .lib
            .get("public.kern2")
            .and_then(|value| value.as_string())
            .map(ToString::to_string),
    }
}

/// Update only the UFO `public.markColor` lib entry in a .glif file.
/// This is used for grid/sidebar mark-color edits that do not load
/// the glyph into the outline editor.
#[wasm_bindgen(js_name = glifWithMarkColor)]
pub fn glif_with_mark_color(bytes: &[u8], mark_color: &str) -> Result<Vec<u8>, JsValue> {
    let mut glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let mark_color = mark_color::canonical_ufo_mark_color(mark_color)
        .ok_or_else(|| JsValue::from_str("invalid UFO public.markColor value"))?;
    if mark_color.is_empty() {
        glyph.lib.remove("public.markColor");
    } else {
        glyph
            .lib
            .insert("public.markColor".to_string(), mark_color.into());
    }
    glyph
        .encode_xml()
        .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
}

/// Update one UFO kerning group lib entry in a .glif file. `side`
/// accepts `left`/`public.kern1` or `right`/`public.kern2`; an empty
/// group or `-` clears that lib entry, matching xilem's active panel.
#[wasm_bindgen(js_name = glifWithKerningGroup)]
pub fn glif_with_kerning_group(bytes: &[u8], side: &str, group: &str) -> Result<Vec<u8>, JsValue> {
    let mut glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let key = match side {
        "left" | "public.kern1" => "public.kern1",
        "right" | "public.kern2" => "public.kern2",
        _ => {
            return Err(JsValue::from_str(
                "kerning group side must be left or right",
            ));
        }
    };
    if group.is_empty() || group == "-" {
        glyph.lib.remove(key);
    } else {
        glyph.lib.insert(key.to_string(), group.to_string().into());
    }
    glyph
        .encode_xml()
        .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
}

/// Update the first Unicode codepoint in a .glif file. Empty input
/// clears codepoints; otherwise `unicode` accepts `0041`, `U+0041`,
/// or `0x41`.
#[wasm_bindgen(js_name = glifWithUnicode)]
pub fn glif_with_unicode(bytes: &[u8], unicode: &str) -> Result<Vec<u8>, JsValue> {
    let mut glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let trimmed = unicode.trim();
    if trimmed.is_empty() {
        glyph.codepoints.clear();
    } else {
        let hex = trimmed
            .strip_prefix("U+")
            .or_else(|| trimmed.strip_prefix("u+"))
            .or_else(|| trimmed.strip_prefix("0x"))
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);
        let value = u32::from_str_radix(hex, 16)
            .map_err(|_| JsValue::from_str("unicode must be hexadecimal"))?;
        let codepoint = char::from_u32(value)
            .ok_or_else(|| JsValue::from_str("unicode is not a valid codepoint"))?;
        glyph.codepoints.set([codepoint]);
    }
    glyph
        .encode_xml()
        .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
}

/// Update the glyph name in a .glif file while preserving the rest
/// of the glyph data through norad's data model.
#[wasm_bindgen(js_name = glifWithName)]
pub fn glif_with_name(bytes: &[u8], name: &str) -> Result<Vec<u8>, JsValue> {
    if norad::Name::new(name).is_err() {
        return Err(JsValue::from_str("glyph name is invalid"));
    }
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let mut renamed = norad::Glyph::new(name);
    renamed.height = glyph.height;
    renamed.width = glyph.width;
    renamed.codepoints = glyph.codepoints;
    renamed.note = glyph.note;
    renamed.guidelines = glyph.guidelines;
    renamed.anchors = glyph.anchors;
    renamed.components = glyph.components;
    renamed.contours = glyph.contours;
    renamed.image = glyph.image;
    renamed.lib = glyph.lib;
    renamed
        .encode_xml()
        .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
}

/// Copy only outline data from one `.glif` into another, preserving
/// target glyph identity/metadata. Used by xilem-style grid copy/paste.
#[wasm_bindgen(js_name = glifWithOutlinesFrom)]
pub fn glif_with_outlines_from(
    source_bytes: &[u8],
    target_bytes: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let source = norad::Glyph::parse_raw(source_bytes)
        .map_err(|e| JsValue::from_str(&format!("parse source .glif: {e}")))?;
    let mut target = norad::Glyph::parse_raw(target_bytes)
        .map_err(|e| JsValue::from_str(&format!("parse target .glif: {e}")))?;

    target.width = source.width;
    target.contours = source.contours;
    target.components = source.components;

    target
        .encode_xml()
        .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
}

/// Parse a .glif file's bytes and return an SVG string fit for an
/// `<img>` or inline render in the glyph grid. Uses the same
/// norad → BezPath path that the live editor uses, then wraps in a
/// viewBox sized to the glyph's own bbox with a Y-flip so UFO's
/// y-up coordinates display correctly.
#[wasm_bindgen(js_name = glifToSvg)]
pub fn glif_to_svg(bytes: &[u8]) -> Result<String, JsValue> {
    let stroke_icon = is_runebender_stroke_icon_xml(bytes);
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let bez = norad_glyph_to_bezpath(&glyph);
    if stroke_icon {
        svg_from_stroke_icon_bezpath(&bez)
    } else {
        svg_from_bezpath(&bez)
    }
}

/// Parse a .glif file's bytes and return an SVG with UFO components
/// resolved against a JSON object of `{ glyphName: glifXml }`.
/// This mirrors xilem's grid/preview behavior for composite glyphs.
#[wasm_bindgen(js_name = glifToSvgWithComponents)]
pub fn glif_to_svg_with_components(
    bytes: &[u8],
    glyph_xml_by_name: &str,
) -> Result<String, JsValue> {
    let stroke_icon = is_runebender_stroke_icon_xml(bytes);
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let glyphs = parse_glif_xml_map(glyph_xml_by_name)?;

    let mut bez = norad_glyph_to_bezpath(&glyph);
    append_norad_components_to_bezpath(&mut bez, &glyph, &glyphs, Affine::IDENTITY, 0);
    if stroke_icon {
        svg_from_stroke_icon_bezpath(&bez)
    } else {
        svg_from_bezpath(&bez)
    }
}

/// Parse one .glif file and return a grid-thumbnail SVG with a constant
/// em-based vertical viewBox, resolving components against
/// `{ glyphName: glifXml }`.
///
/// This is the one-glyph version of `glifMapToSvgs`: edited glyph refreshes
/// should not render every glyph in a master just to update one thumbnail.
#[wasm_bindgen(js_name = glifToGridSvgWithComponents)]
pub fn glif_to_grid_svg_with_components(
    bytes: &[u8],
    glyph_xml_by_name: &str,
    units_per_em: f64,
) -> Result<String, JsValue> {
    let stroke_icon = is_runebender_stroke_icon_xml(bytes);
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let glyphs = parse_glif_xml_map(glyph_xml_by_name)?;

    let mut bez = norad_glyph_to_bezpath(&glyph);
    append_norad_components_to_bezpath(&mut bez, &glyph, &glyphs, Affine::IDENTITY, 0);
    if stroke_icon {
        svg_from_stroke_icon_bezpath(&bez)
    } else {
        let upm = if units_per_em > 0.0 {
            units_per_em
        } else {
            1000.0
        };
        svg_from_bezpath_em(&bez, upm)
    }
}

/// Batch-convert every glyph in a master to SVG thumbnails for the
/// grid view. Takes a JSON object `{ glyphName: glifXml }` and returns
/// a JSON object `{ glyphName: svgString }`.
///
/// Equivalent to calling `glif_to_svg_with_components` once per glyph
/// from JS, but does the work in a single WASM call so we avoid 600+
/// JS↔WASM boundary crossings per master. Profiling showed those
/// crossings, not the actual SVG generation, dominated the edit-to-grid
/// load time (~1.2 s/master in JS, vs ~50 ms in Rust for the same work).
/// Glyphs that fail to parse are silently skipped, mirroring the
/// per-call wrapper's behavior so a single malformed .glif can't sink
/// the whole grid.
#[wasm_bindgen(js_name = glifMapToSvgs)]
pub fn glif_map_to_svgs(glyph_xml_by_name: &str, units_per_em: f64) -> Result<String, JsValue> {
    let xml_by_name: GlifXmlMap = serde_json::from_str(glyph_xml_by_name)
        .map_err(|e| JsValue::from_str(&format!("parse glyph XML map: {e}")))?;

    // Parse all glyphs once so component references can resolve.
    let mut glyphs: HashMap<String, norad::Glyph> = HashMap::with_capacity(xml_by_name.len());
    for (name, xml) in &xml_by_name {
        if let Ok(glyph) = norad::Glyph::parse_raw(xml.as_bytes()) {
            glyphs.insert(name.clone(), glyph);
        }
    }

    let upm = if units_per_em > 0.0 {
        units_per_em
    } else {
        1000.0
    };
    let mut svgs: HashMap<String, String> = HashMap::with_capacity(glyphs.len());
    for (name, glyph) in &glyphs {
        let mut bez = norad_glyph_to_bezpath(glyph);
        append_norad_components_to_bezpath(&mut bez, glyph, &glyphs, Affine::IDENTITY, 0);
        let stroke_icon = xml_by_name
            .get(name)
            .is_some_and(|xml| is_runebender_stroke_icon_xml(xml.as_bytes()));
        let svg_result = if stroke_icon {
            svg_from_stroke_icon_bezpath(&bez)
        } else {
            svg_from_bezpath_em(&bez, upm)
        };
        if let Ok(svg) = svg_result {
            if !svg.is_empty() {
                svgs.insert(name.clone(), svg);
            }
        }
    }

    serde_json::to_string(&svgs).map_err(|e| JsValue::from_str(&format!("serialize svgs: {e}")))
}

fn is_runebender_stroke_icon_xml(bytes: &[u8]) -> bool {
    let Ok(xml) = std::str::from_utf8(bytes) else {
        return false;
    };
    xml.contains("<key>com.runebender.iconRenderMode</key>")
        && xml.contains("<string>stroke</string>")
}

fn svg_from_stroke_icon_bezpath(bez: &BezPath) -> Result<String, JsValue> {
    if bez.elements().is_empty() {
        return Ok(String::new());
    }
    let bbox = bez.bounding_box();
    let pad = 2.0;
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet"><path d="{}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" vector-effect="non-scaling-stroke" transform="scale(1 -1)"/></svg>"#,
        bbox.x0 - pad,
        -bbox.y1 - pad,
        bbox.width() + pad * 2.0,
        bbox.height() + pad * 2.0,
        bez.to_svg(),
    ))
}

/// Grid-thumbnail SVG with a CONSTANT em-based vertical viewBox, so
/// every glyph in a master renders at the same scale and shares one
/// baseline — a period stays a small dot, an M stays tall. Mirrors
/// runebender-xilem's glyph_cell `paint_glyph`:
///   scale  = preview_height / UPM * 0.65   (em fills 65% of the box)
///   baseline at 20% from the bottom of the preview area
///
/// Expressed as a viewBox (path stays in font units, y-flipped by the
/// transform): the vertical extent is constant (UPM / 0.65) and the
/// horizontal extent is the glyph's own bbox (so the SVG's intrinsic
/// width reflects the glyph and it can be centered in the cell with
/// height:100%; width:auto). Contrast svg_from_bezpath, which uses the
/// per-glyph bbox for BOTH axes — that makes every glyph fill its cell
/// (wrong for a grid) and is kept only for single-glyph previews that
/// should fill their panel.
fn svg_from_bezpath_em(bez: &BezPath, upm: f64) -> Result<String, JsValue> {
    if bez.elements().is_empty() {
        return Ok(String::new());
    }
    let bbox = bez.bounding_box();
    // Vertical preview box in font units: em occupies 65% of it.
    let vb_height = upm / 0.65;
    // Baseline 20% up from the bottom. In flipped (svg y-down) space the
    // top of the box is at -(0.80 * vb_height) and it extends vb_height
    // downward, putting the baseline (y=0) 80% down = 20% from bottom.
    let vb_min_y = -0.80 * vb_height;
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet"><path d="{}" fill="currentColor" fill-rule="nonzero" transform="scale(1 -1)"/></svg>"#,
        bbox.x0,
        vb_min_y,
        bbox.width(),
        vb_height,
        bez.to_svg(),
    ))
}

fn parse_glif_xml_map(glyph_xml_by_name: &str) -> Result<HashMap<String, norad::Glyph>, JsValue> {
    let xml_by_name: GlifXmlMap = serde_json::from_str(glyph_xml_by_name)
        .map_err(|e| JsValue::from_str(&format!("parse glyph XML map: {e}")))?;
    let mut glyphs = HashMap::new();
    for (name, xml) in xml_by_name {
        if let Ok(glyph) = norad::Glyph::parse_raw(xml.as_bytes()) {
            glyphs.insert(name, glyph);
        }
    }
    Ok(glyphs)
}

fn svg_from_bezpath(bez: &BezPath) -> Result<String, JsValue> {
    if bez.elements().is_empty() {
        return Ok(String::new());
    }
    let bbox = bez.bounding_box();
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet"><path d="{}" fill="currentColor" fill-rule="nonzero" transform="scale(1 -1)"/></svg>"#,
        bbox.x0,
        -bbox.y1,
        bbox.width(),
        bbox.height(),
        bez.to_svg(),
    ))
}

fn append_norad_components_to_bezpath(
    path: &mut BezPath,
    glyph: &norad::Glyph,
    glyphs: &HashMap<String, norad::Glyph>,
    parent_transform: Affine,
    depth: usize,
) {
    if depth > 16 {
        return;
    }

    for component in &glyph.components {
        let base_name = component.base.to_string();
        let Some(base_glyph) = glyphs.get(&base_name) else {
            continue;
        };
        let t = &component.transform;
        let transform = parent_transform
            * Affine::new([
                t.x_scale, t.xy_scale, t.yx_scale, t.y_scale, t.x_offset, t.y_offset,
            ]);
        let base_path = norad_glyph_to_bezpath(base_glyph);
        let transformed = transform * &base_path;
        path.extend(transformed.elements().iter().cloned());
        append_norad_components_to_bezpath(path, base_glyph, glyphs, transform, depth + 1);
    }
}

fn build_component_previews(
    glyph: &norad::Glyph,
    glyphs: &HashMap<String, norad::Glyph>,
) -> Vec<ComponentPreview> {
    glyph
        .components
        .iter()
        .enumerate()
        .filter_map(|(index, component)| {
            let base_name = component.base.to_string();
            let base_glyph = glyphs.get(&base_name)?;
            let mut path = norad_glyph_to_bezpath(base_glyph);
            append_norad_components_to_bezpath(&mut path, base_glyph, glyphs, Affine::IDENTITY, 0);
            let t = &component.transform;
            let transform = Affine::new([
                t.x_scale, t.xy_scale, t.yx_scale, t.y_scale, t.x_offset, t.y_offset,
            ]);
            let transformed_path = Arc::new(transform * &path);
            let mut anchors = Vec::new();
            collect_component_anchors(base_glyph, glyphs, Affine::IDENTITY, 0, &mut anchors);
            Some(ComponentPreview {
                id: crate::model::EntityId::next(),
                index,
                base: base_name,
                transform,
                transformed_path,
                path: Arc::new(path),
                anchors,
                auto_align: !component_alignment_disabled(component),
            })
        })
        .collect()
}

fn collect_component_anchors(
    glyph: &norad::Glyph,
    glyphs: &HashMap<String, norad::Glyph>,
    parent_transform: Affine,
    depth: usize,
    out: &mut Vec<AnchorPoint>,
) {
    if depth > 16 {
        return;
    }
    glyph.components.iter().for_each(|component| {
        let base_name = component.base.to_string();
        let Some(base_glyph) = glyphs.get(&base_name) else {
            return;
        };
        let t = &component.transform;
        let transform = parent_transform
            * Affine::new([
                t.x_scale, t.xy_scale, t.yx_scale, t.y_scale, t.x_offset, t.y_offset,
            ]);
        for anchor in &base_glyph.anchors {
            let point = transform * Point::new(anchor.x, anchor.y);
            out.push(AnchorPoint {
                id: crate::model::EntityId::next(),
                index: out.len(),
                name: anchor.name.as_ref().map(ToString::to_string),
                point,
                color: anchor.color.clone(),
                identifier: anchor.identifier().cloned(),
                lib: anchor.lib().cloned(),
            });
        }
        collect_component_anchors(base_glyph, glyphs, transform, depth + 1, out);
    });
}

fn component_alignment_disabled(component: &norad::Component) -> bool {
    component
        .lib()
        .and_then(|lib| lib.get("com.glyphsapp.component.alignment"))
        .is_some_and(|value| {
            value.as_signed_integer().is_some_and(|value| value < 0)
                || value.as_boolean() == Some(false)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_GLIF: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<glyph name="A" format="2">
  <advance width="500"/>
</glyph>
"#;

    #[test]
    fn glif_with_kerning_group_round_trips_xilem_lib_fields() {
        let with_left = glif_with_kerning_group(SIMPLE_GLIF, "left", "public.kern1.A")
            .expect("left group update succeeds");
        let glyph = norad::Glyph::parse_raw(&with_left).expect("updated glif parses");
        assert_eq!(
            glyph
                .lib
                .get("public.kern1")
                .and_then(|value| value.as_string()),
            Some("public.kern1.A")
        );

        let cleared = glif_with_kerning_group(&with_left, "public.kern1", "-")
            .expect("left group clear succeeds");
        let glyph = norad::Glyph::parse_raw(&cleared).expect("cleared glif parses");
        assert!(glyph.lib.get("public.kern1").is_none());

        let with_right = glif_with_kerning_group(&cleared, "right", "bare-input")
            .expect("right group update succeeds");
        let glyph = norad::Glyph::parse_raw(&with_right).expect("right-updated glif parses");
        assert_eq!(
            glyph
                .lib
                .get("public.kern2")
                .and_then(|value| value.as_string()),
            Some("bare-input")
        );
    }

    #[test]
    fn to_norad_anchor_round_trips_ufo_anchor_fields() {
        let mut state = EditorState::default();
        let glyph = norad::Glyph::parse_raw(
            br#"<glyph name="A" format="2"><advance width="500"/><anchor x="250" y="700" name="top"/></glyph>"#,
        )
        .expect("valid glyph");
        state.set_glyph_from_norad(&glyph);
        state.select_anchor(state.anchors[0].id);
        assert!(state.translate_selected_anchor(Vec2::new(10.0, -20.0)));

        let mut serialized = glyph.clone();
        serialized.anchors = state
            .anchors
            .iter()
            .map(to_norad_anchor)
            .collect::<Result<Vec<_>, _>>()
            .expect("valid anchors");
        let reparsed = norad::Glyph::parse_raw(
            &serialized
                .encode_xml()
                .expect("serialized glyph encodes as XML"),
        )
        .expect("serialized glyph parses");

        assert_eq!(reparsed.anchors.len(), 1);
        assert_eq!(
            reparsed.anchors[0].name.as_ref().map(ToString::to_string),
            Some("top".to_string())
        );
        assert_eq!(reparsed.anchors[0].x, 260.0);
        assert_eq!(reparsed.anchors[0].y, 680.0);
    }

    #[test]
    fn component_anchors_are_propagated_through_component_transform() {
        let composite = norad::Glyph::parse_raw(
            br#"<glyph name="Aacute" format="2"><advance width="500"/><outline><component base="A" xOffset="25" yOffset="40"/></outline></glyph>"#,
        )
        .expect("valid composite");
        let base = norad::Glyph::parse_raw(
            br#"<glyph name="A" format="2"><advance width="500"/><anchor x="250" y="700" name="top"/></glyph>"#,
        )
        .expect("valid base");
        let glyphs = HashMap::from([("A".to_string(), base)]);

        let mut state = EditorState::default();
        state.set_glyph_from_norad(&composite);
        state.set_component_previews(build_component_previews(&composite, &glyphs));

        assert_eq!(state.propagated_anchors.len(), 1);
        assert_eq!(state.propagated_anchors[0].name.as_deref(), Some("top"));
        assert_eq!(state.propagated_anchors[0].point, Point::new(275.0, 740.0));
    }

    #[test]
    fn mark_component_aligns_to_previous_component_anchor() {
        let composite = norad::Glyph::parse_raw(
            br#"<glyph name="Aacute" format="2"><advance width="500"/><outline><component base="A"/><component base="acute"/></outline></glyph>"#,
        )
        .expect("valid composite");
        let base = norad::Glyph::parse_raw(
            br#"<glyph name="A" format="2"><advance width="500"/><anchor x="250" y="700" name="top"/></glyph>"#,
        )
        .expect("valid base");
        let mark = norad::Glyph::parse_raw(
            br#"<glyph name="acute" format="2"><advance width="200"/><anchor x="100" y="20" name="_top"/></glyph>"#,
        )
        .expect("valid mark");
        let glyphs = HashMap::from([("A".to_string(), base), ("acute".to_string(), mark)]);
        let mut state = EditorState::default();

        state.set_glyph_from_norad(&composite);
        state.set_component_previews(build_component_previews(&composite, &glyphs));

        let transform = state.component_transform(1).expect("mark transform");
        let coeffs = transform.as_coeffs();
        assert_eq!(coeffs[4], 150.0);
        assert_eq!(coeffs[5], 680.0);
    }

    #[test]
    fn disabled_glyphs_component_alignment_preserves_transform() {
        let composite = norad::Glyph::parse_raw(
            br#"<glyph name="Aacute" format="2"><advance width="500"/><outline><component base="A"/><component base="acute" xOffset="10" yOffset="30"><lib><dict><key>com.glyphsapp.component.alignment</key><integer>-1</integer></dict></lib></component></outline></glyph>"#,
        )
        .expect("valid composite");
        let base = norad::Glyph::parse_raw(
            br#"<glyph name="A" format="2"><advance width="500"/><anchor x="250" y="700" name="top"/></glyph>"#,
        )
        .expect("valid base");
        let mark = norad::Glyph::parse_raw(
            br#"<glyph name="acute" format="2"><advance width="200"/><anchor x="100" y="20" name="_top"/></glyph>"#,
        )
        .expect("valid mark");
        let glyphs = HashMap::from([("A".to_string(), base), ("acute".to_string(), mark)]);
        let mut state = EditorState::default();

        state.set_glyph_from_norad(&composite);
        state.set_component_previews(build_component_previews(&composite, &glyphs));

        let transform = state.component_transform(1).expect("mark transform");
        let coeffs = transform.as_coeffs();
        assert_eq!(coeffs[4], 10.0);
        assert_eq!(coeffs[5], 30.0);
    }
}

fn affine_to_norad_transform(transform: Affine) -> norad::AffineTransform {
    let coeffs = transform.as_coeffs();
    norad::AffineTransform {
        x_scale: coeffs[0],
        xy_scale: coeffs[1],
        yx_scale: coeffs[2],
        y_scale: coeffs[3],
        x_offset: coeffs[4],
        y_offset: coeffs[5],
    }
}

/// Parse a .glif file's bytes and return an "x-ray" anatomy SVG:
/// outline stroke, control-handle lines, and point markers. Mirrors
/// the xilem anatomy panel closely enough for preview/editing parity.
#[wasm_bindgen(js_name = glifAnatomySvg)]
pub fn glif_anatomy_svg(bytes: &[u8]) -> Result<String, JsValue> {
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let bez = norad_glyph_to_bezpath(&glyph);
    anatomy_svg_from_bezpath_and_contours(&bez, &glyph.contours)
}

/// Parse a .glif file's bytes and return an anatomy SVG with UFO
/// components resolved against a JSON object of `{ glyphName: glifXml }`.
#[wasm_bindgen(js_name = glifAnatomySvgWithComponents)]
pub fn glif_anatomy_svg_with_components(
    bytes: &[u8],
    glyph_xml_by_name: &str,
) -> Result<String, JsValue> {
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let glyphs = parse_glif_xml_map(glyph_xml_by_name)?;
    let mut bez = norad_glyph_to_bezpath(&glyph);
    append_norad_components_to_bezpath(&mut bez, &glyph, &glyphs, Affine::IDENTITY, 0);
    anatomy_svg_from_bezpath_and_contours(&bez, &glyph.contours)
}

fn anatomy_svg_from_bezpath_and_contours(
    bez: &BezPath,
    contours: &[norad::Contour],
) -> Result<String, JsValue> {
    if bez.elements().is_empty() {
        return Ok(String::new());
    }
    let outline_bbox = bez.bounding_box();
    let mut x0 = outline_bbox.x0;
    let mut y0 = outline_bbox.y0;
    let mut x1 = outline_bbox.x1;
    let mut y1 = outline_bbox.y1;
    for contour in contours {
        for pt in &contour.points {
            x0 = x0.min(pt.x);
            y0 = y0.min(pt.y);
            x1 = x1.max(pt.x);
            y1 = y1.max(pt.y);
        }
    }
    let width = (x1 - x0).max(1.0);
    let height = (y1 - y0).max(1.0);
    let side = width.max(height);
    let outline_stroke = 1.5;
    let handle_stroke = 1.875;
    let point_outline = 1.875;
    // Match the edit renderer's screen-space marker proportions, then
    // convert them into this SVG viewBox's design-space units.
    let unit_per_px = side / 300.0;
    let smooth_radius = (4.5 * unit_per_px).clamp(2.0, 8.0);
    let offcurve_radius = smooth_radius;
    let corner_half = (3.5 * unit_per_px).clamp(2.0, 7.0);
    // The anatomy panel is a small, read-only version of the edit view.
    // Its fit must include off-curve handles and point marker geometry,
    // not just the outline's path bounds, otherwise large glyphs clip at
    // panel edges even though the editor canvas has viewport padding.
    let padding = (smooth_radius.max(offcurve_radius).max(corner_half)
        + outline_stroke
        + handle_stroke
        + 4.0)
        .max(side * 0.035);
    x0 -= padding;
    y0 -= padding;
    x1 += padding;
    y1 += padding;

    let mut out = String::new();
    write!(
        &mut out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet">"#,
        x0,
        -y1,
        x1 - x0,
        y1 - y0
    )
    .expect("write svg header");
    out.push_str(r#"<g transform="scale(1 -1)">"#);
    write!(
        &mut out,
        r##"<path d="{}" fill="none" stroke="var(--rb-canvas-path-stroke, #c0c0c0)" stroke-width="{}" vector-effect="non-scaling-stroke" />"##,
        bez.to_svg(),
        outline_stroke
    )
    .expect("write outline");

    for contour in contours {
        let closed = !matches!(
            contour.points.first().map(|pt| pt.typ.clone()),
            Some(norad::PointType::Move)
        );
        let pts = &contour.points;
        if pts.len() < 2 {
            for pt in pts {
                write_point(
                    &mut out,
                    pt,
                    smooth_radius,
                    offcurve_radius,
                    corner_half,
                    point_outline,
                );
            }
            continue;
        }

        for (i, pt) in pts.iter().enumerate() {
            if matches!(pt.typ, norad::PointType::OffCurve) {
                continue;
            }

            let prev = if i > 0 {
                Some(&pts[i - 1])
            } else if closed {
                Some(&pts[pts.len() - 1])
            } else {
                None
            };
            if let Some(prev) = prev
                && matches!(prev.typ, norad::PointType::OffCurve)
            {
                write_handle(&mut out, prev.x, prev.y, pt.x, pt.y, handle_stroke);
            }

            let next = if i + 1 < pts.len() {
                Some(&pts[i + 1])
            } else if closed {
                Some(&pts[0])
            } else {
                None
            };
            if let Some(next) = next
                && matches!(next.typ, norad::PointType::OffCurve)
            {
                write_handle(&mut out, pt.x, pt.y, next.x, next.y, handle_stroke);
            }
        }

        for pt in pts {
            write_point(
                &mut out,
                pt,
                smooth_radius,
                offcurve_radius,
                corner_half,
                point_outline,
            );
        }
    }

    out.push_str("</g></svg>");
    Ok(out)
}

#[wasm_bindgen]
pub struct GlyphEditor {
    state: EditorState,
    mouse: Mouse,
    tool: ActiveTool,
    renderer: Renderer,
    undo: UndoState<EditorState>,
    point_clipboard: Option<Vec<crate::path::Path>>,
    component_glyphs: HashMap<String, norad::Glyph>,
    source_glyph: Option<SourceGlyphCache>,
    /// Snapshot of `state` taken on a left-button pointerdown, pushed
    /// onto the undo stack on the matching pointerup. `None` between
    /// strokes.
    pending_snapshot: Option<EditorState>,
    /// Snapshot of `state` before a keyboard nudge burst. Repeated
    /// arrow-key events reuse this one snapshot so large glyphs do not
    /// clone the full editor state on every key repeat.
    pending_nudge_snapshot: Option<EditorState>,
}

impl GlyphEditor {
    fn commit_pending_nudge_snapshot(&mut self) {
        let Some(snapshot) = self.pending_nudge_snapshot.take() else {
            return;
        };
        if self.state.edit_revision() != snapshot.edit_revision() {
            self.undo.add_undo_group(snapshot);
        }
    }

    fn discrete_edit_snapshot(&mut self) -> EditorState {
        self.commit_pending_nudge_snapshot();
        self.state.clone()
    }

    fn set_glyph_from_norad_with_component_cache(&mut self, glyph: &norad::Glyph) {
        self.state.set_glyph_from_norad(glyph);
        self.state
            .set_component_previews(build_component_previews(glyph, &self.component_glyphs));
    }

    fn set_glyph_from_norad_preserving_history_with_component_cache(
        &mut self,
        glyph: &norad::Glyph,
    ) {
        self.state.set_glyph_from_norad_preserving_history(glyph);
        self.state
            .set_component_previews(build_component_previews(glyph, &self.component_glyphs));
    }

    fn cache_source_glyph(
        &mut self,
        glyph: norad::Glyph,
        fingerprint: Option<GlyphBytesFingerprint>,
    ) {
        self.source_glyph = Some(SourceGlyphCache { fingerprint, glyph });
    }

    fn source_glyph_for_bytes(&mut self, original_bytes: &[u8]) -> Result<norad::Glyph, JsValue> {
        let fingerprint = glyph_bytes_fingerprint(original_bytes);
        if let Some(cache) = &self.source_glyph {
            if cache.fingerprint.is_none() || cache.fingerprint == Some(fingerprint) {
                return Ok(cache.glyph.clone());
            }
        }

        let glyph = norad::Glyph::parse_raw(original_bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.cache_source_glyph(glyph.clone(), Some(fingerprint));
        Ok(glyph)
    }

    fn text_buffer_snapshot_value(&self) -> TextBufferSnapshot {
        let sorts = self
            .state
            .text_buffer
            .iter()
            .map(|sort| match &sort.kind {
                TextSortKind::Glyph {
                    name,
                    codepoint,
                    advance_width,
                } => TextSortSnapshot {
                    kind: "glyph",
                    glyph_name: Some(name.clone()),
                    char: codepoint.map(|c| c.to_string()),
                    codepoint: codepoint.map(|c| c as u32),
                    advance_width: Some(*advance_width),
                    active: sort.active,
                },
                TextSortKind::LineBreak => TextSortSnapshot {
                    kind: "lineBreak",
                    glyph_name: None,
                    char: None,
                    codepoint: None,
                    advance_width: None,
                    active: sort.active,
                },
            })
            .collect();
        TextBufferSnapshot {
            has_text_session: self.state.has_text_session,
            cursor: self.state.text_buffer.cursor(),
            active_sort: self.state.text_buffer.active_sort(),
            direction: match self.state.text_buffer.direction() {
                TextDirection::LeftToRight => "ltr",
                TextDirection::RightToLeft => "rtl",
            },
            sorts,
        }
    }

    fn text_layout_snapshot_value(&self, line_height: f64) -> TextLayoutSnapshot {
        let layout = self.state.text_buffer.layout(line_height.max(1.0));
        TextLayoutSnapshot {
            cursor_x: layout.cursor_x,
            cursor_y: layout.cursor_y,
            items: layout
                .items
                .into_iter()
                .map(|item| TextLayoutItemSnapshot {
                    index: item.index,
                    x: item.x,
                    y: item.y,
                    advance_width: item.advance_width,
                })
                .collect(),
        }
    }

    fn text_layout_state_values(&self) -> Vec<f64> {
        let layout = self
            .state
            .text_buffer
            .layout(self.state.text_line_height().max(1.0));
        let mut out = Vec::with_capacity(3 + layout.items.len() * 4);
        out.push(layout.cursor_x);
        out.push(layout.cursor_y);
        out.push(layout.items.len() as f64);
        for item in layout.items {
            out.push(item.index as f64);
            out.push(item.x);
            out.push(item.y);
            out.push(item.advance_width);
        }
        out
    }

    fn selection_state_values(&self) -> Vec<f64> {
        let selection_count = self.state.selection.len()
            + usize::from(self.state.selected_component.is_some())
            + usize::from(self.state.selected_anchor.is_some());
        let (selected_contour_count, bounds_state) = self.selection_contours_and_bounds_values();
        let mut out = vec![
            selection_count as f64,
            selected_contour_count as f64,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        if let Some((count, bounds)) = bounds_state {
            let reference = self.state.selection_reference_point(bounds);
            out[2] = 1.0;
            out[3] = count as f64;
            out[4] = reference.x;
            out[5] = reference.y;
            out[6] = bounds.width();
            out[7] = bounds.height();
        }
        if let Some(anchor) = self.state.selected_anchor() {
            out[8] = 1.0;
            out[9] = anchor.point.x;
            out[10] = anchor.point.y;
        }
        out
    }

    fn selection_contours_and_bounds_values(&self) -> (usize, Option<(usize, Rect)>) {
        if let Some(bounds) = self.state.selected_component_bounds() {
            return (0, Some((1, bounds)));
        }
        if let Some(bounds) = self.state.selected_anchor_bounds() {
            return (0, Some((1, bounds)));
        }
        if self.state.selection.is_empty() {
            return (0, None);
        }

        let mut selected_contour_count = 0usize;
        let mut selected_point_count = 0usize;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for path in &self.state.paths {
            let mut contour_has_selection = false;
            for pt in path.points().iter() {
                if !self.state.selection.contains(&pt.id) {
                    continue;
                }
                contour_has_selection = true;
                selected_point_count += 1;
                min_x = min_x.min(pt.point.x);
                min_y = min_y.min(pt.point.y);
                max_x = max_x.max(pt.point.x);
                max_y = max_y.max(pt.point.y);
            }
            if contour_has_selection {
                selected_contour_count += 1;
            }
        }

        let bounds = (selected_point_count > 0)
            .then(|| (selected_point_count, Rect::new(min_x, min_y, max_x, max_y)));
        (selected_contour_count, bounds)
    }

    fn editor_panel_state_values(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(15);
        out.extend(self.editor_metrics_state_values());
        out.extend(self.selection_state_values());
        out
    }

    fn editor_metrics_state_values(&self) -> [f64; 4] {
        let bbox = self.state.glyph_bbox();
        let left_sidebearing = bbox.map(|bbox| bbox.min_x()).unwrap_or(0.0);
        let right_sidebearing = bbox
            .map(|bbox| self.state.advance_width - bbox.max_x())
            .unwrap_or(self.state.advance_width);
        [
            self.state.advance_width,
            self.state.paths.len() as f64,
            left_sidebearing,
            right_sidebearing,
        ]
    }

    fn push_nudge_selection_result_values(&self, out: &mut Vec<f64>, result: NudgeSelectionResult) {
        out.push(result.selection_count as f64);
        out.extend_from_slice(&[0.0; 9]);
        if let Some((count, bounds)) = result.bounds {
            let reference = self.state.selection_reference_point(bounds);
            out[1] = 1.0;
            out[2] = count as f64;
            out[3] = reference.x;
            out[4] = reference.y;
            out[5] = bounds.width();
            out[6] = bounds.height();
        }
        if let Some(anchor) = result.anchor {
            out[7] = 1.0;
            out[8] = anchor.x;
            out[9] = anchor.y;
        }
    }
}

#[wasm_bindgen]
impl GlyphEditor {
    /// Async constructor. Allocates the WebGPU device, attaches to
    /// the canvas. Returns a Promise to JS.
    pub async fn new(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<GlyphEditor, JsValue> {
        let renderer = Renderer::new(canvas, width, height).await?;
        Ok(Self {
            state: EditorState::new(),
            mouse: Mouse::new(),
            tool: ActiveTool::default(),
            renderer,
            undo: UndoState::new(),
            point_clipboard: None,
            component_glyphs: HashMap::new(),
            source_glyph: None,
            pending_snapshot: None,
            pending_nudge_snapshot: None,
        })
    }

    /// Replace the displayed glyph from SVG path data. Each curve
    /// segment is decomposed into editable on/off-curve points.
    /// Clears undo history (loading a new glyph isn't undoable).
    #[wasm_bindgen(js_name = setGlyphSvg)]
    pub fn set_glyph_svg(&mut self, svg: &str) -> Result<(), JsValue> {
        let bez = BezPath::from_svg(svg)
            .map_err(|e| JsValue::from_str(&format!("parse SVG path: {e}")))?;
        self.state.set_glyph_from_bezpath(&bez);
        self.undo.clear();
        self.point_clipboard = None;
        self.component_glyphs.clear();
        self.source_glyph = None;
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        Ok(())
    }

    /// Replace the displayed glyph from a UFO `.glif` file's raw
    /// bytes. Parses via `norad`, then walks the result into the
    /// editor's own contour representation. Clears undo history.
    #[wasm_bindgen(js_name = setGlyphGlif)]
    pub fn set_glyph_glif(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.state.set_glyph_from_norad(&glyph);
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(bytes)));
        self.undo.clear();
        self.point_clipboard = None;
        self.component_glyphs.clear();
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        Ok(())
    }

    /// Replace the displayed glyph from a UFO `.glif` file and render
    /// resolved component references from a JSON `{ glyphName: glifXml }`
    /// map. Component outlines are preview-only for now.
    #[wasm_bindgen(js_name = setGlyphGlifWithComponents)]
    pub fn set_glyph_glif_with_components(
        &mut self,
        bytes: &[u8],
        glyph_xml_by_name: &str,
    ) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.component_glyphs = parse_glif_xml_map(glyph_xml_by_name)?;
        self.state.set_glyph_from_norad(&glyph);
        self.state
            .set_component_previews(build_component_previews(&glyph, &self.component_glyphs));
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(bytes)));
        self.undo.clear();
        self.point_clipboard = None;
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = setGlyphGlifWithComponentsPreserveHistory)]
    pub fn set_glyph_glif_with_components_preserve_history(
        &mut self,
        bytes: &[u8],
        glyph_xml_by_name: &str,
    ) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.component_glyphs = parse_glif_xml_map(glyph_xml_by_name)?;
        self.state.set_glyph_from_norad_preserving_history(&glyph);
        self.state
            .set_component_previews(build_component_previews(&glyph, &self.component_glyphs));
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(bytes)));
        Ok(())
    }

    #[wasm_bindgen(js_name = setComponentGlyphs)]
    pub fn set_component_glyphs(&mut self, glyph_xml_by_name: &str) -> Result<(), JsValue> {
        self.component_glyphs = parse_glif_xml_map(glyph_xml_by_name)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = setComponentGlyph)]
    pub fn set_component_glyph(&mut self, name: &str, bytes: &[u8]) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.component_glyphs.insert(name.to_string(), glyph);
        Ok(())
    }

    #[wasm_bindgen(js_name = deleteComponentGlyph)]
    pub fn delete_component_glyph(&mut self, name: &str) {
        self.component_glyphs.remove(name);
    }

    #[wasm_bindgen(js_name = setGlyphGlifWithCachedComponents)]
    pub fn set_glyph_glif_with_cached_components(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.set_glyph_from_norad_with_component_cache(&glyph);
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(bytes)));
        self.undo.clear();
        self.point_clipboard = None;
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = setGlyphGlifWithCachedComponentsPreserveHistory)]
    pub fn set_glyph_glif_with_cached_components_preserve_history(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), JsValue> {
        let glyph = norad::Glyph::parse_raw(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
        self.set_glyph_from_norad_preserving_history_with_component_cache(&glyph);
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(bytes)));
        Ok(())
    }

    #[wasm_bindgen(js_name = setGlyphNameWithCachedComponents)]
    pub fn set_glyph_name_with_cached_components(&mut self, name: &str) -> Result<bool, JsValue> {
        let Some(glyph) = self.component_glyphs.get(name).cloned() else {
            return Ok(false);
        };
        self.set_glyph_from_norad_with_component_cache(&glyph);
        self.cache_source_glyph(glyph, None);
        self.undo.clear();
        self.point_clipboard = None;
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        Ok(true)
    }

    #[wasm_bindgen(js_name = setGlyphNameWithCachedComponentsPreserveHistory)]
    pub fn set_glyph_name_with_cached_components_preserve_history(
        &mut self,
        name: &str,
    ) -> Result<bool, JsValue> {
        let Some(glyph) = self.component_glyphs.get(name).cloned() else {
            return Ok(false);
        };
        self.set_glyph_from_norad_preserving_history_with_component_cache(&glyph);
        self.cache_source_glyph(glyph, None);
        Ok(true)
    }

    /// Parse a UFO `fontinfo.plist` and store the vertical metrics
    /// (UPM, ascender, descender, x-height, cap-height). The
    /// renderer uses these to draw the metric box guidelines.
    #[wasm_bindgen(js_name = setFontInfo)]
    pub fn set_font_info(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let metrics = crate::editor::FontMetrics::parse_plist(bytes)
            .map_err(|e| JsValue::from_str(&format!("parse fontinfo.plist: {e}")))?;
        self.state.metrics = Some(metrics);
        Ok(())
    }

    /// Auto-zoom and center the loaded glyph for a canvas of the
    /// given backing-store size. Called from JS after loading a real
    /// glyph so the user doesn't have to hunt for it.
    #[wasm_bindgen(js_name = fitToCanvas)]
    pub fn fit_to_canvas(&mut self, width: f64, height: f64) {
        self.state.fit_to_canvas(width, height);
    }

    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.state.viewport.zoom = zoom.max(1e-4);
    }

    #[wasm_bindgen(js_name = zoom)]
    pub fn zoom(&self) -> f64 {
        self.state.viewport.zoom
    }

    #[wasm_bindgen(js_name = setOffset)]
    pub fn set_offset(&mut self, x: f64, y: f64) {
        self.state.viewport.offset = Vec2::new(x, y);
    }

    #[wasm_bindgen(js_name = designToScreen)]
    pub fn design_to_screen(&self, x: f64, y: f64) -> Box<[f64]> {
        let point = self.state.viewport.to_screen(Point::new(x, y));
        Box::new([point.x, point.y])
    }

    #[wasm_bindgen(js_name = screenToDesign)]
    pub fn screen_to_design(&self, x: f64, y: f64) -> Box<[f64]> {
        let point = self.state.viewport.screen_to_design(Point::new(x, y));
        Box::new([point.x, point.y])
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    #[wasm_bindgen(js_name = setDeviceScale)]
    pub fn set_device_scale(&mut self, scale: f64) {
        self.renderer.set_device_scale(scale);
    }

    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&mut self, theme_json: &str) -> Result<(), JsValue> {
        self.renderer.set_theme_json(theme_json)
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        let text_mode_active = self.tool.is_text() && self.state.has_text_session;
        self.renderer
            .render(&self.state, self.tool.is_preview(), text_mode_active)
    }

    #[wasm_bindgen(js_name = setTool)]
    pub fn set_tool(&mut self, tool_id: &str) -> bool {
        let revision = self.state.edit_revision();
        self.mouse.cancel(&mut self.tool, &mut self.state);
        self.tool.set_tool(tool_id);
        self.state.edit_revision() != revision
    }

    #[wasm_bindgen(js_name = setShapeTool)]
    pub fn set_shape_tool(&mut self, shape: &str) -> bool {
        let revision = self.state.edit_revision();
        let kind = match shape {
            "ellipse" => ShapeKind::Ellipse,
            _ => ShapeKind::Rectangle,
        };
        self.mouse.cancel(&mut self.tool, &mut self.state);
        self.tool.set_shape_kind(kind);
        self.state.edit_revision() != revision
    }

    #[wasm_bindgen(js_name = setShapeShiftLocked)]
    pub fn set_shape_shift_locked(&mut self, locked: bool) -> bool {
        self.tool.set_shape_shift_locked(locked, &mut self.state)
    }

    #[wasm_bindgen(js_name = setKnifeShiftLocked)]
    pub fn set_knife_shift_locked(&mut self, locked: bool) -> bool {
        self.tool.set_knife_shift_locked(locked, &mut self.state)
    }

    #[wasm_bindgen(js_name = setTextDirection)]
    pub fn set_text_direction(&mut self, direction: &str) {
        let direction = match direction {
            "rtl" => TextDirection::RightToLeft,
            _ => TextDirection::LeftToRight,
        };
        self.state.text_buffer.set_direction(direction);
    }

    #[wasm_bindgen(js_name = setTextKerningModel)]
    pub fn set_text_kerning_model(&mut self, json: &str) -> Result<(), JsValue> {
        let kerning: TextKerningModel = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("parse text kerning model: {e}")))?;
        self.state.text_buffer.set_kerning_model(kerning);
        Ok(())
    }

    #[wasm_bindgen(js_name = textKerningModel)]
    pub fn text_kerning_model(&self) -> Result<String, JsValue> {
        serde_json::to_string(self.state.text_buffer.kerning_model())
            .map_err(|e| JsValue::from_str(&format!("serialize text kerning model: {e}")))
    }

    #[wasm_bindgen(js_name = setTextGlyphInventory)]
    pub fn set_text_glyph_inventory(&mut self, json: &str) -> Result<(), JsValue> {
        let inventory: TextGlyphInventory = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("parse text glyph inventory: {e}")))?;
        self.state.text_buffer.set_glyph_inventory(inventory);
        Ok(())
    }

    #[wasm_bindgen(js_name = shapeTextBuffer)]
    pub fn shape_text_buffer(&mut self) -> bool {
        self.state.text_buffer.shape_arabic_if_rtl()
    }

    #[wasm_bindgen(js_name = textBufferSnapshot)]
    pub fn text_buffer_snapshot(&self) -> Result<String, JsValue> {
        let snapshot = self.text_buffer_snapshot_value();
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&format!("serialize text buffer: {e}")))
    }

    #[wasm_bindgen(js_name = textBufferLayout)]
    pub fn text_buffer_layout(&self, line_height: f64) -> Result<String, JsValue> {
        let snapshot = self.text_layout_snapshot_value(line_height);
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&format!("serialize text layout: {e}")))
    }

    #[wasm_bindgen(js_name = textBufferState)]
    pub fn text_buffer_state(&self) -> Result<String, JsValue> {
        let snapshot = TextBufferStateSnapshot {
            buffer: self.text_buffer_snapshot_value(),
            layout: self.text_layout_snapshot_value(self.state.text_line_height()),
        };
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&format!("serialize text buffer state: {e}")))
    }

    #[wasm_bindgen(js_name = textLayoutState)]
    pub fn text_layout_state(&self) -> Vec<f64> {
        self.text_layout_state_values()
    }

    #[wasm_bindgen(js_name = textBufferPreviewSvg)]
    pub fn text_buffer_preview_svg(&self) -> Result<String, JsValue> {
        let layout = self.state.text_buffer.preview_layout();
        let active_sort = self.state.text_buffer.active_sort();
        let mut paths = Vec::new();
        let mut bounds: Option<Rect> = None;

        for item in layout {
            let Some(sort) = self.state.text_buffer.sort(item.index) else {
                continue;
            };
            let mut path = if Some(item.index) == active_sort {
                let mut active_path = BezPath::new();
                for path in &self.state.paths {
                    path.append_to_bezpath(&mut active_path);
                }
                active_path.extend(self.state.component_preview.elements().iter().copied());
                active_path
            } else {
                let Some(glyph_name) = sort.glyph_name() else {
                    continue;
                };
                let Some(outline) = self.state.text_buffer.glyph_outline_svg(glyph_name) else {
                    continue;
                };
                BezPath::from_svg(outline)
                    .map_err(|e| JsValue::from_str(&format!("parse text preview SVG path: {e}")))?
            };
            if path.elements().is_empty() {
                continue;
            }
            path.apply_affine(Affine::translate((item.x, item.y)));
            let path_bounds = path.bounding_box();
            bounds = Some(match bounds {
                Some(existing) => existing.union(path_bounds),
                None => path_bounds,
            });
            paths.push(path);
        }

        let Some(bbox) = bounds else {
            return Ok(String::new());
        };

        // Keep a small amount of visual margin in font units. The Vue
        // preview strip scales this SVG by height and clips horizontal
        // overflow, so long strings can still use the panel height instead
        // of shrinking to fit the full run into one viewport.
        let margin_x = bbox.width().max(1.0) * 0.06;
        let margin_y = bbox.height().max(1.0) * 0.06;
        let view_x = bbox.x0 - margin_x;
        let view_y = -(bbox.y1 + margin_y);
        let view_width = (bbox.width() + margin_x * 2.0).max(1.0);
        let view_height = (bbox.height() + margin_y * 2.0).max(1.0);

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet">"#,
            view_width, view_height, view_x, view_y, view_width, view_height,
        );
        for path in paths {
            write!(
                &mut svg,
                r##"<path d="{}" fill="currentColor" fill-rule="nonzero" transform="scale(1 -1)"/>"##,
                path.to_svg()
            )
            .map_err(|e| JsValue::from_str(&format!("write text preview SVG: {e}")))?;
        }
        svg.push_str("</svg>");
        Ok(svg)
    }

    #[wasm_bindgen(js_name = clearTextBuffer)]
    pub fn clear_text_buffer(&mut self) {
        self.state.text_buffer.clear();
        self.state.has_text_session = false;
    }

    #[wasm_bindgen(js_name = insertTextGlyph)]
    pub fn insert_text_glyph(&mut self, name: &str, codepoint: u32, advance_width: f64) {
        let codepoint = text_codepoint_from_wasm(codepoint);
        self.state.has_text_session = true;
        self.state
            .text_buffer
            .insert_glyph(name, codepoint, advance_width);
    }

    #[wasm_bindgen(js_name = insertInactiveTextGlyph)]
    pub fn insert_inactive_text_glyph(&mut self, name: &str, codepoint: u32, advance_width: f64) {
        let codepoint = text_codepoint_from_wasm(codepoint);
        let snapshot = self.discrete_edit_snapshot();
        self.state.has_text_session = true;
        self.state
            .insert_inactive_text_glyph(name, codepoint, advance_width);
        self.undo.add_undo_group(snapshot);
    }

    #[wasm_bindgen(js_name = activateTextSort)]
    pub fn activate_text_sort(&mut self, index: usize) -> bool {
        if !self.state.has_text_session {
            return false;
        }
        self.state.text_buffer.activate_sort(index)
    }

    #[wasm_bindgen(js_name = insertTextCharacter)]
    pub fn insert_text_character(&mut self, codepoint: u32) -> bool {
        let Some(char) = text_codepoint_from_wasm(codepoint) else {
            return false;
        };
        let snapshot = self.discrete_edit_snapshot();
        if self.state.insert_text_character(char) {
            self.state.has_text_session = true;
            self.undo.add_undo_group(snapshot);
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = updateTextGlyph)]
    pub fn update_text_glyph(
        &mut self,
        index: usize,
        name: &str,
        codepoint: u32,
        advance_width: f64,
    ) -> bool {
        let codepoint = text_codepoint_from_wasm(codepoint);
        self.state
            .text_buffer
            .update_glyph(index, name, codepoint, advance_width)
    }

    #[wasm_bindgen(js_name = insertTextLineBreak)]
    pub fn insert_text_line_break(&mut self) {
        let snapshot = self.discrete_edit_snapshot();
        self.state.has_text_session = true;
        self.state.insert_text_line_break();
        self.undo.add_undo_group(snapshot);
    }

    #[wasm_bindgen(js_name = deleteTextBeforeCursor)]
    pub fn delete_text_before_cursor(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.delete_text_before_cursor() {
            self.undo.add_undo_group(snapshot);
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = deleteTextAfterCursor)]
    pub fn delete_text_after_cursor(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.delete_text_after_cursor() {
            self.undo.add_undo_group(snapshot);
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualLeft)]
    pub fn move_text_cursor_visual_left(&mut self) {
        self.state.text_buffer.move_cursor_visual_left();
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualRight)]
    pub fn move_text_cursor_visual_right(&mut self) {
        self.state.text_buffer.move_cursor_visual_right();
    }

    #[wasm_bindgen(js_name = activateTextSortAt)]
    pub fn activate_text_sort_at(&mut self, x: f64, y: f64) -> bool {
        if !self.state.has_text_session {
            return false;
        }
        let design_pos = self.state.viewport.screen_to_design(Point::new(x, y));
        let line_height = self.state.text_line_height();
        let (ascender, descender) = self.state.text_metric_bounds();
        self.state
            .text_buffer
            .activate_sort_at(design_pos.x, design_pos.y, line_height, ascender, descender)
            .is_some()
    }

    #[wasm_bindgen(js_name = activateTextSortAtIndex)]
    pub fn activate_text_sort_at_index(&mut self, x: f64, y: f64) -> i32 {
        if !self.state.has_text_session {
            return -1;
        }
        let design_pos = self.state.viewport.screen_to_design(Point::new(x, y));
        let line_height = self.state.text_line_height();
        let (ascender, descender) = self.state.text_metric_bounds();
        self.state
            .text_buffer
            .activate_sort_at(design_pos.x, design_pos.y, line_height, ascender, descender)
            .map(|activation| activation.index as i32)
            .unwrap_or(-1)
    }

    /// Activate an inactive text sort hit at screen point and return compact
    /// state: `[index, cursor, layout_x, layout_y]`. Empty when no inactive
    /// sort was hit, including when the hit sort is already active.
    #[wasm_bindgen(js_name = activateTextSortAtState)]
    pub fn activate_text_sort_at_state(&mut self, x: f64, y: f64) -> Vec<f64> {
        if !self.state.has_text_session {
            return Vec::new();
        }
        let previous_active = self.state.text_buffer.active_sort();
        let design_pos = self.state.viewport.screen_to_design(Point::new(x, y));
        let line_height = self.state.text_line_height();
        let (ascender, descender) = self.state.text_metric_bounds();
        self.state
            .text_buffer
            .activate_sort_at(design_pos.x, design_pos.y, line_height, ascender, descender)
            .filter(|activation| Some(activation.index) != previous_active)
            .map(|activation| {
                let mut out = Vec::with_capacity(4);
                out.push(activation.index as f64);
                out.push(self.state.text_buffer.cursor() as f64);
                out.push(activation.x);
                out.push(activation.y);
                out
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // Pointer events. JS hands us screen-space coordinates (in
    // backing-store pixels — see Vue side for DPR multiplication).
    //
    // `button`: 0 = left, 1 = middle, 2 = right.
    // `mods` bitfield: 1=shift, 2=ctrl, 4=alt, 8=meta.
    // ------------------------------------------------------------------

    #[wasm_bindgen(js_name = pointerDown)]
    pub fn pointer_down(&mut self, x: f64, y: f64, button: u32, mods: u32) {
        self.commit_pending_nudge_snapshot();
        // Snapshot before mutating for tools that can edit immediately.
        // Select normally only changes selection on down, so defer its
        // snapshot until the drag threshold is crossed. Alt-select can
        // convert a line segment on down, so keep the eager snapshot there.
        if button == 0 && (!self.tool.is_select() || (mods & 4) != 0) {
            self.pending_snapshot = Some(self.state.clone());
        }
        let event = build_event(x, y, button, mods);
        self.mouse
            .mouse_down(event, &mut self.tool, &mut self.state);
    }

    #[wasm_bindgen(js_name = pointerMove)]
    pub fn pointer_move(&mut self, x: f64, y: f64, mods: u32) {
        let event = build_event(x, y, u32::MAX, mods);
        self.mouse
            .mouse_moved(event, &mut self.tool, &mut self.state);
    }

    /// Move the pointer and report whether anything visible changed.
    ///
    /// Used by idle hover paths where Vue should not schedule a frame unless
    /// the hover/preview state actually changed.
    #[wasm_bindgen(js_name = pointerMoveVisualChanged)]
    pub fn pointer_move_visual_changed(&mut self, x: f64, y: f64, mods: u32) -> bool {
        let visual_signature = editor_visual_signature(&self.state);
        let event = build_event(x, y, u32::MAX, mods);
        self.mouse
            .mouse_moved(event, &mut self.tool, &mut self.state);
        editor_visual_signature(&self.state) != visual_signature
    }

    /// Move the pointer and return compact selection state for hot drag paths
    /// that need live coordinate updates without recomputing glyph metrics.
    ///
    /// Shape:
    /// `[0, 0]` when nothing changed; otherwise
    /// `[visual_changed, edit_changed, ...selectionState]`.
    #[wasm_bindgen(js_name = pointerMoveSelectionState)]
    pub fn pointer_move_selection_state(&mut self, x: f64, y: f64, mods: u32) -> Vec<f64> {
        let pos = Point::new(x, y);
        if self.mouse.track_pending_drag_move(pos, MouseButton::Left) {
            return vec![0.0, 0.0];
        }
        let revision = self.state.edit_revision();
        let visual_signature = editor_visual_signature(&self.state);
        if self.pending_snapshot.is_none() && self.mouse.will_start_drag(pos, MouseButton::Left) {
            self.pending_snapshot = Some(self.state.clone());
        }
        let event = build_event(x, y, u32::MAX, mods);
        self.mouse
            .mouse_moved(event, &mut self.tool, &mut self.state);
        let visual_changed = editor_visual_signature(&self.state) != visual_signature;
        let edit_changed = self.state.edit_revision() != revision;
        if !visual_changed && !edit_changed {
            return vec![0.0, 0.0];
        }
        let mut out = Vec::with_capacity(13);
        out.push(visual_changed as u8 as f64);
        out.push(edit_changed as u8 as f64);
        out.extend(self.selection_state_values());
        out
    }

    #[wasm_bindgen(js_name = clearSegmentHover)]
    pub fn clear_segment_hover(&mut self) -> bool {
        if self.state.segment_hover.is_some() {
            self.state.segment_hover = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = pointerUp)]
    pub fn pointer_up(&mut self, x: f64, y: f64, button: u32, mods: u32) -> bool {
        let event = build_event(x, y, button, mods);
        self.mouse.mouse_up(event, &mut self.tool, &mut self.state);
        if button == 0 {
            let snapshot = self.pending_snapshot.take();
            let changed = snapshot
                .as_ref()
                .is_some_and(|snapshot| self.state.edit_revision() != snapshot.edit_revision());
            if changed {
                let snapshot = snapshot.expect("checked above");
                self.undo.add_undo_group(snapshot);
            }
            changed
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = pointerCancel)]
    pub fn pointer_cancel(&mut self) -> bool {
        let revision = self.state.edit_revision();
        self.mouse.cancel(&mut self.tool, &mut self.state);
        self.pending_snapshot = None;
        self.pending_nudge_snapshot = None;
        self.state.edit_revision() != revision
    }

    #[wasm_bindgen(js_name = componentBaseAt)]
    pub fn component_base_at(&self, x: f64, y: f64) -> String {
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        self.state
            .component_base_at(design)
            .unwrap_or_default()
            .to_string()
    }

    #[wasm_bindgen(js_name = clearComponentSelection)]
    pub fn clear_component_selection(&mut self) {
        self.state.clear_component_selection();
    }

    #[wasm_bindgen(js_name = anchorContextAt)]
    pub fn anchor_context_at(&self, x: f64, y: f64) -> String {
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let radius = 16.0 / self.state.viewport.zoom.max(1e-6);
        let Some(id) = self.state.hit_test_anchor(design, radius) else {
            return String::new();
        };
        let Some(anchor) = self.state.anchors.iter().find(|anchor| anchor.id == id) else {
            return String::new();
        };
        let snapshot = AnchorSnapshot {
            name: anchor.name.clone(),
            x: anchor.point.x,
            y: anchor.point.y,
        };
        serde_json::to_string(&snapshot).unwrap_or_default()
    }

    #[wasm_bindgen(js_name = selectedAnchorInfo)]
    pub fn selected_anchor_info(&self) -> String {
        let Some(anchor) = self.state.selected_anchor() else {
            return String::new();
        };
        let snapshot = AnchorSnapshot {
            name: anchor.name.clone(),
            x: anchor.point.x,
            y: anchor.point.y,
        };
        serde_json::to_string(&snapshot).unwrap_or_default()
    }

    #[wasm_bindgen(js_name = selectAnchorAt)]
    pub fn select_anchor_at(&mut self, x: f64, y: f64) -> bool {
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let radius = 16.0 / self.state.viewport.zoom.max(1e-6);
        let Some(id) = self.state.hit_test_anchor(design, radius) else {
            return false;
        };
        self.state.select_anchor(id);
        true
    }

    #[wasm_bindgen(js_name = addAnchorAt)]
    pub fn add_anchor_at(&mut self, x: f64, y: f64, name: &str) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let name = normalize_anchor_name(name);
        self.state.add_anchor(design, name);
        self.undo.add_undo_group(snapshot);
        self.pending_snapshot = None;
        true
    }

    #[wasm_bindgen(js_name = updateSelectedAnchor)]
    pub fn update_selected_anchor(&mut self, name: &str, x: f64, y: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        let name = normalize_anchor_name(name);
        if self.state.update_selected_anchor(name, Point::new(x, y)) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = contourContextAt)]
    pub fn contour_context_at(&self, x: f64, y: f64) -> Vec<f64> {
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let radius = 8.0 / self.state.viewport.zoom.max(1e-6);
        let Some(target) = self.state.contour_context_target(design, radius) else {
            return Vec::new();
        };
        let target_screen = self.state.glyph_to_screen(target.point);
        vec![
            target.path_index as f64,
            if target.can_set_start { 1.0 } else { 0.0 },
            if target.path_index > 0 { 1.0 } else { 0.0 },
            if target.path_index + 1 < self.state.paths.len() {
                1.0
            } else {
                0.0
            },
            target_screen.x,
            target_screen.y,
        ]
    }

    #[wasm_bindgen(js_name = setStartPointAt)]
    pub fn set_start_point_at(&mut self, x: f64, y: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let radius = 8.0 / self.state.viewport.zoom.max(1e-6);
        if self.state.set_start_point_at(design, radius) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = reverseContourAt)]
    pub fn reverse_contour_at(&mut self, x: f64, y: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        let design = self.state.screen_to_glyph_design(Point::new(x, y));
        let radius = 8.0 / self.state.viewport.zoom.max(1e-6);
        if self.state.reverse_contour_at(design, radius) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = moveContour)]
    pub fn move_contour(&mut self, path_index: usize, direction: &str) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        let delta = match direction {
            "up" => -1,
            "down" => 1,
            _ => return false,
        };
        if self.state.move_contour(path_index, delta) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    /// Mouse wheel — zoom around the cursor position. `delta_y`
    /// follows DOM convention (positive = scroll down = zoom out).
    pub fn wheel(&mut self, x: f64, y: f64, delta_y: f64) {
        // 0.0015 gives reasonable response for both notch wheels
        // (~100 px per click) and smooth trackpad scrolling.
        let factor = (-delta_y * 0.0015).exp();
        let cursor_screen = Point::new(x, y);
        let cursor_design = self.state.viewport.screen_to_design(cursor_screen);
        let new_zoom = (self.state.viewport.zoom * factor).clamp(1e-3, 1e4);

        // Solve for new offset that keeps cursor_design under
        // cursor_screen. With viewport applying scale + Y-flip +
        // translate:
        //     screen.x = design.x * zoom + offset.x
        //     screen.y = -design.y * zoom + offset.y
        self.state.viewport.zoom = new_zoom;
        self.state.viewport.offset = Vec2::new(
            cursor_screen.x - cursor_design.x * new_zoom,
            cursor_screen.y + cursor_design.y * new_zoom,
        );
    }

    pub fn undo(&mut self) -> bool {
        self.commit_pending_nudge_snapshot();
        if let Some(prev) = self.undo.undo(self.state.clone()) {
            self.state = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        self.commit_pending_nudge_snapshot();
        if let Some(next) = self.undo.redo(self.state.clone()) {
            self.state = next;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = flipSelectionHorizontal)]
    pub fn flip_selection_horizontal(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.flip_selection_horizontal() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = flipSelectionVertical)]
    pub fn flip_selection_vertical(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.flip_selection_vertical() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = rotateSelectionClockwise)]
    pub fn rotate_selection_clockwise(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.rotate_selection(-90.0) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = rotateSelectionCounterClockwise)]
    pub fn rotate_selection_counter_clockwise(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.rotate_selection(90.0) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = duplicateSelection)]
    pub fn duplicate_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.duplicate_selection() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = duplicateRepeatSelection)]
    pub fn duplicate_repeat_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.duplicate_repeat_selection() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = reverseContours)]
    pub fn reverse_contours(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.reverse_contours() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = convertHyperToCubic)]
    pub fn convert_hyper_to_cubic(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.convert_hyper_to_cubic() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = setAdvanceWidth)]
    pub fn set_advance_width(&mut self, width: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.set_advance_width(width) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = leftSidebearing)]
    pub fn left_sidebearing(&self) -> f64 {
        self.state.left_sidebearing()
    }

    #[wasm_bindgen(js_name = rightSidebearing)]
    pub fn right_sidebearing(&self) -> f64 {
        self.state.right_sidebearing()
    }

    /// Compact glyph sidebar + coordinate panel state for hot glyph-load paths.
    ///
    /// Shape:
    /// `[advance_width, contour_count, left_sidebearing, right_sidebearing,
    ///   ...selectionState]`.
    #[wasm_bindgen(js_name = editorPanelState)]
    pub fn editor_panel_state(&self) -> Vec<f64> {
        self.editor_panel_state_values()
    }

    /// Compact glyph metrics state for hot glyph-load paths that already know
    /// there is no active selection to preserve.
    ///
    /// Shape: `[advance_width, contour_count, left_sidebearing, right_sidebearing]`.
    #[wasm_bindgen(js_name = editorMetricsState)]
    pub fn editor_metrics_state(&self) -> Vec<f64> {
        self.editor_metrics_state_values().to_vec()
    }

    #[wasm_bindgen(js_name = setLeftSidebearing)]
    pub fn set_left_sidebearing(&mut self, value: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.set_left_sidebearing(value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = setRightSidebearing)]
    pub fn set_right_sidebearing(&mut self, value: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.set_right_sidebearing(value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = copySelection)]
    pub fn copy_selection(&mut self) -> bool {
        let Some(paths) = self.state.copy_selection() else {
            return false;
        };
        self.point_clipboard = Some(paths);
        true
    }

    #[wasm_bindgen(js_name = pasteSelection)]
    pub fn paste_selection(&mut self) -> bool {
        let Some(clipboard) = self.point_clipboard.clone() else {
            return false;
        };
        let snapshot = self.discrete_edit_snapshot();
        if self.state.paste_paths(&clipboard) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = deleteSelection)]
    pub fn delete_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.delete_selection() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = togglePointType)]
    pub fn toggle_point_type(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.toggle_point_type() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = roundSelectedCorners)]
    pub fn round_selected_corners(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.round_selected_corners() {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = togglePointTypeAt)]
    pub fn toggle_point_type_at(&mut self, x: f64, y: f64) -> bool {
        const MIN_CLICK_DISTANCE: f64 = 10.0;
        let snapshot = self.discrete_edit_snapshot();
        let screen = Point::new(x, y);
        let design = self.state.screen_to_glyph_design(screen);
        let radius = MIN_CLICK_DISTANCE / self.state.viewport.zoom.max(1e-6);
        if self.state.toggle_point_type_at_point(design, radius) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = selectContourAt)]
    pub fn select_contour_at(&mut self, x: f64, y: f64) -> bool {
        const MIN_CLICK_DISTANCE: f64 = 10.0;
        const POINT_GUARD_DISTANCE: f64 = 20.0;
        let screen = Point::new(x, y);
        let design = self.state.screen_to_glyph_design(screen);
        let zoom = self.state.viewport.zoom.max(1e-6);
        let segment_radius = MIN_CLICK_DISTANCE / zoom;
        let point_guard_radius = POINT_GUARD_DISTANCE / zoom;
        self.state
            .select_contour_at_point(design, segment_radius, point_guard_radius)
    }

    #[wasm_bindgen(js_name = unionSelection)]
    pub fn union_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.boolean_selection(linesweeper::BinaryOp::Union) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = subtractSelection)]
    pub fn subtract_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self
            .state
            .boolean_selection(linesweeper::BinaryOp::Difference)
        {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = intersectSelection)]
    pub fn intersect_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self
            .state
            .boolean_selection(linesweeper::BinaryOp::Intersection)
        {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = excludeSelection)]
    pub fn exclude_selection(&mut self) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.boolean_selection(linesweeper::BinaryOp::Xor) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = moveSelectionReference)]
    pub fn move_selection_reference(&mut self, axis: &str, value: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.move_selection_reference(axis, value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    /// Move the coordinate-panel reference point and return the updated
    /// compact editor panel state in one JS↔WASM crossing.
    ///
    /// Shape:
    /// `[changed, advance_width, contour_count, left_sidebearing,
    ///   right_sidebearing, ...selectionState]`.
    #[wasm_bindgen(js_name = moveSelectionReferenceState)]
    pub fn move_selection_reference_state(&mut self, axis: &str, value: f64) -> Vec<f64> {
        let snapshot = self.discrete_edit_snapshot();
        if !self.state.move_selection_reference(axis, value) {
            return vec![0.0];
        }
        self.undo.add_undo_group(snapshot);
        self.pending_snapshot = None;
        let mut out = Vec::with_capacity(16);
        out.push(1.0);
        out.extend(self.editor_panel_state_values());
        out
    }

    #[wasm_bindgen(js_name = resizeSelectionReference)]
    pub fn resize_selection_reference(&mut self, axis: &str, value: f64) -> bool {
        let snapshot = self.discrete_edit_snapshot();
        if self.state.resize_selection_reference(axis, value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    /// Resize the selection from the coordinate panel and return the updated
    /// compact editor panel state in one JS↔WASM crossing.
    ///
    /// Shape:
    /// `[changed, advance_width, contour_count, left_sidebearing,
    ///   right_sidebearing, ...selectionState]`.
    #[wasm_bindgen(js_name = resizeSelectionReferenceState)]
    pub fn resize_selection_reference_state(&mut self, axis: &str, value: f64) -> Vec<f64> {
        let snapshot = self.discrete_edit_snapshot();
        if !self.state.resize_selection_reference(axis, value) {
            return vec![0.0];
        }
        self.undo.add_undo_group(snapshot);
        self.pending_snapshot = None;
        let mut out = Vec::with_capacity(16);
        out.push(1.0);
        out.extend(self.editor_panel_state_values());
        out
    }

    #[wasm_bindgen(js_name = nudgeSelection)]
    pub fn nudge_selection(
        &mut self,
        dx: f64,
        dy: f64,
        shift: bool,
        ctrl: bool,
        independent: bool,
    ) -> bool {
        self.nudge_selection_result(dx, dy, shift, ctrl, independent)
            .is_some()
    }

    fn nudge_selection_result(
        &mut self,
        dx: f64,
        dy: f64,
        shift: bool,
        ctrl: bool,
        independent: bool,
    ) -> Option<NudgeSelectionResult> {
        if self.state.selection_entity_count() == 0 {
            return None;
        }
        if self.pending_nudge_snapshot.is_none() {
            self.pending_nudge_snapshot = Some(self.state.clone());
        }
        let result = self
            .state
            .nudge_selection_result(dx, dy, shift, ctrl, independent)?;
        self.pending_snapshot = None;
        Some(result)
    }

    /// Move the current selection and return the updated compact nudge state
    /// in the same JS↔WASM crossing.
    ///
    /// Shape:
    /// `[changed, selection_count,
    ///   has_bounds, bounds_count, ref_x, ref_y, width, height,
    ///   has_anchor, anchor_x, anchor_y]`.
    ///
    /// Nudging cannot change the selected contour count, so this intentionally
    /// skips the contour-membership scan used by the full selection snapshot.
    #[wasm_bindgen(js_name = nudgeSelectionState)]
    pub fn nudge_selection_state(
        &mut self,
        dx: f64,
        dy: f64,
        shift: bool,
        ctrl: bool,
        independent: bool,
    ) -> Vec<f64> {
        let Some(result) = self.nudge_selection_result(dx, dy, shift, ctrl, independent) else {
            return vec![0.0];
        };
        let mut out = Vec::with_capacity(11);
        out.push(1.0);
        self.push_nudge_selection_result_values(&mut out, result);
        out
    }

    #[wasm_bindgen(js_name = finishNudgeSelection)]
    pub fn finish_nudge_selection(&mut self) {
        self.commit_pending_nudge_snapshot();
    }

    /// Number of currently selected entities. Useful for status UI.
    #[wasm_bindgen(js_name = selectionCount)]
    pub fn selection_count(&self) -> usize {
        self.state.selection_entity_count()
    }

    /// Number of contours touched by the current point selection.
    #[wasm_bindgen(js_name = selectedContourCount)]
    pub fn selected_contour_count(&self) -> usize {
        self.state.selected_contour_count()
    }

    /// Move point selection by outline order. `backwards` is Shift-Tab.
    #[wasm_bindgen(js_name = cycleSelectedPoint)]
    pub fn cycle_selected_point(&mut self, backwards: bool) -> bool {
        self.state.cycle_selected_point(backwards)
    }

    /// Advance width of the currently-open glyph (design units).
    /// Zero when no glyph is loaded.
    #[wasm_bindgen(js_name = advanceWidth)]
    pub fn advance_width(&self) -> f64 {
        self.state.advance_width
    }

    /// Number of contours (path elements) in the currently-open
    /// glyph. Updates live as the user adds/removes paths.
    #[wasm_bindgen(js_name = contourCount)]
    pub fn contour_count(&self) -> usize {
        self.state.paths.len()
    }

    /// Active font vertical metric bounds as `[ascender, descender]`.
    /// Empty if fontinfo has not supplied both values.
    #[wasm_bindgen(js_name = metricBounds)]
    pub fn metric_bounds(&self) -> Vec<f64> {
        let Some(metrics) = self.state.metrics.as_ref() else {
            return Vec::new();
        };
        let (Some(ascender), Some(descender)) = (metrics.ascender, metrics.descender) else {
            return Vec::new();
        };
        vec![ascender, descender]
    }

    /// Current glyph outline/component bounds as `[x, y, width,
    /// height]`, or `[]` when the open glyph has no drawable bounds.
    #[wasm_bindgen(js_name = glyphBounds)]
    pub fn glyph_bounds(&self) -> Vec<f64> {
        let Some(bounds) = self.state.glyph_bbox() else {
            return Vec::new();
        };
        vec![bounds.x0, bounds.y0, bounds.width(), bounds.height()]
    }

    /// Serialize the current editable contours back into .glif XML,
    /// preserving metadata from `original_bytes` where possible.
    /// `mark_color` is the UFO `public.markColor` value; an empty
    /// string clears that lib entry.
    #[wasm_bindgen(js_name = currentGlyphGlif)]
    pub fn current_glyph_glif(
        &mut self,
        original_bytes: &[u8],
        mark_color: &str,
    ) -> Result<Vec<u8>, JsValue> {
        let mut glyph = self.source_glyph_for_bytes(original_bytes)?;
        glyph.width = self.state.advance_width;
        glyph.contours = self
            .state
            .paths
            .iter()
            .map(|path| to_norad_contour(&path.to_contour()))
            .collect();
        let original_component_count = glyph.components.len();
        let mut component_index = 0usize;
        glyph.components.retain_mut(|component| {
            let index = component_index;
            component_index += 1;
            if self.state.deleted_component_indices.contains(&index) {
                return false;
            }
            if let Some(preview) = self
                .state
                .component_previews
                .iter()
                .find(|preview| preview.index == index)
            {
                component.transform = affine_to_norad_transform(preview.transform);
                if !preview.auto_align {
                    apply_component_alignment_disabled_lib(component);
                }
            }
            true
        });
        let mut inserted_components = self
            .state
            .component_previews
            .iter()
            .filter(|component| component.index >= original_component_count)
            .collect::<Vec<_>>();
        inserted_components.sort_by_key(|component| component.index);
        for component in inserted_components {
            let base = norad::Name::new(&component.base)
                .map_err(|e| JsValue::from_str(&format!("component base name: {e}")))?;
            glyph.components.push(norad::Component::new(
                base,
                affine_to_norad_transform(component.transform),
                None,
                (!component.auto_align).then(component_alignment_disabled_lib),
            ));
        }
        glyph.anchors = self
            .state
            .anchors
            .iter()
            .map(to_norad_anchor)
            .collect::<Result<Vec<_>, _>>()?;

        let mark_color = mark_color::canonical_ufo_mark_color(mark_color)
            .ok_or_else(|| JsValue::from_str("invalid UFO public.markColor value"))?;
        if mark_color.is_empty() {
            glyph.lib.remove("public.markColor");
        } else {
            glyph
                .lib
                .insert("public.markColor".to_string(), mark_color.into());
        }

        let bytes = glyph
            .encode_xml()
            .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))?;
        self.cache_source_glyph(glyph, Some(glyph_bytes_fingerprint(&bytes)));
        Ok(bytes)
    }

    /// Selected point bounds in design space as
    /// `[count, x, y, width, height]`, where x/y are the active
    /// coordinate-panel reference point. Empty when there is no
    /// selection.
    #[wasm_bindgen(js_name = selectionBounds)]
    pub fn selection_bounds(&self) -> Vec<f64> {
        let Some((count, bounds)) = self.state.selection_bounds() else {
            return Vec::new();
        };
        let reference = self.state.selection_reference_point(bounds);
        vec![
            count as f64,
            reference.x,
            reference.y,
            bounds.width(),
            bounds.height(),
        ]
    }

    /// Compact selection snapshot for hot UI refresh paths.
    ///
    /// Shape:
    /// `[selection_count, selected_contour_count,
    ///   has_bounds, bounds_count, ref_x, ref_y, width, height,
    ///   has_anchor, anchor_x, anchor_y]`.
    ///
    /// This intentionally avoids JSON and bundles the common selection
    /// panel inputs into one JS↔WASM crossing.
    #[wasm_bindgen(js_name = selectionState)]
    pub fn selection_state(&self) -> Vec<f64> {
        self.selection_state_values()
    }

    #[wasm_bindgen(js_name = setCoordinateQuadrant)]
    pub fn set_coordinate_quadrant(&mut self, quadrant: &str) {
        self.state
            .set_coord_quadrant(quadrant_from_id(quadrant).unwrap_or_default());
    }

    #[wasm_bindgen(js_name = measureInfo)]
    pub fn measure_info(&self) -> Vec<f64> {
        let Some(preview) = self.state.measure_preview.as_ref() else {
            return Vec::new();
        };
        let mut out = vec![
            preview.line.p1.x,
            preview.line.p1.y,
            preview.distance,
            preview.angle_degrees,
            preview.segment_labels.len() as f64,
        ];
        for label in &preview.segment_labels {
            out.push(label.position.x);
            out.push(label.position.y);
            out.push(label.length);
        }
        out
    }
}

fn quadrant_from_id(id: &str) -> Option<Quadrant> {
    match id {
        "tl" => Some(Quadrant::TopLeft),
        "tc" => Some(Quadrant::Top),
        "tr" => Some(Quadrant::TopRight),
        "cl" => Some(Quadrant::Left),
        "cc" => Some(Quadrant::Center),
        "cr" => Some(Quadrant::Right),
        "bl" => Some(Quadrant::BottomLeft),
        "bc" => Some(Quadrant::Bottom),
        "br" => Some(Quadrant::BottomRight),
        _ => None,
    }
}

fn editor_visual_signature(state: &EditorState) -> u64 {
    let mut hasher = VisualHasher::new();
    state.edit_revision().hash(&mut hasher);
    state.selection.len().hash(&mut hasher);
    for id in state.selection.iter() {
        id.hash(&mut hasher);
    }
    state.selected_component.hash(&mut hasher);
    state.selected_anchor.hash(&mut hasher);
    hash_f64(&mut hasher, state.viewport.offset.x);
    hash_f64(&mut hasher, state.viewport.offset.y);
    hash_f64(&mut hasher, state.viewport.zoom);
    hash_optional_rect(&mut hasher, state.marquee);
    hash_segment_hover(&mut hasher, state.segment_hover);
    hash_shape_preview(&mut hasher, state.shape_preview);
    hash_pen_preview(&mut hasher, state.pen_preview);
    hash_measure_preview(&mut hasher, state.measure_preview.as_ref());
    hash_knife_preview(&mut hasher, state.knife_preview.as_ref());
    hasher.value()
}

struct VisualHasher(u64);

impl VisualHasher {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn value(&self) -> u64 {
        self.0
    }

    fn write_tag(&mut self, tag: u8) {
        self.write_u8(tag);
    }
}

impl Hasher for VisualHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write(&[value]);
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

fn hash_segment_hover(hasher: &mut VisualHasher, preview: Option<SegmentHoverPreview>) {
    match preview {
        Some(SegmentHoverPreview::Line(line)) => {
            hasher.write_tag(1);
            hash_line(hasher, line);
        }
        Some(SegmentHoverPreview::Cubic(cubic)) => {
            hasher.write_tag(2);
            hash_point(hasher, cubic.p0);
            hash_point(hasher, cubic.p1);
            hash_point(hasher, cubic.p2);
            hash_point(hasher, cubic.p3);
        }
        Some(SegmentHoverPreview::Quadratic(quad)) => {
            hasher.write_tag(3);
            hash_point(hasher, quad.p0);
            hash_point(hasher, quad.p1);
            hash_point(hasher, quad.p2);
        }
        None => hasher.write_tag(0),
    }
}

fn hash_shape_preview(hasher: &mut VisualHasher, preview: Option<ShapePreview>) {
    match preview {
        Some(ShapePreview::Rectangle(rect)) => {
            hasher.write_tag(1);
            hash_rect(hasher, rect);
        }
        Some(ShapePreview::Ellipse(rect)) => {
            hasher.write_tag(2);
            hash_rect(hasher, rect);
        }
        None => hasher.write_tag(0),
    }
}

fn hash_pen_preview(hasher: &mut VisualHasher, preview: Option<PenPreview>) {
    let Some(preview) = preview else {
        hasher.write_tag(0);
        return;
    };
    hasher.write_tag(1);
    hash_optional_point(hasher, preview.line_start);
    hash_point(hasher, preview.cursor);
    hash_optional_point(hasher, preview.close_target);
    hash_optional_point(hasher, preview.snap_target);
}

fn hash_measure_preview(hasher: &mut VisualHasher, preview: Option<&MeasurePreview>) {
    let Some(preview) = preview else {
        hasher.write_tag(0);
        return;
    };
    hasher.write_tag(1);
    hash_line(hasher, preview.line);
    hash_f64(hasher, preview.distance);
    hash_f64(hasher, preview.angle_degrees);
    hasher.write_usize(preview.intersections.len());
    for point in &preview.intersections {
        hash_point(hasher, *point);
    }
    hasher.write_usize(preview.segment_labels.len());
    for label in &preview.segment_labels {
        hash_point(hasher, label.position);
        hash_f64(hasher, label.length);
    }
}

fn hash_knife_preview(hasher: &mut VisualHasher, preview: Option<&KnifePreview>) {
    let Some(preview) = preview else {
        hasher.write_tag(0);
        return;
    };
    hasher.write_tag(1);
    hash_line(hasher, preview.line);
    hasher.write_usize(preview.intersections.len());
    for point in &preview.intersections {
        hash_point(hasher, *point);
    }
}

fn hash_optional_rect(hasher: &mut VisualHasher, rect: Option<Rect>) {
    if let Some(rect) = rect {
        hasher.write_tag(1);
        hash_rect(hasher, rect);
    } else {
        hasher.write_tag(0);
    }
}

fn hash_rect(hasher: &mut VisualHasher, rect: Rect) {
    hash_f64(hasher, rect.x0);
    hash_f64(hasher, rect.y0);
    hash_f64(hasher, rect.x1);
    hash_f64(hasher, rect.y1);
}

fn hash_optional_point(hasher: &mut VisualHasher, point: Option<Point>) {
    if let Some(point) = point {
        hasher.write_tag(1);
        hash_point(hasher, point);
    } else {
        hasher.write_tag(0);
    }
}

fn hash_point(hasher: &mut VisualHasher, point: Point) {
    hash_f64(hasher, point.x);
    hash_f64(hasher, point.y);
}

fn hash_line(hasher: &mut VisualHasher, line: Line) {
    hash_point(hasher, line.p0);
    hash_point(hasher, line.p1);
}

fn hash_f64(hasher: &mut VisualHasher, value: f64) {
    hasher.write_u64(value.to_bits());
}

fn normalize_anchor_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn component_alignment_disabled_lib() -> norad::Plist {
    let mut lib = norad::Plist::new();
    lib.insert(
        "com.glyphsapp.component.alignment".to_string(),
        plist::Value::Integer((-1).into()),
    );
    lib
}

fn apply_component_alignment_disabled_lib(component: &mut norad::Component) {
    let mut lib = component.lib().cloned().unwrap_or_else(norad::Plist::new);
    lib.insert(
        "com.glyphsapp.component.alignment".to_string(),
        plist::Value::Integer((-1).into()),
    );
    component.replace_lib(lib);
}

fn to_norad_contour(contour: &WsContour) -> norad::Contour {
    let points = contour.points.iter().map(to_norad_point).collect();
    let is_hyperbezier = contour
        .points
        .iter()
        .any(|pt| matches!(pt.point_type, WsPointType::Hyper | WsPointType::HyperCorner));
    let identifier = if is_hyperbezier {
        Some(norad::Identifier::new("hyperbezier").expect("static identifier is valid"))
    } else {
        None
    };
    norad::Contour::new(points, identifier, None)
}

fn to_norad_anchor(anchor: &AnchorPoint) -> Result<norad::Anchor, JsValue> {
    let name = anchor
        .name
        .as_ref()
        .map(|name| {
            norad::Name::new(name).map_err(|e| JsValue::from_str(&format!("anchor name: {e}")))
        })
        .transpose()?;
    Ok(norad::Anchor::new(
        anchor.point.x,
        anchor.point.y,
        name,
        anchor.color.clone(),
        anchor.identifier.clone(),
        anchor.lib.clone(),
    ))
}

fn to_norad_point(pt: &WsContourPoint) -> norad::ContourPoint {
    let is_hyper = matches!(pt.point_type, WsPointType::Hyper | WsPointType::HyperCorner);
    let (x, y) = if is_hyper {
        (pt.x.round(), pt.y.round())
    } else {
        (pt.x, pt.y)
    };
    norad::ContourPoint::new(
        x,
        y,
        to_norad_point_type(pt.point_type),
        pt.smooth,
        None,
        None,
        None,
    )
}

fn to_norad_point_type(typ: WsPointType) -> norad::PointType {
    match typ {
        WsPointType::Move => norad::PointType::Move,
        WsPointType::Line => norad::PointType::Line,
        WsPointType::OffCurve => norad::PointType::OffCurve,
        WsPointType::Curve => norad::PointType::Curve,
        WsPointType::QCurve => norad::PointType::QCurve,
        WsPointType::Hyper => norad::PointType::Curve,
        WsPointType::HyperCorner => norad::PointType::Line,
    }
}

fn write_handle(out: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, stroke: f64) {
    write!(
        out,
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="var(--rb-primary-text, #909090)" stroke-width="{}" vector-effect="non-scaling-stroke" />"##,
        x1,
        y1,
        x2,
        y2,
        stroke
    )
    .expect("write handle");
}

fn write_point(
    out: &mut String,
    pt: &norad::ContourPoint,
    smooth_radius: f64,
    offcurve_radius: f64,
    corner_half: f64,
    point_outline: f64,
) {
    let (fill, stroke) = if matches!(pt.typ, norad::PointType::OffCurve) {
        (
            "var(--rb-canvas-point-offcurve-inner, #181818)",
            "var(--rb-canvas-point-offcurve-outer, #8c6cff)",
        )
    } else if pt.smooth {
        (
            "var(--rb-canvas-point-smooth-inner, #181818)",
            "var(--rb-canvas-point-smooth-outer, #18b86f)",
        )
    } else {
        (
            "var(--rb-canvas-point-corner-inner, #181818)",
            "var(--rb-canvas-point-corner-outer, #ff980f)",
        )
    };

    match pt.typ {
        norad::PointType::OffCurve => {
            write!(
                out,
                r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" vector-effect="non-scaling-stroke" />"#,
                pt.x,
                pt.y,
                offcurve_radius,
                fill,
                stroke,
                point_outline
            )
            .expect("write offcurve");
        }
        _ => {
            if pt.smooth {
                write!(
                    out,
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" vector-effect="non-scaling-stroke" />"#,
                    pt.x,
                    pt.y,
                    smooth_radius,
                    fill,
                    stroke,
                    point_outline
                )
                .expect("write smooth point");
            } else {
                write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}" vector-effect="non-scaling-stroke" />"#,
                    pt.x - corner_half,
                    pt.y - corner_half,
                    corner_half * 2.0,
                    corner_half * 2.0,
                    fill,
                    stroke,
                    point_outline
                )
                .expect("write corner point");
            }
        }
    }
}

fn build_event(x: f64, y: f64, button: u32, mods: u32) -> MouseEvent {
    let button = match button {
        0 => Some(MouseButton::Left),
        2 => Some(MouseButton::Right),
        u32::MAX => None,
        _ => Some(MouseButton::Other),
    };
    let modifiers = Modifiers {
        shift: mods & 0b0001 != 0,
        ctrl: mods & 0b0010 != 0,
        alt: mods & 0b0100 != 0,
        meta: mods & 0b1000 != 0,
    };
    MouseEvent::with_modifiers(Point::new(x, y), button, modifiers)
}
