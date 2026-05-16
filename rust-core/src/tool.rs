// Editing tools. This is intentionally smaller than xilem's full
// ToolBox, but it keeps the same tool ids and dispatch shape so the
// remaining tools can land one by one.

use kurbo::{
    CubicBez, Ellipse, Line, ParamCurve, ParamCurveArclen, PathEl, Point, Rect, Shape, Vec2,
};

use crate::editing::hit_test::MIN_CLICK_DISTANCE;
use crate::editing::{Drag, MouseDelegate, MouseEvent, Selection};
use crate::editor::{
    EditorState, KnifePreview, MeasurePreview, MeasureSegmentLabel, PenPreview,
    SegmentHoverPreview, ShapePreview,
};
use crate::model::entity_id::EntityId;
use crate::path::{
    CubicPath, Path, PathPoint, PathPoints, PointType, QuadraticPath, Segment, SegmentInfo,
};

const MAX_KNIFE_RECURSE: usize = 16;
const MEASURE_FUZZY_TOLERANCE: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

pub enum ActiveTool {
    Select(SelectTool),
    Pen(PenTool),
    HyperPen(HyperPenTool),
    Knife(KnifeTool),
    Measure(MeasureTool),
    Preview(PreviewTool),
    Shapes(ShapesTool),
    Text(TextTool),
    Inert,
}

impl Default for ActiveTool {
    fn default() -> Self {
        ActiveTool::Select(SelectTool::default())
    }
}

impl ActiveTool {
    pub fn set_tool(&mut self, id: &str) {
        *self = match id {
            "Select" => ActiveTool::Select(SelectTool::default()),
            "Pen" => ActiveTool::Pen(PenTool::default()),
            "HyperPen" => ActiveTool::HyperPen(HyperPenTool::default()),
            "Knife" => ActiveTool::Knife(KnifeTool::default()),
            "Measure" => ActiveTool::Measure(MeasureTool::default()),
            "Preview" => ActiveTool::Preview(PreviewTool::default()),
            "Shapes" => ActiveTool::Shapes(ShapesTool::default()),
            "Text" => ActiveTool::Text(TextTool::default()),
            _ => ActiveTool::Inert,
        };
    }

    pub fn set_shape_kind(&mut self, kind: ShapeKind) {
        match self {
            ActiveTool::Shapes(tool) => tool.shape_kind = kind,
            _ => {
                *self = ActiveTool::Shapes(ShapesTool {
                    shape_kind: kind,
                    ..Default::default()
                });
            }
        }
    }

    pub fn set_shape_shift_locked(&mut self, locked: bool, state: &mut EditorState) -> bool {
        match self {
            ActiveTool::Shapes(tool) => tool.set_shift_locked(locked, state),
            _ => false,
        }
    }

    pub fn set_knife_shift_locked(&mut self, locked: bool, state: &mut EditorState) -> bool {
        match self {
            ActiveTool::Knife(tool) => tool.set_shift_locked(locked, state),
            _ => false,
        }
    }

    pub fn is_preview(&self) -> bool {
        matches!(self, ActiveTool::Preview(_))
    }

    pub fn is_text(&self) -> bool {
        matches!(self, ActiveTool::Text(_))
    }
}

