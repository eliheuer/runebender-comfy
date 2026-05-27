// WebGPU renderer for the Runebender canvas, built on Vello.
//
// Gated on wasm32 because Vello's `util::RenderContext::create_surface`
// expects a `wgpu::SurfaceTarget`, and the only `SurfaceTarget` we
// ever hand it is an `HtmlCanvasElement` — that's a browser-only
// path. The path/model/editing modules build on both native and
// wasm32 so unit tests still run on `cargo test`.

#![cfg(target_arch = "wasm32")]

use kurbo::{Affine, BezPath, Circle, Ellipse, Line, Point, Rect, Stroke};
use runebender_core::theme;
use serde::Deserialize;
use vello::peniko::{Fill, color::AlphaColor};
use vello::wgpu;
use vello::wgpu::util::TextureBlitter;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::editor::{
    EditorState, KnifePreview, MeasurePreview, PenPreview, SegmentHoverPreview, ShapePreview,
};
use crate::path::{Path, PathPoint, PointType};

// ============================================================================
// PALETTE
// ============================================================================

type Srgb = AlphaColor<vello::peniko::color::Srgb>;

const fn srgb(color: theme::ColorRgba) -> Srgb {
    AlphaColor::from_rgba8(color.r, color.g, color.b, color.a)
}

const BG: Srgb = srgb(theme::app::BACKGROUND);
const PATH_STROKE: Srgb = srgb(theme::path::STROKE);
const PREVIEW_FILL: Srgb = srgb(theme::path::PREVIEW_FILL);
const COMPONENT_FILL: Srgb = srgb(theme::component::FILL);
const COMPONENT_SELECTED_FILL: Srgb = srgb(theme::component::SELECTED_FILL);
const HANDLE_LINE: Srgb = srgb(theme::handle::LINE);
const POINT_SMOOTH_INNER: Srgb = srgb(theme::point::SMOOTH_INNER);
const POINT_SMOOTH_OUTER: Srgb = srgb(theme::point::SMOOTH_OUTER);
const POINT_CORNER_INNER: Srgb = srgb(theme::point::CORNER_INNER);
const POINT_CORNER_OUTER: Srgb = srgb(theme::point::CORNER_OUTER);
const POINT_OFFCURVE_INNER: Srgb = srgb(theme::point::OFFCURVE_INNER);
const POINT_OFFCURVE_OUTER: Srgb = srgb(theme::point::OFFCURVE_OUTER);
const POINT_HYPER_INNER: Srgb = srgb(theme::point::HYPER_INNER);
const POINT_HYPER_OUTER: Srgb = srgb(theme::point::HYPER_OUTER);
const POINT_SELECTED_INNER: Srgb = srgb(theme::point::SELECTED_INNER);
const POINT_SELECTED_OUTER: Srgb = srgb(theme::point::SELECTED_OUTER);
const START_NODE_OUTER: Srgb = srgb(theme::point::START_NODE_OUTER);
const MARQUEE_FILL: Srgb = srgb(theme::selection::RECT_FILL);
const MARQUEE_STROKE: Srgb = srgb(theme::selection::RECT_STROKE);
const TOOL_PREVIEW: Srgb = srgb(theme::segment::HOVER);
const METRIC_GUIDE: Srgb = srgb(theme::metrics::GUIDE);
const DESIGN_GRID_FINE: Srgb = srgb(theme::design_grid::FINE);
const DESIGN_GRID_COARSE: Srgb = srgb(theme::design_grid::COARSE);
const TEXT_PREVIEW_FILL: Srgb = srgb(theme::grid::GLYPH);
const TEXT_CURSOR: Srgb = srgb(theme::selection::RECT_STROKE);
const TEXT_KERN_ACTIVE: Srgb = srgb(theme::kerning::ACTIVE_GLYPH);
const TEXT_KERN_PREVIOUS: Srgb = srgb(theme::kerning::PREVIOUS_GLYPH);

#[derive(Clone)]
struct CanvasTheme {
    bg: Srgb,
    path_stroke: Srgb,
    preview_fill: Srgb,
    component_fill: Srgb,
    component_selected_fill: Srgb,
    handle_line: Srgb,
    point_smooth_inner: Srgb,
    point_smooth_outer: Srgb,
    point_corner_inner: Srgb,
    point_corner_outer: Srgb,
    point_offcurve_inner: Srgb,
    point_offcurve_outer: Srgb,
    point_hyper_inner: Srgb,
    point_hyper_outer: Srgb,
    point_selected_inner: Srgb,
    point_selected_outer: Srgb,
    start_node_outer: Srgb,
    marquee_fill: Srgb,
    marquee_stroke: Srgb,
    tool_preview: Srgb,
    metric_guide: Srgb,
    design_grid_fine: Srgb,
    design_grid_coarse: Srgb,
    text_preview_fill: Srgb,
    text_cursor: Srgb,
    text_kern_active: Srgb,
    text_kern_previous: Srgb,
}

