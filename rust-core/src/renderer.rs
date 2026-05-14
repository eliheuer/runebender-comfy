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
// PALETTE — mirrors runebender-xilem/src/theme.rs verbatim.
// Phase 1 of theming: hardcoded match. Phase 2 will move these into
// a JSON file in runebender-core that both editors load at startup.
// ============================================================================

type Srgb = AlphaColor<vello::peniko::color::Srgb>;

// --- App background (xilem APP_BACKGROUND = BASE_A) ---
const BG: Srgb = AlphaColor::from_rgba8(0x10, 0x10, 0x10, 0xff);

// --- Glyph fill (xilem PATH_FILL = BASE_F) ---
const GLYPH_FILL: Srgb = AlphaColor::from_rgba8(0x60, 0x60, 0x60, 0xff);

// --- Handle line (xilem HANDLE_LINE = BASE_I) ---
const HANDLE_LINE: Srgb = AlphaColor::from_rgba8(0x90, 0x90, 0x90, 0xff);

// --- Point colors, color-coded by point type ---
// Smooth on-curve (circle): blue
const POINT_SMOOTH_INNER: Srgb = AlphaColor::from_rgba8(0x57, 0x9a, 0xff, 0xff);
const POINT_SMOOTH_OUTER: Srgb = AlphaColor::from_rgba8(0x44, 0x28, 0xec, 0xff);
// Corner on-curve (square): green
const POINT_CORNER_INNER: Srgb = AlphaColor::from_rgba8(0x6a, 0xe7, 0x56, 0xff);
const POINT_CORNER_OUTER: Srgb = AlphaColor::from_rgba8(0x20, 0x8e, 0x56, 0xff);
// Off-curve (circle): purple
const POINT_OFFCURVE_INNER: Srgb = AlphaColor::from_rgba8(0xcc, 0x99, 0xff, 0xff);
const POINT_OFFCURVE_OUTER: Srgb = AlphaColor::from_rgba8(0x99, 0x00, 0xff, 0xff);
// Selected: yellow inner / orange outer outline
const POINT_SELECTED_INNER: Srgb = AlphaColor::from_rgba8(0xff, 0xee, 0x55, 0xff);
const POINT_SELECTED_OUTER: Srgb = AlphaColor::from_rgba8(0xff, 0xaa, 0x33, 0xff);

// --- Marquee (xilem SELECTION_RECT_*) ---
const MARQUEE_FILL: Srgb = AlphaColor::from_rgba8(0xff, 0xaa, 0x33, 0x20);
const MARQUEE_STROKE: Srgb = AlphaColor::from_rgba8(0xff, 0xaa, 0x33, 0xff);

// --- Metric guides (xilem METRICS_GUIDE) ---
const METRIC_GUIDE: Srgb = AlphaColor::from_rgba8(0x66, 0xEE, 0x88, 0xff);