impl MouseDelegate for ActiveTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.left_down(event, state),
            ActiveTool::Pen(tool) => tool.left_down(event, state),
            ActiveTool::HyperPen(tool) => tool.left_down(event, state),
            ActiveTool::Knife(tool) => tool.left_down(event, state),
            ActiveTool::Measure(tool) => tool.left_down(event, state),
            ActiveTool::Preview(tool) => tool.left_down(event, state),
            ActiveTool::Shapes(tool) => tool.left_down(event, state),
            ActiveTool::Text(tool) => tool.left_down(event, state),
            ActiveTool::Inert => {}
        }
    }

    fn left_drag_began(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        match self {
            ActiveTool::Pen(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::HyperPen(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Knife(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Measure(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Preview(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Shapes(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Text(tool) => tool.left_drag_began(event, drag, state),
            ActiveTool::Select(_) | ActiveTool::Inert => {}
        }
    }

    fn left_drag_changed(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Pen(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::HyperPen(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Knife(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Measure(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Preview(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Shapes(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Text(tool) => tool.left_drag_changed(event, drag, state),
            ActiveTool::Inert => {}
        }
    }

    fn left_drag_ended(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Pen(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::HyperPen(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Knife(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Measure(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Preview(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Shapes(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Text(tool) => tool.left_drag_ended(event, drag, state),
            ActiveTool::Inert => {}
        }
    }

    fn left_up(&mut self, event: MouseEvent, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.left_up(event, state),
            ActiveTool::Pen(tool) => tool.left_up(event, state),
            ActiveTool::HyperPen(tool) => tool.left_up(event, state),
            ActiveTool::Knife(tool) => tool.left_up(event, state),
            ActiveTool::Measure(tool) => tool.left_up(event, state),
            ActiveTool::Preview(tool) => tool.left_up(event, state),
            ActiveTool::Shapes(tool) => tool.left_up(event, state),
            ActiveTool::Text(tool) => tool.left_up(event, state),
            ActiveTool::Inert => {}
        }
    }

    fn left_click(&mut self, event: MouseEvent, state: &mut Self::Data) {
        match self {
            ActiveTool::Pen(tool) => tool.left_click(event, state),
            ActiveTool::HyperPen(tool) => tool.left_click(event, state),
            ActiveTool::Text(tool) => tool.left_click(event, state),
            ActiveTool::Select(_)
            | ActiveTool::Knife(_)
            | ActiveTool::Measure(_)
            | ActiveTool::Preview(_)
            | ActiveTool::Shapes(_)
            | ActiveTool::Inert => {}
        }
    }

    fn other_drag_changed(&mut self, _event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let delta = drag.current - drag.prev;
        state.viewport.offset += delta;
    }

    fn mouse_moved(&mut self, event: MouseEvent, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.mouse_moved(event, state),
            ActiveTool::Pen(tool) => tool.mouse_moved(event, state),
            ActiveTool::HyperPen(tool) => tool.mouse_moved(event, state),
            ActiveTool::Knife(_)
            | ActiveTool::Measure(_)
            | ActiveTool::Preview(_)
            | ActiveTool::Shapes(_)
            | ActiveTool::Text(_)
            | ActiveTool::Inert => {}
        }
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        match self {
            ActiveTool::Select(tool) => tool.cancel(state),
            ActiveTool::Pen(tool) => tool.cancel(state),
            ActiveTool::HyperPen(tool) => tool.cancel(state),
            ActiveTool::Knife(tool) => tool.cancel(state),
            ActiveTool::Measure(tool) => tool.cancel(state),
            ActiveTool::Preview(tool) => tool.cancel(state),
            ActiveTool::Shapes(tool) => tool.cancel(state),
            ActiveTool::Text(tool) => tool.cancel(state),
            ActiveTool::Inert => {}
        }
    }
}

#[derive(Debug, Clone)]
enum SelectDragKind {
    None,
    Translate,
    ComponentTranslate,
    BoxSelect { initial: Selection },
    Pan,
}

#[derive(Default)]
pub struct SelectTool {
    drag_kind: Option<SelectDragKind>,
}

impl MouseDelegate for SelectTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        let design_pt = state.screen_to_glyph_design(event.pos);
        let hit_radius = MIN_CLICK_DISTANCE / state.viewport.zoom.max(1e-6);
        let hit = state.hit_test_point(design_pt, hit_radius);
        let component_hit = state.hit_test_component(design_pt);

        if event.mods.alt {
            if state.convert_line_segment_at_point(design_pt, hit_radius) {
                self.drag_kind = Some(SelectDragKind::None);
                state.segment_hover = None;
                return;
            }
            self.drag_kind = Some(SelectDragKind::Pan);
            return;
        }

        self.drag_kind = Some(match (hit, event.mods.shift) {
            (Some(id), false) => {
                state.clear_component_selection();
                if !state.selection.contains(&id) {
                    let mut sel = Selection::new();
                    sel.insert(id);
                    state.selection = sel;
                }
                SelectDragKind::Translate
            }
            (Some(id), true) => {
                if state.selection.contains(&id) {
                    state.selection.remove(&id);
                    SelectDragKind::None
                } else {
                    state.selection.insert(id);
                    SelectDragKind::Translate
                }
            }
            (None, _) if component_hit.is_some() => {
                state.select_component(component_hit.unwrap());
                SelectDragKind::ComponentTranslate
            }
            (None, false) => {
                state.selection = Selection::new();
                state.clear_component_selection();
                SelectDragKind::BoxSelect {
                    initial: Selection::new(),
                }
            }
            (None, true) => SelectDragKind::BoxSelect {
                initial: state.selection.clone(),
            },
        });
    }

    fn left_drag_changed(&mut self, _event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let Some(kind) = self.drag_kind.clone() else {
            return;
        };
        match kind {
            SelectDragKind::Translate => {
                let screen_delta = drag.current - drag.prev;
                let design_delta = screen_to_design_delta(state, screen_delta);
                state.translate_selection(design_delta);
            }
            SelectDragKind::ComponentTranslate => {
                let screen_delta = drag.current - drag.prev;
                let design_delta = screen_to_design_delta(state, screen_delta);
                state.translate_selected_component(design_delta);
            }
            SelectDragKind::BoxSelect { initial } => {
                let rect = Rect::from_points(drag.start, drag.current);
                state.marquee = Some(rect);
                state.select_in_screen_rect(rect, &initial);
            }
            SelectDragKind::Pan => {
                let delta = drag.current - drag.prev;
                state.viewport.offset += delta;
            }
            SelectDragKind::None => {}
        }
    }

    fn mouse_moved(&mut self, event: MouseEvent, state: &mut Self::Data) {
        if !event.mods.alt {
            state.segment_hover = None;
            return;
        }
        let design_pt = state.screen_to_glyph_design(event.pos);
        let hit_radius = MIN_CLICK_DISTANCE / state.viewport.zoom.max(1e-6);
        let Some(segment_info) = state.nearest_segment(design_pt, hit_radius) else {
            state.segment_hover = None;
            return;
        };
        state.segment_hover = Some(match segment_info.segment {
            Segment::Line(line) => SegmentHoverPreview::Line(screen_line(state, line)),
            Segment::Cubic(cubic) => SegmentHoverPreview::Cubic(screen_cubic(state, cubic)),
            Segment::Quadratic(quad) => {
                SegmentHoverPreview::Quadratic(screen_quadratic(state, quad))
            }
        });
    }

    fn left_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        state.marquee = None;
        state.segment_hover = None;
        self.drag_kind = None;
    }

    fn left_up(&mut self, _event: MouseEvent, _state: &mut Self::Data) {
        self.drag_kind = None;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        state.marquee = None;
        state.segment_hover = None;
        self.drag_kind = None;
    }
}

fn screen_line(state: &EditorState, line: Line) -> Line {
    Line::new(
        state.glyph_to_screen(line.p0),
        state.glyph_to_screen(line.p1),
    )
}

fn screen_cubic(state: &EditorState, cubic: CubicBez) -> CubicBez {
    CubicBez::new(
        state.glyph_to_screen(cubic.p0),
        state.glyph_to_screen(cubic.p1),
        state.glyph_to_screen(cubic.p2),
        state.glyph_to_screen(cubic.p3),
    )
}

fn screen_quadratic(state: &EditorState, quad: kurbo::QuadBez) -> kurbo::QuadBez {
    kurbo::QuadBez::new(
        state.glyph_to_screen(quad.p0),
        state.glyph_to_screen(quad.p1),
        state.glyph_to_screen(quad.p2),
    )
}

#[derive(Default)]
pub struct PenTool {
    current_path: Option<usize>,
    drag_origin: Option<Point>,
    dragging_handles: bool,
}

impl MouseDelegate for PenTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        self.drag_origin = Some(state.screen_to_glyph_design(event.pos));
        self.dragging_handles = false;
    }

    fn left_click(&mut self, event: MouseEvent, state: &mut Self::Data) {
        if self.dragging_handles {
            self.dragging_handles = false;
            self.drag_origin = None;
            return;
        }

        let design_pos = state.screen_to_glyph_design(event.pos);
        let snap_radius = 10.0 / state.viewport.zoom.max(1e-6);

        if self.current_path.is_none()
            && state.insert_point_on_nearest_segment(design_pos, snap_radius)
        {
            state.pen_preview = None;
            self.drag_origin = None;
            return;
        }

        if let Some(path_index) = self.current_path {
            if self.should_close(path_index, design_pos, state) {
                if state.close_pen_path(path_index) {
                    self.current_path = None;
                    state.pen_preview = None;
                }
                return;
            }

            if state.append_pen_point(path_index, design_pos).is_some() {
                return;
            }
        }

        self.current_path = Some(state.start_pen_path(design_pos));
        self.drag_origin = None;
    }

    fn left_drag_began(&mut self, event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        let origin = self
            .drag_origin
            .unwrap_or_else(|| state.screen_to_glyph_design(event.pos));
        let snap_radius = 10.0 / state.viewport.zoom.max(1e-6);
        if self.current_path.is_none()
            && state.nearest_segment_with_t(origin, snap_radius).is_some()
        {
            return;
        }
        if let Some(path_index) = self.current_path
            && self.should_close(path_index, origin, state)
        {
            return;
        }
        let handle_out = state.screen_to_glyph_design(event.pos);
        let path_index = state.append_smooth_pen_point(self.current_path, origin, handle_out);
        self.current_path = Some(path_index);
        self.dragging_handles = true;
    }

    fn left_drag_changed(&mut self, event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        if !self.dragging_handles {
            self.update_preview(event, state);
            return;
        }
        let Some(origin) = self.drag_origin else {
            return;
        };
        let Some(path_index) = self.current_path else {
            return;
        };
        let handle_out = state.screen_to_glyph_design(event.pos);
        state.update_last_smooth_pen_handles(path_index, origin, handle_out);
    }

    fn left_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        state.pen_preview = None;
        self.drag_origin = None;
        self.dragging_handles = false;
    }

    fn mouse_moved(&mut self, event: MouseEvent, state: &mut Self::Data) {
        self.update_preview(event, state);
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        if let Some(path_index) = self.current_path
            && state.pen_path_len(path_index).is_some_and(|len| len < 2)
        {
            state.remove_tool_path(path_index);
        }
        self.current_path = None;
        self.drag_origin = None;
        self.dragging_handles = false;
        state.pen_preview = None;
    }
}

impl PenTool {
    fn update_preview(&self, event: MouseEvent, state: &mut EditorState) {
        let cursor_design = state.screen_to_glyph_design(event.pos);
        let line_start = self
            .current_path
            .and_then(|path_index| state.pen_path_last_on_curve(path_index))
            .map(|pt| state.glyph_to_screen(pt));
        let close_target = self.current_path.and_then(|path_index| {
            if self.should_close(path_index, cursor_design, state) {
                state
                    .pen_path_start(path_index)
                    .map(|pt| state.glyph_to_screen(pt))
            } else {
                None
            }
        });
        let snap_target = snap_segment_target(self.current_path, cursor_design, state);
        let cursor = snap_target.unwrap_or(event.pos);
        state.pen_preview = Some(PenPreview {
            line_start,
            cursor,
            close_target,
            snap_target,
        });
    }

    fn should_close(&self, path_index: usize, point: Point, state: &EditorState) -> bool {
        let Some(len) = state.pen_path_len(path_index) else {
            return false;
        };
        if len < 3 {
            return false;
        }
        let Some(start) = state.pen_path_start(path_index) else {
            return false;
        };
        start.distance(point) <= 20.0
    }
}

#[derive(Default)]
pub struct HyperPenTool {
    current_path: Option<usize>,
}

impl MouseDelegate for HyperPenTool {
    type Data = EditorState;

    fn left_click(&mut self, event: MouseEvent, state: &mut Self::Data) {
        let design_pos = state.screen_to_glyph_design(event.pos);
        let snap_radius = 10.0 / state.viewport.zoom.max(1e-6);

        if self.current_path.is_none()
            && state.insert_point_on_nearest_segment(design_pos, snap_radius)
        {
            state.pen_preview = None;
            return;
        }

        if let Some(path_index) = self.current_path {
            if self.should_close(path_index, design_pos, state) {
                if state.close_hyper_path(path_index) {
                    self.current_path = None;
                    state.pen_preview = None;
                }
                return;
            }

            if state.append_hyper_point(path_index, design_pos).is_some() {
                return;
            }
        }

        self.current_path = Some(state.start_hyper_path(design_pos));
    }

    fn mouse_moved(&mut self, event: MouseEvent, state: &mut Self::Data) {
        self.update_preview(event, state);
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        self.current_path = None;
        state.pen_preview = None;
    }
}

impl HyperPenTool {
    fn update_preview(&self, event: MouseEvent, state: &mut EditorState) {
        let cursor_design = state.screen_to_glyph_design(event.pos);
        let line_start = self
            .current_path
            .and_then(|path_index| state.hyper_path_last_point(path_index))
            .map(|pt| state.glyph_to_screen(pt));
        let close_target = self.current_path.and_then(|path_index| {
            if self.should_close(path_index, cursor_design, state) {
                state
                    .hyper_path_start(path_index)
                    .map(|pt| state.glyph_to_screen(pt))
            } else {
                None
            }
        });
        let snap_target = snap_segment_target(self.current_path, cursor_design, state);
        let cursor = snap_target.unwrap_or(event.pos);
        state.pen_preview = Some(PenPreview {
            line_start,
            cursor,
            close_target,
            snap_target,
        });
    }

    fn should_close(&self, path_index: usize, point: Point, state: &EditorState) -> bool {
        let Some(len) = state.hyper_path_len(path_index) else {
            return false;
        };
        if len < 3 {
            return false;
        }
        let Some(start) = state.hyper_path_start(path_index) else {
            return false;
        };
        start.distance(point) <= 20.0
    }
}

fn snap_segment_target(
    current_path: Option<usize>,
    cursor_design: Point,
    state: &EditorState,
) -> Option<Point> {
    if current_path.is_some() {
        return None;
    }
    let snap_radius = 10.0 / state.viewport.zoom.max(1e-6);
    state
        .nearest_segment_with_t(cursor_design, snap_radius)
        .map(|(segment_info, t)| {
            let target = segment_info.segment.eval(t);
            let target = match segment_info.segment {
                Segment::Line(_) => snap_point_to_grid(target),
                Segment::Cubic(_) | Segment::Quadratic(_) => target,
            };
            state.glyph_to_screen(target)
        })
}

fn snap_point_to_grid(point: Point) -> Point {
    let spacing = 2.0;
    Point::new(
        (point.x / spacing).round() * spacing,
        (point.y / spacing).round() * spacing,
    )
}

#[derive(Default)]
pub struct KnifeTool {
    drag_start: Option<Point>,
    drag_current: Option<Point>,
    shift_locked: bool,
}

impl MouseDelegate for KnifeTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        self.drag_start = Some(event.pos);
        self.drag_current = Some(event.pos);
        self.shift_locked = event.mods.shift;
        state.knife_preview = Some(knife_preview(event.pos, event.pos, state));
    }

    fn left_drag_began(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        if self.drag_start.is_none() {
            self.drag_start = Some(drag.start);
        }
        let start = self.drag_start.unwrap_or(drag.start);
        self.drag_current = Some(event.pos);
        self.shift_locked = event.mods.shift;
        let end = constrain_measure_end(start, event.pos, self.shift_locked);
        state.knife_preview = Some(knife_preview(start, end, state));
    }

    fn left_drag_changed(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let start = self.drag_start.unwrap_or(drag.start);
        self.drag_current = Some(event.pos);
        let end = constrain_measure_end(start, event.pos, self.shift_locked);
        state.knife_preview = Some(knife_preview(start, end, state));
    }

    fn left_drag_ended(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let start = self.drag_start.unwrap_or(drag.start);
        self.drag_current = Some(event.pos);
        let current = self.drag_current.unwrap_or(drag.current);
        let end = constrain_measure_end(start, current, self.shift_locked);
        state.knife_preview = Some(knife_preview(start, end, state));
        let design_line = Line::new(
            state.screen_to_glyph_design(start),
            state.screen_to_glyph_design(end),
        );
        if design_line.p0.distance(design_line.p1) > 1e-6 {
            let sliced = slice_paths(&state.paths, design_line);
            if sliced.len() != state.paths.len() {
                state.replace_paths_from_tool(sliced);
            }
        }
        self.drag_start = None;
        self.drag_current = None;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        self.drag_start = None;
        self.drag_current = None;
        state.knife_preview = None;
    }
}

impl KnifeTool {
    fn set_shift_locked(&mut self, locked: bool, state: &mut EditorState) -> bool {
        if self.shift_locked == locked {
            return false;
        }
        self.shift_locked = locked;
        if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
            let end = constrain_measure_end(start, current, self.shift_locked);
            state.knife_preview = Some(knife_preview(start, end, state));
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    line_t: f64,
    segment_t: f64,
    point: Point,
    segment_info: SegmentInfo,
}

fn knife_preview(start: Point, end: Point, state: &EditorState) -> KnifePreview {
    let design_line = Line::new(
        state.screen_to_glyph_design(start),
        state.screen_to_glyph_design(end),
    );
    let line_ts = cluster_line_ts(design_line, path_intersection_ts(state, design_line));
    let intersections = line_ts
        .into_iter()
        .map(|t| state.glyph_to_screen(design_line.eval(t)))
        .collect();
    KnifePreview {
        line: Line::new(start, end),
        intersections,
    }
}

fn slice_paths(paths: &[Path], line: Line) -> Vec<Path> {
    let mut out = Vec::new();
    for path in paths {
        match path {
            Path::Cubic(cubic_path) => {
                slice_path(cubic_path, line, &mut out);
            }
            Path::Quadratic(quadratic_path) => {
                slice_quadratic_path(quadratic_path, line, &mut out);
            }
            Path::Hyper(hyper_path) => {
                slice_path(&hyper_path.to_cubic(), line, &mut out);
            }
        }
    }
    out
}

fn slice_path(path: &CubicPath, line: Line, acc: &mut Vec<Path>) {
    let mut hits = Vec::new();
    slice_path_impl(path.clone(), line, acc, &mut hits, 0);
}

fn slice_quadratic_path(path: &QuadraticPath, line: Line, acc: &mut Vec<Path>) {
    let mut hits = Vec::new();
    slice_quadratic_path_impl(path.clone(), line, acc, &mut hits, 0);
}

fn slice_quadratic_path_impl(
    path: QuadraticPath,
    line: Line,
    acc: &mut Vec<Path>,
    hit_buf: &mut Vec<Hit>,
    recurse: usize,
) {
    hit_buf.clear();
    for segment in path.iter_segments() {
        for (segment_t, line_t) in intersect_line_segment(line, &segment.segment) {
            hit_buf.push(Hit {
                line_t,
                segment_t,
                point: line.eval(line_t),
                segment_info: segment,
            });
        }
    }

    if hit_buf.len() <= 1 || recurse == MAX_KNIFE_RECURSE {
        acc.push(Path::Quadratic(path));
        return;
    }

    hit_buf.sort_by(|a, b| a.line_t.total_cmp(&b.line_t));
    hit_buf.dedup_by(|a, b| {
        (a.line_t - b.line_t).abs() < 1e-6
            && a.segment_info.start_index == b.segment_info.start_index
            && (a.segment_t - b.segment_t).abs() < 1e-6
    });

    if hit_buf.len() <= 1 {
        acc.push(Path::Quadratic(path));
        return;
    }

    let start = hit_buf[0];
    let end = hit_buf[1];
    let slice_ep = 1.0 / line.arclen(1e-6).max(1.0);
    let next_line_start_t = (end.line_t + slice_ep).min(1.0);
    let (start, end) = order_quadratic_points(&path, start, end);
    let (path_one, path_two) = split_quadratic_path_at_intersections(&path, start, end);

    if next_line_start_t >= 1.0 {
        acc.push(Path::Quadratic(path_one));
        acc.push(Path::Quadratic(path_two));
        return;
    }

    let remaining_line = line_subsegment(line, next_line_start_t, 1.0);
    slice_quadratic_path_impl(path_one, remaining_line, acc, hit_buf, recurse + 1);
    slice_quadratic_path_impl(path_two, remaining_line, acc, hit_buf, recurse + 1);
}

fn slice_path_impl(
    path: CubicPath,
    line: Line,
    acc: &mut Vec<Path>,
    hit_buf: &mut Vec<Hit>,
    recurse: usize,
) {
    hit_buf.clear();
    for segment in path.iter_segments() {
        for (segment_t, line_t) in intersect_line_segment(line, &segment.segment) {
            hit_buf.push(Hit {
                line_t,
                segment_t,
                point: line.eval(line_t),
                segment_info: segment,
            });
        }
    }

    if hit_buf.len() <= 1 || recurse == MAX_KNIFE_RECURSE {
        acc.push(Path::Cubic(path));
        return;
    }

    hit_buf.sort_by(|a, b| a.line_t.total_cmp(&b.line_t));
    hit_buf.dedup_by(|a, b| {
        (a.line_t - b.line_t).abs() < 1e-6
            && a.segment_info.start_index == b.segment_info.start_index
            && (a.segment_t - b.segment_t).abs() < 1e-6
    });

    if hit_buf.len() <= 1 {
        acc.push(Path::Cubic(path));
        return;
    }

    let start = hit_buf[0];
    let end = hit_buf[1];
    let slice_ep = 1.0 / line.arclen(1e-6).max(1.0);
    let next_line_start_t = (end.line_t + slice_ep).min(1.0);
    let (start, end) = order_points(&path, start, end);
    let (path_one, path_two) = split_path_at_intersections(&path, start, end);

    if next_line_start_t >= 1.0 {
        acc.push(Path::Cubic(path_one));
        acc.push(Path::Cubic(path_two));
        return;
    }

    let remaining_line = line_subsegment(line, next_line_start_t, 1.0);
    slice_path_impl(path_one, remaining_line, acc, hit_buf, recurse + 1);
    slice_path_impl(path_two, remaining_line, acc, hit_buf, recurse + 1);
}

fn order_points(path: &CubicPath, start: Hit, end: Hit) -> (Hit, Hit) {
    for segment in path.iter_segments() {
        if segment.start_index == start.segment_info.start_index {
            if segment.start_index == end.segment_info.start_index
                && end.segment_t < start.segment_t
            {
                return (end, start);
            }
            return (start, end);
        } else if segment.start_index == end.segment_info.start_index {
            return (end, start);
        }
    }
    (start, end)
}

fn order_quadratic_points(path: &QuadraticPath, start: Hit, end: Hit) -> (Hit, Hit) {
    for segment in path.iter_segments() {
        if segment.start_index == start.segment_info.start_index {
            if segment.start_index == end.segment_info.start_index
                && end.segment_t < start.segment_t
            {
                return (end, start);
            }
            return (start, end);
        } else if segment.start_index == end.segment_info.start_index {
            return (end, start);
        }
    }
    (start, end)
}

fn split_path_at_intersections(path: &CubicPath, start: Hit, end: Hit) -> (CubicPath, CubicPath) {
    let mut one_points = Vec::new();
    let mut two_points = Vec::new();
    let mut two_is_done = false;

    let points = path.points.to_vec();
    let segments = path.iter_segments().collect::<Vec<_>>();

    for segment in &segments {
        if segment.start_index != start.segment_info.start_index {
            append_segment_points(&mut one_points, &points, segment);
        } else {
            append_subsegment_points(&mut one_points, &points, segment, 0.0, start.segment_t);

            if segment.start_index == end.segment_info.start_index {
                append_subsegment_points(&mut one_points, &points, segment, end.segment_t, 1.0);
                append_subsegment_points(
                    &mut two_points,
                    &points,
                    segment,
                    start.segment_t,
                    end.segment_t,
                );
                two_is_done = true;
            } else {
                append_subsegment_points(&mut two_points, &points, segment, start.segment_t, 1.0);
            }

            if !path.closed {
                two_points.push(PathPoint {
                    id: EntityId::next(),
                    point: start.point,
                    typ: PointType::OnCurve { smooth: false },
                });
            }
            break;
        }
    }

    let mut found_start = false;
    for segment in &segments {
        if segment.start_index == start.segment_info.start_index {
            found_start = true;
            continue;
        }
        if !found_start {
            continue;
        }

        if segment.start_index == end.segment_info.start_index {
            append_subsegment_points(&mut one_points, &points, segment, end.segment_t, 1.0);
            if !two_is_done {
                append_subsegment_points(&mut two_points, &points, segment, 0.0, end.segment_t);
            }
            break;
        } else if !two_is_done {
            append_segment_points(&mut two_points, &points, segment);
        }
    }

    let mut found_end = false;
    for segment in &segments {
        if segment.start_index == end.segment_info.start_index {
            found_end = true;
            continue;
        }
        if found_end {
            append_segment_points(&mut one_points, &points, segment);
        }
    }

    if one_points.first().map(|p| p.point) == one_points.last().map(|p| p.point)
        && one_points.len() > 1
    {
        one_points.pop();
    }

    (
        CubicPath::new(PathPoints::from_vec(one_points), path.closed),
        CubicPath::new(PathPoints::from_vec(two_points), true),
    )
}

fn split_quadratic_path_at_intersections(
    path: &QuadraticPath,
    start: Hit,
    end: Hit,
) -> (QuadraticPath, QuadraticPath) {
    let mut one_points = Vec::new();
    let mut two_points = Vec::new();
    let mut two_is_done = false;

    let points = path.points.to_vec();
    let segments = path.iter_segments().collect::<Vec<_>>();

    for segment in &segments {
        if segment.start_index != start.segment_info.start_index {
            append_segment_points(&mut one_points, &points, segment);
        } else {
            append_quadratic_subsegment_points(
                &mut one_points,
                &points,
                segment,
                0.0,
                start.segment_t,
            );

            if segment.start_index == end.segment_info.start_index {
                append_quadratic_subsegment_points(
                    &mut one_points,
                    &points,
                    segment,
                    end.segment_t,
                    1.0,
                );
                append_quadratic_subsegment_points(
                    &mut two_points,
                    &points,
                    segment,
                    start.segment_t,
                    end.segment_t,
                );
                two_is_done = true;
            } else {
                append_quadratic_subsegment_points(
                    &mut two_points,
                    &points,
                    segment,
                    start.segment_t,
                    1.0,
                );
            }

            if !path.closed {
                two_points.push(PathPoint {
                    id: EntityId::next(),
                    point: start.point,
                    typ: PointType::OnCurve { smooth: false },
                });
            }
            break;
        }
    }

    let mut found_start = false;
    for segment in &segments {
        if segment.start_index == start.segment_info.start_index {
            found_start = true;
            continue;
        }
        if !found_start {
            continue;
        }

        if segment.start_index == end.segment_info.start_index {
            append_quadratic_subsegment_points(
                &mut one_points,
                &points,
                segment,
                end.segment_t,
                1.0,
            );
            if !two_is_done {
                append_quadratic_subsegment_points(
                    &mut two_points,
                    &points,
                    segment,
                    0.0,
                    end.segment_t,
                );
            }
            break;
        } else if !two_is_done {
            append_segment_points(&mut two_points, &points, segment);
        }
    }

    let mut found_end = false;
    for segment in &segments {
        if segment.start_index == end.segment_info.start_index {
            found_end = true;
            continue;
        }
        if found_end {
            append_segment_points(&mut one_points, &points, segment);
        }
    }

    if one_points.first().map(|p| p.point) == one_points.last().map(|p| p.point)
        && one_points.len() > 1
    {
        one_points.pop();
    }

    (
        QuadraticPath::new(PathPoints::from_vec(one_points), path.closed),
        QuadraticPath::new(PathPoints::from_vec(two_points), true),
    )
}

fn append_segment_points(dest: &mut Vec<PathPoint>, points: &[PathPoint], segment: &SegmentInfo) {
    let start = segment.start_index;
    let end = segment.end_index;

    if end <= start {
        let start_typ = points[start].typ;
        match segment.segment {
            Segment::Cubic(cubic) => {
                push_path_point(dest, cubic.p0, start_typ);
                push_path_point(dest, cubic.p1, PointType::OffCurve { auto: false });
                push_path_point(dest, cubic.p2, PointType::OffCurve { auto: false });
                return;
            }
            Segment::Line(line) => {
                push_path_point(dest, line.p0, start_typ);
                return;
            }
            Segment::Quadratic(quad) => {
                push_path_point(dest, quad.p0, start_typ);
                push_path_point(dest, quad.p1, PointType::OffCurve { auto: false });
                return;
            }
        }
    }

    push_path_point(dest, points[start].point, points[start].typ);
    for point in points.iter().take(end).skip(start + 1) {
        push_path_point(dest, point.point, point.typ);
    }
    if end < points.len() && end != start {
        push_path_point(dest, points[end].point, points[end].typ);
    }
}

fn append_subsegment_points(
    dest: &mut Vec<PathPoint>,
    points: &[PathPoint],
    segment: &SegmentInfo,
    t_start: f64,
    t_end: f64,
) {
    if t_start >= t_end {
        return;
    }

    const T_EPS: f64 = 1e-9;
    let start_typ = if t_start < T_EPS {
        points[segment.start_index].typ
    } else {
        PointType::OnCurve { smooth: false }
    };
    let end_typ = if t_end > 1.0 - T_EPS {
        points[segment.end_index].typ
    } else {
        PointType::OnCurve { smooth: false }
    };

    match segment.segment {
        Segment::Line(line) => {
            push_path_point(dest, line.eval(t_start), start_typ);
            push_path_point(dest, line.eval(t_end), end_typ);
        }
        Segment::Cubic(cubic) => {
            let sub = cubic_subsegment(cubic, t_start, t_end);
            push_path_point(dest, sub.p0, start_typ);
            push_path_point(dest, sub.p1, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p2, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p3, end_typ);
        }
        Segment::Quadratic(quad) => {
            let sub = cubic_subsegment(quad.raise(), t_start, t_end);
            push_path_point(dest, sub.p0, start_typ);
            push_path_point(dest, sub.p1, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p2, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p3, end_typ);
        }
    }
}

fn append_quadratic_subsegment_points(
    dest: &mut Vec<PathPoint>,
    points: &[PathPoint],
    segment: &SegmentInfo,
    t_start: f64,
    t_end: f64,
) {
    if t_start >= t_end {
        return;
    }

    const T_EPS: f64 = 1e-9;
    let start_typ = if t_start < T_EPS {
        points[segment.start_index].typ
    } else {
        PointType::OnCurve { smooth: false }
    };
    let end_typ = if t_end > 1.0 - T_EPS {
        points[segment.end_index].typ
    } else {
        PointType::OnCurve { smooth: false }
    };

    match segment.segment {
        Segment::Line(line) => {
            push_path_point(dest, line.eval(t_start), start_typ);
            push_path_point(dest, line.eval(t_end), end_typ);
        }
        Segment::Quadratic(quad) => {
            let sub = quadratic_subsegment(quad, t_start, t_end);
            push_path_point(dest, sub.p0, start_typ);
            push_path_point(dest, sub.p1, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p2, end_typ);
        }
        Segment::Cubic(cubic) => {
            let sub = cubic_subsegment(cubic, t_start, t_end);
            push_path_point(dest, sub.p0, start_typ);
            push_path_point(dest, sub.p1, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p2, PointType::OffCurve { auto: false });
            push_path_point(dest, sub.p3, end_typ);
        }
    }
}

fn push_path_point(dest: &mut Vec<PathPoint>, point: Point, typ: PointType) {
    if dest.last().map(|pt| pt.point) == Some(point) {
        return;
    }
    dest.push(PathPoint {
        id: EntityId::next(),
        point,
        typ,
    });
}

fn line_subsegment(line: Line, t_start: f64, t_end: f64) -> Line {
    Line::new(line.eval(t_start), line.eval(t_end))
}

fn cubic_subsegment(cubic: CubicBez, t_start: f64, t_end: f64) -> CubicBez {
    let (_, right) = Segment::subdivide_cubic(cubic, t_start);
    let adjusted_t = if t_start < 1.0 {
        (t_end - t_start) / (1.0 - t_start)
    } else {
        1.0
    };
    let (left, _) = Segment::subdivide_cubic(right, adjusted_t.min(1.0));
    left
}

fn quadratic_subsegment(quad: kurbo::QuadBez, t_start: f64, t_end: f64) -> kurbo::QuadBez {
    let (_, right) = Segment::subdivide_quadratic(quad, t_start);
    let adjusted_t = if t_start < 1.0 {
        (t_end - t_start) / (1.0 - t_start)
    } else {
        1.0
    };
    let (left, _) = Segment::subdivide_quadratic(right, adjusted_t.min(1.0));
    left
}

#[derive(Default)]
pub struct TextTool {
    suppress_next_click: bool,
}

impl MouseDelegate for TextTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        if !event.mods.shift {
            return;
        }

        let design_pos = state.viewport.screen_to_design(event.pos);
        let line_height = state.text_line_height();
        let hit = state
            .text_buffer
            .hit_test(design_pos.x, design_pos.y, line_height);
        let Some(index) = hit.active_sort else {
            return;
        };

        if state.text_buffer.begin_manual_kerning(index, design_pos.x) {
            self.suppress_next_click = true;
        }
    }

    fn left_click(&mut self, event: MouseEvent, state: &mut Self::Data) {
        if self.suppress_next_click {
            self.suppress_next_click = false;
            return;
        }

        let design_pos = state.viewport.screen_to_design(event.pos);
        let line_height = state.text_line_height();
        let hit = state
            .text_buffer
            .hit_test(design_pos.x, design_pos.y, line_height);
        if let Some(index) = hit.active_sort {
            state.text_buffer.activate_sort(index);
        } else {
            state.text_buffer.set_cursor(hit.cursor);
        }
    }

    fn left_drag_began(&mut self, event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        if state.text_buffer.manual_kerning_sort().is_none() {
            return;
        }
        let design_pos = state.viewport.screen_to_design(event.pos);
        state.text_buffer.drag_manual_kerning(design_pos.x);
    }

    fn left_drag_changed(&mut self, event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        if state.text_buffer.manual_kerning_sort().is_none() {
            return;
        }
        let design_pos = state.viewport.screen_to_design(event.pos);
        state.text_buffer.drag_manual_kerning(design_pos.x);
    }

    fn left_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, state: &mut Self::Data) {
        state.text_buffer.end_manual_kerning();
        self.suppress_next_click = false;
    }

    fn left_up(&mut self, _event: MouseEvent, state: &mut Self::Data) {
        state.text_buffer.end_manual_kerning();
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        state.text_buffer.end_manual_kerning();
        self.suppress_next_click = false;
    }
}

#[derive(Default)]
pub struct MeasureTool {
    drag_start: Option<Point>,
}

impl MouseDelegate for MeasureTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        self.drag_start = Some(event.pos);
        state.measure_preview = Some(measure_preview(event.pos, event.pos, state));
    }

    fn left_drag_began(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        if self.drag_start.is_none() {
            self.drag_start = Some(drag.start);
        }
        let start = self.drag_start.unwrap_or(drag.start);
        let end = constrain_measure_end(start, event.pos, event.mods.shift);
        state.measure_preview = Some(measure_preview(start, end, state));
    }

    fn left_drag_changed(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let start = self.drag_start.unwrap_or(drag.start);
        let end = constrain_measure_end(start, event.pos, event.mods.shift);
        state.measure_preview = Some(measure_preview(start, end, state));
    }

    fn left_drag_ended(&mut self, event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        let start = self.drag_start.unwrap_or(drag.start);
        let end = constrain_measure_end(start, event.pos, event.mods.shift);
        state.measure_preview = Some(measure_preview(start, end, state));
        self.drag_start = None;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        self.drag_start = None;
        state.measure_preview = None;
    }
}

fn constrain_measure_end(start: Point, end: Point, constrain: bool) -> Point {
    if !constrain {
        return end;
    }
    let delta = end - start;
    if delta.x.abs() >= delta.y.abs() {
        Point::new(end.x, start.y)
    } else {
        Point::new(start.x, end.y)
    }
}

fn measure_preview(start: Point, end: Point, state: &EditorState) -> MeasurePreview {
    let design_start = state.screen_to_glyph_design(start);
    let design_end = state.screen_to_glyph_design(end);
    let delta = design_end - design_start;
    let design_line = Line::new(design_start, design_end);
    let design_length = delta.hypot();
    let mut line_ts = vec![0.0, 1.0];
    line_ts.extend(path_intersection_ts(state, design_line));
    line_ts.extend(inactive_text_sort_intersection_ts(state, design_line));
    let line_ts = cluster_line_ts(design_line, line_ts);

    let intersections = line_ts
        .iter()
        .map(|t| state.glyph_to_screen(design_line.eval(*t)))
        .collect::<Vec<_>>();
    let segment_labels = line_ts
        .windows(2)
        .filter_map(|pair| {
            let [t0, t1] = pair else {
                return None;
            };
            if (t1 - t0).abs() < 1e-6 {
                return None;
            }
            let t_mid = (t0 + t1) * 0.5;
            Some(MeasureSegmentLabel {
                position: state.glyph_to_screen(design_line.eval(t_mid)),
                length: design_length * (t1 - t0).abs(),
            })
        })
        .collect::<Vec<_>>();

    MeasurePreview {
        line: Line::new(start, end),
        distance: delta.hypot(),
        angle_degrees: delta.y.atan2(delta.x).to_degrees(),
        intersections,
        segment_labels,
    }
}

fn path_intersection_ts(state: &EditorState, line: Line) -> Vec<f64> {
    path_slice_intersection_ts(&state.paths, line)
}

fn path_slice_intersection_ts(paths: &[Path], line: Line) -> Vec<f64> {
    let mut hits = Vec::new();
    for path in paths {
        match path {
            Path::Cubic(path) => {
                for segment in path.iter_segments() {
                    hits.extend(
                        intersect_line_segment(line, &segment.segment)
                            .into_iter()
                            .map(|(_segment_t, line_t)| line_t.clamp(0.0, 1.0)),
                    );
                }
            }
            Path::Quadratic(path) => {
                for segment in path.iter_segments() {
                    hits.extend(
                        intersect_line_segment(line, &segment.segment)
                            .into_iter()
                            .map(|(_segment_t, line_t)| line_t.clamp(0.0, 1.0)),
                    );
                }
            }
            Path::Hyper(path) => {
                for segment in path.iter_segments() {
                    hits.extend(
                        intersect_line_segment(line, &segment.segment)
                            .into_iter()
                            .map(|(_segment_t, line_t)| line_t.clamp(0.0, 1.0)),
                    );
                }
            }
        }
    }
    hits
}

fn inactive_text_sort_intersection_ts(state: &EditorState, active_local_line: Line) -> Vec<f64> {
    let active_sort = state.text_buffer.active_sort();
    let active_origin = state.active_text_sort_origin();
    let text_line = Line::new(
        active_local_line.p0 + active_origin,
        active_local_line.p1 + active_origin,
    );
    let layout = state.text_buffer.layout(state.text_line_height());
    let mut hits = Vec::new();

    for item in layout.items {
        if Some(item.index) == active_sort {
            continue;
        }
        let Some(sort) = state.text_buffer.sort(item.index) else {
            continue;
        };
        let Some(glyph_name) = sort.glyph_name() else {
            continue;
        };
        let Some(outline) = state.text_buffer.glyph_outline_svg(glyph_name) else {
            continue;
        };
        let Ok(path) = kurbo::BezPath::from_svg(outline) else {
            continue;
        };
        let mut glyph = EditorState::default();
        glyph.set_glyph_from_bezpath(&path);
        let sort_origin = Vec2::new(item.x, item.y);
        let sort_local_line = Line::new(text_line.p0 - sort_origin, text_line.p1 - sort_origin);
        hits.extend(path_slice_intersection_ts(&glyph.paths, sort_local_line));
    }

    hits
}

fn cluster_line_ts(line: Line, mut ts: Vec<f64>) -> Vec<f64> {
    ts.sort_by(|a, b| a.total_cmp(b));
    let line_len = (line.p1 - line.p0).hypot();
    let threshold = if line_len > 1e-6 {
        MEASURE_FUZZY_TOLERANCE / line_len
    } else {
        f64::INFINITY
    };

    let mut result = Vec::with_capacity(ts.len());
    let mut cluster_start = -1.0;
    let mut previous = -1.0;

    for t in ts.into_iter().map(|t| t.clamp(0.0, 1.0)) {
        if t - previous > threshold {
            cluster_start = t;
            result.push(t);
        } else if let Some(last) = result.last_mut() {
            *last = if cluster_start == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else {
                0.5 * (cluster_start + t)
            };
        }
        previous = t;
    }

    result
}

fn intersect_line_segment(line: Line, segment: &Segment) -> Vec<(f64, f64)> {
    match segment {
        Segment::Line(seg_line) => intersect_line_line(line, *seg_line),
        Segment::Cubic(cubic) => intersect_line_cubic(line, *cubic),
        Segment::Quadratic(quad) => intersect_line_cubic(line, quad.raise()),
    }
}

fn intersect_line_line(measure: Line, segment: Line) -> Vec<(f64, f64)> {
    let d1 = measure.p1 - measure.p0;
    let d2 = segment.p1 - segment.p0;
    let cross = d1.x * d2.y - d1.y * d2.x;
    const EPSILON: f64 = 1e-9;
    if cross.abs() < EPSILON {
        return Vec::new();
    }
    let d = segment.p0 - measure.p0;
    let line_t = (d.x * d2.y - d.y * d2.x) / cross;
    let segment_t = (d.x * d1.y - d.y * d1.x) / cross;
    if (0.0..=1.0).contains(&line_t) && (0.0..=1.0).contains(&segment_t) {
        vec![(segment_t, line_t)]
    } else {
        Vec::new()
    }
}

fn intersect_line_cubic(line: Line, cubic: CubicBez) -> Vec<(f64, f64)> {
    let d = line.p1 - line.p0;
    let a = -d.y;
    let b = d.x;
    let c = -(a * line.p0.x + b * line.p0.y);
    let d0 = a * cubic.p0.x + b * cubic.p0.y + c;
    let d1 = a * cubic.p1.x + b * cubic.p1.y + c;
    let d2 = a * cubic.p2.x + b * cubic.p2.y + c;
    let d3 = a * cubic.p3.x + b * cubic.p3.y + c;
    let roots = solve_cubic(
        -d0 + 3.0 * d1 - 3.0 * d2 + d3,
        3.0 * d0 - 6.0 * d1 + 3.0 * d2,
        -3.0 * d0 + 3.0 * d1,
        d0,
    );
    let mut results = Vec::new();
    let line_len_sq = d.hypot2();
    const EPSILON: f64 = 1e-9;
    for t in roots {
        let pt = cubic.eval(t);
        let line_t = if line_len_sq > EPSILON {
            let v = pt - line.p0;
            (v.x * d.x + v.y * d.y) / line_len_sq
        } else {
            0.0
        };
        if (0.0..=1.0).contains(&line_t) {
            results.push((t, line_t));
        }
    }
    results
}

fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    let mut roots = Vec::new();
    const EPSILON: f64 = 1e-9;
    if a.abs() < EPSILON {
        if b.abs() < EPSILON {
            if c.abs() > EPSILON {
                let t = -d / c;
                if (0.0..=1.0).contains(&t) {
                    roots.push(t);
                }
            }
        } else {
            let disc = c * c - 4.0 * b * d;
            if disc >= 0.0 {
                let sqrt_disc = disc.sqrt();
                for t in [(-c + sqrt_disc) / (2.0 * b), (-c - sqrt_disc) / (2.0 * b)] {
                    if (0.0..=1.0).contains(&t)
                        && !roots.iter().any(|&r: &f64| (r - t).abs() < EPSILON)
                    {
                        roots.push(t);
                    }
                }
            }
        }
        return roots;
    }

    let p = b / a;
    let q = c / a;
    let r = d / a;
    let p1 = q - p * p / 3.0;
    let q1 = r - p * q / 3.0 + 2.0 * p * p * p / 27.0;
    let disc = q1 * q1 / 4.0 + p1 * p1 * p1 / 27.0;

    if disc > EPSILON {
        let sqrt_disc = disc.sqrt();
        let u = (-q1 / 2.0 + sqrt_disc).cbrt();
        let v = (-q1 / 2.0 - sqrt_disc).cbrt();
        let t = u + v - p / 3.0;
        if (0.0..=1.0).contains(&t) {
            roots.push(t);
        }
    } else if disc.abs() <= EPSILON {
        if q1.abs() < EPSILON {
            let t = -p / 3.0;
            if (0.0..=1.0).contains(&t) {
                roots.push(t);
            }
        } else {
            let u = (q1 / 2.0).cbrt();
            for t in [2.0 * u - p / 3.0, -u - p / 3.0] {
                if (0.0..=1.0).contains(&t) && !roots.iter().any(|&r: &f64| (r - t).abs() < EPSILON)
                {
                    roots.push(t);
                }
            }
        }
    } else {
        let m = 2.0 * (-p1 / 3.0).sqrt();
        let theta = (3.0 * q1 / (p1 * m)).acos() / 3.0;
        for k in 0..3 {
            let t = m * (theta - 2.0 * std::f64::consts::PI * k as f64 / 3.0).cos() - p / 3.0;
            if (0.0..=1.0).contains(&t) && !roots.iter().any(|&r: &f64| (r - t).abs() < EPSILON) {
                roots.push(t);
            }
        }
    }
    roots
}

