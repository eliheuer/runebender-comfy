// Editor state — the source of truth for what's being edited.
//
// Renderer reads from this; mouse/keyboard handlers mutate it. Lives
// outside the wasm-bindgen surface so it's testable on native too.

use std::collections::HashSet;

use kurbo::{Affine, BezPath, Line, PathEl, Point, Rect, Shape, Vec2};
use kurbo_012::{BezPath as BezPath012, PathEl as PathEl012, Point as Point012};
use serde::Deserialize;

use crate::editing::{Selection, ViewPort};
use crate::model::EntityId;
use crate::model::workspace::{
    Contour as WsContour, ContourPoint as WsContourPoint, PointType as WsPointType,
};
use crate::path::{
    CubicPath, HyperPath, Path, PathPoint, PathPoints, PointType, Quadrant, QuadraticPath, Segment,
    SegmentInfo,
};
use crate::text::TextBuffer;

const DESIGN_GRID_SPACING: f64 = 2.0;
const DEFAULT_ROUND_CORNER_OFFSET: f64 = 32.0;
const DEFAULT_ROUND_CORNER_HANDLE_RATIO: f64 = 0.552_284_749_830_793_6;
const MAX_ROUND_CORNER_SIDE_FRACTION: f64 = 0.45;

// ============================================================================
// FontMetrics
// ============================================================================

/// Vertical metrics from fontinfo.plist. Every field is optional —
/// UFO doesn't require any of them. Coordinates are in design space
/// (y-up, units defined by `units_per_em`).
#[derive(Debug, Clone, Default)]
pub struct FontMetrics {
    pub units_per_em: Option<f64>,
    pub ascender: Option<f64>,
    pub descender: Option<f64>,
    pub x_height: Option<f64>,
    pub cap_height: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum ShapePreview {
    Rectangle(Rect),
    Ellipse(Rect),
}

#[derive(Debug, Clone, Copy)]
pub struct PenPreview {
    pub line_start: Option<Point>,
    pub cursor: Point,
    pub close_target: Option<Point>,
    pub snap_target: Option<Point>,
}

#[derive(Debug, Clone, Copy)]
pub enum SegmentHoverPreview {
    Line(Line),
    Cubic(kurbo::CubicBez),
    Quadratic(kurbo::QuadBez),
}

#[derive(Debug, Clone)]
pub struct MeasurePreview {
    pub line: Line,
    pub distance: f64,
    pub angle_degrees: f64,
    pub intersections: Vec<Point>,
    pub segment_labels: Vec<MeasureSegmentLabel>,
}

#[derive(Debug, Clone)]
pub struct KnifePreview {
    pub line: Line,
    pub intersections: Vec<Point>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeasureSegmentLabel {
    pub position: Point,
    pub length: f64,
}

#[derive(Debug, Clone)]
pub struct ComponentPreview {
    pub id: EntityId,
    pub index: usize,
    pub base: String,
    pub transform: Affine,
    pub path: BezPath,
}

/// Minimal subset of fontinfo.plist — only the fields we care about
/// for rendering vertical metric guidelines. Decoupled from norad's
/// (much larger) `FontInfo` struct so we aren't tied to its
/// every-spec-field surface.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFontInfo {
    units_per_em: Option<f64>,
    ascender: Option<f64>,
    descender: Option<f64>,
    x_height: Option<f64>,
    cap_height: Option<f64>,
}

impl FontMetrics {
    pub fn parse_plist(bytes: &[u8]) -> Result<Self, plist::Error> {
        let raw: RawFontInfo = plist::from_bytes(bytes)?;
        Ok(FontMetrics {
            units_per_em: raw.units_per_em,
            ascender: raw.ascender,
            descender: raw.descender,
            x_height: raw.x_height,
            cap_height: raw.cap_height,
        })
    }
}

/// In-memory state for one open glyph.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub paths: Vec<Path>,
    pub selection: Selection,
    pub viewport: ViewPort,

    /// Advance width of the open glyph (in design units). Drives the
    /// horizontal extent of the metric box. 0 means "no glyph loaded
    /// yet"; renderer skips drawing metric guides in that case.
    pub advance_width: f64,

    /// Resolved component preview outlines. These are not directly
    /// editable yet, but they let composite glyphs render like xilem
    /// while source component references stay preserved in `.glif`.
    pub component_preview: BezPath,
    pub component_previews: Vec<ComponentPreview>,
    pub selected_component: Option<EntityId>,
    pub deleted_component_indices: HashSet<usize>,

    /// Font-wide vertical metrics (baseline, x-height, cap-height,
    /// ascender, descender). Drawn as horizontal guideline lines in
    /// the renderer. `None` until the host loads fontinfo.plist.
    pub metrics: Option<FontMetrics>,

    /// Transient screen-space rectangle for an in-progress
    /// box-selection drag. Renderer draws it as a marquee; cleared
    /// when the drag ends.
    pub marquee: Option<Rect>,

    /// Transient screen-space shape outline for an in-progress Shapes
    /// drag. Cleared before committing the real path.
    pub shape_preview: Option<ShapePreview>,

    /// Transient screen-space Pen preview. Draws the next segment and
    /// close feedback without mutating the contour until input commits.
    pub pen_preview: Option<PenPreview>,

    /// Transient screen-space segment hover for Option/Alt line-to-curve
    /// feedback in Select.
    pub segment_hover: Option<SegmentHoverPreview>,

    /// Transient screen-space measurement overlay.
    pub measure_preview: Option<MeasurePreview>,

    /// Transient screen-space knife overlay. Full contour splitting is
    /// separate; this tracks the cut line and hit markers.
    pub knife_preview: Option<KnifePreview>,

    /// Most recent geometric transform. Used by Duplicate Repeat to
    /// mirror xilem's "duplicate, then apply last transform" behavior.
    pub last_transform: Option<Affine>,

    /// Coordinate-panel reference point for selected-bounds display.
    pub coord_quadrant: Quadrant,

    /// Rust-side Text tool buffer. Vue still renders the first
    /// preview strip, but this gives the editor core a session model
    /// compatible with xilem's Text tool migration.
    pub text_buffer: TextBuffer,

    /// Whether the editor has an open Text session. This intentionally
    /// differs from `text_buffer.is_empty()`: xilem keeps
    /// `text_buffer: Some(...)` even after deleting every sort, so Text
    /// mode, the split preview, and the insertion cursor still exist.
    pub has_text_session: bool,

