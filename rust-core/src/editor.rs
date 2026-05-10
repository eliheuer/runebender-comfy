// Editor state — the source of truth for what's being edited.
//
// Renderer reads from this; mouse/keyboard handlers mutate it. Lives
// outside the wasm-bindgen surface so it's testable on native too.

use kurbo::{BezPath, PathEl, Point};

use crate::editing::{Selection, ViewPort};
use crate::model::EntityId;
use crate::path::{CubicPath, Path, PathPoint, PathPoints, PointType};

/// In-memory state for one open glyph.
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    pub paths: Vec<Path>,
    pub selection: Selection,
    pub viewport: ViewPort,
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

        let mut current_points: Vec<PathPoint> = Vec::new();
        let mut last_on_curve: Option<Point> = None;

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
                    last_on_curve = Some(*p);
                }
                PathEl::LineTo(p) => {
                    current_points.push(on_curve(*p, false));
                    last_on_curve = Some(*p);
                }
                PathEl::QuadTo(c, p) => {
                    current_points.push(off_curve(*c));
                    current_points.push(on_curve(*p, true));
                    last_on_curve = Some(*p);
                }
                PathEl::CurveTo(c1, c2, p) => {
                    current_points.push(off_curve(*c1));
                    current_points.push(off_curve(*c2));
                    current_points.push(on_curve(*p, true));
                    last_on_curve = Some(*p);
                }
                PathEl::ClosePath => {
                    flush(&mut self.paths, &mut current_points, true);
                    last_on_curve = None;
                }
            }
        }
        flush(&mut self.paths, &mut current_points, false);
        let _ = last_on_curve;
    }

    /// Translate every selected point by `delta` (in design space).
    pub fn translate_selection(&mut self, delta: kurbo::Vec2) {
        if self.selection.is_empty() || delta == kurbo::Vec2::ZERO {
            return;
        }
        for path in &mut self.paths {
            translate_in_path(path, &self.selection, delta);
        }
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

fn translate_in_path(path: &mut Path, selection: &Selection, delta: kurbo::Vec2) {
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
