// wasm-bindgen surface — the JS/Vue-facing API. Holds the editor
// state, the mouse state machine, the active tool, the renderer, and
// the undo stack.

#![cfg(target_arch = "wasm32")]

use kurbo::{Affine, BezPath, Point, Shape, Vec2};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use runebender_core::{GlyphCategory, GlyphMetadata, mark_color};

use crate::editing::{Modifiers, Mouse, MouseButton, MouseEvent, UndoState};
use crate::editor::{ComponentPreview, EditorState, norad_glyph_to_bezpath};
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextBufferSnapshot {
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
struct TextSortSnapshot {
    kind: &'static str,
    glyph_name: Option<String>,
    char: Option<String>,
    codepoint: Option<u32>,
    advance_width: Option<f64>,
    active: bool,
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
    let metadata = glyph_metadata_from_norad(&glyph);
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
        glyph.lib.insert(
            "public.markColor".to_string(),
            mark_color.into(),
        );
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
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let bez = norad_glyph_to_bezpath(&glyph);
    svg_from_bezpath(&bez)
}

/// Parse a .glif file's bytes and return an SVG with UFO components
/// resolved against a JSON object of `{ glyphName: glifXml }`.
/// This mirrors xilem's grid/preview behavior for composite glyphs.
#[wasm_bindgen(js_name = glifToSvgWithComponents)]
pub fn glif_to_svg_with_components(
    bytes: &[u8],
    glyph_xml_by_name: &str,
) -> Result<String, JsValue> {
    let glyph = norad::Glyph::parse_raw(bytes)
        .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
    let glyphs = parse_glif_xml_map(glyph_xml_by_name)?;

    let mut bez = norad_glyph_to_bezpath(&glyph);
    append_norad_components_to_bezpath(&mut bez, &glyph, &glyphs, Affine::IDENTITY, 0);
    svg_from_bezpath(&bez)
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
pub fn glif_map_to_svgs(glyph_xml_by_name: &str) -> Result<String, JsValue> {
    let xml_by_name: GlifXmlMap = serde_json::from_str(glyph_xml_by_name)
        .map_err(|e| JsValue::from_str(&format!("parse glyph XML map: {e}")))?;

    // Parse all glyphs once so component references can resolve.
    let mut glyphs: HashMap<String, norad::Glyph> = HashMap::with_capacity(xml_by_name.len());
    for (name, xml) in &xml_by_name {
        if let Ok(glyph) = norad::Glyph::parse_raw(xml.as_bytes()) {
            glyphs.insert(name.clone(), glyph);
        }
    }

    let mut svgs: HashMap<String, String> = HashMap::with_capacity(glyphs.len());
    for (name, glyph) in &glyphs {
        let mut bez = norad_glyph_to_bezpath(glyph);
        append_norad_components_to_bezpath(&mut bez, glyph, &glyphs, Affine::IDENTITY, 0);
        if let Ok(svg) = svg_from_bezpath(&bez) {
            if !svg.is_empty() {
                svgs.insert(name.clone(), svg);
            }
        }
    }

    serde_json::to_string(&svgs)
        .map_err(|e| JsValue::from_str(&format!("serialize svgs: {e}")))
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
            Some(ComponentPreview {
                id: crate::model::EntityId::next(),
                index,
                base: base_name,
                transform: Affine::new([
                    t.x_scale, t.xy_scale, t.yx_scale, t.y_scale, t.x_offset, t.y_offset,
                ]),
                path,
            })
        })
        .collect()
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
    let bbox = bez.bounding_box();
    let side = bbox.width().max(bbox.height()).max(1.0);
    let outline_stroke = (side / 200.0).clamp(1.0, 2.5);
    let handle_stroke = (side / 320.0).clamp(0.5, 1.5);
    let point_radius = (side / 65.0).clamp(2.0, 8.0);
    let corner_half = point_radius * 0.75;

    let mut out = String::new();
    write!(
        &mut out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" preserveAspectRatio="xMidYMid meet">"#,
        bbox.x0,
        -bbox.y1,
        bbox.width(),
        bbox.height()
    )
    .expect("write svg header");
    out.push_str(r#"<g transform="scale(1 -1)">"#);
    write!(
        &mut out,
        r##"<path d="{}" fill="none" stroke="#66EE88" stroke-width="{}" vector-effect="non-scaling-stroke" />"##,
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
                write_point(&mut out, pt, point_radius, corner_half);
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
            write_point(&mut out, pt, point_radius, corner_half);
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
    /// Snapshot of `state` taken on a left-button pointerdown, pushed
    /// onto the undo stack on the matching pointerup. `None` between
    /// strokes.
    pending_snapshot: Option<EditorState>,
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
            pending_snapshot: None,
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
        self.pending_snapshot = None;
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
        self.undo.clear();
        self.point_clipboard = None;
        self.pending_snapshot = None;
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
        let glyphs = parse_glif_xml_map(glyph_xml_by_name)?;
        self.state.set_glyph_from_norad(&glyph);
        self.state
            .set_component_previews(build_component_previews(&glyph, &glyphs));
        self.undo.clear();
        self.point_clipboard = None;
        self.pending_snapshot = None;
        Ok(())
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

    pub fn render(&mut self) -> Result<(), JsValue> {
        self.renderer
            .render(&self.state, self.tool.is_preview(), self.tool.is_text())
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
        self.state.text_buffer.shape_arabic()
    }

    #[wasm_bindgen(js_name = textBufferLen)]
    pub fn text_buffer_len(&self) -> usize {
        self.state.text_buffer.len()
    }

    #[wasm_bindgen(js_name = textCursor)]
    pub fn text_cursor(&self) -> usize {
        self.state.text_buffer.cursor()
    }

    #[wasm_bindgen(js_name = textActiveSort)]
    pub fn text_active_sort(&self) -> i32 {
        self.state
            .text_buffer
            .active_sort()
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    #[wasm_bindgen(js_name = textBufferSnapshot)]
    pub fn text_buffer_snapshot(&self) -> Result<String, JsValue> {
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
        let snapshot = TextBufferSnapshot {
            cursor: self.state.text_buffer.cursor(),
            active_sort: self.state.text_buffer.active_sort(),
            direction: match self.state.text_buffer.direction() {
                TextDirection::LeftToRight => "ltr",
                TextDirection::RightToLeft => "rtl",
            },
            sorts,
        };
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&format!("serialize text buffer: {e}")))
    }

    #[wasm_bindgen(js_name = textBufferLayout)]
    pub fn text_buffer_layout(&self, line_height: f64) -> Result<String, JsValue> {
        let layout = self.state.text_buffer.layout(line_height.max(1.0));
        let snapshot = TextLayoutSnapshot {
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
        };
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&format!("serialize text layout: {e}")))
    }

    #[wasm_bindgen(js_name = clearTextBuffer)]
    pub fn clear_text_buffer(&mut self) {
        self.state.text_buffer.clear();
    }

    #[wasm_bindgen(js_name = insertTextGlyph)]
    pub fn insert_text_glyph(&mut self, name: &str, codepoint: u32, advance_width: f64) {
        let codepoint = text_codepoint_from_wasm(codepoint);
        self.state
            .text_buffer
            .insert_glyph(name, codepoint, advance_width);
    }

    #[wasm_bindgen(js_name = insertTextCharacter)]
    pub fn insert_text_character(&mut self, codepoint: u32) -> bool {
        text_codepoint_from_wasm(codepoint)
            .map(|char| self.state.text_buffer.insert_character(char))
            .unwrap_or(false)
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
        self.state.text_buffer.insert_line_break();
    }

    #[wasm_bindgen(js_name = deleteTextBeforeCursor)]
    pub fn delete_text_before_cursor(&mut self) -> bool {
        self.state.text_buffer.delete_before_cursor().is_some()
    }

    #[wasm_bindgen(js_name = deleteTextAfterCursor)]
    pub fn delete_text_after_cursor(&mut self) -> bool {
        self.state.text_buffer.delete_after_cursor().is_some()
    }

    #[wasm_bindgen(js_name = setTextCursor)]
    pub fn set_text_cursor(&mut self, cursor: usize) {
        self.state.text_buffer.set_cursor(cursor);
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualLeft)]
    pub fn move_text_cursor_visual_left(&mut self) {
        self.state.text_buffer.move_cursor_visual_left();
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualRight)]
    pub fn move_text_cursor_visual_right(&mut self) {
        self.state.text_buffer.move_cursor_visual_right();
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualUp)]
    pub fn move_text_cursor_visual_up(&mut self, line_height: f64) {
        self.state
            .text_buffer
            .move_cursor_visual_up(line_height.max(1.0));
    }

    #[wasm_bindgen(js_name = moveTextCursorVisualDown)]
    pub fn move_text_cursor_visual_down(&mut self, line_height: f64) {
        self.state
            .text_buffer
            .move_cursor_visual_down(line_height.max(1.0));
    }

    #[wasm_bindgen(js_name = moveTextCursorLineStart)]
    pub fn move_text_cursor_line_start(&mut self) {
        self.state.text_buffer.move_cursor_line_start();
    }

    #[wasm_bindgen(js_name = moveTextCursorLineEnd)]
    pub fn move_text_cursor_line_end(&mut self) {
        self.state.text_buffer.move_cursor_line_end();
    }

    #[wasm_bindgen(js_name = activateTextSort)]
    pub fn activate_text_sort(&mut self, index: usize) -> bool {
        self.state.text_buffer.activate_sort(index)
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
        // Snapshot before mutating, but only for left-button strokes —
        // middle-button drags only pan and shouldn't pollute undo.
        if button == 0 {
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        if let Some(prev) = self.undo.undo(self.state.clone()) {
            self.state = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.undo.redo(self.state.clone()) {
            self.state = next;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = flipSelectionHorizontal)]
    pub fn flip_selection_horizontal(&mut self) -> bool {
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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

    #[wasm_bindgen(js_name = setLeftSidebearing)]
    pub fn set_left_sidebearing(&mut self, value: f64) -> bool {
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
        if self.state.toggle_point_type() {
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
        let snapshot = self.state.clone();
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

    #[wasm_bindgen(js_name = unionSelection)]
    pub fn union_selection(&mut self) -> bool {
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
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
        let snapshot = self.state.clone();
        if self.state.move_selection_reference(axis, value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = resizeSelectionReference)]
    pub fn resize_selection_reference(&mut self, axis: &str, value: f64) -> bool {
        let snapshot = self.state.clone();
        if self.state.resize_selection_reference(axis, value) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
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
        let snapshot = self.state.clone();
        if self.state.nudge_selection(dx, dy, shift, ctrl, independent) {
            self.undo.add_undo_group(snapshot);
            self.pending_snapshot = None;
            true
        } else {
            false
        }
    }

    /// Number of currently selected entities. Useful for status UI.
    #[wasm_bindgen(js_name = selectionCount)]
    pub fn selection_count(&self) -> usize {
        self.state.selection.len()
    }

    /// Number of contours touched by the current point selection.
    #[wasm_bindgen(js_name = selectedContourCount)]
    pub fn selected_contour_count(&self) -> usize {
        self.state.selected_contour_count()
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
        &self,
        original_bytes: &[u8],
        mark_color: &str,
    ) -> Result<Vec<u8>, JsValue> {
        let mut glyph = norad::Glyph::parse_raw(original_bytes)
            .map_err(|e| JsValue::from_str(&format!("parse .glif: {e}")))?;
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
            if let Some(transform) = self.state.component_transform(index) {
                component.transform = affine_to_norad_transform(transform);
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
                None,
            ));
        }

        let mark_color = mark_color::canonical_ufo_mark_color(mark_color)
            .ok_or_else(|| JsValue::from_str("invalid UFO public.markColor value"))?;
        if mark_color.is_empty() {
            glyph.lib.remove("public.markColor");
        } else {
            glyph.lib.insert(
                "public.markColor".to_string(),
                mark_color.into(),
            );
        }

        glyph
            .encode_xml()
            .map_err(|e| JsValue::from_str(&format!("serialize .glif: {e}")))
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
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#66EE88" stroke-width="{}" vector-effect="non-scaling-stroke" />"##,
        x1,
        y1,
        x2,
        y2,
        stroke
    )
    .expect("write handle");
}

fn write_point(out: &mut String, pt: &norad::ContourPoint, radius: f64, corner_half: f64) {
    let (fill, stroke) = if matches!(pt.typ, norad::PointType::OffCurve) {
        ("#cc99ff", "#9900ff")
    } else if pt.smooth {
        ("#579aff", "#4428ec")
    } else {
        ("#6ae756", "#208e56")
    };

    match pt.typ {
        norad::PointType::OffCurve => {
            write!(
                out,
                r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="1" vector-effect="non-scaling-stroke" />"#,
                pt.x,
                pt.y,
                radius,
                fill,
                stroke
            )
            .expect("write offcurve");
        }
        _ => {
            if pt.smooth {
                write!(
                    out,
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="1" vector-effect="non-scaling-stroke" />"#,
                    pt.x,
                    pt.y,
                    radius,
                    fill,
                    stroke
                )
                .expect("write smooth point");
            } else {
                write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1" vector-effect="non-scaling-stroke" />"#,
                    pt.x - corner_half,
                    pt.y - corner_half,
                    corner_half * 2.0,
                    corner_half * 2.0,
                    fill,
                    stroke
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
