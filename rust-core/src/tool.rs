// Editing tools — for now just a single Select tool that handles
// click-to-select and drag-to-translate.

use kurbo::Vec2;

use crate::editing::hit_test::{MIN_CLICK_DISTANCE, find_closest};
use crate::editing::{Drag, MouseDelegate, MouseEvent, Selection};
use crate::editor::EditorState;

#[derive(Default)]
pub struct SelectTool {
    /// Last design-space position seen for the active drag, used to
    /// produce frame-by-frame deltas.
    drag_prev: Option<kurbo::Point>,
}

impl MouseDelegate for SelectTool {
    type Data = EditorState;

    fn left_down(&mut self, event: MouseEvent, state: &mut Self::Data) {
        let design_pt = state.viewport.screen_to_design(event.pos);
        // Replace selection with whatever's under the cursor (or
        // clear if nothing's hit).
        let hit_radius_design =
            screen_to_design_distance(state, MIN_CLICK_DISTANCE);
        let candidates = state.paths.iter().flat_map(|path| {
            path.points()
                .iter()
                .map(|pt| (pt.id, pt.point, pt.is_on_curve()))
                .collect::<Vec<_>>()
        });

        let mut new_selection = Selection::new();
        if let Some(hit) = find_closest(design_pt, candidates, hit_radius_design) {
            new_selection.insert(hit.entity);
        }
        state.selection = new_selection;

        self.drag_prev = Some(design_pt);
    }

    fn left_drag_changed(
        &mut self,
        event: MouseEvent,
        _drag: Drag,
        state: &mut Self::Data,
    ) {
        let design_pt = state.viewport.screen_to_design(event.pos);
        let prev = self.drag_prev.unwrap_or(design_pt);
        let delta = Vec2::new(design_pt.x - prev.x, design_pt.y - prev.y);
        state.translate_selection(delta);
        self.drag_prev = Some(design_pt);
    }

    fn left_drag_ended(
        &mut self,
        _event: MouseEvent,
        _drag: Drag,
        _state: &mut Self::Data,
    ) {
        self.drag_prev = None;
    }

    fn left_up(&mut self, _event: MouseEvent, _state: &mut Self::Data) {
        self.drag_prev = None;
    }

    fn cancel(&mut self, _state: &mut Self::Data) {
        self.drag_prev = None;
    }
}

/// Convert a screen-space pixel distance to the equivalent design-space
/// distance at the current zoom. Uses the inverse of `viewport.zoom`.
fn screen_to_design_distance(state: &EditorState, screen_dist: f64) -> f64 {
    let zoom = state.viewport.zoom.max(1e-6);
    screen_dist / zoom
}