    /// Monotonic counter for geometric edits. The wasm boundary uses
    /// this to tell selection-only pointer gestures from glyph edits.
    pub edit_revision: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            selection: Selection::default(),
            viewport: ViewPort::default(),
            advance_width: 0.0,
            component_preview: BezPath::new(),
            component_previews: Vec::new(),
            selected_component: None,
            deleted_component_indices: HashSet::new(),
            metrics: None,
            marquee: None,
            shape_preview: None,
            pen_preview: None,
            segment_hover: None,
            measure_preview: None,
            knife_preview: None,
            last_transform: None,
            coord_quadrant: Quadrant::default(),
            text_buffer: TextBuffer::default(),
            has_text_session: false,
            edit_revision: 0,
        }
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text_line_height(&self) -> f64 {
        let (top, bottom) = self.text_sort_metric_bounds();
        // Line breaks advance by the rendered sort metrics box. This keeps
        // the top of the lower line's UPM box exactly on the previous line's
        // descender instead of using only ascender - descender.
        (top - bottom).max(1.0)
    }

    pub fn text_metric_bounds(&self) -> (f64, f64) {
        let ascender = self
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.ascender)
            .unwrap_or(800.0);
        let descender = self
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.descender)
            .unwrap_or(-200.0);
        (ascender, descender)
    }

    pub fn text_sort_metric_bounds(&self) -> (f64, f64) {
        let (ascender, descender) = self.text_metric_bounds();
        let top = self
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.units_per_em)
            .filter(|units_per_em| units_per_em.is_finite() && *units_per_em > 0.0)
            .map(|units_per_em| units_per_em.max(ascender))
            .unwrap_or(ascender);
        (top, descender)
    }

    pub fn glyph_metric_bounds(&self) -> Option<(f64, f64)> {
        let metrics = self.metrics.as_ref()?;
        let ascender = metrics.ascender.unwrap_or(0.0);
        let descender = metrics.descender.unwrap_or(0.0);
        let top = metrics
            .units_per_em
            .filter(|units_per_em| units_per_em.is_finite() && *units_per_em > 0.0)
            .map(|units_per_em| units_per_em.max(ascender))
            .unwrap_or(ascender);
        Some((top, descender))
    }

    pub fn insert_text_character(&mut self, char: char) -> bool {
        let active_advance_width = self
            .text_buffer
            .active_sort()
            .and_then(|index| self.text_buffer.sort(index))
            .and_then(|sort| match &sort.kind {
                crate::text::TextSortKind::Glyph { codepoint, .. } => *codepoint,
                crate::text::TextSortKind::LineBreak => None,
            })
            .filter(|active_char| *active_char == char)
            .map(|_| self.advance_width);
        if !self
            .text_buffer
            .insert_character_with_active_advance(char, active_advance_width)
        {
            return false;
        }
        self.bump_edit_revision();
        true
    }

    pub fn insert_inactive_text_glyph(
        &mut self,
        name: impl Into<String>,
        codepoint: Option<char>,
        advance_width: f64,
    ) {
        self.text_buffer
            .insert_inactive_glyph(name, codepoint, advance_width);
        self.bump_edit_revision();
    }

    pub fn insert_text_line_break(&mut self) {
        self.text_buffer.insert_line_break();
        self.bump_edit_revision();
    }

    pub fn delete_text_before_cursor(&mut self) -> bool {
        if self.text_buffer.delete_before_cursor().is_none() {
            if !self
                .text_buffer
                .shape_arabic_around_if_rtl(self.text_buffer.cursor())
            {
                return false;
            }
            self.bump_edit_revision();
            return true;
        }
        self.text_buffer
            .shape_arabic_around_if_rtl(self.text_buffer.cursor());
        self.bump_edit_revision();
        true
    }

    pub fn delete_text_after_cursor(&mut self) -> bool {
        if self.text_buffer.delete_after_cursor().is_none() {
            if !self
                .text_buffer
                .shape_arabic_around_if_rtl(self.text_buffer.cursor())
            {
                return false;
            }
            self.bump_edit_revision();
            return true;
        }
        self.text_buffer
            .shape_arabic_around_if_rtl(self.text_buffer.cursor());
        self.bump_edit_revision();
        true
    }

    /// Design-space position of the active Text sort.
    ///
    /// Xilem renders the editable glyph data translated to the active
    /// sort's text-buffer position instead of leaving it at the glyph
    /// origin. Keeping this as a derived value avoids a second mutable
    /// source of truth for Text layout.
    pub fn active_text_sort_origin(&self) -> Vec2 {
        let Some(active_index) = self.text_buffer.active_sort() else {
            return Vec2::ZERO;
        };
        self.text_buffer
            .layout(self.text_line_height())
            .items
            .iter()
            .find(|item| item.index == active_index)
            .map(|item| Vec2::new(item.x, item.y))
            .unwrap_or(Vec2::ZERO)
    }

    pub fn screen_to_glyph_design(&self, screen: Point) -> Point {
        self.viewport.screen_to_design(screen) - self.active_text_sort_origin()
    }

    pub fn glyph_to_screen(&self, point: Point) -> Point {
        self.viewport
            .to_screen(point + self.active_text_sort_origin())
    }

    /// Replace the glyph from a `kurbo::BezPath` (typically parsed
    /// from SVG path data). Each `MoveTo` starts a new contour;
    /// each curve element produces explicit on/off-curve points.
    pub fn set_glyph_from_bezpath(&mut self, bez: &BezPath) {
        self.paths.clear();
        self.component_preview = BezPath::new();
        self.component_previews.clear();
        self.selected_component = None;
        self.deleted_component_indices.clear();
        self.selection = Selection::new();
        self.marquee = None;
        self.shape_preview = None;
        self.pen_preview = None;
        self.segment_hover = None;
        self.measure_preview = None;
        self.knife_preview = None;
        self.last_transform = None;
        self.coord_quadrant = Quadrant::default();
        self.edit_revision = 0;

        let mut current_points: Vec<PathPoint> = Vec::new();

        let flush = |paths: &mut Vec<Path>, points: &mut Vec<PathPoint>, closed: bool| {
            if !points.is_empty() {
                let cubic = CubicPath::new(PathPoints::from_vec(std::mem::take(points)), closed);
                paths.push(Path::Cubic(cubic));
            }
        };

        for el in bez.elements() {
            match el {
                PathEl::MoveTo(p) => {
                    flush(&mut self.paths, &mut current_points, false);
                    current_points.push(on_curve(*p, false));
                }
                PathEl::LineTo(p) => {
                    current_points.push(on_curve(*p, false));
                }
                PathEl::QuadTo(c, p) => {
                    current_points.push(off_curve(*c));
                    current_points.push(on_curve(*p, true));
                }
                PathEl::CurveTo(c1, c2, p) => {
                    current_points.push(off_curve(*c1));
                    current_points.push(off_curve(*c2));
                    current_points.push(on_curve(*p, true));
                }
                PathEl::ClosePath => {
                    flush(&mut self.paths, &mut current_points, true);
                }
            }
        }
        flush(&mut self.paths, &mut current_points, false);
    }

    /// Replace the glyph from a `norad::Glyph` (parsed from a `.glif`
    /// file). Walks norad's contours into our `workspace::Contour`
    /// representation, then uses the existing `Path::from_contour`
    /// dispatch which detects cubic / quadratic / hyperbezier shapes.
    pub fn set_glyph_from_norad(&mut self, glyph: &norad::Glyph) {
        self.paths.clear();
        self.component_preview = BezPath::new();
        self.component_previews.clear();
        self.selected_component = None;
        self.selection = Selection::new();
        self.marquee = None;
        self.shape_preview = None;
        self.pen_preview = None;
        self.segment_hover = None;
        self.measure_preview = None;
        self.knife_preview = None;
        self.last_transform = None;
        self.coord_quadrant = Quadrant::default();
        self.edit_revision = 0;
        self.advance_width = glyph.width;

        for norad_contour in &glyph.contours {
            let ws_contour = convert_norad_contour(norad_contour);
            self.paths.push(Path::from_contour(&ws_contour));
        }
    }

    /// Reload editable glyph data inside an existing editor session.
    ///
    /// Text sort activation mirrors xilem by swapping the active sort's
    /// contours into the same edit session. Unlike opening a new glyph from
    /// the grid, this must not clear undo history, reset the edit revision, or
    /// reset editor-session UI state such as the coordinate reference quadrant.
    pub fn set_glyph_from_norad_preserving_history(&mut self, glyph: &norad::Glyph) {
        let edit_revision = self.edit_revision;
        let coord_quadrant = self.coord_quadrant;
        let last_transform = self.last_transform;
        self.set_glyph_from_norad(glyph);
        self.edit_revision = edit_revision;
        self.coord_quadrant = coord_quadrant;
        self.last_transform = last_transform;
    }

    pub fn set_component_preview(&mut self, preview: BezPath) {
        self.component_preview = preview;
    }

    pub fn set_component_previews(&mut self, previews: Vec<ComponentPreview>) {
        self.component_previews = previews;
        self.deleted_component_indices.clear();
        self.rebuild_component_preview();
    }

    pub fn component_transform(&self, index: usize) -> Option<Affine> {
        self.component_previews
            .iter()
            .find(|component| component.index == index)
            .map(|component| component.transform)
    }

    pub fn hit_test_component(&self, design_pt: Point) -> Option<EntityId> {
        for component in self.component_previews.iter().rev() {
            let transformed = component.transform * &component.path;
            if transformed.winding(design_pt) != 0 {
                return Some(component.id);
            }
        }
        None
    }

    pub fn component_base_at(&self, design_pt: Point) -> Option<&str> {
        let hit = self.hit_test_component(design_pt)?;
        self.component_previews
            .iter()
            .find(|component| component.id == hit)
            .map(|component| component.base.as_str())
    }

    pub fn select_component(&mut self, id: EntityId) {
        self.selection = Selection::new();
        self.selected_component = Some(id);
    }

    pub fn clear_component_selection(&mut self) {
        self.selected_component = None;
    }

    pub fn translate_selected_component(&mut self, delta: Vec2) -> bool {
        let Some(selected) = self.selected_component else {
            return false;
        };
        if delta == Vec2::ZERO {
            return false;
        }
        let Some(component) = self
            .component_previews
            .iter_mut()
            .find(|component| component.id == selected)
        else {
            return false;
        };
        component.transform = Affine::translate(delta) * component.transform;
        self.rebuild_component_preview();
        self.bump_edit_revision();
        true
    }

    pub fn transform_selected_component(&mut self, transform: Affine) -> bool {
        let Some(selected) = self.selected_component else {
            return false;
        };
        let Some(component) = self
            .component_previews
            .iter_mut()
            .find(|component| component.id == selected)
        else {
            return false;
        };
        component.transform = transform * component.transform;
        self.rebuild_component_preview();
        self.bump_edit_revision();
        true
    }

    pub fn delete_selected_component(&mut self) -> bool {
        let Some(selected) = self.selected_component else {
            return false;
        };
        let Some(index) = self
            .component_previews
            .iter()
            .position(|component| component.id == selected)
        else {
            return false;
        };
        let component = self.component_previews.remove(index);
        self.deleted_component_indices.insert(component.index);
        self.selected_component = None;
        self.rebuild_component_preview();
        self.bump_edit_revision();
        true
    }

    pub fn duplicate_selected_component(&mut self) -> bool {
        let Some(selected) = self.selected_component else {
            return false;
        };
        let Some(component) = self
            .component_previews
            .iter()
            .find(|component| component.id == selected)
            .cloned()
        else {
            return false;
        };
        let offset = Vec2::new(20.0, 20.0);
        let mut duplicate = component;
        duplicate.id = EntityId::next();
        duplicate.index = self
            .component_previews
            .iter()
            .map(|component| component.index)
            .max()
            .unwrap_or(0)
            + 1;
        duplicate.transform = Affine::translate(offset) * duplicate.transform;
        self.selected_component = Some(duplicate.id);
        self.component_previews.push(duplicate);
        self.rebuild_component_preview();
        self.bump_edit_revision();
        true
    }

    pub fn selected_component_bounds(&self) -> Option<Rect> {
        let selected = self.selected_component?;
        let component = self
            .component_previews
            .iter()
            .find(|component| component.id == selected)?;
        let transformed = component.transform * &component.path;
        if transformed.elements().is_empty() {
            return None;
        }
        let bbox = transformed.bounding_box();
        if bbox.x0.is_finite() && bbox.y0.is_finite() {
            Some(bbox)
        } else {
            None
        }
    }

    fn rebuild_component_preview(&mut self) {
        let mut preview = BezPath::new();
        for component in &self.component_previews {
            let transformed = component.transform * &component.path;
            preview.extend(transformed.elements().iter().cloned());
        }
        self.component_preview = preview;
    }

    /// Bounding box of all paths in design space, or `None` if empty.
    pub fn glyph_bbox(&self) -> Option<Rect> {
        let mut bbox: Option<Rect> = None;
        if !self.component_preview.elements().is_empty() {
            let b = self.component_preview.bounding_box();
            if b.x0.is_finite() && b.y0.is_finite() {
                bbox = Some(b);
            }
        }
        for path in &self.paths {
            let bez = path.to_bezpath();
            if bez.elements().is_empty() {
                continue;
            }
            let b = bez.bounding_box();
            if !b.x0.is_finite() || !b.y0.is_finite() {
                continue;
            }
            bbox = Some(match bbox {
                Some(prev) => prev.union(b),
                None => b,
            });
        }
        bbox
    }

    /// Auto-zoom and center the glyph in a `width × height` canvas
    /// (in screen-space pixels). Mirrors runebender-xilem's
    /// `initialize_viewport`: fit the font's vertical metrics
    /// (ascender − descender) to 60% of the canvas height — NOT the
    /// per-glyph ink bbox — so every glyph keeps a consistent scale
    /// (a short "a" stays short, a tall "h" stays tall) with generous
    /// margin. Center horizontally on the middle of the advance width.
    pub fn fit_to_canvas(&mut self, width: f64, height: f64) {
        let ascender = self
            .metrics
            .as_ref()
            .and_then(|m| m.ascender)
            .unwrap_or(800.0);
        let descender = self
            .metrics
            .as_ref()
            .and_then(|m| m.descender)
            .unwrap_or(-200.0);
        let design_height = (ascender - descender).max(1.0);

        // Leave 40% padding, matching xilem's `padding = 0.6`.
        let zoom = (height * 0.6 / design_height).max(1e-3);
        self.viewport.zoom = zoom;

        let design_center_x = self.advance_width / 2.0;
        let design_center_y = (ascender + descender) / 2.0;
        // Screen y is flipped: screen.y = -design.y * zoom + offset.y
        self.viewport.offset = Vec2::new(
            width / 2.0 - design_center_x * zoom,
            height / 2.0 + design_center_y * zoom,
        );
    }

    /// Translate every selected point by `delta` (in design space).
    pub fn translate_selection(&mut self, delta: Vec2) {
        if self.selection.is_empty() || delta == Vec2::ZERO {
            return;
        }
        for path in &mut self.paths {
            translate_in_path_with_handles(path, &self.selection, delta);
        }
        self.snap_selection_points_to_grid(true);
        self.bump_edit_revision();
    }

    /// Translate only explicitly selected points by `delta`.
    ///
    /// This is xilem's Option/Alt-arrow behavior: selected on-curve
    /// points move without dragging their adjacent off-curve handles.
    pub fn translate_selection_independent(&mut self, delta: Vec2) {
        if self.selection.is_empty() || delta == Vec2::ZERO {
            return;
        }
        for path in &mut self.paths {
            translate_in_path(path, &self.selection, delta);
        }
        self.snap_selection_points_to_grid(false);
        self.bump_edit_revision();
    }

    /// Nudge selected points or components by xilem's keyboard amounts.
    pub fn nudge_selection(
        &mut self,
        dx: f64,
        dy: f64,
        shift: bool,
        ctrl: bool,
        independent: bool,
    ) -> bool {
        let amount = if ctrl {
            32.0
        } else if shift {
            8.0
        } else {
            2.0
        };
        let delta = Vec2::new(dx * amount, dy * amount);
        if delta == Vec2::ZERO {
            return false;
        }
        if self.selected_component.is_some() {
            return self.translate_selected_component(delta);
        }
        if self.selection.is_empty() {
            return false;
        }
        if independent {
            self.translate_selection_independent(delta);
        } else {
            self.translate_selection(delta);
            self.snap_selection_to_grid();
        }
        true
    }

    /// Snap selected on-curve points to xilem's 2-unit design grid.
    /// Adjacent off-curve handles receive the same snap offset so the
    /// local curve shape is preserved.
    pub fn snap_selection_to_grid(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let changed = self.snap_selection_points_to_grid(true);
        if changed {
            self.bump_edit_revision();
        }
        changed
    }

    /// Snap selected off-curve handles to the 2-unit design grid.
    ///
    /// This is intentionally narrower than `snap_selection_to_grid`: pointer
    /// dragging handles should not leave tiny floating-point residue, but
    /// selected on-curve points keep the exact pointer delta until explicit
    /// grid commands/nudges snap them.
    pub fn snap_selected_offcurves_to_grid(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }

        let mut changed = false;
        for path in &mut self.paths {
            match path {
                Path::Cubic(cubic) => {
                    let closed = cubic.closed;
                    let points = cubic.points.make_mut();
                    let mut path_changed = false;
                    for point in points.iter_mut() {
                        if self.selection.contains(&point.id) && point.is_off_curve() {
                            let snapped = snap_point_to_grid(point.point);
                            if snapped != point.point {
                                point.point = snapped;
                                changed = true;
                                path_changed = true;
                            }
                        }
                    }
                    if path_changed {
                        maintain_smooth_handle_tangents(points, &self.selection, closed);
                    }
                }
                Path::Quadratic(quadratic) => {
                    for point in quadratic.points.make_mut() {
                        if self.selection.contains(&point.id) && point.is_off_curve() {
                            let snapped = snap_point_to_grid(point.point);
                            if snapped != point.point {
                                point.point = snapped;
                                changed = true;
                            }
                        }
                    }
                }
                Path::Hyper(_) => {}
            }
        }
        if changed {
            self.bump_edit_revision();
        }
        changed
    }

    fn snap_selection_points_to_grid(&mut self, include_adjacent_handles: bool) -> bool {
        let snap_ids = self.snap_target_ids(include_adjacent_handles);
        if snap_ids.is_empty() {
            return false;
        }

        let mut changed = false;
        for path in &mut self.paths {
            match path {
                Path::Cubic(cubic) => {
                    let closed = cubic.closed;
                    let points = cubic.points.make_mut();
                    let mut path_changed = false;
                    for point in points.iter_mut() {
                        if snap_ids.contains(&point.id) {
                            let snapped = snap_point_to_grid(point.point);
                            if snapped != point.point {
                                point.point = snapped;
                                changed = true;
                                path_changed = true;
                            }
                        }
                    }
                    if path_changed {
                        changed |= maintain_smooth_handle_tangents(points, &self.selection, closed);
                    }
                }
                Path::Quadratic(quadratic) => {
                    for point in quadratic.points.make_mut() {
                        if snap_ids.contains(&point.id) {
                            let snapped = snap_point_to_grid(point.point);
                            if snapped != point.point {
                                point.point = snapped;
                                changed = true;
                            }
                        }
                    }
                }
                Path::Hyper(hyper) => {
                    let mut hyper_changed = false;
                    for point in hyper.points.make_mut() {
                        if snap_ids.contains(&point.id) {
                            let snapped = snap_point_to_grid(point.point);
                            if snapped != point.point {
                                point.point = snapped;
                                changed = true;
                                hyper_changed = true;
                            }
                        }
                    }
                    if hyper_changed {
                        hyper.after_change();
                    }
                }
            }
        }
        changed
    }

    fn snap_target_ids(&self, include_adjacent_handles: bool) -> HashSet<EntityId> {
        let mut ids = self.selection.iter().copied().collect::<HashSet<_>>();
        if !include_adjacent_handles {
            return ids;
        }
        for path in &self.paths {
            let points = path.points().iter().cloned().collect::<Vec<_>>();
            let closed = path_is_closed(path);
            for (index, point) in points.iter().enumerate() {
                if !self.selection.contains(&point.id) || !point.is_on_curve() {
                    continue;
                }
                for neighbor in [
                    previous_index(index, points.len(), closed),
                    next_index(index, points.len(), closed),
                ]
                .into_iter()
                .flatten()
                {
                    if points[neighbor].is_off_curve() {
                        ids.insert(points[neighbor].id);
                    }
                }
            }
        }
        ids
    }

    /// Move the active coordinate-panel reference point to the given
    /// design-space coordinate. `axis` matches the Comfy/Vue field id:
    /// "x" moves horizontally, "y" moves vertically.
    pub fn move_selection_reference(&mut self, axis: &str, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }

        let Some((_count, bounds)) = self.selection_bounds() else {
            return false;
        };
        let reference = self.selection_reference_point(bounds);
        let delta = match axis {
            "x" => Vec2::new(value - reference.x, 0.0),
            "y" => Vec2::new(0.0, value - reference.y),
            _ => return false,
        };

        if delta.hypot() < 1e-9 {
            return false;
        }

        if self.selected_component.is_some() {
            return self.translate_selected_component(delta);
        }
        self.translate_selection(delta);
        true
    }

    /// Resize selected points by scaling around the active
    /// coordinate-panel reference point. `axis` matches the W/H field:
    /// "width" scales horizontally, "height" scales vertically.
    pub fn resize_selection_reference(&mut self, axis: &str, value: f64) -> bool {
        if !value.is_finite() || value <= 0.0 {
            return false;
        }

        let Some((_count, bounds)) = self.selection_bounds() else {
            return false;
        };
        let current = match axis {
            "width" => bounds.width(),
            "height" => bounds.height(),
            _ => return false,
        };
        if current.abs() < 1e-9 {
            return false;
        }

        let reference = self.selection_reference_point(bounds);
        let scale = value / current;
        if (scale - 1.0).abs() < 1e-9 {
            return false;
        }

        let (sx, sy) = match axis {
            "width" => (scale, 1.0),
            "height" => (1.0, scale),
            _ => unreachable!("axis checked above"),
        };
        let transform = Affine::translate(-reference.to_vec2())
            .then_scale_non_uniform(sx, sy)
            .then_translate(reference.to_vec2());
        self.last_transform = Some(transform);
        if self.selected_component.is_some() {
            return self.transform_selected_component(transform);
        }
        self.transform_selection(transform)
    }

    /// Move the point selection to the next/previous point in outline
    /// storage order, matching Glyphs' Tab / Shift-Tab behavior.
    pub fn cycle_selected_point(&mut self, backwards: bool) -> bool {
        let point_ids = self
            .paths
            .iter()
            .flat_map(|path| path.points().iter().map(|point| point.id))
            .collect::<Vec<_>>();
        if point_ids.is_empty() {
            return false;
        }

        let selected_positions = point_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| self.selection.contains(id).then_some(index))
            .collect::<Vec<_>>();

        let target_index = if selected_positions.is_empty() {
            if backwards { point_ids.len() - 1 } else { 0 }
        } else if backwards {
            let first = selected_positions[0];
            if first == 0 {
                point_ids.len() - 1
            } else {
                first - 1
            }
        } else {
            let last = selected_positions[selected_positions.len() - 1];
            (last + 1) % point_ids.len()
        };

        let target = point_ids[target_index];
        self.clear_component_selection();
        self.selection = Selection::new();
        self.selection.insert(target);
        true
    }

    pub fn push_path_and_select(&mut self, path: Path) {
        let mut selection = Selection::new();
        for pt in path.points().iter() {
            selection.insert(pt.id);
        }
        self.paths.push(path);
        self.selection = selection;
        self.bump_edit_revision();
    }

    pub fn replace_paths_from_tool(&mut self, paths: Vec<Path>) -> bool {
        if paths.is_empty() {
            return false;
        }
        self.paths = paths;
        self.selection = Selection::new();
        self.last_transform = None;
        self.bump_edit_revision();
        true
    }

    pub fn start_pen_path(&mut self, point: Point) -> usize {
        let pt = on_curve(snap_point_to_grid(point), false);
        let id = pt.id;
        let path = Path::Cubic(CubicPath::new(PathPoints::from_vec(vec![pt]), false));
        self.paths.push(path);
        self.selection = Selection::new();
        self.selection.insert(id);
        self.bump_edit_revision();
        self.paths.len() - 1
    }

    pub fn append_pen_point(&mut self, path_index: usize, point: Point) -> Option<EntityId> {
        let Some(Path::Cubic(path)) = self.paths.get_mut(path_index) else {
            return None;
        };
        if path.closed {
            return None;
        }
        let pt = on_curve(snap_point_to_grid(point), false);
        let id = pt.id;
        path.points.make_mut().push(pt);
        self.selection = Selection::new();
        self.selection.insert(id);
        self.bump_edit_revision();
        Some(id)
    }

    pub fn append_smooth_pen_point(
        &mut self,
        path_index: Option<usize>,
        origin: Point,
        handle_out: Point,
    ) -> usize {
        let origin = snap_point_to_grid(origin);
        let handle_out = snap_point_to_grid(handle_out);
        let delta = handle_out - origin;
        let smooth_points = vec![
            PathPoint {
                id: EntityId::next(),
                point: origin - delta,
                typ: PointType::OffCurve { auto: false },
            },
            PathPoint {
                id: EntityId::next(),
                point: origin,
                typ: PointType::OnCurve { smooth: true },
            },
            PathPoint {
                id: EntityId::next(),
                point: handle_out,
                typ: PointType::OffCurve { auto: false },
            },
        ];

        let index = if let Some(index) = path_index
            && let Some(Path::Cubic(path)) = self.paths.get_mut(index)
            && !path.closed
        {
            path.points.make_mut().extend(smooth_points.clone());
            index
        } else {
            // For a new open contour, start with the on-curve point so
            // serialization can emit a UFO Move point. The incoming
            // mirrored handle only makes sense once a previous segment
            // exists.
            let points = vec![
                PathPoint {
                    id: EntityId::next(),
                    point: origin,
                    typ: PointType::OnCurve { smooth: true },
                },
                PathPoint {
                    id: EntityId::next(),
                    point: handle_out,
                    typ: PointType::OffCurve { auto: false },
                },
            ];
            let path = Path::Cubic(CubicPath::new(PathPoints::from_vec(points), false));
            self.paths.push(path);
            self.paths.len() - 1
        };

        self.selection = Selection::new();
        if let Some(Path::Cubic(path)) = self.paths.get(index) {
            let path_points = path.points.to_vec();
            for pt in path_points.iter().rev().take(3) {
                self.selection.insert(pt.id);
            }
        }
        self.bump_edit_revision();
        index
    }

    pub fn update_last_smooth_pen_handles(
        &mut self,
        path_index: usize,
        origin: Point,
        handle_out: Point,
    ) -> bool {
        let Some(Path::Cubic(path)) = self.paths.get_mut(path_index) else {
            return false;
        };
        let points = path.points.make_mut();
        if points.len() < 2 {
            return false;
        }
        let len = points.len();
        let origin = snap_point_to_grid(origin);
        let handle_out = snap_point_to_grid(handle_out);
        let delta = handle_out - origin;
        if len >= 3 && points[len - 3].is_off_curve() {
            points[len - 3].point = origin - delta;
            points[len - 2].point = origin;
            points[len - 1].point = handle_out;
        } else {
            points[len - 2].point = origin;
            points[len - 1].point = handle_out;
        }
        self.bump_edit_revision();
        true
    }

    pub fn close_pen_path(&mut self, path_index: usize) -> bool {
        let Some(Path::Cubic(path)) = self.paths.get_mut(path_index) else {
            return false;
        };
        if path.closed || path.points.len() < 3 {
            return false;
        }
        path.closed = true;
        self.selection = Selection::new();
        for pt in path.points.iter() {
            self.selection.insert(pt.id);
        }
        self.bump_edit_revision();
        true
    }

    pub fn pen_path_start(&self, path_index: usize) -> Option<Point> {
        let Path::Cubic(path) = self.paths.get(path_index)? else {
            return None;
        };
        path.points.iter().next().map(|pt| pt.point)
    }

    pub fn pen_path_last_on_curve(&self, path_index: usize) -> Option<Point> {
        let Path::Cubic(path) = self.paths.get(path_index)? else {
            return None;
        };
        path.points
            .to_vec()
            .into_iter()
            .rev()
            .find(|pt| pt.is_on_curve())
            .map(|pt| pt.point)
    }

    pub fn pen_path_len(&self, path_index: usize) -> Option<usize> {
        let Path::Cubic(path) = self.paths.get(path_index)? else {
            return None;
        };
        Some(path.points.len())
    }

    pub fn remove_tool_path(&mut self, path_index: usize) -> bool {
        if path_index >= self.paths.len() {
            return false;
        }
        self.paths.remove(path_index);
        self.selection = Selection::new();
        self.bump_edit_revision();
        true
    }

    pub fn start_hyper_path(&mut self, point: Point) -> usize {
        let path = Path::Hyper(crate::path::HyperPath::new(snap_point_to_grid(point)));
        self.paths.push(path);
        self.selection = Selection::new();
        if let Some(Path::Hyper(path)) = self.paths.last()
            && let Some(start) = path.start_point()
        {
            self.selection.insert(start.id);
        }
        self.bump_edit_revision();
        self.paths.len() - 1
    }

    pub fn append_hyper_point(&mut self, path_index: usize, point: Point) -> Option<EntityId> {
        let Some(Path::Hyper(path)) = self.paths.get_mut(path_index) else {
            return None;
        };
        if path.closed {
            return None;
        }
        path.add_on_curve_point(snap_point_to_grid(point));
        let id = path.points.to_vec().last().map(|pt| pt.id)?;
        self.selection = Selection::new();
        self.selection.insert(id);
        self.bump_edit_revision();
        Some(id)
    }

    pub fn close_hyper_path(&mut self, path_index: usize) -> bool {
        let Some(Path::Hyper(path)) = self.paths.get_mut(path_index) else {
            return false;
        };
        if path.closed || path.len() < 3 {
            return false;
        }
        path.close_path();
        self.selection = Selection::new();
        for pt in path.points.iter() {
            self.selection.insert(pt.id);
        }
        self.bump_edit_revision();
        true
    }

    pub fn hyper_path_start(&self, path_index: usize) -> Option<Point> {
        let Path::Hyper(path) = self.paths.get(path_index)? else {
            return None;
        };
        path.start_point().map(|pt| pt.point)
    }

    pub fn hyper_path_last_point(&self, path_index: usize) -> Option<Point> {
        let Path::Hyper(path) = self.paths.get(path_index)? else {
            return None;
        };
        path.points.to_vec().last().map(|pt| pt.point)
    }

    pub fn hyper_path_len(&self, path_index: usize) -> Option<usize> {
        let Path::Hyper(path) = self.paths.get(path_index)? else {
            return None;
        };
        Some(path.len())
    }

    /// Mirror selected points horizontally around the active
    /// coordinate-panel reference point. Returns true when at least one
    /// point actually moved.
    pub fn flip_selection_horizontal(&mut self) -> bool {
        let Some((_count, bounds)) = self.selection_bounds() else {
            return false;
        };
        let reference = self.selection_reference_point(bounds);
        let transform = Affine::translate(-reference.to_vec2())
            .then_scale_non_uniform(-1.0, 1.0)
            .then_translate(reference.to_vec2());
        self.last_transform = Some(transform);
        self.transform_selection(transform)
    }

    /// Mirror selected points vertically around the active
    /// coordinate-panel reference point. Returns true when at least one
    /// point actually moved.
    pub fn flip_selection_vertical(&mut self) -> bool {
        let Some((_count, bounds)) = self.selection_bounds() else {
            return false;
        };
        let reference = self.selection_reference_point(bounds);
        let transform = Affine::translate(-reference.to_vec2())
            .then_scale_non_uniform(1.0, -1.0)
            .then_translate(reference.to_vec2());
        self.last_transform = Some(transform);
        self.transform_selection(transform)
    }

    /// Rotate selected points around the selected bounds center.
    /// Degrees follow the same sign convention as kurbo/xilem.
    pub fn rotate_selection(&mut self, degrees: f64) -> bool {
        let Some((_count, bounds)) = self.selection_bounds() else {
            return false;
        };
        let transform = Affine::rotate_about(degrees.to_radians(), bounds.center());
        self.last_transform = Some(transform);
        self.transform_selection(transform)
    }

    /// Apply an affine transform to selected points.
    pub fn transform_selection(&mut self, transform: Affine) -> bool {
        if self.selected_component.is_some() {
            return self.transform_selected_component(transform);
        }
        let mut changed = false;
        for path in &mut self.paths {
            changed |= map_selected_points(path, &self.selection, |pt| transform * pt);
        }
        if changed {
            self.bump_edit_revision();
        }
        changed
    }

    /// Clone every contour containing a selected point, offset the
    /// duplicates by (+20, +20), and select the duplicate points.
    /// This mirrors runebender-xilem's duplicate-selection behavior.
    pub fn duplicate_selection(&mut self) -> bool {
        if self.selected_component.is_some() {
            return self.duplicate_selected_component();
        }
        if self.selection.is_empty() {
            return false;
        }

        let offset = Vec2::new(20.0, 20.0);
        let mut new_paths = Vec::new();
        let mut new_selection = Selection::new();

        for path in &self.paths {
            if !path
                .points()
                .iter()
                .any(|pt| self.selection.contains(&pt.id))
            {
                continue;
            }
            new_paths.push(duplicate_path(path, offset, &mut new_selection));
        }

        if new_paths.is_empty() {
            return false;
        }

        self.paths.extend(new_paths);
        self.selection = new_selection;
        self.bump_edit_revision();
        true
    }

    /// Duplicate selected contours, then apply the last geometric
    /// transform if one exists.
    pub fn duplicate_repeat_selection(&mut self) -> bool {
        if !self.duplicate_selection() {
            return false;
        }
        if let Some(transform) = self.last_transform {
            self.transform_selection(transform);
        }
        true
    }

    pub fn reverse_contours(&mut self) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        for path in &mut self.paths {
            reverse_path_points(path);
        }
        self.bump_edit_revision();
        true
    }

    pub fn convert_hyper_to_cubic(&mut self) -> bool {
        let has_selection = !self.selection.is_empty();
        let mut changed = false;
        for path in &mut self.paths {
            let should_convert = match path {
                Path::Hyper(hyper) if has_selection => hyper
                    .points()
                    .iter()
                    .any(|point| self.selection.contains(&point.id)),
                Path::Hyper(_) => true,
                _ => false,
            };
            if should_convert {
                let Path::Hyper(hyper) = path else {
                    continue;
                };
                *path = Path::Cubic(hyper.to_cubic());
                changed = true;
            }
        }
        if changed {
            self.selection = Selection::new();
            self.bump_edit_revision();
        }
        changed
    }

    pub fn set_advance_width(&mut self, width: f64) -> bool {
        if !width.is_finite() || width < 0.0 {
            return false;
        }
        if (self.advance_width - width).abs() < 1e-9 {
            return false;
        }
        self.advance_width = width;
        self.bump_edit_revision();
        true
    }

    pub fn left_sidebearing(&self) -> f64 {
        self.glyph_bbox().map(|bbox| bbox.min_x()).unwrap_or(0.0)
    }

    pub fn right_sidebearing(&self) -> f64 {
        self.glyph_bbox()
            .map(|bbox| self.advance_width - bbox.max_x())
            .unwrap_or(self.advance_width)
    }

    pub fn set_left_sidebearing(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(bbox) = self.glyph_bbox() else {
            return false;
        };
        let delta = value - bbox.min_x();
        if delta.abs() < 1e-9 {
            return false;
        }
        translate_all_paths(&mut self.paths, Vec2::new(delta, 0.0));
        self.bump_edit_revision();
        true
    }

    pub fn set_right_sidebearing(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(bbox) = self.glyph_bbox() else {
            return false;
        };
        self.set_advance_width(bbox.max_x() + value)
    }

    pub fn copy_selection(&self) -> Option<Vec<Path>> {
        if self.selection.is_empty() {
            return None;
        }

        let mut copied = Vec::new();
        for path in &self.paths {
            let points = path.points().to_vec();
            let has_selected_on_curve = points
                .iter()
                .any(|pt| pt.is_on_curve() && self.selection.contains(&pt.id));
            if !has_selected_on_curve {
                continue;
            }

            let all_on_curve_selected = points
                .iter()
                .filter(|pt| pt.is_on_curve())
                .all(|pt| self.selection.contains(&pt.id));
            if all_on_curve_selected {
                copied.push(path.clone());
                continue;
            }

            if let Some(extracted) = extract_selected_point_span(path, &self.selection) {
                copied.push(extracted);
            }
        }

        (!copied.is_empty()).then_some(copied)
    }

    pub fn paste_paths(&mut self, clipboard: &[Path]) -> bool {
        if clipboard.is_empty() {
            return false;
        }

        let mut new_selection = Selection::new();
        let new_paths = clipboard
            .iter()
            .map(|path| clone_path_with_fresh_ids(path, true, &mut new_selection))
            .collect::<Vec<_>>();
        if new_paths.is_empty() {
            return false;
        }

        self.paths.extend(new_paths);
        self.selection = new_selection;
        self.bump_edit_revision();
        true
    }

    /// Delete selected points from their contours. When one cubic
    /// off-curve handle is deleted, remove its paired handle as well
    /// so the segment collapses to a clean line.
    pub fn delete_selection(&mut self) -> bool {
        if self.selected_component.is_some() {
            return self.delete_selected_component();
        }
        if self.selection.is_empty() {
            return false;
        }

        let mut changed = false;
        let selection = self.selection.clone();
        self.paths.retain_mut(|path| {
            let before = path.points().len();
            let keep = retain_path_after_deletion(path, &selection);
            changed |= !keep || path.points().len() != before;
            keep
        });

        if changed {
            self.selection = Selection::new();
            self.bump_edit_revision();
        }
        changed
    }

    /// Toggle selected on-curve points between corner and smooth.
    pub fn toggle_point_type(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }

        let mut changed = false;
        for path in &mut self.paths {
            changed |= toggle_point_type_in_path(path, &self.selection);
        }

        if changed {
            self.bump_edit_revision();
        }
        changed
    }

    /// Toggle the nearest on-curve point at `point`, selecting only
    /// that point first. Mirrors xilem's double-click point edit.
    pub fn toggle_point_type_at_point(&mut self, point: Point, radius: f64) -> bool {
        let Some(id) = self.hit_test_on_curve_point(point, radius) else {
            return false;
        };
        self.clear_component_selection();
        let mut selection = Selection::new();
        selection.insert(id);
        self.selection = selection;
        self.toggle_point_type()
    }

    /// Select the editable segment nearest to `point`.
    ///
    /// A line segment selects its two endpoints. A curve segment selects
    /// the start point, its interior off-curve handles, and the end point.
    /// Returns `Some(true)` when the caller should begin a translate drag,
    /// `Some(false)` when a Shift-click removed the segment from selection,
    /// and `None` when no segment was hit.
    pub fn select_segment_at_point(
        &mut self,
        point: Point,
        radius: f64,
        extend: bool,
    ) -> Option<bool> {
        let segment_info = self.nearest_segment(point, radius)?;
        let path = self.paths.get(segment_info.path_index)?;
        let ids = segment_point_ids(path, &segment_info);
        if ids.is_empty() {
            return None;
        }

        self.clear_component_selection();
        if extend {
            let all_selected = ids.iter().all(|id| self.selection.contains(id));
            if all_selected {
                for id in ids {
                    self.selection.remove(&id);
                }
                return Some(false);
            }
            for id in ids {
                self.selection.insert(id);
            }
            return Some(true);
        }

        let mut selection = Selection::new();
        for id in ids {
            selection.insert(id);
        }
        self.selection = selection;
        Some(true)
    }

    /// Select the full contour nearest to `point`.
    ///
    /// This is the double-click path selection used by font editors:
    /// double-clicking a segment selects the complete outline so it can
    /// be moved or deleted as one unit. Point hits are intentionally
    /// excluded so double-clicking on-curve/off-curve points keeps
    /// point-specific editing behavior available.
    pub fn select_contour_at_point(
        &mut self,
        point: Point,
        segment_radius: f64,
        point_guard_radius: f64,
    ) -> bool {
        if self
            .hit_test_point_with_path_index(point, point_guard_radius)
            .is_some()
        {
            return false;
        }

        let Some(segment_info) = self.nearest_segment(point, segment_radius) else {
            return false;
        };
        let path_index = segment_info.path_index;
        let Some(path) = self.paths.get(path_index) else {
            return false;
        };

        let mut selection = Selection::new();
        for point in path.points().iter() {
            selection.insert(point.id);
        }
        if selection.is_empty() {
            return false;
        }

        self.clear_component_selection();
        self.selection = selection;
        true
    }

    /// Convert the nearest editable line segment to a cubic curve by
    /// inserting two off-curve handles at 1/3 and 2/3 along the line.
    pub fn convert_line_segment_at_point(&mut self, point: Point, radius: f64) -> bool {
        let Some(segment_info) = self.nearest_segment(point, radius) else {
            return false;
        };
        self.convert_line_to_curve(&segment_info)
    }

    pub fn convert_line_to_curve(&mut self, segment_info: &SegmentInfo) -> bool {
        let Segment::Line(line) = segment_info.segment else {
            return false;
        };
        let Some(path) = self.paths.get_mut(segment_info.path_index) else {
            return false;
        };
        let Some(points) = cubic_path_points_mut(path) else {
            return false;
        };

        let ctrl1_pt = off_curve(snap_point_to_grid(line.p0 + (line.p1 - line.p0) / 3.0));
        let ctrl2_pt = off_curve(snap_point_to_grid(
            line.p0 + (line.p1 - line.p0) * (2.0 / 3.0),
        ));
        let ctrl1_id = ctrl1_pt.id;
        let ctrl2_id = ctrl2_pt.id;

        let insert_index = if segment_info.end_index > segment_info.start_index {
            segment_info.end_index
        } else {
            segment_info.start_index + 1
        };
        points.insert(insert_index, ctrl2_pt);
        points.insert(insert_index, ctrl1_pt);

        self.selection = Selection::new();
        self.selection.insert(ctrl1_id);
        self.selection.insert(ctrl2_id);
        self.bump_edit_revision();
        true
    }

    /// Replace selected sharp line-line corners with rounded cubic
    /// corners. The generated points are snapped to the design grid
    /// and the offset/handle proportions are inferred from existing
    /// rounded corners in the current glyph when possible.
    pub fn round_selected_corners(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }

        let profile = infer_round_corner_profile(&self.paths);
        let selection = self.selection.clone();
        let mut next_selection = Selection::new();
        let mut changed = false;

        for path in &mut self.paths {
            let Path::Cubic(cubic) = path else {
                continue;
            };
            let points = cubic.points.to_vec();
            let Some(next_points) = round_selected_corners_in_cubic_points(
                &points,
                cubic.closed,
                &selection,
                profile,
                &mut next_selection,
            ) else {
                continue;
            };
            cubic.points = PathPoints::from_vec(next_points);
            changed = true;
        }

        if !changed {
            return false;
        }

        self.clear_component_selection();
        self.selection = next_selection;
        self.bump_edit_revision();
        true
    }

    pub fn insert_point_on_segment(&mut self, segment_info: &SegmentInfo, t: f64) -> bool {
        let Some(path) = self.paths.get_mut(segment_info.path_index) else {
            return false;
        };
        let t = t.clamp(0.0, 1.0);
        let inserted_id = match segment_info.segment {
            Segment::Line(_) => insert_point_on_line(path, segment_info, t),
            Segment::Cubic(cubic) => insert_point_on_cubic(path, segment_info, cubic, t),
            Segment::Quadratic(quad) => insert_point_on_quadratic(path, segment_info, quad, t),
        };
        let Some(inserted_id) = inserted_id else {
            return false;
        };
        self.selection = Selection::new();
        self.selection.insert(inserted_id);
        self.bump_edit_revision();
        true
    }

    pub fn insert_point_on_nearest_segment(&mut self, point: Point, radius: f64) -> bool {
        let Some((segment_info, t)) = self.nearest_segment_with_t(point, radius) else {
            return false;
        };
        self.insert_point_on_segment(&segment_info, t)
    }

    /// Apply a boolean operation to every contour in the glyph.
    ///
    /// For union, all contours are merged together. For
    /// subtract/intersect/exclude, the first contour is the left-hand
    /// operand and all remaining contours form the right-hand operand,
    /// matching xilem's contour-order behavior.
    pub fn boolean_selection(&mut self, op: linesweeper::BinaryOp) -> bool {
        if self.paths.len() < 2 {
            return false;
        }

        let mut glyph_paths = self
            .paths
            .iter()
            .map(path_to_bezpath_012)
            .collect::<Vec<_>>();

        let (set_a, set_b) = match op {
            linesweeper::BinaryOp::Union => {
                let mut combined = BezPath012::new();
                for path in &glyph_paths {
                    append_bezpath_012_from_012(&mut combined, path);
                }
                (combined, BezPath012::new())
            }
            _ => {
                let set_a = glyph_paths
                    .drain(..1)
                    .next()
                    .expect("paths len checked above");
                let mut rest = BezPath012::new();
                for path in &glyph_paths {
                    append_bezpath_012_from_012(&mut rest, path);
                }
                (set_a, rest)
            }
        };

        let result =
            match linesweeper::binary_op(&set_a, &set_b, linesweeper::FillRule::NonZero, op) {
                Ok(contours) => contours,
                Err(_) => return false,
            };

        let original_types = self
            .paths
            .iter()
            .flat_map(|path| path.points().iter().cloned())
            .filter(|point| point.is_on_curve())
            .map(|point| (point.point, point.typ))
            .collect::<Vec<_>>();

        let result_paths: Vec<Path> = result
            .contours()
            .map(|contour| {
                let mut cubic = bezpath_to_cubic(&contour.path);
                restore_original_point_types(&mut cubic, &original_types);
                Path::Cubic(cubic)
            })
            .collect();

        if result_paths.is_empty() {
            return false;
        }

        self.paths = result_paths;
        self.selection = Selection::new();
        self.last_transform = None;
        self.bump_edit_revision();
        true
    }

    /// Build a fresh selection containing every point that lies
    /// inside `screen_rect` (a rectangle in screen-space pixels), then
    /// union with `base`. Used during box-select drags.
    pub fn select_in_screen_rect(&mut self, screen_rect: Rect, base: &Selection) {
        let mut next = base.clone();
        for path in &self.paths {
            for pt in path.points().iter() {
                let screen_pt = self.glyph_to_screen(pt.point);
                if rect_contains(&screen_rect, screen_pt) {
                    next.insert(pt.id);
                }
            }
        }
        self.selection = next;
    }

    /// Hit-test for a point near `design_pt` within `radius` design-
    /// space units. Returns the hit point's id if any.
    pub fn hit_test_point(&self, design_pt: Point, radius: f64) -> Option<EntityId> {
        self.hit_test_point_with_path_index(design_pt, radius)
            .map(|(_, id)| id)
    }

    pub fn hit_test_on_curve_point(&self, design_pt: Point, radius: f64) -> Option<EntityId> {
        use crate::editing::hit_test::find_closest;
        let candidates: Vec<_> = self
            .paths
            .iter()
            .flat_map(|p| {
                p.points()
                    .iter()
                    .filter(|pt| pt.is_on_curve())
                    .map(|pt| (pt.id, pt.point, true))
                    .collect::<Vec<_>>()
            })
            .collect();
        find_closest(design_pt, candidates.into_iter(), radius).map(|h| h.entity)
    }

    pub fn nearest_segment(&self, design_pt: Point, radius: f64) -> Option<SegmentInfo> {
        self.nearest_segment_with_t(design_pt, radius)
            .map(|(segment, _)| segment)
    }

    pub fn contour_context_at(&self, design_pt: Point, radius: f64) -> Option<(usize, bool)> {
        self.contour_context_target(design_pt, radius)
            .map(|target| (target.path_index, target.can_set_start))
    }

    pub fn contour_context_target(
        &self,
        design_pt: Point,
        radius: f64,
    ) -> Option<ContourContextTarget> {
        if let Some((path_index, point_index)) = self.on_curve_point_at(design_pt, radius) {
            let point = self.paths[path_index]
                .points()
                .iter()
                .nth(point_index)
                .expect("point index came from this path")
                .point;
            return Some(ContourContextTarget {
                path_index,
                point_index,
                point,
                can_set_start: point_index != 0 && path_is_closed(&self.paths[path_index]),
            });
        }

        self.first_selected_on_curve_point()
    }

    fn on_curve_point_at(&self, design_pt: Point, radius: f64) -> Option<(usize, usize)> {
        let max_dist_sq = radius * radius;
        let mut best: Option<(usize, usize, f64)> = None;
        for (path_index, path) in self.paths.iter().enumerate() {
            for (point_index, point) in path.points().iter().enumerate() {
                if !point.is_on_curve() {
                    continue;
                }
                let dist_sq = point.point.distance_squared(design_pt);
                if dist_sq > max_dist_sq {
                    continue;
                }
                if best.is_none_or(|(_, _, best_dist)| dist_sq < best_dist) {
                    best = Some((path_index, point_index, dist_sq));
                }
            }
        }
        best.map(|(path_index, point_index, _)| (path_index, point_index))
    }

    fn first_selected_on_curve_point(&self) -> Option<ContourContextTarget> {
        for (path_index, path) in self.paths.iter().enumerate() {
            for (point_index, point) in path.points().iter().enumerate() {
                if !point.is_on_curve() || !self.selection.contains(&point.id) {
                    continue;
                }
                return Some(ContourContextTarget {
                    path_index,
                    point_index,
                    point: point.point,
                    can_set_start: point_index != 0 && path_is_closed(path),
                });
            }
        }
        None
    }

    pub fn set_start_point_at(&mut self, design_pt: Point, radius: f64) -> bool {
        let Some((path_index, _)) = self.contour_context_at(design_pt, radius) else {
            return false;
        };
        let Some(point_index) = self.on_curve_point_index_at(path_index, design_pt, radius) else {
            return false;
        };
        let Some(path) = self.paths.get_mut(path_index) else {
            return false;
        };
        if !path_is_closed(path) || point_index == 0 {
            return false;
        }
        path_points_mut(path).rotate_left(point_index);
        if let Path::Hyper(hyper) = path {
            hyper.after_change();
        }
        self.bump_edit_revision();
        true
    }

    pub fn reverse_contour_at(&mut self, design_pt: Point, radius: f64) -> bool {
        let Some((path_index, _)) = self.contour_context_at(design_pt, radius) else {
            return false;
        };
        let Some(path) = self.paths.get_mut(path_index) else {
            return false;
        };
        reverse_path_points(path);
        self.bump_edit_revision();
        true
    }

    pub fn move_contour(&mut self, path_index: usize, delta: isize) -> bool {
        if self.paths.len() < 2 {
            return false;
        }
        let next = if delta < 0 {
            path_index.checked_sub(1)
        } else {
            path_index.checked_add(1)
        };
        let Some(next) = next else {
            return false;
        };
        if path_index >= self.paths.len() || next >= self.paths.len() {
            return false;
        }
        self.paths.swap(path_index, next);
        self.selection = Selection::new();
        self.bump_edit_revision();
        true
    }

    pub fn nearest_segment_with_t(
        &self,
        design_pt: Point,
        radius: f64,
    ) -> Option<(SegmentInfo, f64)> {
        let max_dist_sq = radius * radius;
        let mut best: Option<(SegmentInfo, f64, f64)> = None;

        for (path_index, path) in self.paths.iter().enumerate() {
            let segments = match path {
                Path::Cubic(cubic) => cubic.iter_segments().collect::<Vec<_>>(),
                Path::Quadratic(quadratic) => quadratic.iter_segments().collect::<Vec<_>>(),
                Path::Hyper(hyper) => hyper.iter_segments().collect::<Vec<_>>(),
            };
            for mut segment in segments {
                let (t, dist_sq) = segment.segment.nearest(design_pt);
                if dist_sq > max_dist_sq {
                    continue;
                }
                if best.is_none_or(|(_, _, best_dist)| dist_sq < best_dist) {
                    segment.path_index = path_index;
                    best = Some((segment, t, dist_sq));
                }
            }
        }

        best.map(|(segment, t, _)| (segment, t))
    }

    fn on_curve_point_index_at(
        &self,
        path_index: usize,
        design_pt: Point,
        radius: f64,
    ) -> Option<usize> {
        let path = self.paths.get(path_index)?;
        let max_dist_sq = radius * radius;
        let mut best: Option<(usize, f64)> = None;
        for (point_index, point) in path.points().iter().enumerate() {
            if !point.is_on_curve() {
                continue;
            }
            let dist_sq = point.point.distance_squared(design_pt);
            if dist_sq > max_dist_sq {
                continue;
            }
            if best.is_none_or(|(_, best_dist)| dist_sq < best_dist) {
                best = Some((point_index, dist_sq));
            }
        }
        best.map(|(point_index, _)| point_index)
    }

    /// Bounding box of the selected points in design space, plus the
    /// number of selected points found in the current path list.
    pub fn selection_bounds(&self) -> Option<(usize, Rect)> {
        if let Some(bounds) = self.selected_component_bounds() {
            return Some((1, bounds));
        }
        if self.selection.is_empty() {
            return None;
        }

        let mut count = 0;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for path in &self.paths {
            for pt in path.points().iter() {
                if !self.selection.contains(&pt.id) {
                    continue;
                }
                count += 1;
                min_x = min_x.min(pt.point.x);
                min_y = min_y.min(pt.point.y);
                max_x = max_x.max(pt.point.x);
                max_y = max_y.max(pt.point.y);
            }
        }

        if count == 0 {
            None
        } else {
            Some((count, Rect::new(min_x, min_y, max_x, max_y)))
        }
    }

    pub fn selection_reference_point(&self, bounds: Rect) -> Point {
        self.coord_quadrant.point_in_dspace_rect(bounds)
    }

    pub fn set_coord_quadrant(&mut self, quadrant: Quadrant) {
        self.coord_quadrant = quadrant;
    }

    pub fn edit_revision(&self) -> u64 {
        self.edit_revision
    }

    pub fn selected_contour_count(&self) -> usize {
        self.selected_contour_indices().len()
    }

    pub(crate) fn bump_edit_revision(&mut self) {
        self.edit_revision = self.edit_revision.wrapping_add(1);
    }

    fn selected_contour_indices(&self) -> Vec<usize> {
        self.paths
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                path.points()
                    .iter()
                    .any(|pt| self.selection.contains(&pt.id))
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn hit_test_point_with_path_index(
        &self,
        design_pt: Point,
        radius: f64,
    ) -> Option<(usize, EntityId)> {
        let max_dist_sq = radius * radius;
        let mut best: Option<(usize, EntityId, f64)> = None;
        for (path_index, path) in self.paths.iter().enumerate() {
            for point in path.points().iter() {
                let dist_sq = point.point.distance_squared(design_pt);
                if dist_sq > max_dist_sq {
                    continue;
                }
                if best.is_none_or(|(_, _, best_dist)| dist_sq < best_dist) {
                    best = Some((path_index, point.id, dist_sq));
                }
            }
        }
        best.map(|(path_index, id, _)| (path_index, id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContourContextTarget {
    pub path_index: usize,
    pub point_index: usize,
    pub point: Point,
    pub can_set_start: bool,
}

fn on_curve(p: Point, smooth: bool) -> PathPoint {
    PathPoint {
        id: EntityId::next(),
        point: p,
        typ: PointType::OnCurve { smooth },
    }
}

fn off_curve(p: Point) -> PathPoint {
    PathPoint {
        id: EntityId::next(),
        point: p,
        typ: PointType::OffCurve { auto: false },
    }
}

fn translate_in_path(path: &mut Path, selection: &Selection, delta: Vec2) {
    match path {
        Path::Cubic(cubic) => {
            let closed = cubic.closed;
            let points = cubic.points.make_mut();
            let mut changed = false;
            for pt in points.iter_mut() {
                if selection.contains(&pt.id) {
                    pt.point += delta;
                    changed = true;
                }
            }
            if changed {
                maintain_smooth_handle_tangents(points, selection, closed);
            }
        }
        Path::Quadratic(_) | Path::Hyper(_) => {
            map_selected_points(path, selection, |pt| pt + delta);
        }
    }
}

fn maintain_smooth_handle_tangents(
    points: &mut [PathPoint],
    selection: &Selection,
    closed: bool,
) -> bool {
    let len = points.len();
    if len < 3 {
        return false;
    }

    let mut updates = Vec::new();
    for index in 0..len {
        if !selection.contains(&points[index].id) || !points[index].is_off_curve() {
            continue;
        }

        if let Some(on_index) = previous_index(index, len, closed)
            && matches!(points[on_index].typ, PointType::OnCurve { smooth: true })
            && let Some(opposite_index) = previous_index(on_index, len, closed)
            && !selection.contains(&points[opposite_index].id)
        {
            if points[opposite_index].is_off_curve() {
                if let Some(point) = mirrored_smooth_handle(
                    points[index].point,
                    points[on_index].point,
                    points[opposite_index].point,
                ) {
                    updates.push((opposite_index, point));
                }
            } else if points[opposite_index].is_on_curve()
                && let Some(point) = projected_smooth_handle(
                    points[index].point,
                    points[on_index].point,
                    points[opposite_index].point,
                )
            {
                updates.push((index, point));
            }
        }

        if let Some(on_index) = next_index(index, len, closed)
            && matches!(points[on_index].typ, PointType::OnCurve { smooth: true })
            && let Some(opposite_index) = next_index(on_index, len, closed)
            && !selection.contains(&points[opposite_index].id)
        {
            if points[opposite_index].is_off_curve() {
                if let Some(point) = mirrored_smooth_handle(
                    points[index].point,
                    points[on_index].point,
                    points[opposite_index].point,
                ) {
                    updates.push((opposite_index, point));
                }
            } else if points[opposite_index].is_on_curve()
                && let Some(point) = projected_smooth_handle(
                    points[index].point,
                    points[on_index].point,
                    points[opposite_index].point,
                )
            {
                updates.push((index, point));
            }
        }
    }

    let mut changed = false;
    for (index, point) in updates {
        if points[index].point != point {
            points[index].point = point;
            changed = true;
        }
    }
    changed
}

fn mirrored_smooth_handle(moved: Point, anchor: Point, opposite: Point) -> Option<Point> {
    let moved_vector = moved - anchor;
    let moved_len = moved_vector.hypot();
    if moved_len < 1e-9 {
        return None;
    }
    let opposite_len = (opposite - anchor).hypot();
    if opposite_len < 1e-9 {
        return None;
    }
    Some(snap_point_to_grid(
        anchor - (moved_vector / moved_len) * opposite_len,
    ))
}

fn projected_smooth_handle(moved: Point, anchor: Point, line_point: Point) -> Option<Point> {
    let tangent = anchor - line_point;
    let tangent_len = tangent.hypot();
    if tangent_len < 1e-9 {
        return None;
    }
    let unit = tangent / tangent_len;
    let distance = ((moved - anchor).x * unit.x + (moved - anchor).y * unit.y).abs();
    let projected = anchor + unit * distance;
    if tangent.x.abs() < 1e-9 || tangent.y.abs() < 1e-9 {
        Some(snap_point_to_grid(projected))
    } else {
        Some(projected)
    }
}

fn translate_in_path_with_handles(path: &mut Path, selection: &Selection, delta: Vec2) {
    let ids = selected_and_adjacent_offcurve_ids(path, selection);
    match path {
        Path::Cubic(cubic) => {
            let closed = cubic.closed;
            let points = cubic.points.make_mut();
            let mut changed = false;
            for point in points.iter_mut() {
                if ids.contains(&point.id) {
                    point.point += delta;
                    changed = true;
                }
            }
            if changed {
                maintain_smooth_handle_tangents(points, selection, closed);
            }
        }
        Path::Quadratic(_) => {
            for point in path_points_mut(path) {
                if ids.contains(&point.id) {
                    point.point += delta;
                }
            }
        }
        Path::Hyper(hyper) => {
            for point in hyper.points.make_mut() {
                if ids.contains(&point.id) {
                    point.point += delta;
                }
            }
            hyper.after_change();
        }
    }
}

fn selected_and_adjacent_offcurve_ids(path: &Path, selection: &Selection) -> HashSet<EntityId> {
    let points = path.points().iter().cloned().collect::<Vec<_>>();
    let mut ids = selection.iter().copied().collect::<HashSet<_>>();
    if points.is_empty() {
        return ids;
    }
    let closed = path_is_closed(path);
    for (index, point) in points.iter().enumerate() {
        if !selection.contains(&point.id) || !point.is_on_curve() {
            continue;
        }
        for neighbor in [
            previous_index(index, points.len(), closed),
            next_index(index, points.len(), closed),
        ]
        .into_iter()
        .flatten()
        {
            if points[neighbor].is_off_curve() {
                ids.insert(points[neighbor].id);
            }
        }
    }
    ids
}

fn translate_all_paths(paths: &mut [Path], delta: Vec2) {
    for path in paths {
        match path {
            Path::Cubic(cubic) => {
                for pt in cubic.points.make_mut().iter_mut() {
                    pt.point += delta;
                }
            }
            Path::Quadratic(quadratic) => {
                for pt in quadratic.points.make_mut().iter_mut() {
                    pt.point += delta;
                }
            }
            Path::Hyper(hyper) => {
                for pt in hyper.points.make_mut().iter_mut() {
                    pt.point += delta;
                }
                hyper.after_change();
            }
        }
    }
}

fn map_selected_points(
    path: &mut Path,
    selection: &Selection,
    mut transform: impl FnMut(Point) -> Point,
) -> bool {
    let mut changed = false;
    match path {
        Path::Cubic(cubic) => {
            for pt in cubic.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    let next = transform(pt.point);
                    if next != pt.point {
                        pt.point = next;
                        changed = true;
                    }
                }
            }
        }
        Path::Quadratic(quadratic) => {
            for pt in quadratic.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    let next = transform(pt.point);
                    if next != pt.point {
                        pt.point = next;
                        changed = true;
                    }
                }
            }
        }
        Path::Hyper(hyper) => {
            for pt in hyper.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    let next = transform(pt.point);
                    if next != pt.point {
                        pt.point = next;
                        changed = true;
                    }
                }
            }
            if changed {
                hyper.after_change();
            }
        }
    }
    changed
}