#[derive(Default)]
pub struct PreviewTool {
    start_offset: Option<Vec2>,
}

impl MouseDelegate for PreviewTool {
    type Data = EditorState;

    fn left_down(&mut self, _event: MouseEvent, state: &mut Self::Data) {
        self.start_offset = Some(state.viewport.offset);
    }

    fn left_drag_changed(&mut self, _event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        if let Some(start_offset) = self.start_offset {
            state.viewport.offset = start_offset + (drag.current - drag.start);
        }
    }

    fn left_up(&mut self, _event: MouseEvent, _state: &mut Self::Data) {
        self.start_offset = None;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        if let Some(start_offset) = self.start_offset {
            state.viewport.offset = start_offset;
        }
        self.start_offset = None;
    }
}

pub struct ShapesTool {
    shape_kind: ShapeKind,
    drag_start: Option<Point>,
    drag_current: Option<Point>,
    shift_locked: bool,
}

impl Default for ShapesTool {
    fn default() -> Self {
        Self {
            shape_kind: ShapeKind::Rectangle,
            drag_start: None,
            drag_current: None,
            shift_locked: false,
        }
    }
}

impl MouseDelegate for ShapesTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, _state: &mut Self::Data) {
        self.drag_start = Some(event.pos);
        self.drag_current = Some(event.pos);
    }

    fn left_drag_began(&mut self, event: MouseEvent, _drag: Drag, _state: &mut Self::Data) {
        if self.drag_start.is_none() {
            self.drag_start = Some(event.pos);
        }
        self.drag_current = Some(event.pos);
    }

    fn left_drag_changed(&mut self, _event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        self.drag_start = Some(drag.start);
        self.drag_current = Some(drag.current);
        state.shape_preview = Some(self.preview_for_drag(drag.start, drag.current, state));
    }

    fn left_drag_ended(&mut self, _event: MouseEvent, drag: Drag, state: &mut Self::Data) {
        state.shape_preview = None;
        let start = state.screen_to_glyph_design(drag.start);
        let current = state.screen_to_glyph_design(drag.current);
        let rect = Rect::from_points(start, constrain_point(start, current, self.shift_locked));
        if rect.width().abs() < 1e-6 || rect.height().abs() < 1e-6 {
            self.drag_start = None;
            self.drag_current = None;
            return;
        }

        let path = match self.shape_kind {
            ShapeKind::Rectangle => rect_path(rect),
            ShapeKind::Ellipse => ellipse_path(rect),
        };
        state.push_path_and_select(path);
        self.drag_start = None;
        self.drag_current = None;
    }

    fn left_up(&mut self, _event: MouseEvent, _state: &mut Self::Data) {
        self.drag_start = None;
        self.drag_current = None;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        state.shape_preview = None;
        self.drag_start = None;
        self.drag_current = None;
    }
}

