// Copyright 2026 the Runebender Authors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vello + Kurbo WASM core for the Runebender ComfyUI node, ported from
//! [runebender-xilem](https://github.com/eliheuer/runebender-xilem) (Apache-2.0).
//!
//! Xilem is skipped deliberately: the host UI is Vue (ComfyUI's frontend), so
//! this crate exposes a thin `wasm-bindgen` surface that Vue drives directly
//! rather than running Xilem's view-tree loop.

// LINEBENDER LINT SET - lib.rs - v1
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// END LINEBENDER LINT SET
// `unused_crate_dependencies` (also in the canonical set) is omitted: the
// render stack (vello, web-sys, serde_json, console_error_panic_hook) is only
// reached under cfg(target_arch = "wasm32"), which the lint mis-reports as
// unused on the native `cargo test` build.
// Canonical-set lints (declared in Cargo.toml) deferred for now — chiefly the
// crate-wide doc requirements. Tracked in .agents/CODE_QUALITY_CLEANUP.md.
#![allow(
    missing_docs,
    unreachable_pub,
    missing_debug_implementations,
    elided_lifetimes_in_paths,
    single_use_lifetimes,
    unnameable_types,
    unused_qualifications,
    variant_size_differences,
    clippy::use_self,
    clippy::return_self_not_must_use,
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_assert_message,
    clippy::exhaustive_enums,
    clippy::match_same_arms,
    clippy::partial_pub_fields,
    clippy::shadow_unrelated,
    clippy::wildcard_imports,
    clippy::doc_markdown,
    clippy::semicolon_if_nothing_returned,
    clippy::trivially_copy_pass_by_ref
)]

pub mod editing;
pub mod editor;
pub mod model;
pub mod path;
pub mod text;
pub mod tool;

#[cfg(target_arch = "wasm32")]
pub mod renderer;

#[cfg(target_arch = "wasm32")]
mod wasm_api;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}
