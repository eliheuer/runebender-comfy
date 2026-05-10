// wasm-bindgen surface — the JS/Vue-facing API. Holds the editor
// state, the mouse state machine, the active tool, the renderer, and
// the undo stack.

#![cfg(target_arch = "wasm32")]

use kurbo::{BezPath, Point, Vec2};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::editing::{Modifiers, Mouse, MouseButton, MouseEvent, UndoState};
use crate::editor::EditorState;
use crate::renderer::Renderer;
use crate::tool::SelectTool;

#[wasm_bindgen]
pub struct GlyphEditor {
    state: EditorState,
    mouse: Mouse,
    tool: SelectTool,
    renderer: Renderer,
    undo: UndoState<EditorState>,
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
            tool: SelectTool::default(),
            renderer,
            undo: UndoState::new(),
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
        self.pending_snapshot = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.state.viewport.zoom = zoom.max(1e-4);
    }

    #[wasm_bindgen(js_name = setOffset)]
    pub fn set_offset(&mut self, x: f64, y: f64) {
        self.state.viewport.offset = Vec2::new(x, y);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        self.renderer.render(&self.state)
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
        self.mouse.mouse_down(event, &mut self.tool, &mut self.state);
    }

    #[wasm_bindgen(js_name = pointerMove)]
    pub fn pointer_move(&mut self, x: f64, y: f64, mods: u32) {
        let event = build_event(x, y, u32::MAX, mods);
        self.mouse.mouse_moved(event, &mut self.tool, &mut self.state);
    }

    #[wasm_bindgen(js_name = pointerUp)]
    pub fn pointer_up(&mut self, x: f64, y: f64, button: u32, mods: u32) {
        let event = build_event(x, y, button, mods);
        self.mouse.mouse_up(event, &mut self.tool, &mut self.state);
        if button == 0
            && let Some(snapshot) = self.pending_snapshot.take()
        {
            self.undo.add_undo_group(snapshot);
        }
    }

    #[wasm_bindgen(js_name = pointerCancel)]
    pub fn pointer_cancel(&mut self) {
        self.mouse.cancel(&mut self.tool, &mut self.state);
        self.pending_snapshot = None;
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

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.undo(self.state.clone()) {
            self.state = prev;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.undo.redo(self.state.clone()) {
            self.state = next;
        }
    }

    /// Number of currently selected entities. Useful for status UI.
    #[wasm_bindgen(js_name = selectionCount)]
    pub fn selection_count(&self) -> usize {
        self.state.selection.len()
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
