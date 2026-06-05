// Ported from runebender-xilem/src/editing/mouse.rs (Apache-2.0).

//! Mouse event handling and state machine.
//!
//! State machine for tracking mouse events and converting them into
//! high-level gestures (clicks, drags, etc.). Independent of any
//! windowing system — Vue mouse handlers will translate DOM events
//! into `MouseEvent` and drive the state machine via wasm-bindgen.

use kurbo::Point;

/// Threshold distance (in screen pixels) before a drag is recognized.
const DRAG_THRESHOLD: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MouseButton {
    Left,
    Right,
    Other,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub pos: Point,
    pub button: Option<MouseButton>,
    pub mods: Modifiers,
}

#[allow(dead_code)]
impl MouseEvent {
    pub fn new(pos: Point, button: Option<MouseButton>) -> Self {
        Self {
            pos,
            button,
            mods: Modifiers::default(),
        }
    }

    pub fn with_modifiers(pos: Point, button: Option<MouseButton>, mods: Modifiers) -> Self {
        Self { pos, button, mods }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Drag {
    pub start: Point,
    pub prev: Point,
    pub current: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseState {
    Up,
    Down,
    Drag,
}

/// Mouse state machine. Tracks events and converts them into
/// high-level gestures, dispatched through `MouseDelegate`.
pub struct Mouse {
    state: MouseState,
    current_button: Option<MouseButton>,
    down_pos: Point,
    last_pos: Point,
}

impl Mouse {
    /// Whether a button is down or a drag is active. During pointer
    /// capture the widget owns live editing state (tool state,
    /// modified paths) that must not be overwritten by a view rebuild.
    /// Callers use this to guard session updates.
    pub fn is_active(&self) -> bool {
        matches!(self.state, MouseState::Down | MouseState::Drag)
    }

    /// Whether this move will promote a button-down gesture into a drag.
    pub fn will_start_drag(&self, pos: Point, button: MouseButton) -> bool {
        matches!(self.state, MouseState::Down)
            && self.current_button == Some(button)
            && Self::should_start_drag(self.down_pos, pos)
    }

    /// Track pointer movement while a button is down but still below the
    /// drag threshold. This matches `handle_move_while_down`'s no-delegate
    /// path and lets hot callers skip heavier editor-state change detection.
    pub fn track_pending_drag_move(&mut self, pos: Point, button: MouseButton) -> bool {
        if !matches!(self.state, MouseState::Down) || self.current_button != Some(button) {
            return false;
        }
        if Self::should_start_drag(self.down_pos, pos) {
            return false;
        }
        self.last_pos = pos;
        true
    }

    pub fn new() -> Self {
        Self {
            state: MouseState::Up,
            current_button: None,
            down_pos: Point::ZERO,
            last_pos: Point::ZERO,
        }
    }

    pub fn mouse_down<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        if let MouseState::Up = self.state {
            self.handle_button_down(event, delegate, data);
        }
    }

    fn handle_button_down<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        self.state = MouseState::Down;
        self.current_button = event.button;
        self.down_pos = event.pos;
        self.last_pos = event.pos;

        Self::call_button_down(event.button, delegate, event, data);
    }

    fn call_button_down<T: MouseDelegate>(
        button: Option<MouseButton>,
        delegate: &mut T,
        event: MouseEvent,
        data: &mut T::Data,
    ) {
        match button {
            Some(MouseButton::Left) => delegate.left_down(event, data),
            Some(MouseButton::Right) => delegate.right_down(event, data),
            Some(MouseButton::Other) => delegate.other_down(event, data),
            None => {}
        }
    }

    pub fn mouse_up<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        match self.state {
            MouseState::Down => self.handle_click_up(event, delegate, data),
            MouseState::Drag => self.handle_drag_up(event, delegate, data),
            MouseState::Up => {}
        }
    }