impl ShapesTool {
    fn preview_for_drag(
        &self,
        start_screen: Point,
        current_screen: Point,
        state: &EditorState,
    ) -> ShapePreview {
        let start = state.screen_to_glyph_design(start_screen);
        let current = state.screen_to_glyph_design(current_screen);
        let current = constrain_point(start, current, self.shift_locked);
        let rect = Rect::from_points(state.glyph_to_screen(start), state.glyph_to_screen(current));
        match self.shape_kind {
            ShapeKind::Rectangle => ShapePreview::Rectangle(rect),
            ShapeKind::Ellipse => ShapePreview::Ellipse(rect),
        }
    }

    fn set_shift_locked(&mut self, locked: bool, state: &mut EditorState) -> bool {
        if self.shift_locked == locked {
            return false;
        }
        self.shift_locked = locked;
        if let (Some(start), Some(current)) = (self.drag_start, self.drag_current) {
            state.shape_preview = Some(self.preview_for_drag(start, current, state));
        }
        true
    }
}

fn constrain_point(start: Point, current: Point, constrain: bool) -> Point {
    if !constrain {
        return current;
    }
    let delta = current - start;
    let size = delta.x.abs().max(delta.y.abs());
    Point::new(
        start.x + size * delta.x.signum(),
        start.y + size * delta.y.signum(),
    )
}