fn retain_path_after_deletion(path: &mut Path, selection: &Selection) -> bool {
    match path {
        Path::Cubic(cubic) => {
            let points = cubic.points.make_mut();
            let extra_ids = cubic_partner_offcurve_ids(points, selection);
            points.retain(|point| !selection.contains(&point.id) && !extra_ids.contains(&point.id));
            points.len() >= 2
        }
        Path::Quadratic(quadratic) => {
            let points = quadratic.points.make_mut();
            points.retain(|point| !selection.contains(&point.id));
            points.len() >= 2
        }
        Path::Hyper(hyper) => {
            let points = hyper.points.make_mut();
            points.retain(|point| !selection.contains(&point.id));
            let keep = points.len() >= 2;
            hyper.after_change();
            keep
        }
    }
}

fn cubic_partner_offcurve_ids(points: &[PathPoint], selection: &Selection) -> HashSet<EntityId> {
    let mut partners = HashSet::new();
    if points.is_empty() {
        return partners;
    }

    for (index, point) in points.iter().enumerate() {
        if !selection.contains(&point.id) || !point.is_off_curve() {
            continue;
        }

        let previous = if index == 0 {
            points.len() - 1
        } else {
            index - 1
        };
        let next = (index + 1) % points.len();

        if points[next].is_off_curve() {
            partners.insert(points[next].id);
        } else if points[previous].is_off_curve() {
            partners.insert(points[previous].id);
        }
    }

    for point in points {
        if selection.contains(&point.id) {
            partners.remove(&point.id);
        }
    }
    partners
}