    fn handle_click_up<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        Self::call_click_up(event.button, delegate, event, data);
        self.reset_state();
    }

    fn handle_drag_up<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        let drag = Self::create_drag(self.down_pos, self.last_pos, event.pos);
        Self::call_drag_ended(self.current_button, delegate, event, drag, data);
        self.reset_state();
    }

    fn reset_state(&mut self) {
        self.state = MouseState::Up;
        self.current_button = None;
    }

    fn call_click_up<T: MouseDelegate>(
        button: Option<MouseButton>,
        delegate: &mut T,
        event: MouseEvent,
        data: &mut T::Data,
    ) {
        match button {
            Some(MouseButton::Left) => {
                delegate.left_up(event, data);
                delegate.left_click(event, data);
            }
            Some(MouseButton::Right) => {
                delegate.right_up(event, data);
                delegate.right_click(event, data);
            }
            Some(MouseButton::Other) => {
                delegate.other_up(event, data);
                delegate.other_click(event, data);
            }
            None => {}
        }
    }

    fn call_drag_ended<T: MouseDelegate>(
        button: Option<MouseButton>,
        delegate: &mut T,
        event: MouseEvent,
        drag: Drag,
        data: &mut T::Data,
    ) {
        match button {
            Some(MouseButton::Left) => {
                delegate.left_drag_ended(event, drag, data);
                delegate.left_up(event, data);
            }
            Some(MouseButton::Right) => {
                delegate.right_drag_ended(event, drag, data);
                delegate.right_up(event, data);
            }
            Some(MouseButton::Other) => {
                delegate.other_drag_ended(event, drag, data);
                delegate.other_up(event, data);
            }
            None => {}
        }
    }

    fn create_drag(start: Point, prev: Point, current: Point) -> Drag {
        Drag {
            start,
            prev,
            current,
        }
    }

    pub fn mouse_moved<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        match self.state {
            MouseState::Up => self.handle_move_without_button(event, delegate, data),
            MouseState::Down => self.handle_move_while_down(event, delegate, data),
            MouseState::Drag => self.handle_move_while_dragging(event, delegate, data),
        }
    }

    fn handle_move_without_button<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        self.last_pos = event.pos;
        delegate.mouse_moved(event, data);
    }

    fn handle_move_while_down<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        if Self::should_start_drag(self.down_pos, event.pos) {
            self.start_drag(event, delegate, data);
        }
        self.last_pos = event.pos;
    }

    fn handle_move_while_dragging<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        let drag = Self::create_drag(self.down_pos, self.last_pos, event.pos);
        Self::call_drag_changed(self.current_button, delegate, event, drag, data);
        self.last_pos = event.pos;
    }

    fn should_start_drag(down_pos: Point, current_pos: Point) -> bool {
        let delta_x = current_pos.x - down_pos.x;
        let delta_y = current_pos.y - down_pos.y;
        let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
        distance >= DRAG_THRESHOLD
    }

    fn start_drag<T: MouseDelegate>(
        &mut self,
        event: MouseEvent,
        delegate: &mut T,
        data: &mut T::Data,
    ) {
        self.state = MouseState::Drag;
        let drag = Self::create_drag(self.down_pos, self.last_pos, event.pos);
        Self::call_drag_began(self.current_button, delegate, event, drag, data);
    }

    fn call_drag_began<T: MouseDelegate>(
        button: Option<MouseButton>,
        delegate: &mut T,
        event: MouseEvent,
        drag: Drag,
        data: &mut T::Data,
    ) {
        match button {
            Some(MouseButton::Left) => delegate.left_drag_began(event, drag, data),
            Some(MouseButton::Right) => delegate.right_drag_began(event, drag, data),
            Some(MouseButton::Other) => delegate.other_drag_began(event, drag, data),
            None => {}
        }
    }

    fn call_drag_changed<T: MouseDelegate>(
        button: Option<MouseButton>,
        delegate: &mut T,
        event: MouseEvent,
        drag: Drag,
        data: &mut T::Data,
    ) {
        match button {
            Some(MouseButton::Left) => delegate.left_drag_changed(event, drag, data),
            Some(MouseButton::Right) => delegate.right_drag_changed(event, drag, data),
            Some(MouseButton::Other) => delegate.other_drag_changed(event, drag, data),
            None => {}
        }
    }

    pub fn cancel<T: MouseDelegate>(&mut self, delegate: &mut T, data: &mut T::Data) {
        delegate.cancel(data);
        self.reset_state();
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for handling high-level mouse events. Tools implement this
/// to respond to clicks, drags, etc.
pub trait MouseDelegate {
    type Data;

    fn left_down(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn left_up(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn left_click(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn left_drag_began(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn left_drag_changed(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn left_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}

    fn right_down(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn right_up(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn right_click(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn right_drag_began(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn right_drag_changed(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn right_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}

    fn other_down(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn other_up(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn other_click(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn other_drag_began(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn other_drag_changed(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}
    fn other_drag_ended(&mut self, _event: MouseEvent, _drag: Drag, _data: &mut Self::Data) {}

    fn mouse_moved(&mut self, _event: MouseEvent, _data: &mut Self::Data) {}
    fn cancel(&mut self, _data: &mut Self::Data) {}
}