// --- Sizes (xilem size::*; STROKE_SCALE = 1.5) ---
const STROKE_SCALE: f64 = 1.5;
const SMOOTH_POINT_RADIUS_PX: f64 = 4.5;
const CORNER_POINT_HALF_PX: f64 = 3.5;
const OFFCURVE_POINT_RADIUS_PX: f64 = 3.0;
const POINT_OUTLINE_PX: f64 = 1.0 * STROKE_SCALE;
const HANDLE_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const MARQUEE_STROKE_PX: f64 = 1.0 * STROKE_SCALE;
const METRIC_LINE_PX: f64 = 1.0 * STROKE_SCALE;

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

        // Metric guides go in first so the glyph fill paints on top.
        self.draw_metric_guides(state, view);

        // Glyph fill (in design space — viewport applies the Y-flip).
        // Combine every contour into ONE BezPath before filling so the
        // NonZero winding rule treats opposite-wound inner contours as
        // holes (UFO/PostScript convention). Filling each contour
        // separately would paint counters solid.
        let mut combined = kurbo::BezPath::new();
        for path in &state.paths {
            for el in path.to_bezpath().elements() {
                combined.push(*el);
            }
        }
        if !combined.elements().is_empty() {
            self.scene
                .fill(Fill::NonZero, view, GLYPH_FILL, None, &combined);
        }

        // Handle lines and points are drawn in screen space so they
        // stay at constant pixel size regardless of zoom.
        for path in &state.paths {
            self.draw_handle_lines(path, view);
        }
        for path in &state.paths {
            self.draw_points(path, view, &state.selection);
        }

        if let Some(rect) = state.marquee {
            self.draw_marquee(rect);
        }
    }

    /// Draw thin lines connecting each on-curve point to its
    /// IMMEDIATELY-adjacent off-curve handle, in either direction.
    /// Matches runebender-xilem's `draw_control_handles` —
    /// iterating off-curves and searching for the nearest on-curve
    /// would leap over intermediate off-curves and draw lines across
    /// the glyph.
    fn draw_handle_lines(&mut self, path: &Path, view: Affine) {
        let points: Vec<PathPoint> = path.points().iter().cloned().collect();
        if points.len() < 2 {
            return;
        }
        let closed = path_is_closed(path);
        let stroke = Stroke::new(HANDLE_LINE_PX);
        let n = points.len();
        for (i, pt) in points.iter().enumerate() {
            if !pt.is_on_curve() {
                continue;
            }
            let on = view * pt.point;

            // Forward neighbour.
            let next_i = if i + 1 < n {
                Some(i + 1)
            } else if closed {
                Some(0)
            } else {
                None
            };
            if let Some(ni) = next_i
                && points[ni].is_off_curve()
            {
                let off = view * points[ni].point;
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    HANDLE_LINE,
                    None,
                    &Line::new(on, off),
                );
            }

            // Backward neighbour.
            let prev_i = if i > 0 {
                Some(i - 1)
            } else if closed {
                Some(n - 1)
            } else {
                None
            };
            if let Some(pi) = prev_i
                && points[pi].is_off_curve()
            {
                let off = view * points[pi].point;
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    HANDLE_LINE,
                    None,
                    &Line::new(on, off),
                );
            }
        }
    }

    /// Draw an outlined node at every PathPoint. Shape + color
    /// depend on the point type, matching runebender-xilem:
    ///   - smooth on-curve  → blue circle
    ///   - corner on-curve  → green square
    ///   - off-curve        → purple circle
    ///   - any selected     → yellow inner + orange outline
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

            let (inner, outer) = if selected {
                (POINT_SELECTED_INNER, POINT_SELECTED_OUTER)
            } else {
                match pt.typ {
                    PointType::OnCurve { smooth: true } => {
                        (POINT_SMOOTH_INNER, POINT_SMOOTH_OUTER)
                    }
                    PointType::OnCurve { smooth: false } => {
                        (POINT_CORNER_INNER, POINT_CORNER_OUTER)
                    }
                    PointType::OffCurve { .. } => {
                        (POINT_OFFCURVE_INNER, POINT_OFFCURVE_OUTER)
                    }
                }
            };

            match pt.typ {
                PointType::OnCurve { smooth: true } => {
                    let circle = Circle::new(center, SMOOTH_POINT_RADIUS_PX);
                    self.scene.fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
                    self.scene.stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
                }
                PointType::OnCurve { smooth: false } => {
                    let square = Rect::new(
                        center.x - CORNER_POINT_HALF_PX,
                        center.y - CORNER_POINT_HALF_PX,
                        center.x + CORNER_POINT_HALF_PX,
                        center.y + CORNER_POINT_HALF_PX,
                    );
                    self.scene.fill(Fill::NonZero, Affine::IDENTITY, inner, None, &square);
                    self.scene.stroke(&outline_stroke, Affine::IDENTITY, outer, None, &square);
                }
                PointType::OffCurve { .. } => {
                    let circle = Circle::new(center, OFFCURVE_POINT_RADIUS_PX);
                    self.scene.fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
                    self.scene.stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
                }
            }
        }
    }

    /// Draw the font's metric box: vertical lines at x=0 and
    /// x=advance_width, horizontal lines at each defined metric Y.
    /// Bounded to the glyph's advance-width rectangle so it reads as
    /// "the glyph's space," matching runebender-xilem's
    /// `draw_metrics_guides`.
    fn draw_metric_guides(&mut self, state: &EditorState, view: Affine) {
        let Some(metrics) = state.metrics.as_ref() else {
            return;
        };
        if state.advance_width <= 0.0 {
            return;
        }

        let stroke = Stroke::new(METRIC_LINE_PX);
        let width = state.advance_width;
        let ascender = metrics.ascender.unwrap_or(0.0);
        let descender = metrics.descender.unwrap_or(0.0);

        // Vertical edges of the metric box.
        let stamp_line = |scene: &mut Scene, p0: (f64, f64), p1: (f64, f64)| {
            scene.stroke(&stroke, view, METRIC_GUIDE, None, &Line::new(p0, p1));
        };
        if ascender > descender {
            stamp_line(&mut self.scene, (0.0, descender), (0.0, ascender));
            stamp_line(&mut self.scene, (width, descender), (width, ascender));
        }

        // Horizontal metric lines. Baseline is always drawn (y=0);
        // others appear only when defined in fontinfo.
        let mut ys: Vec<f64> = vec![0.0];
        for opt in [
            metrics.ascender,
            metrics.descender,
            metrics.x_height,
            metrics.cap_height,
        ] {
            if let Some(y) = opt {
                ys.push(y);
            }
        }
        for y in ys {
            stamp_line(&mut self.scene, (0.0, y), (width, y));
        }
    }

    fn draw_marquee(&mut self, rect: kurbo::Rect) {
        // Marquee is already in screen space; draw with identity.
        self.scene
            .fill(Fill::NonZero, Affine::IDENTITY, MARQUEE_FILL, None, &rect);
        self.scene.stroke(
            &Stroke::new(MARQUEE_STROKE_PX),
            Affine::IDENTITY,
            MARQUEE_STROKE,
            None,
            &rect,
        );
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

/// Whether the path is a closed contour (so handle/point wrap-around
/// is allowed). All three Path variants expose a `closed: bool`.
fn path_is_closed(path: &Path) -> bool {
    match path {
        Path::Cubic(c) => c.closed,
        Path::Quadratic(q) => q.closed,
        Path::Hyper(h) => h.closed,
    }
}