fn rect_path(rect: Rect) -> Path {
    let points = vec![
        path_point(rect.origin(), false),
        path_point(Point::new(rect.max_x(), rect.min_y()), false),
        path_point(Point::new(rect.max_x(), rect.max_y()), false),
        path_point(Point::new(rect.min_x(), rect.max_y()), false),
    ];
    Path::Cubic(CubicPath::new(PathPoints::from_vec(points), true))
}

#[cfg(test)]
fn quadratic_rect_path(rect: Rect) -> Path {
    let points = vec![
        path_point(rect.origin(), false),
        path_point(Point::new(rect.max_x(), rect.min_y()), false),
        path_point(Point::new(rect.max_x(), rect.max_y()), false),
        path_point(Point::new(rect.min_x(), rect.max_y()), false),
    ];
    Path::Quadratic(QuadraticPath::new(PathPoints::from_vec(points), true))
}

#[cfg(test)]
fn quadratic_curve_path() -> Path {
    let points = vec![
        path_point(Point::new(0.0, 0.0), false),
        off_curve(Point::new(50.0, 100.0)),
        path_point(Point::new(100.0, 0.0), true),
        path_point(Point::new(100.0, 100.0), false),
        path_point(Point::new(0.0, 100.0), false),
    ];
    Path::Quadratic(QuadraticPath::new(PathPoints::from_vec(points), true))
}

