// wasm-bindgen surface — the JS/Vue-facing API. Wraps `Renderer`
// behind a `GlyphEditor` type that JS will instantiate with a canvas.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::renderer::Renderer;

#[wasm_bindgen]
pub struct GlyphEditor {
    renderer: Renderer,
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
        Ok(Self { renderer })
    }

    /// Replace the displayed glyph from SVG path data.
    #[wasm_bindgen(js_name = setGlyphSvg)]
    pub fn set_glyph_svg(&mut self, svg: &str) -> Result<(), JsValue> {
        self.renderer.set_glyph_svg(svg)
    }

    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.renderer.set_zoom(zoom);
    }

    #[wasm_bindgen(js_name = setOffset)]
    pub fn set_offset(&mut self, x: f64, y: f64) {
        self.renderer.set_offset(x, y);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// Paint a frame. JS calls this from `requestAnimationFrame` or
    /// after any state change.
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.renderer.render()
    }
}