fn toggle_point_type_in_path(path: &mut Path, selection: &Selection) -> bool {
    let mut changed = false;
    match path {
        Path::Cubic(cubic) => {
            changed |= toggle_point_type_in_points(cubic.points.make_mut(), selection);
        }
        Path::Quadratic(quadratic) => {
            changed |= toggle_point_type_in_points(quadratic.points.make_mut(), selection);
        }
        Path::Hyper(hyper) => {
            changed |= toggle_point_type_in_points(hyper.points.make_mut(), selection);
            if changed {
                hyper.after_change();
            }
        }
    }
    changed
}

fn toggle_point_type_in_points(points: &mut [PathPoint], selection: &Selection) -> bool {
    let mut changed = false;
    for point in points {
        if selection.contains(&point.id)
            && let PointType::OnCurve { smooth } = &mut point.typ
        {
            *smooth = !*smooth;
            changed = true;
        }
    }
    changed
}

fn cubic_path_points_mut(path: &mut Path) -> Option<&mut Vec<PathPoint>> {
    match path {
        Path::Cubic(cubic) => Some(cubic.points.make_mut()),
        Path::Quadratic(_) | Path::Hyper(_) => None,
    }
}

fn path_points_mut(path: &mut Path) -> &mut Vec<PathPoint> {
    match path {
        Path::Cubic(cubic) => cubic.points.make_mut(),
        Path::Quadratic(quadratic) => quadratic.points.make_mut(),
        Path::Hyper(hyper) => hyper.points.make_mut(),
    }
}