#[cfg(test)]
fn hyper_curve_path() -> Path {
    let points = vec![
        path_point(Point::new(0.0, 0.0), true),
        path_point(Point::new(100.0, 100.0), true),
        path_point(Point::new(0.0, 100.0), true),
        path_point(Point::new(100.0, 0.0), true),
    ];
    Path::Hyper(crate::path::HyperPath::from_points(
        PathPoints::from_vec(points),
        true,
    ))
}

fn ellipse_path(rect: Rect) -> Path {
    let ellipse = Ellipse::from_rect(rect);
    let mut points = Vec::new();
    for el in ellipse.to_path(0.1).elements() {
        match el {
            PathEl::MoveTo(p) => points.push(path_point(*p, true)),
            PathEl::LineTo(p) => points.push(path_point(*p, false)),
            PathEl::QuadTo(_, _) => {}
            PathEl::CurveTo(c1, c2, p) => {
                points.push(off_curve(*c1));
                points.push(off_curve(*c2));
                points.push(path_point(*p, true));
            }
            PathEl::ClosePath => {}
        }
    }
    Path::Cubic(CubicPath::new(PathPoints::from_vec(points), true))
}

fn path_point(point: Point, smooth: bool) -> PathPoint {
    PathPoint {
        id: EntityId::next(),
        point,
        typ: PointType::OnCurve { smooth },
    }
}

