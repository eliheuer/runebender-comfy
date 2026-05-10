// Editor state — the source of truth for what's being edited.
//
// Renderer reads from this; mouse/keyboard handlers mutate it. Lives
// outside the wasm-bindgen surface so it's testable on native too.

use kurbo::{BezPath, PathEl, Point, Rect, Vec2};

use crate::editing::{Selection, ViewPort};
use crate::model::EntityId;
use crate::path::{CubicPath, Path, PathPoint, PathPoints, PointType};

/// In-memory state for one open glyph.
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    pub paths: Vec<Path>,
    pub selection: Selection,
    pub viewport: ViewPort,

    /// Transient screen-space rectangle for an in-progress
    /// box-selection drag. Renderer draws it as a marquee; cleared
    /// when the drag ends.
    pub marquee: Option<Rect>,
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