fn snap_point_to_grid(point: Point) -> Point {
    Point::new(
        (point.x / DESIGN_GRID_SPACING).round() * DESIGN_GRID_SPACING,
        (point.y / DESIGN_GRID_SPACING).round() * DESIGN_GRID_SPACING,
    )
}

#[derive(Clone, Copy)]
struct RoundCornerProfile {
    offset: f64,
    handle_ratio: f64,
}

fn infer_round_corner_profile(paths: &[Path]) -> RoundCornerProfile {
    let mut offsets = Vec::new();
    let mut handle_ratios = Vec::new();

    for path in paths {
        let Path::Cubic(cubic) = path else {
            continue;
        };
        collect_round_corner_samples(
            &cubic.points.to_vec(),
            cubic.closed,
            &mut offsets,
            &mut handle_ratios,
        );
    }

    RoundCornerProfile {
        offset: median_or_default(offsets, DEFAULT_ROUND_CORNER_OFFSET),
        handle_ratio: median_or_default(handle_ratios, DEFAULT_ROUND_CORNER_HANDLE_RATIO)
            .clamp(0.1, 1.0),
    }
}

fn collect_round_corner_samples(
    points: &[PathPoint],
    closed: bool,
    offsets: &mut Vec<f64>,
    handle_ratios: &mut Vec<f64>,
) {
    for start in 0..points.len() {
        let Some(cp1) = next_raw_index(start, points.len(), closed) else {
            continue;
        };
        let Some(cp2) = next_raw_index(cp1, points.len(), closed) else {
            continue;
        };
        let Some(end) = next_raw_index(cp2, points.len(), closed) else {
            continue;
        };
        let Some(prev) = previous_raw_index(start, points.len(), closed) else {
            continue;
        };
        let Some(next) = next_raw_index(end, points.len(), closed) else {
            continue;
        };
        if !all_distinct(&[prev, start, cp1, cp2, end, next]) {
            continue;
        }
        if !points[start].is_on_curve()
            || !points[cp1].is_off_curve()
            || !points[cp2].is_off_curve()
            || !points[end].is_on_curve()
            || !points[prev].is_on_curve()
            || !points[next].is_on_curve()
        {
            continue;
        }

        let Some(corner) = line_intersection(
            points[prev].point,
            points[start].point,
            points[end].point,
            points[next].point,
        ) else {
            continue;
        };
        let start_offset = corner.distance(points[start].point);
        let end_offset = corner.distance(points[end].point);
        if start_offset < DESIGN_GRID_SPACING || end_offset < DESIGN_GRID_SPACING {
            continue;
        }

        let handle_one = points[cp1].point.distance(points[start].point);
        let handle_two = points[end].point.distance(points[cp2].point);
        offsets.push((start_offset + end_offset) * 0.5);
        if handle_one > 0.0 {
            handle_ratios.push(handle_one / start_offset);
        }
        if handle_two > 0.0 {
            handle_ratios.push(handle_two / end_offset);
        }
    }
}

fn round_selected_corners_in_cubic_points(
    points: &[PathPoint],
    closed: bool,
    selection: &Selection,
    profile: RoundCornerProfile,
    next_selection: &mut Selection,
) -> Option<Vec<PathPoint>> {
    let mut next_points = Vec::with_capacity(points.len());
    let mut changed = false;

    for (index, point) in points.iter().enumerate() {
        if selection.contains(&point.id)
            && let Some(rounded) = rounded_corner_points(points, index, closed, profile)
        {
            for point in rounded {
                if point.is_on_curve() {
                    next_selection.insert(point.id);
                }
                next_points.push(point);
            }
            changed = true;
        } else {
            next_points.push(point.clone());
        }
    }

    changed.then_some(next_points)
}

fn rounded_corner_points(
    points: &[PathPoint],
    index: usize,
    closed: bool,
    profile: RoundCornerProfile,
) -> Option<[PathPoint; 4]> {
    if !points.get(index)?.is_on_curve() {
        return None;
    }
    let prev = previous_raw_index(index, points.len(), closed)?;
    let next = next_raw_index(index, points.len(), closed)?;
    if !points[prev].is_on_curve() || !points[next].is_on_curve() {
        return None;
    }

    let corner = points[index].point;
    let prev_vec = points[prev].point - corner;
    let next_vec = points[next].point - corner;
    let prev_len = prev_vec.hypot();
    let next_len = next_vec.hypot();
    if prev_len < DESIGN_GRID_SPACING * 2.0 || next_len < DESIGN_GRID_SPACING * 2.0 {
        return None;
    }

    let offset = profile
        .offset
        .min(prev_len * MAX_ROUND_CORNER_SIDE_FRACTION)
        .min(next_len * MAX_ROUND_CORNER_SIDE_FRACTION);
    if offset < DESIGN_GRID_SPACING {
        return None;
    }

    let prev_unit = prev_vec / prev_len;
    let next_unit = next_vec / next_len;
    let handle_len = offset * profile.handle_ratio;
    let first_on = snap_point_to_grid(corner + prev_unit * offset);
    let second_on = snap_point_to_grid(corner + next_unit * offset);
    let first_handle = snap_point_to_grid(first_on - prev_unit * handle_len);
    let second_handle = snap_point_to_grid(second_on - next_unit * handle_len);
    if first_on == corner || second_on == corner || first_on == second_on {
        return None;
    }

    Some([
        on_curve(first_on, true),
        off_curve(first_handle),
        off_curve(second_handle),
        on_curve(second_on, true),
    ])
}

fn median_or_default(mut values: Vec<f64>, default: f64) -> f64 {
    values.retain(|value| value.is_finite() && *value > 0.0);
    if values.is_empty() {
        return default;
    }
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn line_intersection(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    let r = b - a;
    let s = d - c;
    let cross = r.x * s.y - r.y * s.x;
    if cross.abs() < 1e-6 {
        return None;
    }
    let delta = c - a;
    let t = (delta.x * s.y - delta.y * s.x) / cross;
    Some(a + r * t)
}

fn all_distinct(indices: &[usize]) -> bool {
    indices
        .iter()
        .enumerate()
        .all(|(pos, index)| !indices[pos + 1..].contains(index))
}

fn previous_raw_index(index: usize, len: usize, closed: bool) -> Option<usize> {
    if index > 0 {
        Some(index - 1)
    } else if closed && len > 0 {
        Some(len - 1)
    } else {
        None
    }
}

fn next_raw_index(index: usize, len: usize, closed: bool) -> Option<usize> {
    if index + 1 < len {
        Some(index + 1)
    } else if closed && len > 0 {
        Some(0)
    } else {
        None
    }
}

fn insert_point_on_line(path: &mut Path, segment_info: &SegmentInfo, t: f64) -> Option<EntityId> {
    let point = snap_point_to_grid(segment_info.segment.eval(t));
    let new_point = on_curve(point, false);
    let id = new_point.id;
    let insert_index = if segment_info.end_index > segment_info.start_index {
        segment_info.end_index
    } else {
        segment_info.start_index + 1
    };
    path_points_mut(path).insert(insert_index, new_point);
    if let Path::Hyper(hyper) = path {
        hyper.after_change();
    }
    Some(id)
}

fn insert_point_on_cubic(
    path: &mut Path,
    segment_info: &SegmentInfo,
    cubic: kurbo::CubicBez,
    t: f64,
) -> Option<EntityId> {
    let Path::Cubic(path) = path else {
        return None;
    };
    let points = path.points.make_mut();
    let (left, right) = Segment::subdivide_cubic(cubic, t);
    let split = on_curve(snap_point_to_grid(left.p3), false);
    let split_id = split.id;
    let new_points = vec![
        off_curve(snap_point_to_grid(left.p1)),
        off_curve(snap_point_to_grid(left.p2)),
        split,
        off_curve(snap_point_to_grid(right.p1)),
        off_curve(snap_point_to_grid(right.p2)),
    ];
    replace_segment_interior(points, segment_info, new_points);
    Some(split_id)
}

fn insert_point_on_quadratic(
    path: &mut Path,
    segment_info: &SegmentInfo,
    quad: kurbo::QuadBez,
    t: f64,
) -> Option<EntityId> {
    let Path::Quadratic(path) = path else {
        return None;
    };
    let points = path.points.make_mut();
    let (left, right) = Segment::subdivide_quadratic(quad, t);
    let split = on_curve(snap_point_to_grid(left.p2), false);
    let split_id = split.id;
    let new_points = vec![
        off_curve(snap_point_to_grid(left.p1)),
        split,
        off_curve(snap_point_to_grid(right.p1)),
    ];
    replace_segment_interior(points, segment_info, new_points);
    Some(split_id)
}

fn replace_segment_interior(
    points: &mut Vec<PathPoint>,
    segment_info: &SegmentInfo,
    new_points: Vec<PathPoint>,
) {
    let remove_count = points_between_indices(
        segment_info.start_index,
        segment_info.end_index,
        points.len(),
    );
    for _ in 0..remove_count {
        let index = (segment_info.start_index + 1) % points.len();
        points.remove(index);
    }
    let mut insert_index = segment_info.start_index + 1;
    for point in new_points {
        if insert_index > points.len() {
            insert_index = points.len();
        }
        points.insert(insert_index, point);
        insert_index += 1;
    }
}

fn points_between_indices(start: usize, end: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if end > start {
        end - start - 1
    } else {
        len - start - 1 + end
    }
}

fn duplicate_path(path: &Path, offset: Vec2, new_selection: &mut Selection) -> Path {
    match path {
        Path::Cubic(cubic) => {
            let points = duplicate_points(cubic.points.iter(), offset, new_selection);
            Path::Cubic(CubicPath::new(PathPoints::from_vec(points), cubic.closed))
        }
        Path::Quadratic(quadratic) => {
            let points = duplicate_points(quadratic.points.iter(), offset, new_selection);
            Path::Quadratic(crate::path::QuadraticPath::new(
                PathPoints::from_vec(points),
                quadratic.closed,
            ))
        }
        Path::Hyper(hyper) => {
            let points = duplicate_points(hyper.points.iter(), offset, new_selection);
            Path::Hyper(crate::path::HyperPath::from_points(
                PathPoints::from_vec(points),
                hyper.closed,
            ))
        }
    }
}

fn extract_selected_point_span(path: &Path, selection: &Selection) -> Option<Path> {
    let points = path.points().to_vec();
    let closed = path_is_closed(path);
    let len = points.len();
    let mut keep = vec![false; len];

    for (index, point) in points.iter().enumerate() {
        if !point.is_on_curve() || !selection.contains(&point.id) {
            continue;
        }

        keep[index] = true;
        if let Some(previous) = previous_index(index, len, closed)
            && points[previous].is_off_curve()
        {
            keep[previous] = true;
        }
        if let Some(next) = next_index(index, len, closed)
            && points[next].is_off_curve()
        {
            keep[next] = true;
        }
    }

    let extracted = points
        .into_iter()
        .enumerate()
        .filter_map(|(index, point)| keep[index].then_some(point))
        .collect::<Vec<_>>();
    if extracted.is_empty() {
        return None;
    }

    Some(match path {
        Path::Cubic(_) => Path::Cubic(CubicPath::new(PathPoints::from_vec(extracted), false)),
        Path::Quadratic(_) => {
            Path::Quadratic(QuadraticPath::new(PathPoints::from_vec(extracted), false))
        }
        Path::Hyper(_) => {
            let mut hyper = HyperPath::from_points(PathPoints::from_vec(extracted), false);
            hyper.after_change();
            Path::Hyper(hyper)
        }
    })
}

fn clone_path_with_fresh_ids(path: &Path, closed: bool, selection: &mut Selection) -> Path {
    match path {
        Path::Cubic(cubic) => Path::Cubic(CubicPath::new(
            clone_points_with_fresh_ids(cubic.points.iter(), selection),
            closed,
        )),
        Path::Quadratic(quadratic) => Path::Quadratic(QuadraticPath::new(
            clone_points_with_fresh_ids(quadratic.points.iter(), selection),
            closed,
        )),
        Path::Hyper(hyper) => {
            let mut new_hyper = HyperPath::from_points(
                clone_points_with_fresh_ids(hyper.points.iter(), selection),
                closed,
            );
            new_hyper.after_change();
            Path::Hyper(new_hyper)
        }
    }
}

fn clone_points_with_fresh_ids<'a>(
    points: impl Iterator<Item = &'a PathPoint>,
    selection: &mut Selection,
) -> PathPoints {
    PathPoints::from_vec(
        points
            .map(|point| {
                let id = EntityId::next();
                selection.insert(id);
                PathPoint {
                    id,
                    point: point.point,
                    typ: point.typ,
                }
            })
            .collect(),
    )
}