fn off_curve(point: Point) -> PathPoint {
    PathPoint {
        id: EntityId::next(),
        point,
        typ: PointType::OffCurve { auto: false },
    }
}

/// Screen-space pixel delta -> design-space delta (divide by zoom,
/// flip Y).
fn screen_to_design_delta(state: &EditorState, screen_delta: Vec2) -> Vec2 {
    let zoom = state.viewport.zoom.max(1e-6);
    Vec2::new(screen_delta.x / zoom, -screen_delta.y / zoom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_alt_mouse_move_sets_segment_hover() {
        let mut state = EditorState::default();
        state
            .paths
            .push(rect_path(Rect::new(0.0, 0.0, 100.0, 100.0)));
        let mut tool = ActiveTool::Select(SelectTool::default());
        let event = MouseEvent {
            pos: Point::new(50.0, 0.0),
            button: None,
            mods: crate::editing::Modifiers {
                alt: true,
                ..Default::default()
            },
        };

        tool.mouse_moved(event, &mut state);

        let Some(SegmentHoverPreview::Line(line)) = state.segment_hover else {
            panic!("alt-hover should highlight nearest segment");
        };
        assert_eq!(line.p0, Point::new(0.0, 0.0));
        assert_eq!(line.p1, Point::new(100.0, 0.0));
    }

    #[test]
    fn text_click_activates_canvas_sort() {
        let mut state = EditorState::default();
        state.text_buffer.insert_glyph("A", Some('A'), 500.0);
        state.text_buffer.insert_glyph("B", Some('B'), 500.0);
        assert_eq!(state.text_buffer.active_sort(), Some(1));

        let mut tool = ActiveTool::Text(TextTool::default());
        let event = MouseEvent {
            pos: Point::new(250.0, 0.0),
            button: None,
            mods: Default::default(),
        };

        tool.left_click(event, &mut state);

        assert_eq!(state.text_buffer.active_sort(), Some(0));
        assert_eq!(state.text_buffer.cursor(), 1);
    }

    #[test]
    fn text_click_hit_testing_uses_editor_line_height() {
        let mut state = EditorState {
            metrics: Some(crate::editor::FontMetrics {
                units_per_em: Some(2048.0),
                ascender: Some(700.0),
                descender: Some(-300.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        state.text_buffer.insert_glyph("A", Some('A'), 500.0);
        state.text_buffer.insert_line_break();
        state.text_buffer.insert_glyph("B", Some('B'), 500.0);

        let mut tool = ActiveTool::Text(TextTool::default());
        let event = MouseEvent {
            pos: state.viewport.to_screen(Point::new(250.0, -1000.0)),
            button: None,
            mods: Default::default(),
        };

        tool.left_click(event, &mut state);

        assert_eq!(state.text_line_height(), 1000.0);
        assert_eq!(state.text_buffer.active_sort(), Some(2));
        assert_eq!(state.text_buffer.cursor(), 3);
    }

    #[test]
    fn pen_cancel_discards_single_point_path_but_keeps_open_path() {
        let mut state = EditorState::default();
        let mut tool = PenTool::default();

        tool.left_click(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(10.0, 10.0)),
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        assert_eq!(state.paths.len(), 1);

        tool.cancel(&mut state);
        assert!(state.paths.is_empty());

        tool.left_click(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(10.0, 10.0)),
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        tool.left_click(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(100.0, 10.0)),
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        assert_eq!(state.paths.len(), 1);

        tool.cancel(&mut state);
        assert_eq!(state.paths.len(), 1);
        assert_eq!(state.pen_path_len(0), Some(2));
    }

    #[test]
    fn pen_drag_on_close_target_does_not_add_smooth_point() {
        let mut state = EditorState::default();
        let mut tool = PenTool::default();

        let path_index = state.start_pen_path(Point::new(0.0, 0.0));
        state.append_pen_point(path_index, Point::new(100.0, 0.0));
        state.append_pen_point(path_index, Point::new(100.0, 100.0));
        tool.current_path = Some(path_index);
        let start = state.viewport.to_screen(Point::new(0.0, 0.0));
        let current = state.viewport.to_screen(Point::new(50.0, 50.0));

        tool.left_down(
            MouseEvent {
                pos: start,
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        tool.left_drag_began(
            MouseEvent {
                pos: current,
                button: None,
                mods: Default::default(),
            },
            Drag {
                start,
                prev: start,
                current,
            },
            &mut state,
        );

        assert_eq!(state.pen_path_len(path_index), Some(3));
        assert!(!tool.dragging_handles);
    }

    #[test]
    fn pen_close_distance_is_in_design_units() {
        let mut state = EditorState::default();
        state.viewport.zoom = 10.0;
        let mut tool = PenTool::default();

        let path_index = state.start_pen_path(Point::new(0.0, 0.0));
        state.append_pen_point(path_index, Point::new(100.0, 0.0));
        state.append_pen_point(path_index, Point::new(100.0, 100.0));
        tool.current_path = Some(path_index);

        tool.left_click(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(10.0, 0.0)),
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );

        let Path::Cubic(path) = &state.paths[path_index] else {
            panic!("pen path should be cubic");
        };
        assert!(path.closed);
        assert!(tool.current_path.is_none());
    }

    #[test]
    fn shapes_shift_lock_updates_preview_and_committed_shape() {
        let mut state = EditorState::default();
        let mut tool = ShapesTool::default();
        let start = state.viewport.to_screen(Point::new(0.0, 0.0));
        let current = state.viewport.to_screen(Point::new(100.0, 50.0));

        tool.left_down(
            MouseEvent {
                pos: start,
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        tool.left_drag_changed(
            MouseEvent {
                pos: current,
                button: None,
                mods: Default::default(),
            },
            Drag {
                start,
                prev: start,
                current,
            },
            &mut state,
        );
        assert!(matches!(
            state.shape_preview,
            Some(ShapePreview::Rectangle(rect)) if rect.width() == 100.0 && rect.height() == 50.0
        ));

        assert!(tool.set_shift_locked(true, &mut state));
        assert!(matches!(
            state.shape_preview,
            Some(ShapePreview::Rectangle(rect)) if rect.width() == 100.0 && rect.height() == 100.0
        ));

        tool.left_drag_ended(
            MouseEvent {
                pos: current,
                button: None,
                mods: Default::default(),
            },
            Drag {
                start,
                prev: start,
                current,
            },
            &mut state,
        );

        let Path::Cubic(path) = &state.paths[0] else {
            panic!("rectangle should be cubic path");
        };
        let bbox = path.to_bezpath().bounding_box();
        assert_eq!(bbox.width(), 100.0);
        assert_eq!(bbox.height(), 100.0);
    }

    #[test]
    fn knife_shift_lock_updates_active_preview() {
        let mut state = EditorState::default();
        let mut tool = KnifeTool::default();
        let start = state.viewport.to_screen(Point::new(0.0, 0.0));
        let current = state.viewport.to_screen(Point::new(100.0, 50.0));

        tool.left_down(
            MouseEvent {
                pos: start,
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );
        tool.left_drag_changed(
            MouseEvent {
                pos: current,
                button: None,
                mods: Default::default(),
            },
            Drag {
                start,
                prev: start,
                current,
            },
            &mut state,
        );
        assert!(matches!(
            state.knife_preview,
            Some(KnifePreview { line, .. }) if line.p1 == current
        ));

        assert!(tool.set_shift_locked(true, &mut state));
        assert!(matches!(
            state.knife_preview,
            Some(KnifePreview { line, .. })
                if line.p1.x == current.x && line.p1.y == start.y
        ));

        assert!(tool.set_shift_locked(false, &mut state));
        assert!(matches!(
            state.knife_preview,
            Some(KnifePreview { line, .. }) if line.p1 == current
        ));
    }

    #[test]
    fn measure_intersects_inactive_text_sorts() {
        let mut state = EditorState::default();
        state.text_buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "widths": { "A": 100, "B": 100 },
                    "outlines": {
                        "A": "M0 0 L100 0 L100 100 L0 100 Z",
                        "B": "M0 0 L100 0 L100 100 L0 100 Z"
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        state.text_buffer.insert_glyph("A", Some('A'), 100.0);
        state.text_buffer.insert_glyph("B", Some('B'), 100.0);
        state.text_buffer.activate_sort(1);

        let preview = measure_preview(
            state.viewport.to_screen(Point::new(-10.0, 50.0)),
            state.viewport.to_screen(Point::new(110.0, 50.0)),
            &state,
        );

        assert_eq!(
            preview.intersections.len(),
            4,
            "endpoints plus the inactive A sort's two outline hits"
        );
        assert!(preview.intersections.iter().any(|point| {
            (point.x - state.viewport.to_screen(Point::new(0.0, 50.0)).x).abs() < 1e-6
        }));
        assert!(preview.intersections.iter().any(|point| {
            (point.x - state.viewport.to_screen(Point::new(100.0, 50.0)).x).abs() < 1e-6
        }));
    }

    #[test]
    fn select_shift_click_component_selects_component() {
        let mut state = EditorState::default();
        let mut path = kurbo::BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.close_path();
        let component_id = EntityId::next();
        state.set_component_previews(vec![crate::editor::ComponentPreview {
            id: component_id,
            index: 0,
            base: "acute".to_string(),
            transform: kurbo::Affine::IDENTITY,
            path,
        }]);
        state.selection.insert(EntityId::next());

        let mut tool = SelectTool::default();
        tool.left_down(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(50.0, 50.0)),
                button: None,
                mods: crate::editing::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
            &mut state,
        );

        assert_eq!(state.selected_component, Some(component_id));
        assert!(state.selection.is_empty());
    }

    #[test]
    fn select_click_selected_point_preserves_multi_selection_for_drag() {
        let mut state = EditorState::default();
        let first = path_point(Point::new(0.0, 0.0), false);
        let first_id = first.id;
        let second = path_point(Point::new(100.0, 0.0), false);
        let second_id = second.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![first, second]),
            false,
        )));
        state.selection.insert(first_id);
        state.selection.insert(second_id);

        let mut tool = SelectTool::default();
        tool.left_down(
            MouseEvent {
                pos: state.viewport.to_screen(Point::new(0.0, 0.0)),
                button: None,
                mods: Default::default(),
            },
            &mut state,
        );

        assert!(state.selection.contains(&first_id));
        assert!(state.selection.contains(&second_id));
        assert_eq!(state.selection.len(), 2);
    }

    #[test]
    fn select_hit_testing_uses_active_text_sort_origin() {
        let mut state = EditorState::default();
        let point = path_point(Point::new(0.0, 0.0), false);
        let point_id = point.id;
        state.paths.push(Path::Cubic(CubicPath::new(
            PathPoints::from_vec(vec![point]),
            false,
        )));
        state.text_buffer.insert_glyph("A", Some('A'), 500.0);
        state.text_buffer.insert_glyph("B", Some('B'), 500.0);
        assert_eq!(state.active_text_sort_origin(), Vec2::new(500.0, 0.0));

        let mut tool = ActiveTool::Select(SelectTool::default());
        let event = MouseEvent {
            pos: Point::new(500.0, 0.0),
            button: Some(crate::editing::MouseButton::Left),
            mods: Default::default(),
        };

        tool.left_down(event, &mut state);

        assert!(state.selection.contains(&point_id));
    }

    #[test]
    fn measure_intersection_clustering_merges_nearby_hits() {
        let line = Line::new(Point::new(0.0, 0.0), Point::new(100.0, 0.0));

        let clustered = cluster_line_ts(line, vec![0.0, 0.5, 0.5005, 0.9, 1.0]);

        assert_eq!(clustered.len(), 4);
        assert_eq!(clustered[0], 0.0);
        assert!((clustered[1] - 0.50025).abs() < 1e-9);
        assert_eq!(clustered[2], 0.9);
        assert_eq!(clustered[3], 1.0);
    }

    #[test]
    fn knife_splits_closed_rectangle_into_two_paths() {
        let paths = vec![rect_path(Rect::new(0.0, 0.0, 100.0, 100.0))];
        let sliced = slice_paths(
            &paths,
            Line::new(Point::new(50.0, -10.0), Point::new(50.0, 110.0)),
        );

        assert_eq!(sliced.len(), 2);
        for path in sliced {
            let Path::Cubic(path) = path else {
                panic!("knife should preserve cubic path type");
            };
            assert!(path.closed);
            assert!(path.points.len() >= 4);
        }
    }

    #[test]
    fn knife_preserves_path_without_two_intersections() {
        let paths = vec![rect_path(Rect::new(0.0, 0.0, 100.0, 100.0))];
        let sliced = slice_paths(
            &paths,
            Line::new(Point::new(150.0, -10.0), Point::new(150.0, 110.0)),
        );

        assert_eq!(sliced.len(), 1);
    }

    #[test]
    fn knife_splits_closed_quadratic_rectangle_into_two_paths() {
        let paths = vec![quadratic_rect_path(Rect::new(0.0, 0.0, 100.0, 100.0))];
        let sliced = slice_paths(
            &paths,
            Line::new(Point::new(50.0, -10.0), Point::new(50.0, 110.0)),
        );

        assert_eq!(sliced.len(), 2);
        for path in sliced {
            let Path::Quadratic(path) = path else {
                panic!("knife should preserve quadratic path type");
            };
            assert!(path.closed);
            assert!(path.points.len() >= 4);
        }
    }

    #[test]
    fn knife_splits_quadratic_curve_segments_without_raising_to_cubic() {
        let paths = vec![quadratic_curve_path()];
        let sliced = slice_paths(
            &paths,
            Line::new(Point::new(50.0, -10.0), Point::new(50.0, 110.0)),
        );

        assert_eq!(sliced.len(), 2);
        for path in sliced {
            let Path::Quadratic(path) = path else {
                panic!("knife should preserve quadratic path type");
            };
            assert!(
                path.points.iter().any(PathPoint::is_off_curve),
                "sliced quadratic curve should retain quadratic control points"
            );
        }
    }

    #[test]
    fn knife_splits_hyperbezier_as_explicit_cubic_paths() {
        let paths = vec![hyper_curve_path()];
        let sliced = slice_paths(
            &paths,
            Line::new(Point::new(50.0, -10.0), Point::new(50.0, 110.0)),
        );

        assert_eq!(sliced.len(), 2);
        for path in sliced {
            let Path::Cubic(path) = path else {
                panic!("knife should convert sliced hyperbeziers to cubic paths");
            };
            assert!(path.closed);
            assert!(
                path.points.iter().any(PathPoint::is_off_curve),
                "sliced hyperbezier should retain explicit cubic controls"
            );
        }
    }
}
