// WebGPU renderer for the Runebender canvas, built on Vello.
//
// Gated on wasm32 because Vello's `util::RenderContext::create_surface`
// expects a `wgpu::SurfaceTarget`, and the only `SurfaceTarget` we
// ever hand it is an `HtmlCanvasElement` — that's a browser-only
// path. The path/model/editing modules build on both native and
// wasm32 so unit tests still run on `cargo test`.

#![cfg(target_arch = "wasm32")]

use kurbo::{Affine, Circle, Line, Rect, Stroke};
use vello::peniko::{Fill, color::AlphaColor};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::editor::EditorState;
use crate::path::{Path, PathPoint, PointType};

// ============================================================================
// PALETTE — warm campfire
// ============================================================================

const BG: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0x1f, 0x1a, 0x14, 0xff);
const GLYPH_FILL: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0xf0, 0xe6, 0xd2, 0xee);
const HANDLE_LINE: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0x8a, 0x6f, 0x52, 0xff);
const POINT_ON_CURVE: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0xf0, 0xe6, 0xd2, 0xff);
const POINT_OFF_CURVE: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0xc8, 0xae, 0x88, 0xff);
const POINT_SELECTED: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0xff, 0xa6, 0x40, 0xff);
const POINT_OUTLINE: AlphaColor<vello::peniko::color::Srgb> =
    AlphaColor::from_rgba8(0x1f, 0x1a, 0x14, 0xff);

const ON_CURVE_RADIUS_PX: f64 = 4.5;
const OFF_CURVE_HALF_PX: f64 = 3.0;
const POINT_OUTLINE_PX: f64 = 1.0;
const HANDLE_LINE_PX: f64 = 1.0;

// ============================================================================
// RENDERER
// ============================================================================

pub struct Renderer {
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    vello: VelloRenderer,
    scene: Scene,
    width: u32,
    height: u32,
}

impl Renderer {
    pub async fn new(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<Self, JsValue> {
        let mut render_cx = RenderContext::new();

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
            width,
            height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.render_cx.resize_surface(&mut self.surface, width, height);
        self.width = width;
        self.height = height;
    }

    /// Paint one frame against the given editor state.
    pub fn render(&mut self, state: &EditorState) -> Result<(), JsValue> {
        self.scene.reset();
        self.draw_state(state);
        self.present()
    }

    fn draw_state(&mut self, state: &EditorState) {
        let view = state.viewport.affine();

        // Glyph fill (in design space — viewport applies the Y-flip).
        for path in &state.paths {
            self.scene.fill(
                Fill::NonZero,
                view,
                GLYPH_FILL,
                None,
                &path.to_bezpath(),
            );
        }

        // Handle lines and points are drawn in screen space so they
        // stay at constant pixel size regardless of zoom.
        for path in &state.paths {
            self.draw_handle_lines(path, view);
        }
        for path in &state.paths {
            self.draw_points(path, view, &state.selection);
        }
    }

    /// Draw thin lines connecting each off-curve handle to its
    /// adjacent on-curve point(s).
    fn draw_handle_lines(&mut self, path: &Path, view: Affine) {
        let points: Vec<PathPoint> = path.points().iter().cloned().collect();
        if points.len() < 2 {
            return;
        }
        let stroke = Stroke::new(HANDLE_LINE_PX);
        for (i, pt) in points.iter().enumerate() {
            if !pt.is_off_curve() {
                continue;
            }
            let off = view * pt.point;
            // Connect to nearest on-curve neighbour on each side.
            if let Some(prev_on) = nearest_on_curve_before(&points, i) {
                let on = view * prev_on.point;
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    HANDLE_LINE,
                    None,
                    &Line::new(on, off),
                );
            }
            if let Some(next_on) = nearest_on_curve_after(&points, i) {
                let on = view * next_on.point;
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    HANDLE_LINE,
                    None,
                    &Line::new(off, on),
                );
            }
        }
    }

    /// Draw an outlined node at every PathPoint.
    fn draw_points(
        &mut self,
        path: &Path,
        view: Affine,
        selection: &crate::editing::Selection,
    ) {
        let outline_stroke = Stroke::new(POINT_OUTLINE_PX);
        for pt in path.points().iter() {
            let center = view * pt.point;
            let selected = selection.contains(&pt.id);
            let fill_color = if selected {
                POINT_SELECTED
            } else if pt.is_on_curve() {
                POINT_ON_CURVE
            } else {
                POINT_OFF_CURVE
            };
            match pt.typ {
                PointType::OnCurve { .. } => {
                    let circle = Circle::new(center, ON_CURVE_RADIUS_PX);
                    self.scene
                        .fill(Fill::NonZero, Affine::IDENTITY, fill_color, None, &circle);
                    self.scene.stroke(
                        &outline_stroke,
                        Affine::IDENTITY,
                        POINT_OUTLINE,
                        None,
                        &circle,
                    );
                }
                PointType::OffCurve { .. } => {
                    let square = Rect::new(
                        center.x - OFF_CURVE_HALF_PX,
                        center.y - OFF_CURVE_HALF_PX,
                        center.x + OFF_CURVE_HALF_PX,
                        center.y + OFF_CURVE_HALF_PX,
                    );
                    self.scene
                        .fill(Fill::NonZero, Affine::IDENTITY, fill_color, None, &square);
                    self.scene.stroke(
                        &outline_stroke,
                        Affine::IDENTITY,
                        POINT_OUTLINE,
                        None,
                        &square,
                    );
                }
            }
        }
    }

    fn present(&mut self) -> Result<(), JsValue> {
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
                    base_color: BG.into(),
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| JsValue::from_str(&format!("render_to_texture: {e:?}")))?;

        // Vello can't bind the surface as a compute output directly,
        // so it renders into the intermediate `target_texture` and we
        // blit from there to the actual surface.
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
}

fn nearest_on_curve_before(points: &[PathPoint], idx: usize) -> Option<&PathPoint> {
    for offset in 1..=points.len() {
        let i = (idx + points.len() - offset) % points.len();
        if i == idx {
            break;
        }
        if points[i].is_on_curve() {
            return Some(&points[i]);
        }
    }
    None
}

fn nearest_on_curve_after(points: &[PathPoint], idx: usize) -> Option<&PathPoint> {
    for offset in 1..=points.len() {
        let i = (idx + offset) % points.len();
        if i == idx {
            break;
        }
        if points[i].is_on_curve() {
            return Some(&points[i]);
        }
    }
    None
}

