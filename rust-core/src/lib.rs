// runebender-comfy-core — Vello + Kurbo WASM core for the Runebender
// ComfyUI node, ported from runebender-xilem (Apache-2.0).
//
// Skips Xilem deliberately: the host UI is Vue (ComfyUI's frontend),
// so we expose a thin wasm-bindgen surface that Vue can drive.

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
