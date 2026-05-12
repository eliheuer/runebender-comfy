// Font data model.
//
// `entity_id` and `kerning` live in the shared `runebender-core`
// crate (sibling repo); they're re-exported here so existing
// `crate::model::*` paths keep working. The kurbo-touching modules
// (`workspace`, `glyph_renderer`) stay local.

pub mod glyph_renderer;
pub mod workspace;

pub use runebender_core::model::{EntityId, entity_id, kerning};
