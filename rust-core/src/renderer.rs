// WebGPU renderer for the Runebender canvas, built on Vello.
//
// Gated on wasm32 because Vello's `util::RenderContext::create_surface`
// expects a `wgpu::SurfaceTarget`, and the only `SurfaceTarget` we ever
// hand it is an `HtmlCanvasElement` — that's a browser-only path. The
// path/model/editing modules build on both native and wasm32 so unit
// tests still run on `cargo test`.

#![cfg(target_arch = "wasm32")]

use vello::peniko::color::palette::css;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::editing::ViewPort;

pub struct Renderer {
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    vello: VelloRenderer,
    scene: Scene,
    viewport: ViewPort,
    glyph: kurbo::BezPath,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Async constructor — WebGPU adapter request is inherently async.
    pub async fn new(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<Self, JsValue> {
        let mut render_cx = RenderContext::new();

        // `SurfaceTarget::Canvas` takes the canvas directly — the
        // implicit `Into` route would require `HasWindowHandle`,
        // which `HtmlCanvasElement` does not implement.
        let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
        let surface = render_cx
            .create_surface(
                surface_target,
                width,
                height,
                wgpu::PresentMode::AutoVsync,
            )
            .await
            .map_err(|e| JsValue::from_str(&format!("create_surface: {e:?}")))?;

        let device_handle = &render_cx.devices[surface.dev_id];
        let vello = VelloRenderer::new(
            &device_handle.device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .map_err(|e| JsValue::from_str(&format!("Renderer::new: {e:?}")))?;

        Ok(Self {
            render_cx,
            surface,
            vello,
            scene: Scene::new(),
            viewport: ViewPort::default(),
            glyph: kurbo::BezPath::new(),
            width,
            height,
        })
    }

    /// Replace the displayed glyph with a `kurbo::BezPath` parsed from
    /// SVG path data. Coordinates are interpreted in design space
    /// (Y-up) — the viewport handles the screen-space conversion.
    pub fn set_glyph_svg(&mut self, svg: &str) -> Result<(), JsValue> {
        let path = kurbo::BezPath::from_svg(svg)
            .map_err(|e| JsValue::from_str(&format!("parse SVG path: {e}")))?;
        self.glyph = path;
        Ok(())
    }

    /// Replace the displayed glyph with a `kurbo::BezPath` directly
    /// (called from Rust callers, not exposed via wasm-bindgen).
    pub fn set_glyph(&mut self, path: kurbo::BezPath) {
        self.glyph = path;
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.viewport.zoom = zoom.max(0.0001);
    }

    pub fn set_offset(&mut self, x: f64, y: f64) {
        self.viewport.offset = kurbo::Vec2::new(x, y);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.render_cx.resize_surface(&mut self.surface, width, height);
        self.width = width;
        self.height = height;
    }

    /// Paint one frame. Stamps the current glyph (transformed by the
    /// viewport) into the scene, renders to the intermediate texture,
    /// blits to the surface, presents.
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.scene.reset();
        self.draw_glyph();

        let surface_texture = self
            .surface
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture: {e:?}")))?;

        let device_handle = &self.render_cx.devices[self.surface.dev_id];

        self.vello
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &self.surface.target_view,
                &vello::RenderParams {
                    base_color: css::WHITE.into(),
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| JsValue::from_str(&format!("render_to_texture: {e:?}")))?;

        // Blit intermediate target into the surface texture. Vello
        // can't bind the surface as a compute output directly, so it
        // renders into target_texture and we copy to the surface.
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runebender blit"),
                });
        self.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &self.surface.target_view,
            &surface_view,
        );
        device_handle.queue.submit([encoder.finish()]);

        surface_texture.present();
        Ok(())
    }

    /// Stamp the current glyph into the scene with a black fill,
    /// transformed from design space to screen space via the viewport.
    fn draw_glyph(&mut self) {
        if self.glyph.elements().is_empty() {
            return;
        }
        self.scene.fill(
            vello::peniko::Fill::NonZero,
            self.viewport.affine(),
            css::BLACK,
            None,
            &self.glyph,
        );
    }
}
