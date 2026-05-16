// Editing model & interaction.
//
// `edit_types`, `selection`, and `undo` live in the shared
// `runebender-core` crate (sibling repo); they're re-exported here so
// existing `crate::editing::*` paths keep working. The
// kurbo-touching modules (`mouse`, `hit_test`, `viewport`) stay local
// because comfy uses kurbo 0.13 and runebender-core stays on no
// kurbo at all — see runebender-core's README.

pub mod compat;
pub mod hit_test;
pub mod mouse;
pub mod viewport;

// `selection` re-exported as a module too because some callers use
// the deep-path form (matching runebender-xilem).
pub use runebender_core::editing::selection;
pub use runebender_core::editing::{EditType, Selection, UndoState};

pub use mouse::{Drag, Modifiers, Mouse, MouseButton, MouseDelegate, MouseEvent};
pub use viewport::ViewPort;