impl Default for CanvasTheme {
    fn default() -> Self {
        Self {
            bg: BG,
            path_stroke: PATH_STROKE,
            preview_fill: PREVIEW_FILL,
            component_fill: COMPONENT_FILL,
            component_selected_fill: COMPONENT_SELECTED_FILL,
            handle_line: HANDLE_LINE,
            point_smooth_inner: POINT_SMOOTH_INNER,
            point_smooth_outer: POINT_SMOOTH_OUTER,
            point_corner_inner: POINT_CORNER_INNER,
            point_corner_outer: POINT_CORNER_OUTER,
            point_offcurve_inner: POINT_OFFCURVE_INNER,
            point_offcurve_outer: POINT_OFFCURVE_OUTER,
            point_hyper_inner: POINT_HYPER_INNER,
            point_hyper_outer: POINT_HYPER_OUTER,
            point_selected_inner: POINT_SELECTED_INNER,
            point_selected_outer: POINT_SELECTED_OUTER,
            start_node_outer: START_NODE_OUTER,
            marquee_fill: MARQUEE_FILL,
            marquee_stroke: MARQUEE_STROKE,
            tool_preview: TOOL_PREVIEW,
            metric_guide: METRIC_GUIDE,
            design_grid_fine: DESIGN_GRID_FINE,
            design_grid_coarse: DESIGN_GRID_COARSE,
            text_preview_fill: TEXT_PREVIEW_FILL,
            text_cursor: TEXT_CURSOR,
            text_kern_active: TEXT_KERN_ACTIVE,
            text_kern_previous: TEXT_KERN_PREVIOUS,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanvasThemeInput {
    bg: Option<[u8; 4]>,
    path_stroke: Option<[u8; 4]>,
    preview_fill: Option<[u8; 4]>,
    component_fill: Option<[u8; 4]>,
    component_selected_fill: Option<[u8; 4]>,
    handle_line: Option<[u8; 4]>,
    point_smooth_inner: Option<[u8; 4]>,
    point_smooth_outer: Option<[u8; 4]>,
    point_corner_inner: Option<[u8; 4]>,
    point_corner_outer: Option<[u8; 4]>,
    point_offcurve_inner: Option<[u8; 4]>,
    point_offcurve_outer: Option<[u8; 4]>,
    point_hyper_inner: Option<[u8; 4]>,
    point_hyper_outer: Option<[u8; 4]>,
    point_selected_inner: Option<[u8; 4]>,
    point_selected_outer: Option<[u8; 4]>,
    start_node_outer: Option<[u8; 4]>,
    marquee_fill: Option<[u8; 4]>,
    marquee_stroke: Option<[u8; 4]>,
    tool_preview: Option<[u8; 4]>,
    metric_guide: Option<[u8; 4]>,
    design_grid_fine: Option<[u8; 4]>,
    design_grid_coarse: Option<[u8; 4]>,
    text_preview_fill: Option<[u8; 4]>,
    text_cursor: Option<[u8; 4]>,
    text_kern_active: Option<[u8; 4]>,
    text_kern_previous: Option<[u8; 4]>,
}

impl CanvasTheme {
    fn apply_input(&mut self, input: CanvasThemeInput) {
        macro_rules! apply_color {
            ($field:ident) => {
                if let Some([r, g, b, a]) = input.$field {
                    self.$field = AlphaColor::from_rgba8(r, g, b, a);
                }
            };
        }
        apply_color!(bg);
        apply_color!(path_stroke);
        apply_color!(preview_fill);
        apply_color!(component_fill);
        apply_color!(component_selected_fill);
        apply_color!(handle_line);
        apply_color!(point_smooth_inner);
        apply_color!(point_smooth_outer);
        apply_color!(point_corner_inner);
        apply_color!(point_corner_outer);
        apply_color!(point_offcurve_inner);
        apply_color!(point_offcurve_outer);
        apply_color!(point_hyper_inner);
        apply_color!(point_hyper_outer);
        apply_color!(point_selected_inner);
        apply_color!(point_selected_outer);
        apply_color!(start_node_outer);
        apply_color!(marquee_fill);
        apply_color!(marquee_stroke);
        apply_color!(tool_preview);
        apply_color!(metric_guide);
        apply_color!(design_grid_fine);
        apply_color!(design_grid_coarse);
        apply_color!(text_preview_fill);
        apply_color!(text_cursor);
        apply_color!(text_kern_active);
        apply_color!(text_kern_previous);
    }
}

// --- Sizes (xilem size::*; STROKE_SCALE = 1.5) ---
const STROKE_SCALE: f64 = 1.5;
const SMOOTH_POINT_RADIUS_PX: f64 = 4.5;
const SMOOTH_POINT_SELECTED_RADIUS_PX: f64 = 5.5;
const CORNER_POINT_HALF_PX: f64 = 3.5;
const CORNER_POINT_SELECTED_HALF_PX: f64 = 4.5;
const OFFCURVE_POINT_RADIUS_PX: f64 = 3.0;
const OFFCURVE_POINT_SELECTED_RADIUS_PX: f64 = 4.0;
const HYPER_POINT_RADIUS_PX: f64 = 4.0;
const HYPER_POINT_SELECTED_RADIUS_PX: f64 = 5.0;
const START_NODE_HALF_PX: f64 = 5.5;
const START_NODE_SELECTED_HALF_PX: f64 = 6.5;
const START_NODE_OFFSET_PX: f64 = 8.0;
const POINT_OUTLINE_PX: f64 = 1.0 * STROKE_SCALE;
const PATH_STROKE_PX: f64 = 1.0 * STROKE_SCALE;
const COMPONENT_SELECTION_STROKE_PX: f64 = 2.0;
const HANDLE_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const MARQUEE_STROKE_PX: f64 = 1.0 * STROKE_SCALE;
const METRIC_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const TOOL_PREVIEW_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const SEGMENT_HOVER_LINE_PX: f64 = 3.0;
const TOOL_PREVIEW_DOT_RADIUS_PX: f64 = 3.0;
const TEXT_CURSOR_LINE_PX: f64 = 1.5;
const TEXT_CURSOR_TRIANGLE_WIDTH_PX: f64 = 24.0;
const TEXT_CURSOR_TRIANGLE_HEIGHT_PX: f64 = 16.0;
const TEXT_METRIC_CROSS_SIZE: f64 = 24.0;
const DESIGN_GRID_MID_MIN_ZOOM: f64 = 2.5;
const DESIGN_GRID_MID_FINE: f64 = 8.0;
const DESIGN_GRID_MID_COARSE_N: u32 = 4;
const DESIGN_GRID_CLOSE_MIN_ZOOM: f64 = 12.0;
const DESIGN_GRID_CLOSE_FINE: f64 = 2.0;
const DESIGN_GRID_CLOSE_COARSE_N: u32 = 4;
const DESIGN_GRID_FINE_LINE_PX: f64 = 0.5;
const DESIGN_GRID_COARSE_LINE_PX: f64 = 1.0;

// ============================================================================
// RENDERER
// ============================================================================

pub struct Renderer {
    // Hand-rolled wgpu setup (instead of vello::util::RenderContext) so
    // we can request the adapter's full max_texture_dimension_2d. Vello
    // 0.8's RenderContext hardcodes Limits::default(), which caps
    // textures at 8192 — too small for full-DPR rendering on Retina/5K
    // displays.
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    vello: VelloRenderer,
    scene: Scene,
    theme: CanvasTheme,
    device_scale: f64,
    width: u32,
    height: u32,
}

impl Renderer {
    pub async fn new(canvas: HtmlCanvasElement, width: u32, height: u32) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("create_surface: {e:?}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_adapter: {e:?}")))?;

        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_texture_dimension_2d = adapter_limits.max_texture_dimension_2d;

        let optional_features = wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE;
        let required_features = adapter.features() & optional_features;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("runebender device"),
                required_features,
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_device: {e:?}")))?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .into_iter()
            .find(|fmt| {
                matches!(
                    fmt,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .ok_or_else(|| JsValue::from_str("no compatible surface format"))?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let (target_texture, target_view) = create_intermediate_target(width, height, &device);
        let blitter = TextureBlitter::new(&device, surface_format);

        let vello = VelloRenderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .map_err(|e| JsValue::from_str(&format!("Renderer::new: {e:?}")))?;

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            target_texture,
            target_view,
            blitter,
            vello,
            scene: Scene::new(),
            theme: CanvasTheme::default(),
            device_scale: 1.0,
            width,
            height,
        })
    }

    pub fn set_theme_json(&mut self, theme_json: &str) -> Result<(), JsValue> {
        let input: CanvasThemeInput = serde_json::from_str(theme_json)
            .map_err(|e| JsValue::from_str(&format!("parse canvas theme: {e}")))?;
        self.theme.apply_input(input);
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        let (target_texture, target_view) = create_intermediate_target(width, height, &self.device);
        self.target_texture = target_texture;
        self.target_view = target_view;
        self.width = width;
        self.height = height;
    }

    pub fn set_device_scale(&mut self, scale: f64) {
        self.device_scale = scale.clamp(1.0, 8.0);
    }

    fn px(&self, value: f64) -> f64 {
        value * self.device_scale
    }

    fn point_scale(&self, zoom: f64) -> f64 {
        // xilem grows point affordances slightly at high zoom. In comfy,
        // viewport zoom is measured in backing pixels, so convert it back
        // to CSS/logical pixels before applying the same curve.
        const THRESHOLD: f64 = 4.0;
        let logical_zoom = zoom / self.device_scale.max(1.0);
        let zoom_scale = if logical_zoom <= THRESHOLD {
            1.0
        } else {
            1.0 + (logical_zoom / THRESHOLD).ln() * 0.5
        };
        self.device_scale * zoom_scale
    }

    /// Paint one frame against the given editor state.
    pub fn render(
        &mut self,
        state: &EditorState,
        preview_mode: bool,
        text_mode_active: bool,
    ) -> Result<(), JsValue> {
        self.scene.reset();
        self.draw_state(state, preview_mode, text_mode_active);
        self.present()
    }

    fn draw_state(&mut self, state: &EditorState, preview_mode: bool, text_mode_active: bool) {
        let view = state.viewport.affine();
        let active_sort_origin = state.active_text_sort_origin();
        let glyph_view = view * Affine::translate(active_sort_origin);
        let has_text_session = state.has_text_session;

        if !preview_mode {
            self.draw_design_grid(state, view, active_sort_origin.x);

            // Metric guides go in first so the glyph fill paints on top.
            if !has_text_session {
                self.draw_metric_guides(state, glyph_view);
            }
        }

        // Glyph fill (in design space — viewport applies the Y-flip).
        // Combine every contour into ONE BezPath before filling so the
        // NonZero winding rule treats opposite-wound inner contours as
        // holes (UFO/PostScript convention). Filling each contour
        // separately would paint counters solid.
        let mut combined = editable_outline_path(state);
        if has_text_session {
            self.draw_text_buffer(state, view, preview_mode, text_mode_active);
            if !preview_mode && !text_mode_active {
                self.draw_edit_controls(state, glyph_view);
            }
            return;
        }

        if preview_mode {
            for component in &state.component_previews {
                let transformed = component.transform * &component.path;
                for el in transformed.elements() {
                    combined.push(*el);
                }
            }
            if !combined.elements().is_empty() {
                self.scene.fill(
                    Fill::NonZero,
                    glyph_view,
                    self.theme.preview_fill,
                    None,
                    &combined,
                );
            }
            self.draw_text_buffer(state, view, true, text_mode_active);
            return;
        }
        if !combined.elements().is_empty() {
            // Edit mode: STROKE the outline at a constant screen-pixel
            // width and leave the interior empty, matching
            // runebender-xilem's paint_glyph_edit_mode. Transform the
            // path into screen space first so the stroke width doesn't
            // scale with zoom. (Components below are still filled.)
            let screen_path = glyph_view * &combined;
            self.scene.stroke(
                &Stroke::new(self.px(PATH_STROKE_PX)),
                Affine::IDENTITY,
                self.theme.path_stroke,
                None,
                &screen_path,
            );
        }
        for component in &state.component_previews {
            let transformed = component.transform * &component.path;
            if transformed.elements().is_empty() {
                continue;
            }
            let fill = if state.selected_component == Some(component.id) {
                self.theme.component_selected_fill
            } else {
                self.theme.component_fill
            };
            self.scene
                .fill(Fill::NonZero, glyph_view, fill, None, &transformed);
            if state.selected_component == Some(component.id) {
                let screen_path = glyph_view * &transformed;
                self.scene.stroke(
                    &Stroke::new(self.px(COMPONENT_SELECTION_STROKE_PX)),
                    Affine::IDENTITY,
                    self.theme.text_cursor,
                    None,
                    &screen_path,
                );
            }
        }
        self.draw_edit_controls(state, glyph_view);
    }

    fn draw_edit_controls(&mut self, state: &EditorState, glyph_view: Affine) {
        // Handle lines and points are drawn in screen space so they
        // stay at constant pixel size regardless of zoom.
        for path in &state.paths {
            self.draw_handle_lines(path, glyph_view);
        }
        for path in &state.paths {
            self.draw_points(path, glyph_view, &state.selection, state.viewport.zoom);
        }

        if let Some(preview) = state.segment_hover {
            self.draw_segment_hover(preview);
        }
        if let Some(rect) = state.marquee {
            self.draw_marquee(rect);
        }
        if let Some(preview) = state.shape_preview {
            self.draw_shape_preview(preview);
        }
        if let Some(preview) = state.pen_preview {
            self.draw_pen_preview(preview);
        }
        if let Some(preview) = state.measure_preview.as_ref() {
            self.draw_measure_preview(preview);
        }
        if let Some(preview) = state.knife_preview.as_ref() {
            self.draw_knife_preview(preview);
        }
    }

    fn draw_text_buffer(
        &mut self,
        state: &EditorState,
        view: Affine,
        preview_mode: bool,
        text_mode_active: bool,
    ) {
        let (ascender, descender) = state.text_metric_bounds();
        let line_height = state.text_line_height();
        let layout = state.text_buffer.layout(line_height);
        let kern_sort_index = state.text_buffer.manual_kerning_sort();

        if !preview_mode {
            for item in &layout.items {
                let sort_active = state
                    .text_buffer
                    .sort(item.index)
                    .map(|sort| sort.active)
                    .unwrap_or(false);
                if !text_mode_active && sort_active {
                    self.draw_text_sort_metrics(state, item.x, item.y, item.advance_width, view);
                    continue;
                }
                let metric_color = if text_mode_active {
                    match kern_sort_index {
                        Some(index) if index == item.index => self.theme.text_kern_active,
                        Some(index) if index == item.index + 1 => self.theme.text_kern_previous,
                        _ => self.theme.metric_guide,
                    }
                } else if sort_active {
                    self.theme.text_cursor
                } else {
                    self.theme.metric_guide
                };
                self.draw_text_sort_minimal_metrics(
                    item.x,
                    item.y,
                    item.advance_width,
                    ascender,
                    descender,
                    view,
                    metric_color,
                );
            }
        }

        for item in &layout.items {
            let Some(sort) = state.text_buffer.sort(item.index) else {
                continue;
            };
            let render_active_editable = !preview_mode && sort.active && !text_mode_active;
            if render_active_editable {
                let active_outline = editable_outline_path(state);
                if !active_outline.elements().is_empty() {
                    let screen_path =
                        (view * Affine::translate((item.x, item.y))) * &active_outline;
                    self.scene.stroke(
                        &Stroke::new(self.px(PATH_STROKE_PX)),
                        Affine::IDENTITY,
                        self.theme.path_stroke,
                        None,
                        &screen_path,
                    );
                }
                for component in &state.component_previews {
                    let transformed = component.transform * &component.path;
                    if transformed.elements().is_empty() {
                        continue;
                    }
                    let component_fill = if state.selected_component == Some(component.id) {
                        self.theme.component_selected_fill
                    } else {
                        self.theme.component_fill
                    };
                    self.scene.fill(
                        Fill::NonZero,
                        view * Affine::translate((item.x, item.y)),
                        component_fill,
                        None,
                        &transformed,
                    );
                    if state.selected_component == Some(component.id) {
                        let screen_path =
                            (view * Affine::translate((item.x, item.y))) * &transformed;
                        self.scene.stroke(
                            &Stroke::new(self.px(COMPONENT_SELECTION_STROKE_PX)),
                            Affine::IDENTITY,
                            self.theme.text_cursor,
                            None,
                            &screen_path,
                        );
                    }
                }
            } else {
                let Some(glyph_name) = sort.glyph_name() else {
                    continue;
                };
                let Some(outline) = state.text_buffer.glyph_outline_svg(glyph_name) else {
                    continue;
                };
                let Ok(path) = BezPath::from_svg(outline) else {
                    continue;
                };
                if path.elements().is_empty() {
                    continue;
                }
                self.scene.fill(
                    Fill::NonZero,
                    view * Affine::translate((item.x, item.y)),
                    self.theme.text_preview_fill,
                    None,
                    &path,
                );
            }
        }

        if !preview_mode && text_mode_active {
            self.draw_text_cursor(layout.cursor_x, layout.cursor_y, ascender, descender, view);
        }
    }

    fn draw_text_cursor(
        &mut self,
        cursor_x: f64,
        baseline_y: f64,
        ascender: f64,
        descender: f64,
        view: Affine,
    ) {
        let top = view * Point::new(cursor_x, baseline_y + ascender);
        let bottom = view * Point::new(cursor_x, baseline_y + descender);
        self.scene.stroke(
            &Stroke::new(TEXT_CURSOR_LINE_PX),
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &Line::new(top, bottom),
        );

        let mut top_triangle = BezPath::new();
        top_triangle.move_to((top.x - TEXT_CURSOR_TRIANGLE_WIDTH_PX / 2.0, top.y));
        top_triangle.line_to((top.x + TEXT_CURSOR_TRIANGLE_WIDTH_PX / 2.0, top.y));
        top_triangle.line_to((top.x, top.y + TEXT_CURSOR_TRIANGLE_HEIGHT_PX));
        top_triangle.close_path();
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &top_triangle,
        );

        let mut bottom_triangle = BezPath::new();
        bottom_triangle.move_to((bottom.x - TEXT_CURSOR_TRIANGLE_WIDTH_PX / 2.0, bottom.y));
        bottom_triangle.line_to((bottom.x + TEXT_CURSOR_TRIANGLE_WIDTH_PX / 2.0, bottom.y));
        bottom_triangle.line_to((bottom.x, bottom.y - TEXT_CURSOR_TRIANGLE_HEIGHT_PX));
        bottom_triangle.close_path();
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &bottom_triangle,
        );
    }

    fn draw_text_sort_minimal_metrics(
        &mut self,
        x: f64,
        baseline_y: f64,
        advance_width: f64,
        ascender: f64,
        descender: f64,
        view: Affine,
        color: Srgb,
    ) {
        let stroke = Stroke::new(METRIC_LINE_PX);
        for edge_x in [x, x + advance_width] {
            for y in [baseline_y + descender, baseline_y, baseline_y + ascender] {
                let h = Line::new(
                    Point::new(edge_x - TEXT_METRIC_CROSS_SIZE, y),
                    Point::new(edge_x + TEXT_METRIC_CROSS_SIZE, y),
                );
                let v = Line::new(
                    Point::new(edge_x, y - TEXT_METRIC_CROSS_SIZE),
                    Point::new(edge_x, y + TEXT_METRIC_CROSS_SIZE),
                );
                self.scene.stroke(&stroke, view, color, None, &h);
                self.scene.stroke(&stroke, view, color, None, &v);
            }
        }
    }

    fn draw_text_sort_metrics(
        &mut self,
        state: &EditorState,
        x: f64,
        baseline_y: f64,
        advance_width: f64,
        view: Affine,
    ) {
        let stroke = Stroke::new(METRIC_LINE_PX);
        let (ascender, descender) = state.text_metric_bounds();
        if ascender > descender {
            self.scene.stroke(
                &stroke,
                view,
                self.theme.metric_guide,
                None,
                &Line::new(
                    Point::new(x, baseline_y + descender),
                    Point::new(x, baseline_y + ascender),
                ),
            );
            self.scene.stroke(
                &stroke,
                view,
                self.theme.metric_guide,
                None,
                &Line::new(
                    Point::new(x + advance_width, baseline_y + descender),
                    Point::new(x + advance_width, baseline_y + ascender),
                ),
            );
        }

        let mut ys = vec![0.0, ascender, descender];
        if let Some(metrics) = state.metrics.as_ref() {
            ys.extend([metrics.x_height, metrics.cap_height].into_iter().flatten());
        }
        for y in ys {
            self.scene.stroke(
                &stroke,
                view,
                self.theme.metric_guide,
                None,
                &Line::new(
                    Point::new(x, baseline_y + y),
                    Point::new(x + advance_width, baseline_y + y),
                ),
            );
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
        let stroke = Stroke::new(self.px(HANDLE_LINE_PX));
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
                    self.theme.handle_line,
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
                    self.theme.handle_line,
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
        zoom: f64,
    ) {
        let scale = self.point_scale(zoom);
        let outline_stroke = Stroke::new(POINT_OUTLINE_PX * scale);
        let points = path.points().to_vec();
        let closed = path_is_closed(path);
        let start_index = closed
            .then(|| points.iter().position(PathPoint::is_on_curve))
            .flatten();
        for (index, pt) in points.iter().enumerate() {
            let center = view * pt.point;
            let selected = selection.contains(&pt.id);

            if matches!(path, Path::Hyper(_)) && pt.is_on_curve() {
                let radius = (if selected {
                    HYPER_POINT_SELECTED_RADIUS_PX
                } else {
                    HYPER_POINT_RADIUS_PX
                }) * scale;
                let (inner, outer) = if selected {
                    (
                        self.theme.point_selected_inner,
                        self.theme.point_selected_outer,
                    )
                } else {
                    (self.theme.point_hyper_inner, self.theme.point_hyper_outer)
                };
                let circle = Circle::new(center, radius);
                self.scene
                    .fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
                self.scene
                    .stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
            } else {
                let (inner, outer) = if selected {
                    (
                        self.theme.point_selected_inner,
                        self.theme.point_selected_outer,
                    )
                } else {
                    match pt.typ {
                        PointType::OnCurve { smooth: true } => {
                            (self.theme.point_smooth_inner, self.theme.point_smooth_outer)
                        }
                        PointType::OnCurve { smooth: false } => {
                            (self.theme.point_corner_inner, self.theme.point_corner_outer)
                        }
                        PointType::OffCurve { .. } => (
                            self.theme.point_offcurve_inner,
                            self.theme.point_offcurve_outer,
                        ),
                    }
                };

                match pt.typ {
                    PointType::OnCurve { smooth: true } => {
                        let radius = (if selected {
                            SMOOTH_POINT_SELECTED_RADIUS_PX
                        } else {
                            SMOOTH_POINT_RADIUS_PX
                        }) * scale;
                        let circle = Circle::new(center, radius);
                        self.scene
                            .fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
                        self.scene
                            .stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
                    }
                    PointType::OnCurve { smooth: false } => {
                        let half = (if selected {
                            CORNER_POINT_SELECTED_HALF_PX
                        } else {
                            CORNER_POINT_HALF_PX
                        }) * scale;
                        let square = Rect::new(
                            center.x - half,
                            center.y - half,
                            center.x + half,
                            center.y + half,
                        );
                        self.scene
                            .fill(Fill::NonZero, Affine::IDENTITY, inner, None, &square);
                        self.scene
                            .stroke(&outline_stroke, Affine::IDENTITY, outer, None, &square);
                    }
                    PointType::OffCurve { .. } => {
                        let radius = (if selected {
                            OFFCURVE_POINT_SELECTED_RADIUS_PX
                        } else {
                            OFFCURVE_POINT_RADIUS_PX
                        }) * scale;
                        let circle = Circle::new(center, radius);
                        self.scene
                            .fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
                        self.scene
                            .stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
                    }
                }
            }
            if start_index == Some(index) {
                let next = next_point_pos(&points, index, closed);
                self.draw_start_arrow(center, view * next, selected, scale);
            }
        }
    }

    fn draw_start_arrow(
        &mut self,
        screen_pos: Point,
        next_screen: Point,
        selected: bool,
        scale: f64,
    ) {
        let arrow_size = (if selected {
            START_NODE_SELECTED_HALF_PX
        } else {
            START_NODE_HALF_PX
        }) * scale;
        let direction = next_screen - screen_pos;
        let len = direction.hypot();
        if len < 0.001 {
            return;
        }
        let forward = direction / len;
        let perpendicular = kurbo::Vec2::new(-forward.y, forward.x);
        let center = screen_pos + perpendicular * (START_NODE_OFFSET_PX * scale);
        let tip = center + forward * arrow_size;
        let base_center = center - forward * (arrow_size * 0.5);
        let base_left = base_center + perpendicular * (arrow_size * 0.5);
        let base_right = base_center - perpendicular * (arrow_size * 0.5);
        let mut arrow = BezPath::new();
        arrow.move_to(tip);
        arrow.line_to(base_left);
        arrow.line_to(base_right);
        arrow.close_path();
        let fill = if selected {
            self.theme.point_selected_outer
        } else {
            self.theme.start_node_outer
        };
        self.scene
            .fill(Fill::NonZero, Affine::IDENTITY, fill, None, &arrow);
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
            scene.stroke(
                &stroke,
                view,
                self.theme.metric_guide,
                None,
                &Line::new(p0, p1),
            );
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

    /// Draw the zoom-dependent design-space grid behind the glyph.
    ///
    /// The mid level shows 8-unit spacing with 32-unit coarse lines;
    /// the close level adds a 2-unit grid with 8-unit coarse lines.
    /// Comfy shows more surrounding canvas than xilem, so both levels
    /// intentionally appear later to avoid filling the screen with grid
    /// lines while the glyph is still small.
    fn draw_design_grid(&mut self, state: &EditorState, view: Affine, origin_x: f64) {
        let zoom = state.viewport.zoom;
        if zoom < DESIGN_GRID_MID_MIN_ZOOM {
            return;
        }

        let top_left = state.viewport.screen_to_design(Point::ZERO);
        let bottom_right = state
            .viewport
            .screen_to_design(Point::new(self.width as f64, self.height as f64));
        let min_x = top_left.x.min(bottom_right.x);
        let max_x = top_left.x.max(bottom_right.x);
        let min_y = top_left.y.min(bottom_right.y);
        let max_y = top_left.y.max(bottom_right.y);

        self.draw_grid_level(
            view,
            DESIGN_GRID_MID_FINE,
            DESIGN_GRID_MID_COARSE_N,
            min_x,
            max_x,
            min_y,
            max_y,
            origin_x,
        );

        if zoom >= DESIGN_GRID_CLOSE_MIN_ZOOM {
            self.draw_grid_level(
                view,
                DESIGN_GRID_CLOSE_FINE,
                DESIGN_GRID_CLOSE_COARSE_N,
                min_x,
                max_x,
                min_y,
                max_y,
                origin_x,
            );
        }
    }

    fn draw_grid_level(
        &mut self,
        view: Affine,
        spacing: f64,
        coarse_n: u32,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        origin_x: f64,
    ) {
        let fine_stroke = Stroke::new(DESIGN_GRID_FINE_LINE_PX);
        let coarse_stroke = Stroke::new(DESIGN_GRID_COARSE_LINE_PX);
        let start_x = ((min_x - origin_x) / spacing).floor() as i64;
        let end_x = ((max_x - origin_x) / spacing).ceil() as i64;
        let start_y = (min_y / spacing).floor() as i64;
        let end_y = (max_y / spacing).ceil() as i64;

        for ix in start_x..=end_x {
            let x = origin_x + ix as f64 * spacing;
            let is_coarse = coarse_n > 0 && (ix.unsigned_abs() % coarse_n as u64 == 0);
            let (stroke, color) = if is_coarse {
                (&coarse_stroke, self.theme.design_grid_coarse)
            } else {
                (&fine_stroke, self.theme.design_grid_fine)
            };
            let p0 = view * Point::new(x, min_y);
            let p1 = view * Point::new(x, max_y);
            self.scene
                .stroke(stroke, Affine::IDENTITY, color, None, &Line::new(p0, p1));
        }

        for iy in start_y..=end_y {
            let y = iy as f64 * spacing;
            let is_coarse = coarse_n > 0 && (iy.unsigned_abs() % coarse_n as u64 == 0);
            let (stroke, color) = if is_coarse {
                (&coarse_stroke, self.theme.design_grid_coarse)
            } else {
                (&fine_stroke, self.theme.design_grid_fine)
            };
            let p0 = view * Point::new(min_x, y);
            let p1 = view * Point::new(max_x, y);
            self.scene
                .stroke(stroke, Affine::IDENTITY, color, None, &Line::new(p0, p1));
        }
    }

    fn draw_marquee(&mut self, rect: kurbo::Rect) {
        // Marquee is already in screen space; draw with identity.
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.marquee_fill,
            None,
            &rect,
        );
        self.scene.stroke(
            &Stroke::new(self.px(MARQUEE_STROKE_PX)).with_dashes(0.0, [self.px(4.0), self.px(4.0)]),
            Affine::IDENTITY,
            self.theme.marquee_stroke,
            None,
            &rect,
        );
    }

    fn draw_shape_preview(&mut self, preview: ShapePreview) {
        let stroke = Stroke::new(TOOL_PREVIEW_LINE_PX);
        let rect = match preview {
            ShapePreview::Rectangle(rect) => {
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    self.theme.tool_preview,
                    None,
                    &rect,
                );
                rect
            }
            ShapePreview::Ellipse(rect) => {
                let ellipse = Ellipse::from_rect(rect);
                self.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    self.theme.tool_preview,
                    None,
                    &ellipse,
                );
                rect
            }
        };

        for point in [rect.origin(), rect.origin() + rect.size().to_vec2()] {
            let dot = Circle::new(point, TOOL_PREVIEW_DOT_RADIUS_PX);
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &dot,
            );
        }
    }

    fn draw_segment_hover(&mut self, preview: SegmentHoverPreview) {
        let stroke = Stroke::new(SEGMENT_HOVER_LINE_PX);
        let mut path = BezPath::new();
        match preview {
            SegmentHoverPreview::Line(line) => {
                path.move_to(line.p0);
                path.line_to(line.p1);
            }
            SegmentHoverPreview::Cubic(cubic) => {
                path.move_to(cubic.p0);
                path.curve_to(cubic.p1, cubic.p2, cubic.p3);
            }
            SegmentHoverPreview::Quadratic(quad) => {
                path.move_to(quad.p0);
                path.quad_to(quad.p1, quad.p2);
            }
        }
        self.scene.stroke(
            &stroke,
            Affine::IDENTITY,
            self.theme.tool_preview,
            None,
            &path,
        );
    }

    fn draw_pen_preview(&mut self, preview: PenPreview) {
        let stroke = Stroke::new(TOOL_PREVIEW_LINE_PX);
        if let Some(start) = preview.line_start {
            let target = preview
                .close_target
                .or(preview.snap_target)
                .unwrap_or(preview.cursor);
            self.scene.stroke(
                &stroke,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &Line::new(start, target),
            );
        }

        let dot = Circle::new(preview.cursor, TOOL_PREVIEW_DOT_RADIUS_PX);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.tool_preview,
            None,
            &dot,
        );

        if let Some(close_target) = preview.close_target {
            let close_zone = Circle::new(close_target, TOOL_PREVIEW_DOT_RADIUS_PX * 2.0);
            self.scene.stroke(
                &stroke,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &close_zone,
            );
        }
        if let Some(snap_target) = preview.snap_target {
            let snap_zone = Circle::new(snap_target, TOOL_PREVIEW_DOT_RADIUS_PX * 2.5);
            self.scene.stroke(
                &stroke,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &snap_zone,
            );
        }
    }

    fn draw_measure_preview(&mut self, preview: &MeasurePreview) {
        let stroke = Stroke::new(TOOL_PREVIEW_LINE_PX);
        self.scene.stroke(
            &stroke,
            Affine::IDENTITY,
            self.theme.tool_preview,
            None,
            &preview.line,
        );

        for point in [preview.line.p0, preview.line.p1] {
            let dot = Circle::new(point, TOOL_PREVIEW_DOT_RADIUS_PX);
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &dot,
            );
        }
        for point in &preview.intersections {
            let dot = Circle::new(*point, TOOL_PREVIEW_DOT_RADIUS_PX * 1.4);
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &dot,
            );
        }
    }

    fn draw_knife_preview(&mut self, preview: &KnifePreview) {
        let stroke = Stroke::new(TOOL_PREVIEW_LINE_PX);
        self.scene.stroke(
            &stroke,
            Affine::IDENTITY,
            self.theme.tool_preview,
            None,
            &preview.line,
        );
        for point in &preview.intersections {
            let size = TOOL_PREVIEW_DOT_RADIUS_PX * 1.8;
            self.scene.stroke(
                &stroke,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &Line::new(
                    (point.x - size, point.y - size),
                    (point.x + size, point.y + size),
                ),
            );
            self.scene.stroke(
                &stroke,
                Affine::IDENTITY,
                self.theme.tool_preview,
                None,
                &Line::new(
                    (point.x - size, point.y + size),
                    (point.x + size, point.y - size),
                ),
            );
        }
    }

    fn present(&mut self) -> Result<(), JsValue> {
        let surface_texture = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture: {e:?}")))?;

        self.vello
            .render_to_texture(
                &self.device,
                &self.queue,
                &self.scene,
                &self.target_view,
                &vello::RenderParams {
                    base_color: self.theme.bg.into(),
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("runebender blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit([encoder.finish()]);

        surface_texture.present();
        Ok(())
    }
}

fn create_intermediate_target(
    width: u32,
    height: u32,
    device: &wgpu::Device,
) -> (wgpu::Texture, wgpu::TextureView) {
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("runebender intermediate target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        format: wgpu::TextureFormat::Rgba8Unorm,
        view_formats: &[],
    });
    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (target_texture, target_view)
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

fn next_point_pos(points: &[PathPoint], index: usize, closed: bool) -> Point {
    if index + 1 < points.len() {
        points[index + 1].point
    } else if closed && !points.is_empty() {
        points[0].point
    } else {
        points[index].point + kurbo::Vec2::new(1.0, 0.0)
    }
}

fn editable_outline_path(state: &EditorState) -> BezPath {
    let mut combined = BezPath::new();
    for path in &state.paths {
        for el in path.to_bezpath().elements() {
            combined.push(*el);
        }
    }
    combined
}