fn path_is_closed(path: &Path) -> bool {
    match path {
        Path::Cubic(cubic) => cubic.closed,
        Path::Quadratic(quadratic) => quadratic.closed,
        Path::Hyper(hyper) => hyper.closed,
    }
}

fn previous_index(index: usize, len: usize, closed: bool) -> Option<usize> {
    if index > 0 {
        Some(index - 1)
    } else if closed && len > 0 {
        Some(len - 1)
    } else {
        None
    }
}

fn next_index(index: usize, len: usize, closed: bool) -> Option<usize> {
    if index + 1 < len {
        Some(index + 1)
    } else if closed && len > 0 {
        Some(0)
    } else {
        None
    }
}

fn duplicate_points<'a>(
    points: impl Iterator<Item = &'a PathPoint>,
    offset: Vec2,
    new_selection: &mut Selection,
) -> Vec<PathPoint> {
    points
        .map(|pt| {
            let id = EntityId::next();
            new_selection.insert(id);
            PathPoint {
                id,
                point: pt.point + offset,
                typ: pt.typ,
            }
        })
        .collect()
}

fn reverse_path_points(path: &mut Path) {
    match path {
        Path::Cubic(cubic) => cubic.points.make_mut().reverse(),
        Path::Quadratic(quadratic) => quadratic.points.make_mut().reverse(),
        Path::Hyper(hyper) => {
            hyper.points.make_mut().reverse();
            hyper.after_change();
        }
    }
}

fn path_to_bezpath_012(path: &Path) -> BezPath012 {
    let mut bez = BezPath012::new();
    append_bezpath_012_from_013(&mut bez, &path.to_bezpath());
    bez
}

fn append_bezpath_012_from_013(dst: &mut BezPath012, src: &BezPath) {
    for el in src.elements() {
        match el {
            PathEl::MoveTo(p) => dst.move_to(Point012::new(p.x, p.y)),
            PathEl::LineTo(p) => dst.line_to(Point012::new(p.x, p.y)),
            PathEl::QuadTo(c, p) => dst.quad_to(Point012::new(c.x, c.y), Point012::new(p.x, p.y)),
            PathEl::CurveTo(c1, c2, p) => dst.curve_to(
                Point012::new(c1.x, c1.y),
                Point012::new(c2.x, c2.y),
                Point012::new(p.x, p.y),
            ),
            PathEl::ClosePath => dst.close_path(),
        }
    }
}

fn append_bezpath_012_from_012(dst: &mut BezPath012, src: &BezPath012) {
    for el in src.elements() {
        match el {
            PathEl012::MoveTo(p) => dst.move_to(Point012::new(p.x, p.y)),
            PathEl012::LineTo(p) => dst.line_to(Point012::new(p.x, p.y)),
            PathEl012::QuadTo(c, p) => {
                dst.quad_to(Point012::new(c.x, c.y), Point012::new(p.x, p.y))
            }
            PathEl012::CurveTo(c1, c2, p) => dst.curve_to(
                Point012::new(c1.x, c1.y),
                Point012::new(c2.x, c2.y),
                Point012::new(p.x, p.y),
            ),
            PathEl012::ClosePath => dst.close_path(),
        }
    }
}

fn bezpath_to_cubic(bezpath: &BezPath012) -> CubicPath {
    let mut points = Vec::new();
    let has_close = bezpath
        .elements()
        .iter()
        .any(|el| matches!(el, PathEl012::ClosePath));

    for el in bezpath.elements() {
        match *el {
            PathEl012::MoveTo(p) => {
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(p.x, p.y),
                    typ: PointType::OnCurve { smooth: false },
                });
            }
            PathEl012::LineTo(p) => {
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(p.x, p.y),
                    typ: PointType::OnCurve { smooth: false },
                });
            }
            PathEl012::CurveTo(cp1, cp2, end) => {
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(cp1.x, cp1.y),
                    typ: PointType::OffCurve { auto: false },
                });
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(cp2.x, cp2.y),
                    typ: PointType::OffCurve { auto: false },
                });
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(end.x, end.y),
                    typ: PointType::OnCurve { smooth: true },
                });
            }
            PathEl012::QuadTo(cp, end) => {
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(cp.x, cp.y),
                    typ: PointType::OffCurve { auto: false },
                });
                points.push(PathPoint {
                    id: EntityId::next(),
                    point: Point::new(end.x, end.y),
                    typ: PointType::OnCurve { smooth: true },
                });
            }
            PathEl012::ClosePath => {}
        }
    }

    dedup_on_curve_points(&mut points);
    if has_close && !points.is_empty() {
        points.rotate_left(1);
    }
    fix_on_curve_smoothness(&mut points, has_close);

    CubicPath::new(PathPoints::from_vec(points), has_close)
}

fn dedup_on_curve_points(points: &mut Vec<PathPoint>) {
    const EPSILON: f64 = 0.5;

    points.dedup_by(|b, a| {
        if !a.is_on_curve() || !b.is_on_curve() {
            return false;
        }
        (a.point.x - b.point.x).abs() < EPSILON && (a.point.y - b.point.y).abs() < EPSILON
    });
}

fn fix_on_curve_smoothness(points: &mut [PathPoint], closed: bool) {
    let len = points.len();
    if len == 0 {
        return;
    }

    for i in 0..len {
        if !points[i].is_on_curve() {
            continue;
        }

        let prev_idx = if i > 0 {
            Some(i - 1)
        } else if closed {
            Some(len - 1)
        } else {
            None
        };
        let next_idx = if i + 1 < len {
            Some(i + 1)
        } else if closed {
            Some(0)
        } else {
            None
        };

        let prev_off = prev_idx.is_some_and(|j| points[j].is_off_curve());
        let next_off = next_idx.is_some_and(|j| points[j].is_off_curve());
        let smooth = prev_off && next_off;

        points[i].typ = PointType::OnCurve { smooth };
    }
}

fn restore_original_point_types(cubic: &mut CubicPath, originals: &[(Point, PointType)]) {
    const TOLERANCE: f64 = 0.5;
    let tol_sq = TOLERANCE * TOLERANCE;

    for point in cubic.points.make_mut() {
        if !point.is_on_curve() {
            continue;
        }
        if let Some((_, original_type)) = originals.iter().find(|(original_point, _)| {
            let dx = point.point.x - original_point.x;
            let dy = point.point.y - original_point.y;
            dx * dx + dy * dy < tol_sq
        }) {
            point.typ = *original_type;
        }
    }
}

/// Inclusive containment — a point on the rectangle's edge counts as
/// inside. `Rect` itself is normalized so min/max are correct
/// regardless of which corner the user dragged from.
fn rect_contains(rect: &Rect, p: Point) -> bool {
    p.x >= rect.min_x() && p.x <= rect.max_x() && p.y >= rect.min_y() && p.y <= rect.max_y()
}

fn segment_point_ids(path: &Path, segment: &SegmentInfo) -> Vec<EntityId> {
    let points = path.points().iter().collect::<Vec<_>>();
    if points.is_empty() {
        return Vec::new();
    }

    segment_point_indices(points.len(), segment.start_index, segment.end_index)
        .into_iter()
        .filter_map(|idx| points.get(idx).map(|point| point.id))
        .collect()
}

fn segment_point_indices(len: usize, start: usize, end: usize) -> Vec<usize> {
    if len == 0 || start >= len || end >= len {
        return Vec::new();
    }

    let mut indices = vec![start];
    let mut idx = start;
    while idx != end {
        idx = (idx + 1) % len;
        indices.push(idx);
        if indices.len() > len {
            return Vec::new();
        }
    }
    indices
}

/// Convert a norad Glyph into a single combined BezPath. Used by
/// both the live editor (via `set_glyph_from_norad`) and any callers
/// that just need a renderable path — e.g. the wasm `glifToSvg`
/// helper that builds the grid view.
pub fn norad_glyph_to_bezpath(glyph: &norad::Glyph) -> BezPath {
    let mut combined = BezPath::new();
    for norad_contour in &glyph.contours {
        let ws_contour = convert_norad_contour(norad_contour);
        for el in Path::from_contour(&ws_contour).to_bezpath().elements() {
            combined.push(*el);
        }
    }
    combined
}

pub fn convert_norad_contour(contour: &norad::Contour) -> WsContour {
    WsContour {
        points: contour.points.iter().map(convert_norad_point).collect(),
    }
}

