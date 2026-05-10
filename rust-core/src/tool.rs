// Editing tools — for now just a single Select tool that handles
// click-to-select, drag-to-translate, box-select, and pan.

use kurbo::{Rect, Vec2};

use crate::editing::hit_test::MIN_CLICK_DISTANCE;
use crate::editing::{Drag, MouseDelegate, MouseEvent, Selection};
use crate::editor::EditorState;

/// What the in-progress left-button drag is doing. Set in `left_down`
/// and consulted by the drag callbacks.
#[derive(Debug, Clone)]
enum DragKind {
    None,
    Translate,
    BoxSelect { initial: Selection },
    Pan,
}

#[derive(Default)]
pub struct SelectTool {
    drag_kind: Option<DragKind>,
}

impl MouseDelegate for SelectTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        let design_pt = state.viewport.screen_to_design(event.pos);
        let hit_radius = MIN_CLICK_DISTANCE / state.viewport.zoom.max(1e-6);
        let hit = state.hit_test_point(design_pt, hit_radius);

        // alt+left-drag is a pan, regardless of what's under the cursor.
        if event.mods.alt {
            self.drag_kind = Some(DragKind::Pan);
            return;
        }

        match (hit, event.mods.shift) {
            (Some(id), false) => {
                // Plain click on a point — replace selection with it,
                // prepare for translate.
                let mut sel = Selection::new();
                sel.insert(id);
                state.selection = sel;
                self.drag_kind = Some(DragKind::Translate);
            }
            (Some(id), true) => {
                // Shift-click toggles the hit point. If we just added
                // it, prepare for translate; if we removed it, no
                // drag (None).
                if state.selection.contains(&id) {
                    state.selection.remove(&id);
                    self.drag_kind = Some(DragKind::None);
                } else {
                    state.selection.insert(id);
                    self.drag_kind = Some(DragKind::Translate);
                }
            }
            (None, false) => {
                // Click in empty space — clear and set up box-select.
                state.selection = Selection::new();
                self.drag_kind = Some(DragKind::BoxSelect {
                    initial: Selection::new(),
                });
            }
            (None, true) => {
                // Shift+click in empty space — start box-select that
                // adds to the existing selection.
                self.drag_kind = Some(DragKind::BoxSelect {
                    initial: state.selection.clone(),
                });
            }
        }
    }

    fn left_drag_changed(
        &mut self,
        _event: MouseEvent,
        drag: Drag,
        state: &mut Self::Data,
    ) {
        let kind = match self.drag_kind.clone() {
            Some(k) => k,
            None => return,
        };
        match kind {
            DragKind::Translate => {
                let screen_delta = drag.current - drag.prev;
                let design_delta = screen_to_design_delta(state, screen_delta);
                state.translate_selection(design_delta);
            }
            DragKind::BoxSelect { initial } => {
                let rect = Rect::from_points(drag.start, drag.current);
                state.marquee = Some(rect);
                state.select_in_screen_rect(rect, &initial);
            }
            DragKind::Pan => {
                let delta = drag.current - drag.prev;
                state.viewport.offset += delta;
            }
            DragKind::None => {}
        }
    }

    fn left_drag_ended(
        &mut self,
        _event: MouseEvent,
        _drag: Drag,
        state: &mut Self::Data,
    ) {
        state.marquee = None;
        self.drag_kind = None;
    }

    fn left_up(&mut self, _event: MouseEvent, _state: &mut Self::Data) {
        // No-op for clicks (drag-end already cleaned up).
        self.drag_kind = None;
    }

    // Middle-button drag pans the viewport.
    fn other_drag_changed(
        &mut self,
        _event: MouseEvent,
        drag: Drag,
        state: &mut Self::Data,
    ) {
        let delta = drag.current - drag.prev;
        state.viewport.offset += delta;
    }

    fn cancel(&mut self, state: &mut Self::Data) {
        state.marquee = None;
        self.drag_kind = None;
    }
}

/// Screen-space pixel delta → design-space delta (divide by zoom,
/// flip Y).
fn screen_to_design_delta(state: &EditorState, screen_delta: Vec2) -> Vec2 {
    let zoom = state.viewport.zoom.max(1e-6);
    Vec2::new(screen_delta.x / zoom, -screen_delta.y / zoom)
}
