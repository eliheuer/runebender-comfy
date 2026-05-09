// runebender-comfy-core — Vello + Kurbo WASM core for the Runebender node.
//
// Skips Xilem deliberately: the host UI is Vue (ComfyUI's frontend),
// so we expose a thin wasm-bindgen surface that Vue can drive.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct GlyphEditor {
    // TODO: hold Vello renderer, Kurbo BezPath store, edit state.
}

#[wasm_bindgen]
impl GlyphEditor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {}
    }

    /// Serialize current glyph as SVG path data.
    pub fn to_svg(&self) -> String {
        String::new()
    }

    /// Load glyph from SVG path data.
    pub fn load_svg(&mut self, _svg: &str) {
        // TODO
    }
}