pub fn convert_norad_point(pt: &norad::ContourPoint) -> WsContourPoint {
    WsContourPoint {
        x: pt.x,
        y: pt.y,
        point_type: match pt.typ {
            norad::PointType::Move => WsPointType::Move,
            norad::PointType::Line => WsPointType::Line,
            norad::PointType::OffCurve => WsPointType::OffCurve,
            norad::PointType::Curve => WsPointType::Curve,
            norad::PointType::QCurve => WsPointType::QCurve,
        },
        smooth: pt.smooth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{PathPoints, QuadraticPath};
    use crate::text::{TextDirection, TextSort};

    #[test]
    fn toggle_point_type_flips_selected_on_curve_points() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.0, 0.0), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![selected, on_curve(Point::new(100.0, 0.0), false)]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.toggle_point_type());

        let point = state.paths[0].points().iter().next().expect("point exists");
        assert!(matches!(point.typ, PointType::OnCurve { smooth: true }));
    }

    #[test]
    fn toggle_point_type_at_point_selects_hit_on_curve_point() {
        let mut state = EditorState::default();
        let first = on_curve(Point::new(0.0, 0.0), false);
        let first_id = first.id;
        let second = on_curve(Point::new(100.0, 0.0), false);
        let second_id = second.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first, off_curve(Point::new(50.0, 50.0)), second]),
            false,
        )));
        state.selection.insert(second_id);

        assert!(state.toggle_point_type_at_point(Point::new(0.0, 0.0), 10.0));

        assert!(state.selection.contains(&first_id));
        assert!(!state.selection.contains(&second_id));
        let point = state.paths[0].points().iter().next().expect("point exists");
        assert!(matches!(point.typ, PointType::OnCurve { smooth: true }));
    }

    #[test]
    fn select_segment_at_point_selects_curve_endpoints_and_handles() {
        let mut state = EditorState::default();
        let start = on_curve(Point::new(0.0, 0.0), false);
        let handle_a = off_curve(Point::new(0.0, 100.0));
        let handle_b = off_curve(Point::new(100.0, 100.0));
        let end = on_curve(Point::new(100.0, 0.0), false);
        let ids = [start.id, handle_a.id, handle_b.id, end.id];
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![start, handle_a, handle_b, end]),
            false,
        )));

        assert_eq!(
            state.select_segment_at_point(Point::new(50.0, 75.0), 10.0, false),
            Some(true)
        );

        for id in ids {
            assert!(state.selection.contains(&id));
        }
        assert_eq!(state.selection.len(), 4);
    }

    #[test]
    fn shift_clicking_selected_segment_removes_it_from_selection() {
        let mut state = EditorState::default();
        let start = on_curve(Point::new(0.0, 0.0), false);
        let end = on_curve(Point::new(100.0, 0.0), false);
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![start, end]),
            false,
        )));

        assert_eq!(
            state.select_segment_at_point(Point::new(50.0, 0.0), 10.0, false),
            Some(true)
        );
        assert_eq!(
            state.select_segment_at_point(Point::new(50.0, 0.0), 10.0, true),
            Some(false)
        );

        assert!(state.selection.is_empty());
    }

    #[test]
    fn select_contour_at_point_selects_full_hit_outline_only() {
        let mut state = EditorState::default();
        let first_a = on_curve(Point::new(0.0, 0.0), false);
        let first_b = on_curve(Point::new(100.0, 0.0), false);
        let first_ids = [first_a.id, first_b.id];
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first_a, first_b]),
            false,
        )));

        let second_a = on_curve(Point::new(200.0, 0.0), false);
        let second_handle_a = off_curve(Point::new(200.0, 100.0));
        let second_handle_b = off_curve(Point::new(300.0, 100.0));
        let second_b = on_curve(Point::new(300.0, 0.0), false);
        let second_ids = [
            second_a.id,
            second_handle_a.id,
            second_handle_b.id,
            second_b.id,
        ];
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![second_a, second_handle_a, second_handle_b, second_b]),
            false,
        )));

        assert!(state.select_contour_at_point(Point::new(250.0, 75.0), 10.0, 20.0));

        for id in second_ids {
            assert!(state.selection.contains(&id));
        }
        for id in first_ids {
            assert!(!state.selection.contains(&id));
        }
        assert_eq!(state.selection.len(), 4);
    }

    #[test]
    fn select_contour_at_point_ignores_on_curve_point_hits() {
        let mut state = EditorState::default();
        let start = on_curve(Point::new(0.0, 0.0), false);
        let end = on_curve(Point::new(100.0, 0.0), false);
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![start, end]),
            false,
        )));

        assert!(!state.select_contour_at_point(Point::new(0.0, 0.0), 10.0, 20.0));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn select_contour_at_point_ignores_off_curve_point_hits() {
        let mut state = EditorState::default();
        let start = on_curve(Point::new(0.0, 0.0), false);
        let handle_a = off_curve(Point::new(0.0, 100.0));
        let handle_b = off_curve(Point::new(100.0, 100.0));
        let end = on_curve(Point::new(100.0, 0.0), false);
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![start, handle_a, handle_b, end]),
            false,
        )));

        assert!(!state.select_contour_at_point(Point::new(0.0, 100.0), 10.0, 20.0));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn select_contour_at_point_uses_larger_point_guard_than_segment_hitbox() {
        let mut state = EditorState::default();
        let start = on_curve(Point::new(0.0, 0.0), false);
        let end = on_curve(Point::new(100.0, 0.0), false);
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![start, end]),
            false,
        )));

        assert!(!state.select_contour_at_point(Point::new(12.0, 0.0), 10.0, 20.0));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn moving_smooth_handle_keeps_opposite_handle_collinear() {
        let mut state = EditorState::default();
        let moved = off_curve(Point::new(0.0, 0.0));
        let moved_id = moved.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                moved,
                on_curve(Point::new(10.0, 0.0), true),
                off_curve(Point::new(20.0, 0.0)),
            ]),
            false,
        )));
        state.selection.insert(moved_id);

        state.translate_selection_independent(Vec2::new(10.0, 10.0));

        let points = state.paths[0].points().to_vec();
        assert_eq!(points[0].point, Point::new(10.0, 10.0));
        assert_eq!(points[2].point, Point::new(10.0, -10.0));
    }

    #[test]
    fn moving_smooth_handle_preserves_opposite_handle_length() {
        let mut state = EditorState::default();
        let moved = off_curve(Point::new(0.0, 0.0));
        let moved_id = moved.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                moved,
                on_curve(Point::new(10.0, 0.0), true),
                off_curve(Point::new(30.0, 0.0)),
            ]),
            false,
        )));
        state.selection.insert(moved_id);

        state.translate_selection_independent(Vec2::new(10.0, 10.0));

        let points = state.paths[0].points().to_vec();
        assert_eq!(points[0].point, Point::new(10.0, 10.0));
        assert_eq!(points[2].point, Point::new(10.0, -20.0));
    }

    #[test]
    fn moving_handle_after_smooth_line_endpoint_stays_tangent_to_line() {
        let mut state = EditorState::default();
        let moved = off_curve(Point::new(20.0, 0.0));
        let moved_id = moved.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(10.0, 0.0), true),
                moved,
                on_curve(Point::new(30.0, -20.0), false),
            ]),
            false,
        )));
        state.selection.insert(moved_id);

        state.translate_selection(Vec2::new(0.0, -10.0));

        let points = state.paths[0].points().to_vec();
        assert_eq!(points[2].point, Point::new(20.0, 0.0));
    }

    #[test]
    fn moving_handle_before_smooth_line_endpoint_stays_tangent_to_line() {
        let mut state = EditorState::default();
        let moved = off_curve(Point::new(0.0, 0.0));
        let moved_id = moved.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(-20.0, -20.0), false),
                moved,
                on_curve(Point::new(10.0, 0.0), true),
                on_curve(Point::new(20.0, 0.0), false),
            ]),
            false,
        )));
        state.selection.insert(moved_id);

        state.translate_selection(Vec2::new(0.0, 10.0));

        let points = state.paths[0].points().to_vec();
        assert_eq!(points[1].point, Point::new(0.0, 0.0));
    }

    #[test]
    fn snapping_selected_offcurve_handles_uses_two_unit_grid() {
        let mut state = EditorState::default();
        let selected = off_curve(Point::new(1.25, 3.75));
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                selected,
                on_curve(Point::new(10.0, 0.0), true),
                off_curve(Point::new(20.0, 0.0)),
            ]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.snap_selected_offcurves_to_grid());

        let points = state.paths[0].points().to_vec();
        assert_eq!(points[0].point, Point::new(2.0, 4.0));
        let moved = points[0].point - points[1].point;
        let opposite = points[2].point - points[1].point;
        assert!((moved.x * opposite.y - moved.y * opposite.x).abs() < 1e-9);
    }

    #[test]
    fn pointer_translate_snaps_selected_offcurve_handles_by_default() {
        let mut state = EditorState::default();
        let selected = off_curve(Point::new(1.25, 3.75));
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![selected, on_curve(Point::new(100.0, 100.0), false)]),
            false,
        )));
        state.selection.insert(selected_id);

        state.translate_selection_independent(Vec2::new(0.2, 0.2));

        let point = state.paths[0].points().iter().next().expect("point exists");
        assert_eq!(point.point, Point::new(2.0, 4.0));
    }

    #[test]
    fn pen_smooth_handles_snap_to_grid_while_dragging() {
        let mut state = EditorState::default();

        let path_index =
            state.append_smooth_pen_point(None, Point::new(10.4, 20.6), Point::new(15.1, 25.7));

        let points = state.paths[path_index].points().to_vec();
        assert_eq!(points[0].point, Point::new(10.0, 20.0));
        assert_eq!(points[1].point, Point::new(16.0, 26.0));
    }

    #[test]
    fn cycle_selected_point_moves_forward_in_outline_order() {
        let mut state = EditorState::default();
        let first = on_curve(Point::new(0.0, 0.0), false);
        let first_id = first.id;
        let second = off_curve(Point::new(50.0, 50.0));
        let second_id = second.id;
        let third = on_curve(Point::new(100.0, 0.0), false);
        let third_id = third.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first, second, third]),
            false,
        )));
        state.selection.insert(first_id);

        assert!(state.cycle_selected_point(false));
        assert!(!state.selection.contains(&first_id));
        assert!(state.selection.contains(&second_id));

        assert!(state.cycle_selected_point(false));
        assert!(!state.selection.contains(&second_id));
        assert!(state.selection.contains(&third_id));
    }

    #[test]
    fn cycle_selected_point_moves_backward_and_wraps() {
        let mut state = EditorState::default();
        let first = on_curve(Point::new(0.0, 0.0), false);
        let first_id = first.id;
        let second = on_curve(Point::new(100.0, 0.0), false);
        let second_id = second.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first, second]),
            false,
        )));
        state.selection.insert(first_id);

        assert!(state.cycle_selected_point(true));
        assert!(!state.selection.contains(&first_id));
        assert!(state.selection.contains(&second_id));
    }

    #[test]
    fn active_text_sort_origin_uses_text_layout_position() {
        let mut state = EditorState::default();
        state.metrics = Some(FontMetrics {
            ascender: Some(700.0),
            descender: Some(-300.0),
            ..Default::default()
        });
        state.text_buffer.insert_glyph("A", Some('A'), 500.0);
        state.text_buffer.insert_line_break();
        state.text_buffer.insert_glyph("B", Some('B'), 600.0);

        assert!(state.text_buffer.activate_sort(2));

        assert_eq!(state.text_line_height(), 1000.0);
        assert_eq!(state.active_text_sort_origin(), Vec2::new(0.0, -1000.0));
    }

    #[test]
    fn text_line_height_extends_to_upm_sort_bounds() {
        let mut state = EditorState::default();
        state.metrics = Some(FontMetrics {
            units_per_em: Some(1000.0),
            ascender: Some(700.0),
            descender: Some(-300.0),
            ..Default::default()
        });

        assert_eq!(state.text_metric_bounds(), (700.0, -300.0));
        assert_eq!(state.text_sort_metric_bounds(), (1000.0, -300.0));
        assert_eq!(state.text_line_height(), 1300.0);
    }

    #[test]
    fn text_sort_metric_bounds_keep_ascender_when_it_exceeds_upm() {
        let mut state = EditorState::default();
        state.metrics = Some(FontMetrics {
            units_per_em: Some(600.0),
            ascender: Some(700.0),
            descender: Some(-300.0),
            ..Default::default()
        });

        assert_eq!(state.text_sort_metric_bounds(), (700.0, -300.0));
    }

    #[test]
    fn glyph_metric_bounds_extend_to_upm_for_icon_fonts() {
        let mut state = EditorState::default();
        state.metrics = Some(FontMetrics {
            units_per_em: Some(1000.0),
            ascender: Some(768.0),
            descender: Some(-200.0),
            cap_height: Some(800.0),
            ..Default::default()
        });

        assert_eq!(state.glyph_metric_bounds(), Some((1000.0, -200.0)));
    }

    #[test]
    fn nudge_selection_uses_xilem_keyboard_amounts() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(10.0, 20.0), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![selected, on_curve(Point::new(100.0, 20.0), false)]),
            false,
        )));
        state.selection.insert(selected_id);
        let revision = state.edit_revision;

        assert!(state.nudge_selection(1.0, -1.0, true, false, false));

        let point = state.paths[0].points().iter().next().expect("point exists");
        assert_eq!(point.point, Point::new(18.0, 12.0));
        assert!(state.edit_revision > revision);
    }

    #[test]
    fn transform_actions_apply_to_selected_component() {
        let mut state = EditorState::default();
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(30.0, 0.0));
        path.line_to(Point::new(30.0, 20.0));
        path.close_path();
        let component_id = EntityId::next();
        state.set_component_previews(vec![ComponentPreview {
            id: component_id,
            index: 0,
            base: "acute".to_string(),
            transform: Affine::IDENTITY,
            path,
        }]);
        state.select_component(component_id);
        let revision = state.edit_revision;

        assert!(state.flip_selection_horizontal());

        let transform = state.component_transform(0).expect("component transform");
        assert_ne!(transform, Affine::IDENTITY);
        assert!(state.edit_revision > revision);
    }

    #[test]
    fn flip_selection_horizontal_uses_center_reference_in_place() {
        let mut state = EditorState::default();
        let p0 = on_curve(Point::new(0.0, 0.0), false);
        let p1 = on_curve(Point::new(100.0, 0.0), false);
        let p2 = on_curve(Point::new(100.0, 50.0), false);
        let p3 = on_curve(Point::new(0.0, 50.0), false);
        for id in [p0.id, p1.id, p2.id, p3.id] {
            state.selection.insert(id);
        }
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![p0, p1, p2, p3]),
            true,
        )));

        assert!(state.flip_selection_horizontal());

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[0].point, Point::new(100.0, 0.0));
        assert_eq!(points[1].point, Point::new(0.0, 0.0));
        assert_eq!(points[2].point, Point::new(0.0, 50.0));
        assert_eq!(points[3].point, Point::new(100.0, 50.0));
        let Some((_count, bounds)) = state.selection_bounds() else {
            panic!("flipped selection should still have bounds");
        };
        assert_eq!(bounds.center(), Point::new(50.0, 25.0));
    }

    #[test]
    fn flip_selection_vertical_uses_center_reference_in_place() {
        let mut state = EditorState::default();
        let p0 = on_curve(Point::new(0.0, 0.0), false);
        let p1 = on_curve(Point::new(100.0, 0.0), false);
        let p2 = on_curve(Point::new(100.0, 50.0), false);
        let p3 = on_curve(Point::new(0.0, 50.0), false);
        for id in [p0.id, p1.id, p2.id, p3.id] {
            state.selection.insert(id);
        }
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![p0, p1, p2, p3]),
            true,
        )));

        assert!(state.flip_selection_vertical());

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[0].point, Point::new(0.0, 50.0));
        assert_eq!(points[1].point, Point::new(100.0, 50.0));
        assert_eq!(points[2].point, Point::new(100.0, 0.0));
        assert_eq!(points[3].point, Point::new(0.0, 0.0));
        let Some((_count, bounds)) = state.selection_bounds() else {
            panic!("flipped selection should still have bounds");
        };
        assert_eq!(bounds.center(), Point::new(50.0, 25.0));
    }

    #[test]
    fn flip_selection_uses_active_coordinate_reference() {
        let mut state = EditorState::default();
        state.set_coord_quadrant(Quadrant::Left);
        let p0 = on_curve(Point::new(20.0, 0.0), false);
        let p1 = on_curve(Point::new(100.0, 0.0), false);
        for id in [p0.id, p1.id] {
            state.selection.insert(id);
        }
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![p0, p1]),
            false,
        )));

        assert!(state.flip_selection_horizontal());

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[0].point, Point::new(20.0, 0.0));
        assert_eq!(points[1].point, Point::new(-60.0, 0.0));
    }

    #[test]
    fn resize_selection_reference_scales_selected_points() {
        let mut state = EditorState::default();
        let p0 = on_curve(Point::new(0.0, 0.0), false);
        let p1 = on_curve(Point::new(100.0, 0.0), false);
        let p2 = on_curve(Point::new(100.0, 100.0), false);
        let p3 = on_curve(Point::new(0.0, 100.0), false);
        for id in [p0.id, p1.id, p2.id, p3.id] {
            state.selection.insert(id);
        }
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![p0, p1, p2, p3]),
            true,
        )));
        let revision = state.edit_revision;

        assert!(state.resize_selection_reference("width", 200.0));

        let Some((_count, bounds)) = state.selection_bounds() else {
            panic!("selection should still have bounds");
        };
        assert_eq!(bounds.x0, -50.0);
        assert_eq!(bounds.x1, 150.0);
        assert_eq!(bounds.y0, 0.0);
        assert_eq!(bounds.y1, 100.0);
        assert!(state.edit_revision > revision);
        assert!(state.last_transform.is_some());
    }

    #[test]
    fn resize_selection_reference_scales_selected_component() {
        let mut state = EditorState::default();
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.line_to(Point::new(0.0, 100.0));
        path.close_path();
        let component_id = EntityId::next();
        state.set_component_previews(vec![ComponentPreview {
            id: component_id,
            index: 0,
            base: "acute".to_string(),
            transform: Affine::IDENTITY,
            path,
        }]);
        state.select_component(component_id);
        let revision = state.edit_revision;

        assert!(state.resize_selection_reference("height", 50.0));

        let Some((_count, bounds)) = state.selection_bounds() else {
            panic!("component selection should still have bounds");
        };
        assert_eq!(bounds.x0, 0.0);
        assert_eq!(bounds.x1, 100.0);
        assert_eq!(bounds.y0, 25.0);
        assert_eq!(bounds.y1, 75.0);
        assert!(state.edit_revision > revision);
        assert!(state.last_transform.is_some());
    }

    #[test]
    fn delete_selection_removes_selected_component() {
        let mut state = EditorState::default();
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(10.0, 10.0));
        path.close_path();
        let component_id = EntityId::next();
        state.set_component_previews(vec![ComponentPreview {
            id: component_id,
            index: 3,
            base: "acute".to_string(),
            transform: Affine::IDENTITY,
            path,
        }]);
        state.select_component(component_id);

        assert!(state.delete_selection());

        assert!(state.component_previews.is_empty());
        assert!(state.selected_component.is_none());
        assert!(state.deleted_component_indices.contains(&3));
    }

    #[test]
    fn duplicate_selection_copies_selected_component() {
        let mut state = EditorState::default();
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(10.0, 10.0));
        path.close_path();
        let component_id = EntityId::next();
        state.set_component_previews(vec![ComponentPreview {
            id: component_id,
            index: 0,
            base: "acute".to_string(),
            transform: Affine::IDENTITY,
            path,
        }]);
        state.select_component(component_id);

        assert!(state.duplicate_selection());

        assert_eq!(state.component_previews.len(), 2);
        let duplicate = state.component_previews.last().expect("duplicate exists");
        assert_eq!(duplicate.index, 1);
        assert_eq!(duplicate.base, "acute");
        assert_eq!(state.selected_component, Some(duplicate.id));
        assert_eq!(
            duplicate.transform,
            Affine::translate(Vec2::new(20.0, 20.0))
        );
    }

    #[test]
    fn nudge_selection_snaps_to_xilem_design_grid() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(10.3, 20.7), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![selected, on_curve(Point::new(100.0, 20.0), false)]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.nudge_selection(1.0, 0.0, false, false, false));

        let point = state.paths[0].points().iter().next().expect("point exists");
        assert_eq!(point.point, Point::new(12.0, 20.0));
    }

    #[test]
    fn nudge_selection_moves_adjacent_handles_unless_independent() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(10.0, 0.0), true);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                off_curve(Point::new(6.0, 10.0)),
                selected,
                off_curve(Point::new(14.0, 10.0)),
                on_curve(Point::new(20.0, 0.0), false),
            ]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.nudge_selection(1.0, 0.0, false, false, false));
        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[1].point, Point::new(8.0, 10.0));
        assert_eq!(points[2].point, Point::new(12.0, 0.0));
        assert_eq!(points[3].point, Point::new(16.0, 10.0));

        assert!(state.nudge_selection(1.0, 0.0, false, false, true));
        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[1].point, Point::new(8.0, 10.0));
        assert_eq!(points[2].point, Point::new(14.0, 0.0));
        assert_eq!(points[3].point, Point::new(16.0, 10.0));
    }

    #[test]
    fn delete_selection_removes_paired_cubic_offcurve_handle() {
        let mut state = EditorState::default();
        let off_a = off_curve(Point::new(30.0, 0.0));
        let selected_id = off_a.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                off_a,
                off_curve(Point::new(70.0, 0.0)),
                on_curve(Point::new(100.0, 0.0), false),
            ]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.delete_selection());

        let remaining = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|point| point.is_on_curve()));
        assert_eq!(state.selection.len(), 0);
    }

    #[test]
    fn convert_line_segment_inserts_selected_cubic_handles() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(90.0, 0.0), false),
            ]),
            false,
        )));

        assert!(state.convert_line_segment_at_point(Point::new(45.0, 1.0), 10.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points.len(), 4);
        assert!(points[1].is_off_curve());
        assert!(points[2].is_off_curve());
        assert_eq!(points[1].point, Point::new(30.0, 0.0));
        assert_eq!(points[2].point, Point::new(60.0, 0.0));
        assert!(state.selection.contains(&points[1].id));
        assert!(state.selection.contains(&points[2].id));
    }

    #[test]
    fn insert_point_on_line_segment_selects_new_point() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
            ]),
            false,
        )));

        assert!(state.insert_point_on_nearest_segment(Point::new(50.0, 0.0), 10.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points.len(), 3);
        assert_eq!(points[1].point, Point::new(50.0, 0.0));
        assert!(points[1].is_on_curve());
        assert!(state.selection.contains(&points[1].id));
    }

    #[test]
    fn insert_point_on_line_segment_snaps_to_design_grid() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(101.0, 0.0), false),
            ]),
            false,
        )));

        assert!(state.insert_point_on_nearest_segment(Point::new(51.0, 0.0), 10.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points.len(), 3);
        assert_eq!(points[1].point, Point::new(52.0, 0.0));
    }

    #[test]
    fn insert_point_on_cubic_segment_subdivides_handles() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                off_curve(Point::new(0.0, 100.0)),
                off_curve(Point::new(100.0, 100.0)),
                on_curve(Point::new(100.0, 0.0), false),
            ]),
            false,
        )));

        assert!(state.insert_point_on_nearest_segment(Point::new(50.0, 75.0), 10.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points.len(), 7);
        assert!(points[1].is_off_curve());
        assert!(points[2].is_off_curve());
        assert!(points[3].is_on_curve());
        assert!(points[4].is_off_curve());
        assert!(points[5].is_off_curve());
        assert!(state.selection.contains(&points[3].id));
    }

    #[test]
    fn insert_point_on_quadratic_segment_subdivides_handle() {
        let mut state = EditorState::default();
        state.paths.push(Path::Quadratic(QuadraticPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                off_curve(Point::new(50.0, 100.0)),
                on_curve(Point::new(100.0, 0.0), false),
            ]),
            false,
        )));

        assert!(state.insert_point_on_nearest_segment(Point::new(50.0, 50.0), 10.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points.len(), 5);
        assert!(points[1].is_off_curve());
        assert!(points[2].is_on_curve());
        assert!(points[3].is_off_curve());
        assert!(state.selection.contains(&points[2].id));
    }

    #[test]
    fn round_selected_corners_matches_existing_corner_profile() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.0, 100.0), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                selected,
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(80.0, 0.0), true),
                off_curve(Point::new(92.0, 0.0)),
                off_curve(Point::new(100.0, 8.0)),
                on_curve(Point::new(100.0, 20.0), true),
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));
        state.selection.insert(selected_id);

        assert!(state.round_selected_corners());

        let Path::Cubic(path) = &state.paths[0] else {
            panic!("rounded corners should preserve cubic path type");
        };
        let points = path.points.to_vec();
        assert_eq!(points.len(), 10);
        assert_eq!(points[0].point, Point::new(20.0, 100.0));
        assert_eq!(points[1].point, Point::new(8.0, 100.0));
        assert_eq!(points[2].point, Point::new(0.0, 92.0));
        assert_eq!(points[3].point, Point::new(0.0, 80.0));
        assert!(points[0].is_on_curve());
        assert!(points[1].is_off_curve());
        assert!(points[2].is_off_curve());
        assert!(points[3].is_on_curve());
        assert!(state.selection.contains(&points[0].id));
        assert!(state.selection.contains(&points[3].id));
        assert_eq!(state.selection.len(), 2);
    }

    #[test]
    fn round_selected_corners_snaps_generated_points_to_grid() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.3, 100.7), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                selected,
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));
        state.selection.insert(selected_id);

        assert!(state.round_selected_corners());

        let Path::Cubic(path) = &state.paths[0] else {
            panic!("rounded corners should preserve cubic path type");
        };
        for point in path.points.iter().take(4) {
            assert_eq!(point.point.x % DESIGN_GRID_SPACING, 0.0);
            assert_eq!(point.point.y % DESIGN_GRID_SPACING, 0.0);
        }
    }

    #[test]
    fn round_selected_corners_ignores_curve_connected_points() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.0, 0.0), true);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                selected,
                off_curve(Point::new(20.0, 80.0)),
                off_curve(Point::new(80.0, 80.0)),
                on_curve(Point::new(100.0, 0.0), true),
            ]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(!state.round_selected_corners());
    }

    #[test]
    fn copy_paste_selection_appends_fresh_selected_path() {
        let mut state = EditorState::default();
        let first = on_curve(Point::new(0.0, 0.0), false);
        let second = on_curve(Point::new(100.0, 0.0), false);
        let first_id = first.id;
        let second_id = second.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first, second]),
            false,
        )));
        state.selection.insert(first_id);
        state.selection.insert(second_id);

        let clipboard = state.copy_selection().expect("selection copies");
        assert!(state.paste_paths(&clipboard));

        assert_eq!(state.paths.len(), 2);
        assert_eq!(state.selection.len(), 2);
        let pasted = state.paths[1].points().iter().collect::<Vec<_>>();
        assert!(
            pasted
                .iter()
                .all(|point| state.selection.contains(&point.id))
        );
        assert!(
            pasted
                .iter()
                .all(|point| point.id != first_id && point.id != second_id)
        );
    }

    #[test]
    fn reverse_contours_reverses_path_order() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));

        assert!(state.reverse_contours());

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[0].point, Point::new(100.0, 100.0));
        assert_eq!(points[2].point, Point::new(0.0, 0.0));
    }

    #[test]
    fn boolean_selection_restores_original_on_curve_types() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
                on_curve(Point::new(0.0, 100.0), false),
            ]),
            true,
        )));
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(200.0, 0.0), false),
                on_curve(Point::new(300.0, 0.0), false),
                on_curve(Point::new(300.0, 100.0), false),
                on_curve(Point::new(200.0, 100.0), false),
            ]),
            true,
        )));
        let Path::Cubic(first_path) = &mut state.paths[0] else {
            panic!("rect path should be cubic");
        };
        first_path.points.make_mut()[0].typ = PointType::OnCurve { smooth: true };
        for id in state
            .paths
            .iter()
            .flat_map(|path| path.points().iter().map(|point| point.id))
            .collect::<Vec<_>>()
        {
            state.selection.insert(id);
        }

        assert!(state.boolean_selection(linesweeper::BinaryOp::Union));

        let restored = state.paths.iter().any(|path| {
            path.points().iter().any(|point| {
                point.point.distance(Point::new(0.0, 0.0)) < 0.5
                    && matches!(point.typ, PointType::OnCurve { smooth: true })
            })
        });
        assert!(
            restored,
            "boolean output should restore matching input point type"
        );
    }

    #[test]
    fn boolean_selection_operates_on_all_contours_without_selection() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
                on_curve(Point::new(0.0, 100.0), false),
            ]),
            true,
        )));
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(50.0, 0.0), false),
                on_curve(Point::new(150.0, 0.0), false),
                on_curve(Point::new(150.0, 100.0), false),
                on_curve(Point::new(50.0, 100.0), false),
            ]),
            true,
        )));

        assert!(state.selection.is_empty());
        assert!(state.boolean_selection(linesweeper::BinaryOp::Union));
        assert_eq!(state.paths.len(), 1);
    }

    #[test]
    fn selected_contour_count_counts_contours_not_points() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(200.0, 0.0), false),
                on_curve(Point::new(300.0, 0.0), false),
                on_curve(Point::new(300.0, 100.0), false),
            ]),
            true,
        )));

        let first_path_ids = state.paths[0]
            .points()
            .iter()
            .map(|point| point.id)
            .collect::<Vec<_>>();
        state.selection.insert(first_path_ids[0]);
        state.selection.insert(first_path_ids[1]);
        assert_eq!(state.selected_contour_count(), 1);

        let second_path_id = state.paths[1].points().iter().next().unwrap().id;
        state.selection.insert(second_path_id);
        assert_eq!(state.selected_contour_count(), 2);
    }

    #[test]
    fn set_start_point_at_rotates_closed_contour() {
        let mut state = EditorState::default();
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(0.0, 0.0), false),
                on_curve(Point::new(100.0, 0.0), false),
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));

        assert!(state.set_start_point_at(Point::new(100.0, 0.0), 5.0));

        let points = state.paths[0].points().iter().collect::<Vec<_>>();
        assert_eq!(points[0].point, Point::new(100.0, 0.0));
    }

    #[test]
    fn contour_context_falls_back_to_selected_on_curve_point() {
        let mut state = EditorState::default();
        let first = on_curve(Point::new(0.0, 0.0), false);
        let selected = on_curve(Point::new(100.0, 0.0), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                first,
                selected,
                on_curve(Point::new(100.0, 100.0), false),
            ]),
            true,
        )));
        state.selection.insert(selected_id);

        let target = state
            .contour_context_target(Point::new(900.0, 900.0), 5.0)
            .expect("selected on-curve point should provide context");

        assert_eq!(target.path_index, 0);
        assert_eq!(target.point, Point::new(100.0, 0.0));
        assert!(target.can_set_start);
    }

    #[test]
    fn move_contour_swaps_order_and_clears_selection() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.0, 0.0), false);
        let selected_id = selected.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![selected]),
            false,
        )));
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![on_curve(Point::new(200.0, 0.0), false)]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.move_contour(1, -1));

        let first = state.paths[0].points().iter().next().expect("point exists");
        assert_eq!(first.point, Point::new(200.0, 0.0));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn convert_hyper_to_cubic_converts_only_selected_hyper_paths() {
        let mut state = EditorState::default();
        let selected = on_curve(Point::new(0.0, 0.0), true);
        let selected_id = selected.id;
        let unselected = on_curve(Point::new(200.0, 0.0), true);
        state.paths.push(Path::Hyper(HyperPath::from_points(
            PathPoints::from_vec(vec![
                selected,
                on_curve(Point::new(100.0, 100.0), true),
                on_curve(Point::new(100.0, 0.0), true),
            ]),
            false,
        )));
        state.paths.push(Path::Hyper(HyperPath::from_points(
            PathPoints::from_vec(vec![
                unselected,
                on_curve(Point::new(300.0, 100.0), true),
                on_curve(Point::new(300.0, 0.0), true),
            ]),
            false,
        )));
        state.selection.insert(selected_id);

        assert!(state.convert_hyper_to_cubic());

        assert!(matches!(state.paths[0], Path::Cubic(_)));
        assert!(matches!(state.paths[1], Path::Hyper(_)));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn set_advance_width_updates_revision() {
        let mut state = EditorState::default();
        let before = state.edit_revision();

        assert!(state.set_advance_width(720.0));

        assert_eq!(state.advance_width, 720.0);
        assert!(state.edit_revision() > before);
        assert!(!state.set_advance_width(720.0));
        assert!(!state.set_advance_width(-1.0));
    }

    #[test]
    fn text_sort_glyph_reload_preserves_edit_revision() {
        let glyph = norad::Glyph::parse_raw(
            br#"<glyph name="A" format="2"><advance width="600"/></glyph>"#,
        )
        .expect("valid glyph");
        let mut state = EditorState::default();
        state.bump_edit_revision();
        state.set_coord_quadrant(Quadrant::TopRight);
        state.last_transform = Some(Affine::translate((10.0, 20.0)));
        let before = state.edit_revision();
        let last_transform = state.last_transform;

        state.set_glyph_from_norad_preserving_history(&glyph);

        assert_eq!(state.edit_revision(), before);
        assert_eq!(state.coord_quadrant, Quadrant::TopRight);
        assert_eq!(state.last_transform, last_transform);
        assert_eq!(state.advance_width, 600.0);
    }

    #[test]
    fn text_keyboard_insert_updates_revision() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A" },
                    "widths": { "A": 700 }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        let before = state.edit_revision();

        assert!(state.insert_text_character('A'));

        assert_eq!(state.text_buffer.len(), 1);
        assert!(state.edit_revision() > before);
    }

    #[test]
    fn text_keyboard_insert_reuses_live_active_width_like_xilem() {
        let mut state = EditorState {
            advance_width: 900.0,
            ..Default::default()
        };
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A" },
                    "widths": { "A": 700 }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.insert_glyph("A", Some('A'), 700.0);

        assert!(state.insert_text_character('A'));

        let sort = state.text_buffer.sort(1).expect("inserted sort");
        let crate::text::TextSortKind::Glyph { advance_width, .. } = &sort.kind else {
            panic!("expected inserted glyph sort");
        };
        assert_eq!(*advance_width, 900.0);
    }

    #[test]
    fn rtl_text_non_arabic_insert_reuses_live_active_width_like_xilem() {
        let mut state = EditorState {
            advance_width: 920.0,
            ..Default::default()
        };
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A" },
                    "widths": { "A": 700 }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.insert_glyph("A", Some('A'), 700.0);

        assert!(state.insert_text_character('A'));

        let sort = state.text_buffer.sort(1).expect("inserted sort");
        let crate::text::TextSortKind::Glyph { advance_width, .. } = &sort.kind else {
            panic!("expected inserted glyph sort");
        };
        assert_eq!(*advance_width, 920.0);
    }

    #[test]
    fn rtl_arabic_insert_keeps_shaped_inventory_width_like_xilem() {
        let mut state = EditorState {
            advance_width: 920.0,
            ..Default::default()
        };
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1605": "meem-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "meem-ar": 520,
                        "meem-ar.fina": 500
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state
            .text_buffer
            .insert_glyph("beh-ar", Some('\u{0628}'), 500.0);

        assert!(state.insert_text_character('\u{0645}'));

        let sort = state.text_buffer.sort(1).expect("inserted sort");
        let crate::text::TextSortKind::Glyph {
            name,
            advance_width,
            ..
        } = &sort.kind
        else {
            panic!("expected inserted glyph sort");
        };
        assert_eq!(name, "meem-ar.fina");
        assert_eq!(*advance_width, 500.0);
    }

    #[test]
    fn inactive_text_glyph_insert_updates_revision() {
        let mut state = EditorState::default();
        state.text_buffer.insert_glyph("A", Some('A'), 700.0);
        state.text_buffer.set_cursor(0);
        let before = state.edit_revision();

        state.insert_inactive_text_glyph("B", Some('B'), 710.0);

        assert_eq!(state.text_buffer.len(), 2);
        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("B")
        );
        assert_eq!(state.text_buffer.active_sort(), Some(1));
        assert!(state.edit_revision() > before);
    }

    #[test]
    fn text_keyboard_delete_updates_revision() {
        let mut state = EditorState::default();
        state.text_buffer.insert_glyph("A", Some('A'), 700.0);
        state.text_buffer.insert_glyph("B", Some('B'), 700.0);
        let before = state.edit_revision();

        assert!(state.delete_text_before_cursor());

        assert_eq!(state.text_buffer.len(), 1);
        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("A")
        );
        assert!(state.edit_revision() > before);
    }

    #[test]
    fn ltr_text_delete_preserves_existing_shaped_forms() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        assert!(state.insert_text_character('\u{0628}'));
        assert!(state.insert_text_character('\u{0647}'));
        state.text_buffer.set_direction(TextDirection::LeftToRight);
        state.text_buffer.insert_glyph("A", Some('A'), 700.0);

        assert!(state.delete_text_before_cursor());

        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            state.text_buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn ltr_text_line_break_preserves_existing_shaped_forms() {
        let mut state = EditorState::default();
        state
            .text_buffer
            .insert_glyph("beh-ar.init", Some('\u{0628}'), 480.0);
        state
            .text_buffer
            .insert_glyph("heh-ar.fina", Some('\u{0647}'), 490.0);
        state.text_buffer.set_direction(TextDirection::LeftToRight);

        state.insert_text_line_break();

        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            state.text_buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn rtl_text_line_break_matches_xilem_no_reshape_on_enter() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        assert!(state.insert_text_character('\u{0628}'));
        assert!(state.insert_text_character('\u{0647}'));
        state.text_buffer.set_cursor(1);

        state.insert_text_line_break();

        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert!(matches!(
            state.text_buffer.sort(1).map(|sort| &sort.kind),
            Some(crate::text::TextSortKind::LineBreak)
        ));
        assert_eq!(
            state.text_buffer.sort(2).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn rtl_text_delete_repairs_neighboring_joining_forms() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "beh-ar.fina": 470,
                        "heh-ar": 510,
                        "heh-ar.init": 500,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        state
            .text_buffer
            .insert_glyph("beh-ar.init", Some('\u{0628}'), 480.0);
        state
            .text_buffer
            .insert_glyph("heh-ar.fina", Some('\u{0647}'), 490.0);
        state
            .text_buffer
            .insert_glyph("beh-ar.custom", Some('\u{0628}'), 470.0);
        state.text_buffer.set_cursor(1);

        assert!(state.delete_text_before_cursor());

        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("heh-ar.init")
        );
        assert_eq!(
            state.text_buffer.sort(1).and_then(TextSort::glyph_name),
            Some("beh-ar.fina")
        );
    }

    #[test]
    fn rtl_text_boundary_backspace_can_repair_stale_joining_form() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        state
            .text_buffer
            .insert_inactive_glyph("beh-ar", Some('\u{0628}'), 500.0);
        state
            .text_buffer
            .insert_inactive_glyph("heh-ar", Some('\u{0647}'), 510.0);
        state.text_buffer.set_cursor(0);
        let before = state.edit_revision();

        assert!(state.delete_text_before_cursor());

        assert_eq!(
            state.text_buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert!(state.edit_revision() > before);
    }

    #[test]
    fn rtl_text_boundary_delete_can_repair_stale_joining_form() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.set_direction(TextDirection::RightToLeft);
        state
            .text_buffer
            .insert_inactive_glyph("beh-ar", Some('\u{0628}'), 500.0);
        state
            .text_buffer
            .insert_inactive_glyph("heh-ar", Some('\u{0647}'), 510.0);
        state.text_buffer.set_cursor(2);
        let before = state.edit_revision();

        assert!(state.delete_text_after_cursor());

        assert_eq!(
            state.text_buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
        assert!(state.edit_revision() > before);
    }

    #[test]
    fn sidebearing_edits_shift_outlines_or_width() {
        let mut state = EditorState::default();
        state.advance_width = 600.0;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![
                on_curve(Point::new(10.0, 0.0), false),
                on_curve(Point::new(110.0, 0.0), false),
            ]),
            false,
        )));

        assert_eq!(state.left_sidebearing(), 10.0);
        assert_eq!(state.right_sidebearing(), 490.0);
        assert!(state.set_left_sidebearing(20.0));
        let point = state.paths[0].points().iter().next().expect("point exists");
        assert_eq!(point.point.x, 20.0);
        assert_eq!(state.advance_width, 600.0);
        assert!(state.set_right_sidebearing(400.0));
        assert_eq!(state.advance_width, 520.0);
    }
}
