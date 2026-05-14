// Editor state — the source of truth for what's being edited.
//
// Renderer reads from this; mouse/keyboard handlers mutate it. Lives
// outside the wasm-bindgen surface so it's testable on native too.

use kurbo::{BezPath, PathEl, Point, Rect, Shape, Vec2};
use serde::Deserialize;

use crate::editing::{Selection, ViewPort};
use crate::model::EntityId;
use crate::model::workspace::{self, Contour as WsContour, ContourPoint as WsContourPoint,
    PointType as WsPointType};
use crate::path::{CubicPath, Path, PathPoint, PathPoints, PointType};

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

    /// Font-wide vertical metrics (baseline, x-height, cap-height,
    /// ascender, descender). Drawn as horizontal guideline lines in
    /// the renderer. `None` until the host loads fontinfo.plist.
    pub metrics: Option<FontMetrics>,

    /// Transient screen-space rectangle for an in-progress
    /// box-selection drag. Renderer draws it as a marquee; cleared
    /// when the drag ends.
    pub marquee: Option<Rect>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            selection: Selection::default(),
            viewport: ViewPort::default(),
            advance_width: 0.0,
            metrics: None,
            marquee: None,
        }
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the glyph from a `kurbo::BezPath` (typically parsed
    /// from SVG path data). Each `MoveTo` starts a new contour;
    /// each curve element produces explicit on/off-curve points.
    pub fn set_glyph_from_bezpath(&mut self, bez: &BezPath) {
        self.paths.clear();
        self.selection = Selection::new();
        self.marquee = None;

        let mut current_points: Vec<PathPoint> = Vec::new();

        let flush = |paths: &mut Vec<Path>, points: &mut Vec<PathPoint>, closed: bool| {
            if !points.is_empty() {
                let cubic = CubicPath::new(
                    PathPoints::from_vec(std::mem::take(points)),
                    closed,
                );
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
        self.selection = Selection::new();
        self.marquee = None;
        self.advance_width = glyph.width;

        for norad_contour in &glyph.contours {
            let ws_contour = convert_norad_contour(norad_contour);
            self.paths.push(Path::from_contour(&ws_contour));
        }
    }

    /// Bounding box of all paths in design space, or `None` if empty.
    pub fn glyph_bbox(&self) -> Option<Rect> {
        let mut bbox: Option<Rect> = None;
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
    /// (in screen-space pixels). Adds 10% margin around the bbox.
    pub fn fit_to_canvas(&mut self, width: f64, height: f64) {
        let Some(bbox) = self.glyph_bbox() else {
            return;
        };
        let bw = bbox.width().max(1.0);
        let bh = bbox.height().max(1.0);
        let margin = 0.9;
        let zoom_x = width * margin / bw;
        let zoom_y = height * margin / bh;
        let zoom = zoom_x.min(zoom_y).max(1e-3);
        self.viewport.zoom = zoom;
        // Center bbox.center() in screen space. Screen y is flipped:
        //   screen.y = -design.y * zoom + offset.y
        let center = bbox.center();
        self.viewport.offset = Vec2::new(
            width / 2.0 - center.x * zoom,
            height / 2.0 + center.y * zoom,
        );
    }

    /// Translate every selected point by `delta` (in design space).
    pub fn translate_selection(&mut self, delta: Vec2) {
        if self.selection.is_empty() || delta == Vec2::ZERO {
            return;
        }
        for path in &mut self.paths {
            translate_in_path(path, &self.selection, delta);
        }
    }

    /// Build a fresh selection containing every point that lies
    /// inside `screen_rect` (a rectangle in screen-space pixels), then
    /// union with `base`. Used during box-select drags.
    pub fn select_in_screen_rect(
        &mut self,
        screen_rect: Rect,
        base: &Selection,
    ) {
        let mut next = base.clone();
        for path in &self.paths {
            for pt in path.points().iter() {
                let screen_pt = self.viewport.to_screen(pt.point);
                if rect_contains(&screen_rect, screen_pt) {
                    next.insert(pt.id);
                }
            }
        }
        self.selection = next;
    }

    /// Hit-test for a point near `design_pt` within `radius` design-
    /// space units. Returns the hit point's id if any.
    pub fn hit_test_point(
        &self,
        design_pt: Point,
        radius: f64,
    ) -> Option<EntityId> {
        use crate::editing::hit_test::find_closest;
        let candidates: Vec<_> = self
            .paths
            .iter()
            .flat_map(|p| {
                p.points()
                    .iter()
                    .map(|pt| (pt.id, pt.point, pt.is_on_curve()))
                    .collect::<Vec<_>>()
            })
            .collect();
        find_closest(design_pt, candidates.into_iter(), radius).map(|h| h.entity)
    }
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
            for pt in cubic.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    pt.point += delta;
                }
            }
        }
        Path::Quadratic(quadratic) => {
            for pt in quadratic.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    pt.point += delta;
                }
            }
        }
        Path::Hyper(hyper) => {
            let mut changed = false;
            for pt in hyper.points.make_mut().iter_mut() {
                if selection.contains(&pt.id) {
                    pt.point += delta;
                    changed = true;
                }
            }
            if changed {
                hyper.after_change();
            }
        }
    }
}

/// Inclusive containment — a point on the rectangle's edge counts as
/// inside. `Rect` itself is normalized so min/max are correct
/// regardless of which corner the user dragged from.
fn rect_contains(rect: &Rect, p: Point) -> bool {
    p.x >= rect.min_x() && p.x <= rect.max_x() && p.y >= rect.min_y() && p.y <= rect.max_y()
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

// Quiet unused-import lint on `workspace` while the only use is via
// the WsContour / WsContourPoint / WsPointType aliases above.
#[allow(dead_code)]
fn _suppress(_: workspace::Glyph) {}
