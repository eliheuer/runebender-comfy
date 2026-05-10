// Ported from runebender-xilem/src/editing/undo.rs (Apache-2.0).

//! Undo/redo system for edit operations.

use std::collections::VecDeque;

const MAX_UNDO_HISTORY: usize = 128;

/// Undo/redo state manager.
///
/// Stores a history of states using a deque. The current state is not
/// stored in the history — it's managed externally. The undo stack
/// contains previous states, the redo stack contains future states.
#[derive(Debug, Clone)]
pub struct UndoState<T> {
    undo_stack: VecDeque<T>,
    redo_stack: VecDeque<T>,
}

#[allow(dead_code)]
impl<T: Clone> UndoState<T> {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            redo_stack: VecDeque::new(),
        }
    }

    /// Push a new state onto the undo stack and clear redo. Bounded
    /// by `MAX_UNDO_HISTORY`.
    pub fn add_undo_group(&mut self, state: T) {
        self.redo_stack.clear();
        self.undo_stack.push_back(state);

        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
    }

    /// Update the most recent undo state without creating a new
    /// group. Used for grouping rapid edits of the same type (e.g.,
    /// dragging) into a single undo operation.
    pub fn update_current_undo(&mut self, state: T) {
        if let Some(last) = self.undo_stack.back_mut() {
            *last = state;
        }
    }

    /// Undo: returns the previous state and pushes `current` onto
    /// redo. Caller applies the returned state.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let previous = self.undo_stack.pop_back()?;
        self.redo_stack.push_back(current);
        Some(previous)
    }

    /// Redo: returns the next state and pushes `current` back onto
    /// undo. Caller applies the returned state.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo_stack.pop_back()?;
        self.undo_stack.push_back(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}

impl<T: Clone> Default for UndoState<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_undo_redo() {
        let mut undo: UndoState<i32> = UndoState::new();

        assert!(!undo.can_undo());
        assert!(!undo.can_redo());

        undo.add_undo_group(1);
        undo.add_undo_group(2);

        assert!(undo.can_undo());
        assert!(!undo.can_redo());
        assert_eq!(undo.undo_depth(), 2);

        let prev = undo.undo(3);
        assert_eq!(prev, Some(2));
        assert_eq!(undo.undo_depth(), 1);
        assert_eq!(undo.redo_depth(), 1);

        let next = undo.redo(2);
        assert_eq!(next, Some(3));
        assert_eq!(undo.undo_depth(), 2);
        assert_eq!(undo.redo_depth(), 0);
    }

    #[test]
    fn test_add_clears_redo() {
        let mut undo: UndoState<i32> = UndoState::new();

        undo.add_undo_group(1);
        undo.add_undo_group(2);

        undo.undo(3);
        assert_eq!(undo.redo_depth(), 1);

        undo.add_undo_group(4);
        assert_eq!(undo.redo_depth(), 0);
    }

    #[test]
    fn test_max_history() {
        let mut undo: UndoState<i32> = UndoState::new();

        for i in 0..(MAX_UNDO_HISTORY + 10) {
            undo.add_undo_group(i as i32);
        }

        assert_eq!(undo.undo_depth(), MAX_UNDO_HISTORY);

        let prev = undo.undo(999);
        assert_eq!(prev, Some((MAX_UNDO_HISTORY + 9) as i32));
    }

    #[test]
    fn test_update_current_undo() {
        let mut undo: UndoState<i32> = UndoState::new();

        undo.add_undo_group(1);
        undo.update_current_undo(2);

        assert_eq!(undo.undo_depth(), 1);
        let prev = undo.undo(3);
        assert_eq!(prev, Some(2));
    }
}
