// WebGPU renderer for the Runebender canvas, built on Vello.
//
// Gated on wasm32 because Vello's `util::RenderContext::create_surface`
// expects a `wgpu::SurfaceTarget`, and the only `SurfaceTarget` we
// ever hand it is an `HtmlCanvasElement` — that's a browser-only
// path. The path/model/editing modules build on both native and
// wasm32 so unit tests still run on `cargo test`. (Gating lives in lib.rs.)

use kurbo::{Affine, BezPath, Circle, Ellipse, Line, Point, Rect, Stroke};
use runebender_core::theme;
use serde::Deserialize;
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use vello::peniko::{Fill, color::AlphaColor};
use vello::wgpu;
use vello::wgpu::util::TextureBlitter;
use vello::{AaConfig, Renderer as VelloRenderer, RendererOptions, Scene};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use crate::editor::{
    EditorState, KnifePreview, MeasurePreview, PenPreview, SegmentHoverPreview, ShapePreview,
};
use crate::model::EntityId;
use crate::path::{Path, PathPoint, PointType};
use crate::text::TextLayout;

// ============================================================================
// PALETTE
// ============================================================================

type Srgb = AlphaColor<vello::peniko::color::Srgb>;

const fn srgb(color: theme::ColorRgba) -> Srgb {
    AlphaColor::from_rgba8(color.r, color.g, color.b, color.a)
}

const BG: Srgb = srgb(theme::app::BACKGROUND);
const PATH_STROKE: Srgb = srgb(theme::path::STROKE);
const PATH_EDIT_FILL: Srgb = AlphaColor::from_rgba8(0x60, 0x60, 0x60, 0x22);
const PREVIEW_FILL: Srgb = srgb(theme::path::PREVIEW_FILL);
const COMPONENT_FILL: Srgb = srgb(theme::component::FILL);
const COMPONENT_SELECTED_FILL: Srgb = srgb(theme::component::SELECTED_FILL);
const HANDLE_LINE: Srgb = srgb(theme::handle::LINE);
const POINT_INNER: Srgb = AlphaColor::from_rgba8(0x18, 0x18, 0x18, 0xff);
const POINT_MARK_RED: Srgb = AlphaColor::from_rgba8(0xff, 0x4a, 0x3d, 0xff);
const POINT_MARK_GREEN: Srgb = AlphaColor::from_rgba8(0x18, 0xb8, 0x6f, 0xff);
const POINT_MARK_PURPLE: Srgb = AlphaColor::from_rgba8(0x8c, 0x6c, 0xff, 0xff);
const POINT_MARK_YELLOW: Srgb = AlphaColor::from_rgba8(0xff, 0xdc, 0x32, 0xff);
const POINT_MARK_ORANGE: Srgb = AlphaColor::from_rgba8(0xff, 0x98, 0x0f, 0xff);
const POINT_SMOOTH_INNER: Srgb = POINT_INNER;
const POINT_SMOOTH_OUTER: Srgb = POINT_MARK_GREEN;
const POINT_CORNER_INNER: Srgb = POINT_INNER;
const POINT_CORNER_OUTER: Srgb = POINT_MARK_ORANGE;
const POINT_OFFCURVE_INNER: Srgb = POINT_INNER;
const POINT_OFFCURVE_OUTER: Srgb = POINT_MARK_PURPLE;
const POINT_HYPER_INNER: Srgb = POINT_INNER;
const POINT_HYPER_OUTER: Srgb = POINT_MARK_PURPLE;
const POINT_SELECTED_INNER: Srgb = POINT_MARK_YELLOW;
const POINT_SELECTED_OUTER: Srgb = POINT_MARK_ORANGE;
const START_NODE_OUTER: Srgb = POINT_MARK_ORANGE;
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
    path_edit_fill: Srgb,
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
            path_edit_fill: PATH_EDIT_FILL,
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
const OFFCURVE_POINT_RADIUS_PX: f64 = SMOOTH_POINT_RADIUS_PX;
const OFFCURVE_POINT_SELECTED_RADIUS_PX: f64 = SMOOTH_POINT_SELECTED_RADIUS_PX;
const HYPER_POINT_RADIUS_PX: f64 = 4.0;
const HYPER_POINT_SELECTED_RADIUS_PX: f64 = 5.0;
const START_NODE_HALF_PX: f64 = 5.5;
const START_NODE_SELECTED_HALF_PX: f64 = 6.5;
const START_NODE_OFFSET_PX: f64 = 8.0;
const POINT_OUTLINE_PX: f64 = 1.25 * STROKE_SCALE;
const PATH_STROKE_PX: f64 = 1.0 * STROKE_SCALE;
const COMPONENT_SELECTION_STROKE_PX: f64 = 2.0;
const HANDLE_LINE_PX: f64 = 1.25 * STROKE_SCALE;
const MARQUEE_STROKE_PX: f64 = 1.0 * STROKE_SCALE;
const METRIC_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const TOOL_PREVIEW_LINE_PX: f64 = 1.0 * STROKE_SCALE;
const SEGMENT_HOVER_LINE_PX: f64 = 3.0;
const TOOL_PREVIEW_DOT_RADIUS_PX: f64 = 3.0;
const KNIFE_PREVIEW_DASH: [f64; 2] = [4.0, 4.0];
const TEXT_CURSOR_LINE_PX: f64 = 1.5;
const TEXT_CURSOR_LINE_MAX_PX: f64 = 4.0;
const TEXT_CURSOR_TRIANGLE_WIDTH_PX: f64 = 24.0;
const TEXT_CURSOR_TRIANGLE_HEIGHT_PX: f64 = 16.0;
const TEXT_METRIC_CROSS_SIZE: f64 = 24.0;
const TEXT_METRIC_CROSS_MIN_SIZE: f64 = 12.0;
const DESIGN_GRID_MID_MIN_ZOOM: f64 = 0.8;
const DESIGN_GRID_MID_FINE: f64 = 8.0;
const DESIGN_GRID_MID_COARSE_N: u32 = 4;
const DESIGN_GRID_CLOSE_MIN_ZOOM: f64 = 8.0;
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
    editable_outline_cache: Option<EditableOutlineCacheEntry>,
    path_outline_cache: HashMap<EntityId, PathOutlineCacheEntry>,
    edit_controls_cache: HashMap<EntityId, EditControlsCacheEntry>,
    design_grid_cache: Vec<DesignGridCacheEntry>,
    text_outline_cache: HashMap<String, TextOutlineCacheEntry>,
    device_scale: f64,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct EditableOutlineCacheEntry {
    edit_revision: u64,
    path: Rc<BezPath>,
}

#[derive(Clone)]
struct PathOutlineCacheEntry {
    signature: u64,
    path: Rc<BezPath>,
}

#[derive(Clone)]
struct EditControlsCacheEntry {
    key: EditControlsCacheKey,
    geometry: Rc<EditControlsGeometry>,
}

#[derive(Clone, Copy, PartialEq)]
struct EditControlsCacheKey {
    path_signature: u64,
    selection_signature: u64,
    view_coeffs: [u64; 6],
    point_scale_bits: u64,
}

impl EditControlsCacheKey {
    fn new(
        path: &Path,
        selection: &crate::editing::Selection,
        view: Affine,
        point_scale: f64,
    ) -> Self {
        Self {
            path_signature: path_outline_signature(path),
            selection_signature: path_selection_signature(path, selection),
            view_coeffs: view.as_coeffs().map(f64::to_bits),
            point_scale_bits: point_scale.to_bits(),
        }
    }
}

#[derive(Clone, Default)]
struct EditControlsGeometry {
    handle_lines: BezPath,
    smooth_circles: BezPath,
    corner_squares: BezPath,
    offcurve_circles: BezPath,
    hyper_circles: BezPath,
    selected_circles: BezPath,
    selected_squares: BezPath,
    start_arrow: Option<StartArrowGeometry>,
}

impl EditControlsGeometry {
    fn with_capacity(capacity: EditControlsGeometryCapacity) -> Self {
        Self {
            handle_lines: BezPath::with_capacity(capacity.handle_lines),
            smooth_circles: BezPath::with_capacity(capacity.smooth_circles),
            corner_squares: BezPath::with_capacity(capacity.corner_squares),
            offcurve_circles: BezPath::with_capacity(capacity.offcurve_circles),
            hyper_circles: BezPath::with_capacity(capacity.hyper_circles),
            selected_circles: BezPath::with_capacity(capacity.selected_circles),
            selected_squares: BezPath::with_capacity(capacity.selected_squares),
            start_arrow: None,
        }
    }

    fn capacity(&self) -> EditControlsGeometryCapacity {
        EditControlsGeometryCapacity {
            handle_lines: self.handle_lines.elements().len(),
            smooth_circles: self.smooth_circles.elements().len(),
            corner_squares: self.corner_squares.elements().len(),
            offcurve_circles: self.offcurve_circles.elements().len(),
            hyper_circles: self.hyper_circles.elements().len(),
            selected_circles: self.selected_circles.elements().len(),
            selected_squares: self.selected_squares.elements().len(),
        }
    }

    fn append(&mut self, other: &Self) {
        append_bezpath(&mut self.handle_lines, &other.handle_lines);
        append_bezpath(&mut self.smooth_circles, &other.smooth_circles);
        append_bezpath(&mut self.corner_squares, &other.corner_squares);
        append_bezpath(&mut self.offcurve_circles, &other.offcurve_circles);
        append_bezpath(&mut self.hyper_circles, &other.hyper_circles);
        append_bezpath(&mut self.selected_circles, &other.selected_circles);
        append_bezpath(&mut self.selected_squares, &other.selected_squares);
    }
}

#[derive(Clone, Copy, Default)]
struct EditControlsGeometryCapacity {
    handle_lines: usize,
    smooth_circles: usize,
    corner_squares: usize,
    offcurve_circles: usize,
    hyper_circles: usize,
    selected_circles: usize,
    selected_squares: usize,
}

impl EditControlsGeometryCapacity {
    fn add(&mut self, other: Self) {
        self.handle_lines += other.handle_lines;
        self.smooth_circles += other.smooth_circles;
        self.corner_squares += other.corner_squares;
        self.offcurve_circles += other.offcurve_circles;
        self.hyper_circles += other.hyper_circles;
        self.selected_circles += other.selected_circles;
        self.selected_squares += other.selected_squares;
    }
}

#[derive(Clone, Copy)]
struct StartArrowGeometry {
    center: Point,
    next: Point,
    selected: bool,
}

#[derive(Clone)]
struct DesignGridCacheEntry {
    key: DesignGridCacheKey,
    fine_path: Rc<BezPath>,
    coarse_path: Rc<BezPath>,
}

#[derive(Clone, Copy, PartialEq)]
struct DesignGridCacheKey {
    spacing_bits: u64,
    coarse_n: u32,
    width: u32,
    height: u32,
    view_coeffs: [u64; 6],
    bounds: [u64; 4],
    origin: [u64; 2],
}

impl DesignGridCacheKey {
    #[allow(clippy::too_many_arguments)]
    fn new(
        spacing: f64,
        coarse_n: u32,
        width: u32,
        height: u32,
        view: Affine,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        origin_x: f64,
        origin_y: f64,
    ) -> Self {
        Self {
            spacing_bits: spacing.to_bits(),
            coarse_n,
            width,
            height,
            view_coeffs: view.as_coeffs().map(f64::to_bits),
            bounds: [
                min_x.to_bits(),
                max_x.to_bits(),
                min_y.to_bits(),
                max_y.to_bits(),
            ],
            origin: [origin_x.to_bits(), origin_y.to_bits()],
        }
    }
}

#[derive(Clone)]
struct TextOutlineCacheEntry {
    outline_ptr: usize,
    outline_len: usize,
    path: Rc<BezPath>,
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
            desired_maximum_frame_latency: 1,
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
            editable_outline_cache: None,
            path_outline_cache: HashMap::new(),
            edit_controls_cache: HashMap::new(),
            design_grid_cache: Vec::new(),
            text_outline_cache: HashMap::new(),
            device_scale: 1.0,
            width,
            height,
        })
    }

    pub fn set_theme_json(&mut self, theme_json: &str) -> Result<(), JsValue> {
        let input: CanvasThemeInput = serde_json::from_str(theme_json)
            .map_err(|e| JsValue::from_str(&format!("parse canvas theme: {e}")))?;
        self.theme.apply_input(input);
        self.design_grid_cache.clear();
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
        self.design_grid_cache.clear();
    }

    pub fn set_device_scale(&mut self, scale: f64) {
        let next = scale.clamp(1.0, 8.0);
        if (self.device_scale - next).abs() > f64::EPSILON {
            self.device_scale = next;
            self.design_grid_cache.clear();
        }
    }

    fn px(&self, value: f64) -> f64 {
        value * self.device_scale
    }

    fn point_scale(&self, zoom: f64) -> f64 {
        // Keep points readable at close zoom without letting them dominate
        // the outline at wide zoom. Viewport zoom is measured in backing
        // pixels, so compute the scale curve in CSS/logical pixels.
        const MIN_ZOOM_SCALE: f64 = 0.72;
        const BASE_ZOOM_SCALE: f64 = 1.0;
        const FINE_GRID_ZOOM_SCALE: f64 = 1.34;
        const MAX_ZOOM_SCALE: f64 = 1.75;
        let logical_zoom = zoom / self.device_scale.max(1.0);
        let fine_grid_zoom = DESIGN_GRID_CLOSE_MIN_ZOOM / self.device_scale.max(1.0);
        let wide_t = (logical_zoom / DESIGN_GRID_MID_MIN_ZOOM).clamp(0.0, 1.0);
        let mid_t = ((logical_zoom - DESIGN_GRID_MID_MIN_ZOOM)
            / (fine_grid_zoom - DESIGN_GRID_MID_MIN_ZOOM).max(1e-6))
        .clamp(0.0, 1.0);
        let close_t =
            ((logical_zoom - fine_grid_zoom) / (fine_grid_zoom * 2.5).max(1e-6)).clamp(0.0, 1.0);
        let zoom_scale = if logical_zoom <= DESIGN_GRID_MID_MIN_ZOOM {
            lerp(MIN_ZOOM_SCALE, BASE_ZOOM_SCALE, smoothstep(wide_t))
        } else if logical_zoom <= fine_grid_zoom {
            lerp(BASE_ZOOM_SCALE, FINE_GRID_ZOOM_SCALE, smoothstep(mid_t))
        } else {
            lerp(FINE_GRID_ZOOM_SCALE, MAX_ZOOM_SCALE, smoothstep(close_t))
        };
        self.device_scale * zoom_scale
    }

    fn text_overlay_zoom_t(&self, zoom: f64) -> f64 {
        let logical_zoom = zoom / self.device_scale.max(1.0);
        let fine_grid_zoom = DESIGN_GRID_CLOSE_MIN_ZOOM / self.device_scale.max(1.0);
        (logical_zoom / (fine_grid_zoom * 1.5).max(1e-6)).clamp(0.0, 1.0)
    }

    fn text_cursor_line_px(&self, zoom: f64) -> f64 {
        lerp(
            TEXT_CURSOR_LINE_PX,
            TEXT_CURSOR_LINE_MAX_PX,
            smoothstep(self.text_overlay_zoom_t(zoom)),
        )
    }

    fn text_cursor_marker_scale(&self, zoom: f64) -> f64 {
        lerp(1.0, 1.45, smoothstep(self.text_overlay_zoom_t(zoom)))
    }

    fn text_metric_cross_size(&self, zoom: f64) -> f64 {
        lerp(
            TEXT_METRIC_CROSS_MIN_SIZE,
            TEXT_METRIC_CROSS_SIZE,
            smoothstep(self.text_overlay_zoom_t(zoom)),
        )
    }

    /// Paint one frame against the given editor state.
    pub fn render(
        &mut self,
        state: &EditorState,
        preview_mode: bool,
        text_mode_active: bool,
    ) -> Result<(), JsValue> {
        self.scene.reset();
        self.draw_state(state, preview_mode, text_mode_active, None);
        self.present()
    }

    /// Paint one frame while a keyboard nudge burst is active.
    ///
    /// The editor already knows which contours are being translated
    /// during a nudge burst, so reuse cached geometry for all other
    /// contours and only rebuild the paths that actually changed.
    pub fn render_changed_paths(
        &mut self,
        state: &EditorState,
        changed_path_indices: &[usize],
        preview_mode: bool,
        text_mode_active: bool,
    ) -> Result<(), JsValue> {
        self.scene.reset();
        let changed_paths = changed_path_indices.iter().copied().collect::<HashSet<_>>();
        self.draw_state(state, preview_mode, text_mode_active, Some(&changed_paths));
        self.present()
    }

    fn draw_state(
        &mut self,
        state: &EditorState,
        preview_mode: bool,
        text_mode_active: bool,
        changed_path_indices: Option<&HashSet<usize>>,
    ) {
        let view = state.viewport.affine();
        let has_text_session = state.has_text_session;
        let text_layout =
            has_text_session.then(|| state.text_buffer.layout(state.text_line_height()));
        let active_sort_origin = text_layout
            .as_ref()
            .and_then(|layout| {
                let active_index = state.text_buffer.active_sort()?;
                layout
                    .items
                    .iter()
                    .find(|item| item.index == active_index)
                    .map(|item| (item.x, item.y))
            })
            .unwrap_or((0.0, 0.0));
        let glyph_view = view * Affine::translate(active_sort_origin);

        if !preview_mode {
            self.draw_design_grid(state, view, active_sort_origin.0, active_sort_origin.1);

            // Metric guides go in first so the glyph fill paints on top.
            if !has_text_session {
                self.draw_metric_guides(state, glyph_view);
            }
        }

        if has_text_session {
            self.draw_text_buffer(
                state,
                view,
                preview_mode,
                text_mode_active,
                text_layout.as_ref(),
            );
            if !preview_mode && !text_mode_active {
                self.draw_edit_controls(state, glyph_view, changed_path_indices);
            }
            return;
        }

        // Glyph fill (in design space — viewport applies the Y-flip).
        // Combine every contour into ONE BezPath before filling so the
        // NonZero winding rule treats opposite-wound inner contours as
        // holes (UFO/PostScript convention). Filling each contour
        // separately would paint counters solid.
        let outline = changed_path_indices
            .filter(|indices| !indices.is_empty() && !preview_mode)
            .map(|indices| self.editable_outline_path_for_changed_paths(state, indices))
            .unwrap_or_else(|| self.editable_outline_path(state));
        if preview_mode {
            let mut combined = outline.as_ref().clone();
            for component in &state.component_previews {
                for el in component.transformed_path.elements() {
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
            self.draw_text_buffer(state, view, true, text_mode_active, None);
            return;
        }
        if !outline.elements().is_empty() {
            self.scene.fill(
                Fill::NonZero,
                glyph_view,
                self.theme.path_edit_fill,
                None,
                outline.as_ref(),
            );
            // Edit mode: stroke the outline at a constant screen-pixel
            // width. Transform the path into screen space first so the
            // stroke width doesn't scale with zoom.
            let screen_path = glyph_view * outline.as_ref();
            self.scene.stroke(
                &Stroke::new(self.px(PATH_STROKE_PX)),
                Affine::IDENTITY,
                self.theme.path_stroke,
                None,
                &screen_path,
            );
        }
        for component in &state.component_previews {
            if component.transformed_path.elements().is_empty() {
                continue;
            }
            let fill = if state.selected_component == Some(component.id) {
                self.theme.component_selected_fill
            } else {
                self.theme.component_fill
            };
            self.scene.fill(
                Fill::NonZero,
                glyph_view,
                fill,
                None,
                component.transformed_path.as_ref(),
            );
            if state.selected_component == Some(component.id) {
                let screen_path = glyph_view * component.transformed_path.as_ref();
                self.scene.stroke(
                    &Stroke::new(self.px(COMPONENT_SELECTION_STROKE_PX)),
                    Affine::IDENTITY,
                    self.theme.text_cursor,
                    None,
                    &screen_path,
                );
            }
        }
        self.draw_edit_controls(state, glyph_view, changed_path_indices);
    }

    fn draw_edit_controls(
        &mut self,
        state: &EditorState,
        glyph_view: Affine,
        changed_path_indices: Option<&HashSet<usize>>,
    ) {
        // Handle lines and points are drawn in screen space so they
        // stay at constant pixel size regardless of zoom.
        let point_scale = self.point_scale(state.viewport.zoom);
        let mut current_path_ids = HashSet::with_capacity(state.paths.len());
        let mut controls_by_path = Vec::with_capacity(state.paths.len());
        let mut combined_capacity = EditControlsGeometryCapacity::default();
        let mut start_arrows = Vec::new();
        for (index, path) in state.paths.iter().enumerate() {
            let id = path_id(path);
            current_path_ids.insert(id);
            let controls = self.edit_controls_for_path(
                index,
                path,
                glyph_view,
                &state.selection,
                point_scale,
                changed_path_indices,
            );
            combined_capacity.add(controls.capacity());
            if let Some(start_arrow) = controls.start_arrow {
                start_arrows.push(start_arrow);
            }
            controls_by_path.push(controls);
        }
        self.edit_controls_cache
            .retain(|id, _| current_path_ids.contains(id));
        let mut combined = EditControlsGeometry::with_capacity(combined_capacity);
        for controls in &controls_by_path {
            combined.append(controls);
        }
        self.draw_edit_controls_geometry(&combined, &start_arrows, point_scale);

        self.draw_propagated_anchors(state, glyph_view);
        self.draw_anchors(state, glyph_view);

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
            self.draw_knife_preview(preview, state.viewport.zoom);
        }
    }

    fn draw_anchors(&mut self, state: &EditorState, view: Affine) {
        let scale = self.point_scale(state.viewport.zoom);
        let outline_stroke = Stroke::new(POINT_OUTLINE_PX * scale);
        for anchor in &state.anchors {
            let center = view * anchor.point;
            let selected = state.selected_anchor == Some(anchor.id);
            let radius = (if selected {
                SMOOTH_POINT_SELECTED_RADIUS_PX
            } else {
                SMOOTH_POINT_RADIUS_PX
            }) * scale;
            let circle = Circle::new(center, radius);
            let (inner, outer) = if selected {
                (
                    self.theme.point_selected_inner,
                    self.theme.point_selected_outer,
                )
            } else {
                (POINT_INNER, POINT_MARK_GREEN)
            };
            self.scene
                .fill(Fill::NonZero, Affine::IDENTITY, inner, None, &circle);
            self.scene
                .stroke(&outline_stroke, Affine::IDENTITY, outer, None, &circle);
        }
    }

    fn draw_propagated_anchors(&mut self, state: &EditorState, view: Affine) {
        let scale = self.point_scale(state.viewport.zoom);
        let radius = SMOOTH_POINT_RADIUS_PX * scale;
        let outline_stroke = Stroke::new(POINT_OUTLINE_PX * scale);
        for anchor in &state.propagated_anchors {
            let circle = Circle::new(view * anchor.point, radius);
            self.scene.stroke(
                &outline_stroke,
                Affine::IDENTITY,
                POINT_MARK_GREEN,
                None,
                &circle,
            );
        }
    }

    fn draw_text_buffer(
        &mut self,
        state: &EditorState,
        view: Affine,
        preview_mode: bool,
        text_mode_active: bool,
        frame_layout: Option<&TextLayout>,
    ) {
        let (ascender, descender) = state.text_metric_bounds();
        let (sort_top, sort_bottom) = state.text_sort_metric_bounds();
        let line_height = state.text_line_height();
        let layout_storage;
        let layout = if let Some(layout) = frame_layout {
            layout
        } else {
            layout_storage = state.text_buffer.layout(line_height);
            &layout_storage
        };
        let kern_sort_index = state.text_buffer.manual_kerning_sort();

        if !preview_mode {
            let mut active_metric_path = BezPath::new();
            let mut previous_metric_path = BezPath::new();
            let mut guide_metric_path = BezPath::new();
            let mut cursor_metric_path = BezPath::new();
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
                let metric_path = if metric_color == self.theme.text_kern_active {
                    &mut active_metric_path
                } else if metric_color == self.theme.text_kern_previous {
                    &mut previous_metric_path
                } else if metric_color == self.theme.text_cursor {
                    &mut cursor_metric_path
                } else {
                    &mut guide_metric_path
                };
                append_text_sort_minimal_metrics(
                    metric_path,
                    item.x,
                    item.y,
                    item.advance_width,
                    ascender,
                    descender,
                    sort_top,
                    sort_bottom,
                    view,
                    self.px(self.text_metric_cross_size(state.viewport.zoom)),
                );
            }
            let stroke = Stroke::new(self.px(METRIC_LINE_PX));
            self.stroke_metric_batch(&guide_metric_path, self.theme.metric_guide, &stroke);
            self.stroke_metric_batch(&active_metric_path, self.theme.text_kern_active, &stroke);
            self.stroke_metric_batch(
                &previous_metric_path,
                self.theme.text_kern_previous,
                &stroke,
            );
            self.stroke_metric_batch(&cursor_metric_path, self.theme.text_cursor, &stroke);
        }

        for item in &layout.items {
            let Some(sort) = state.text_buffer.sort(item.index) else {
                continue;
            };
            let render_active_editable = !preview_mode && sort.active && !text_mode_active;
            if render_active_editable {
                let active_outline = self.editable_outline_path(state);
                if !active_outline.elements().is_empty() {
                    let item_view = view * Affine::translate((item.x, item.y));
                    self.scene.fill(
                        Fill::NonZero,
                        item_view,
                        self.theme.path_edit_fill,
                        None,
                        active_outline.as_ref(),
                    );
                    let screen_path = item_view * active_outline.as_ref();
                    self.scene.stroke(
                        &Stroke::new(self.px(PATH_STROKE_PX)),
                        Affine::IDENTITY,
                        self.theme.path_stroke,
                        None,
                        &screen_path,
                    );
                }
                for component in &state.component_previews {
                    if component.transformed_path.elements().is_empty() {
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
                        component.transformed_path.as_ref(),
                    );
                    if state.selected_component == Some(component.id) {
                        let screen_path = (view * Affine::translate((item.x, item.y)))
                            * component.transformed_path.as_ref();
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
                let Some(path) = self.text_preview_path(glyph_name, outline) else {
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
                    path.as_ref(),
                );
            }
        }

        if !preview_mode && text_mode_active {
            self.draw_text_cursor(
                layout.cursor_x,
                layout.cursor_y,
                sort_top,
                sort_bottom,
                view,
                state.viewport.zoom,
            );
        }
    }

    fn editable_outline_path(&mut self, state: &EditorState) -> Rc<BezPath> {
        let edit_revision = state.edit_revision();
        if let Some(entry) = &self.editable_outline_cache {
            if entry.edit_revision == edit_revision {
                return Rc::clone(&entry.path);
            }
        }

        let path = self.build_editable_outline_path_with_cache(state);
        let path = Rc::new(path);
        self.editable_outline_cache = Some(EditableOutlineCacheEntry {
            edit_revision,
            path: Rc::clone(&path),
        });
        path
    }

    fn editable_outline_path_for_changed_paths(
        &mut self,
        state: &EditorState,
        changed_path_indices: &HashSet<usize>,
    ) -> Rc<BezPath> {
        let mut current_path_ids = HashSet::with_capacity(state.paths.len());
        let mut path_outlines = Vec::with_capacity(state.paths.len());
        let mut combined_capacity = 0usize;

        for (index, path) in state.paths.iter().enumerate() {
            let id = path_id(path);
            current_path_ids.insert(id);
            let path_changed = changed_path_indices.contains(&index);
            let cached = (!path_changed)
                .then(|| {
                    self.path_outline_cache
                        .get(&id)
                        .map(|entry| Rc::clone(&entry.path))
                })
                .flatten();
            let path_outline = if let Some(cached) = cached {
                cached
            } else {
                let signature = path_outline_signature(path);
                let cached = self
                    .path_outline_cache
                    .get(&id)
                    .filter(|entry| entry.signature == signature)
                    .map(|entry| Rc::clone(&entry.path));
                if let Some(cached) = cached {
                    cached
                } else {
                    let mut outline = BezPath::new();
                    path.append_to_bezpath(&mut outline);
                    let outline = Rc::new(outline);
                    self.path_outline_cache.insert(
                        id,
                        PathOutlineCacheEntry {
                            signature,
                            path: Rc::clone(&outline),
                        },
                    );
                    outline
                }
            };
            combined_capacity += path_outline.elements().len();
            path_outlines.push(path_outline);
        }

        self.path_outline_cache
            .retain(|id, _| current_path_ids.contains(id));

        let mut combined = BezPath::with_capacity(combined_capacity);
        for path_outline in &path_outlines {
            append_bezpath(&mut combined, path_outline);
        }

        let path = Rc::new(combined);
        self.editable_outline_cache = Some(EditableOutlineCacheEntry {
            edit_revision: state.edit_revision(),
            path: Rc::clone(&path),
        });
        path
    }

    fn build_editable_outline_path_with_cache(&mut self, state: &EditorState) -> BezPath {
        let mut current_path_ids = HashSet::with_capacity(state.paths.len());
        let mut path_outlines = Vec::with_capacity(state.paths.len());
        let mut combined_capacity = 0usize;

        for path in &state.paths {
            let id = path_id(path);
            let signature = path_outline_signature(path);
            current_path_ids.insert(id);

            let cached = self
                .path_outline_cache
                .get(&id)
                .filter(|entry| entry.signature == signature)
                .map(|entry| Rc::clone(&entry.path));
            let path_outline = if let Some(cached) = cached {
                cached
            } else {
                let mut outline = BezPath::new();
                path.append_to_bezpath(&mut outline);
                let outline = Rc::new(outline);
                self.path_outline_cache.insert(
                    id,
                    PathOutlineCacheEntry {
                        signature,
                        path: Rc::clone(&outline),
                    },
                );
                outline
            };
            combined_capacity += path_outline.elements().len();
            path_outlines.push(path_outline);
        }

        self.path_outline_cache
            .retain(|id, _| current_path_ids.contains(id));

        let mut combined = BezPath::with_capacity(combined_capacity);
        for path_outline in &path_outlines {
            append_bezpath(&mut combined, path_outline);
        }

        combined
    }

    fn text_preview_path(&mut self, glyph_name: &str, outline: &str) -> Option<Rc<BezPath>> {
        let outline_ptr = outline.as_ptr() as usize;
        let outline_len = outline.len();
        if let Some(entry) = self.text_outline_cache.get(glyph_name) {
            if entry.outline_ptr == outline_ptr && entry.outline_len == outline_len {
                return Some(Rc::clone(&entry.path));
            }
        }

        let path = Rc::new(BezPath::from_svg(outline).ok()?);
        self.text_outline_cache.insert(
            glyph_name.to_string(),
            TextOutlineCacheEntry {
                outline_ptr,
                outline_len,
                path: Rc::clone(&path),
            },
        );
        Some(path)
    }

    fn draw_text_cursor(
        &mut self,
        cursor_x: f64,
        baseline_y: f64,
        ascender: f64,
        descender: f64,
        view: Affine,
        zoom: f64,
    ) {
        let top = view * Point::new(cursor_x, baseline_y + ascender);
        let bottom = view * Point::new(cursor_x, baseline_y + descender);
        let line_width = self.px(self.text_cursor_line_px(zoom));
        let marker_scale = self.text_cursor_marker_scale(zoom);
        let triangle_width = self.px(TEXT_CURSOR_TRIANGLE_WIDTH_PX * marker_scale);
        let triangle_height = self.px(TEXT_CURSOR_TRIANGLE_HEIGHT_PX * marker_scale);

        self.scene.stroke(
            &Stroke::new(line_width),
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &Line::new(top, bottom),
        );

        let mut top_triangle = BezPath::new();
        top_triangle.move_to((top.x - triangle_width / 2.0, top.y));
        top_triangle.line_to((top.x + triangle_width / 2.0, top.y));
        top_triangle.line_to((top.x, top.y + triangle_height));
        top_triangle.close_path();
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &top_triangle,
        );

        let mut bottom_triangle = BezPath::new();
        bottom_triangle.move_to((bottom.x - triangle_width / 2.0, bottom.y));
        bottom_triangle.line_to((bottom.x + triangle_width / 2.0, bottom.y));
        bottom_triangle.line_to((bottom.x, bottom.y - triangle_height));
        bottom_triangle.close_path();
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.theme.text_cursor,
            None,
            &bottom_triangle,
        );
    }

    fn stroke_metric_batch(&mut self, path: &BezPath, color: Srgb, stroke: &Stroke) {
        if !path.elements().is_empty() {
            self.scene
                .stroke(stroke, Affine::IDENTITY, color, None, path);
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
        let (ascender, descender) = state.text_metric_bounds();
        let (box_top, box_bottom) = state.text_sort_metric_bounds();
        let mut metric_path = BezPath::new();
        if box_top > box_bottom {
            append_screen_line(
                &mut metric_path,
                view,
                Point::new(x, baseline_y + box_bottom),
                Point::new(x, baseline_y + box_top),
            );
            append_screen_line(
                &mut metric_path,
                view,
                Point::new(x + advance_width, baseline_y + box_bottom),
                Point::new(x + advance_width, baseline_y + box_top),
            );
        }

        let mut ys = vec![0.0, ascender, descender, box_top, box_bottom];
        if let Some(metrics) = state.metrics.as_ref() {
            ys.extend(
                [metrics.units_per_em, metrics.x_height, metrics.cap_height]
                    .into_iter()
                    .flatten(),
            );
        }
        ys.retain(|y| y.is_finite());
        ys.sort_by(|a, b| a.total_cmp(b));
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        for y in ys {
            append_screen_line(
                &mut metric_path,
                view,
                Point::new(x, baseline_y + y),
                Point::new(x + advance_width, baseline_y + y),
            );
        }
        self.stroke_metric_batch(
            &metric_path,
            self.theme.metric_guide,
            &Stroke::new(self.px(METRIC_LINE_PX)),
        );
    }

    fn edit_controls_for_path(
        &mut self,
        path_index: usize,
        path: &Path,
        view: Affine,
        selection: &crate::editing::Selection,
        point_scale: f64,
        changed_path_indices: Option<&HashSet<usize>>,
    ) -> Rc<EditControlsGeometry> {
        let id = path_id(path);
        let path_changed = changed_path_indices
            .map(|indices| indices.contains(&path_index))
            .unwrap_or(true);
        if !path_changed {
            let view_coeffs = view.as_coeffs().map(f64::to_bits);
            let point_scale_bits = point_scale.to_bits();
            if let Some(entry) = self.edit_controls_cache.get(&id)
                && entry.key.view_coeffs == view_coeffs
                && entry.key.point_scale_bits == point_scale_bits
            {
                return Rc::clone(&entry.geometry);
            }
        }
        let key = EditControlsCacheKey::new(path, selection, view, point_scale);
        if let Some(entry) = self.edit_controls_cache.get(&id)
            && entry.key == key
        {
            return Rc::clone(&entry.geometry);
        }
        let geometry = Rc::new(Self::build_edit_controls_geometry(
            path,
            view,
            selection,
            point_scale,
        ));
        self.edit_controls_cache.insert(
            id,
            EditControlsCacheEntry {
                key,
                geometry: Rc::clone(&geometry),
            },
        );
        geometry
    }

    fn draw_edit_controls_geometry(
        &mut self,
        controls: &EditControlsGeometry,
        start_arrows: &[StartArrowGeometry],
        point_scale: f64,
    ) {
        if !controls.handle_lines.elements().is_empty() {
            self.scene.stroke(
                &Stroke::new(self.px(HANDLE_LINE_PX)),
                Affine::IDENTITY,
                self.theme.handle_line,
                None,
                &controls.handle_lines,
            );
        }
        let outline_stroke = Stroke::new(POINT_OUTLINE_PX * point_scale);
        self.draw_point_batch(
            &controls.smooth_circles,
            self.theme.point_smooth_inner,
            self.theme.point_smooth_outer,
            &outline_stroke,
        );
        self.draw_point_batch(
            &controls.corner_squares,
            self.theme.point_corner_inner,
            self.theme.point_corner_outer,
            &outline_stroke,
        );
        self.draw_point_batch(
            &controls.offcurve_circles,
            self.theme.point_offcurve_inner,
            self.theme.point_offcurve_outer,
            &outline_stroke,
        );
        self.draw_point_batch(
            &controls.hyper_circles,
            self.theme.point_hyper_inner,
            self.theme.point_hyper_outer,
            &outline_stroke,
        );
        self.draw_point_batch(
            &controls.selected_circles,
            self.theme.point_selected_inner,
            self.theme.point_selected_outer,
            &outline_stroke,
        );
        self.draw_point_batch(
            &controls.selected_squares,
            self.theme.point_selected_inner,
            self.theme.point_selected_outer,
            &outline_stroke,
        );
        for start_arrow in start_arrows {
            self.draw_start_arrow(
                start_arrow.center,
                start_arrow.next,
                start_arrow.selected,
                point_scale,
            );
        }
    }

    fn build_edit_controls_geometry(
        path: &Path,
        view: Affine,
        selection: &crate::editing::Selection,
        point_scale: f64,
    ) -> EditControlsGeometry {
        let mut geometry = Self::build_point_geometry(path, view, selection, point_scale);
        geometry.handle_lines = Self::build_handle_lines(path, view);
        geometry
    }

    fn build_handle_lines(path: &Path, view: Affine) -> BezPath {
        let points = path.points().as_slice();
        if points.len() < 2 {
            return BezPath::new();
        }
        let closed = path_is_closed(path);
        let mut lines = BezPath::new();
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
                lines.move_to(on);
                lines.line_to(off);
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
                lines.move_to(on);
                lines.line_to(off);
            }
        }
        lines
    }

    fn build_point_geometry(
        path: &Path,
        view: Affine,
        selection: &crate::editing::Selection,
        point_scale: f64,
    ) -> EditControlsGeometry {
        let points = path.points().as_slice();
        let closed = path_is_closed(path);
        let start_index = closed
            .then(|| points.iter().position(PathPoint::is_on_curve))
            .flatten();
        let mut smooth_circles = BezPath::new();
        let mut corner_squares = BezPath::new();
        let mut offcurve_circles = BezPath::new();
        let mut hyper_circles = BezPath::new();
        let mut selected_circles = BezPath::new();
        let mut selected_squares = BezPath::new();
        let mut start_arrow = None;
        for (index, pt) in points.iter().enumerate() {
            let center = view * pt.point;
            let selected = selection.contains(&pt.id);

            if matches!(path, Path::Hyper(_)) && pt.is_on_curve() {
                let radius = (if selected {
                    HYPER_POINT_SELECTED_RADIUS_PX
                } else {
                    HYPER_POINT_RADIUS_PX
                }) * point_scale;
                if selected {
                    append_circle_path(&mut selected_circles, center, radius);
                } else {
                    append_circle_path(&mut hyper_circles, center, radius);
                }
            } else {
                match pt.typ {
                    PointType::OnCurve { smooth: true } => {
                        let radius = (if selected {
                            SMOOTH_POINT_SELECTED_RADIUS_PX
                        } else {
                            SMOOTH_POINT_RADIUS_PX
                        }) * point_scale;
                        if selected {
                            append_circle_path(&mut selected_circles, center, radius);
                        } else {
                            append_circle_path(&mut smooth_circles, center, radius);
                        }
                    }
                    PointType::OnCurve { smooth: false } => {
                        let half = (if selected {
                            CORNER_POINT_SELECTED_HALF_PX
                        } else {
                            CORNER_POINT_HALF_PX
                        }) * point_scale;
                        let target = if selected {
                            &mut selected_squares
                        } else {
                            &mut corner_squares
                        };
                        append_rect_path(
                            target,
                            Rect::new(
                                center.x - half,
                                center.y - half,
                                center.x + half,
                                center.y + half,
                            ),
                        );
                    }
                    PointType::OffCurve { .. } => {
                        let radius = (if selected {
                            OFFCURVE_POINT_SELECTED_RADIUS_PX
                        } else {
                            OFFCURVE_POINT_RADIUS_PX
                        }) * point_scale;
                        if selected {
                            append_circle_path(&mut selected_circles, center, radius);
                        } else {
                            append_circle_path(&mut offcurve_circles, center, radius);
                        }
                    }
                }
            }
            if start_index == Some(index) {
                let next = next_point_pos(&points, index, closed);
                start_arrow = Some(StartArrowGeometry {
                    center,
                    next: view * next,
                    selected,
                });
            }
        }
        EditControlsGeometry {
            handle_lines: BezPath::new(),
            smooth_circles,
            corner_squares,
            offcurve_circles,
            hyper_circles,
            selected_circles,
            selected_squares,
            start_arrow,
        }
    }

    fn draw_point_batch(&mut self, path: &BezPath, inner: Srgb, outer: Srgb, stroke: &Stroke) {
        if path.elements().is_empty() {
            return;
        }
        self.scene
            .fill(Fill::NonZero, Affine::IDENTITY, inner, None, path);
        self.scene
            .stroke(stroke, Affine::IDENTITY, outer, None, path);
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

        let width = state.advance_width;
        let Some((box_top, box_bottom)) = state.glyph_metric_bounds() else {
            return;
        };
        let mut guide_path = BezPath::new();
        if box_top > box_bottom {
            append_screen_line(
                &mut guide_path,
                view,
                Point::new(0.0, box_bottom),
                Point::new(0.0, box_top),
            );
            append_screen_line(
                &mut guide_path,
                view,
                Point::new(width, box_bottom),
                Point::new(width, box_top),
            );
        }

        // Horizontal metric lines. Baseline is always drawn (y=0);
        // others appear only when defined in fontinfo.
        let mut ys: Vec<f64> = vec![0.0, box_top, box_bottom];
        for opt in [
            metrics.units_per_em,
            metrics.ascender,
            metrics.descender,
            metrics.x_height,
            metrics.cap_height,
        ] {
            if let Some(y) = opt {
                ys.push(y);
            }
        }
        ys.retain(|y| y.is_finite());
        ys.sort_by(|a, b| a.total_cmp(b));
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        for y in ys {
            append_screen_line(
                &mut guide_path,
                view,
                Point::new(0.0, y),
                Point::new(width, y),
            );
        }
        if !guide_path.elements().is_empty() {
            self.scene.stroke(
                &Stroke::new(self.px(METRIC_LINE_PX)),
                Affine::IDENTITY,
                self.theme.metric_guide,
                None,
                &guide_path,
            );
        }
    }

    /// Draw the zoom-dependent design-space grid behind the glyph.
    ///
    /// The mid level shows 8-unit spacing with 32-unit coarse lines;
    /// the close level adds a 2-unit grid with 8-unit coarse lines.
    /// Match xilem's calibration so the coarser grid remains visible
    /// near the default editing zoom, while the dense 2-unit grid waits
    /// until the user is very close in. Anchor both axes to the active
    /// sort origin so the primary horizontal gridline lands on the
    /// font baseline in text mode.
    fn draw_design_grid(
        &mut self,
        state: &EditorState,
        view: Affine,
        origin_x: f64,
        origin_y: f64,
    ) {
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
            origin_y,
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
                origin_y,
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
        origin_y: f64,
    ) {
        let fine_stroke = Stroke::new(DESIGN_GRID_FINE_LINE_PX);
        let coarse_stroke = Stroke::new(DESIGN_GRID_COARSE_LINE_PX);
        let (fine_path, coarse_path) = self.design_grid_paths(
            view, spacing, coarse_n, min_x, max_x, min_y, max_y, origin_x, origin_y,
        );

        if !fine_path.elements().is_empty() {
            self.scene.stroke(
                &fine_stroke,
                Affine::IDENTITY,
                self.theme.design_grid_fine,
                None,
                fine_path.as_ref(),
            );
        }
        if !coarse_path.elements().is_empty() {
            self.scene.stroke(
                &coarse_stroke,
                Affine::IDENTITY,
                self.theme.design_grid_coarse,
                None,
                coarse_path.as_ref(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn design_grid_paths(
        &mut self,
        view: Affine,
        spacing: f64,
        coarse_n: u32,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        origin_x: f64,
        origin_y: f64,
    ) -> (Rc<BezPath>, Rc<BezPath>) {
        let key = DesignGridCacheKey::new(
            spacing,
            coarse_n,
            self.width,
            self.height,
            view,
            min_x,
            max_x,
            min_y,
            max_y,
            origin_x,
            origin_y,
        );
        if let Some(entry) = self.design_grid_cache.iter().find(|entry| entry.key == key) {
            return (Rc::clone(&entry.fine_path), Rc::clone(&entry.coarse_path));
        }

        let (fine_path, coarse_path) = build_grid_level_paths(
            view, spacing, coarse_n, min_x, max_x, min_y, max_y, origin_x, origin_y,
        );
        let fine_path = Rc::new(fine_path);
        let coarse_path = Rc::new(coarse_path);
        self.design_grid_cache.push(DesignGridCacheEntry {
            key,
            fine_path: Rc::clone(&fine_path),
            coarse_path: Rc::clone(&coarse_path),
        });
        if self.design_grid_cache.len() > 4 {
            self.design_grid_cache.remove(0);
        }
        (fine_path, coarse_path)
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
            self.theme.point_smooth_outer,
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

    fn draw_knife_preview(&mut self, preview: &KnifePreview, zoom: f64) {
        let stroke = Stroke::new(self.px(TOOL_PREVIEW_LINE_PX))
            .with_dashes(0.0, KNIFE_PREVIEW_DASH.map(|dash| self.px(dash)).to_vec());
        self.scene.stroke(
            &stroke,
            Affine::IDENTITY,
            self.theme.tool_preview,
            None,
            &preview.line,
        );

        let marker_radius = SMOOTH_POINT_RADIUS_PX * self.point_scale(zoom);
        for point in [preview.line.p0, preview.line.p1] {
            let dot = Circle::new(point, marker_radius);
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                POINT_MARK_ORANGE,
                None,
                &dot,
            );
        }

        for point in &preview.intersections {
            let dot = Circle::new(*point, marker_radius);
            self.scene
                .fill(Fill::NonZero, Affine::IDENTITY, POINT_MARK_RED, None, &dot);
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

fn text_sort_minimal_metric_ys(
    baseline_y: f64,
    ascender: f64,
    descender: f64,
    box_top: f64,
    box_bottom: f64,
) -> Vec<f64> {
    let mut ys = vec![
        baseline_y + box_bottom,
        baseline_y + descender,
        baseline_y,
        baseline_y + ascender,
        baseline_y + box_top,
    ];
    ys.retain(|y| y.is_finite());
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    ys
}

fn append_text_sort_minimal_metrics(
    path: &mut BezPath,
    x: f64,
    baseline_y: f64,
    advance_width: f64,
    ascender: f64,
    descender: f64,
    box_top: f64,
    box_bottom: f64,
    view: Affine,
    size: f64,
) {
    let metric_ys =
        text_sort_minimal_metric_ys(baseline_y, ascender, descender, box_top, box_bottom);
    for edge_x in [x, x + advance_width] {
        for y in metric_ys.iter().copied() {
            let center = view * Point::new(edge_x, y);
            path.move_to((center.x - size, center.y));
            path.line_to((center.x + size, center.y));
            path.move_to((center.x, center.y - size));
            path.line_to((center.x, center.y + size));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::text_sort_minimal_metric_ys;

    #[test]
    fn minimal_text_metrics_include_upm_top_cross_when_above_ascender() {
        assert_eq!(
            text_sort_minimal_metric_ys(0.0, 700.0, -300.0, 1000.0, -300.0),
            vec![-300.0, 0.0, 700.0, 1000.0]
        );
    }

    #[test]
    fn minimal_text_metrics_deduplicate_upm_top_when_equal_to_ascender() {
        assert_eq!(
            text_sort_minimal_metric_ys(0.0, 800.0, -200.0, 800.0, -200.0),
            vec![-200.0, 0.0, 800.0]
        );
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

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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

fn hash_outline_part(mut hash: u64, value: u64) -> u64 {
    hash ^= value;
    hash.wrapping_mul(0x100000001b3)
}

fn path_id(path: &Path) -> EntityId {
    match path {
        Path::Cubic(path) => path.id,
        Path::Quadratic(path) => path.id,
        Path::Hyper(path) => path.id,
    }
}

fn path_outline_signature(path: &Path) -> u64 {
    let (kind, closed) = match path {
        Path::Cubic(path) => (1, path.closed),
        Path::Quadratic(path) => (2, path.closed),
        Path::Hyper(path) => (3, path.closed),
    };
    let points = path.points().as_slice();
    let mut hash = hash_outline_part(0xcbf29ce484222325u64, kind);
    hash = hash_outline_part(hash, points.len() as u64);
    hash = hash_outline_part(hash, u64::from(closed));
    for point in points {
        hash = hash_outline_part(hash, hash_entity_id(point.id));
        hash = hash_outline_part(hash, point.point.x.to_bits());
        hash = hash_outline_part(hash, point.point.y.to_bits());
        hash = hash_outline_part(hash, point_type_signature(point.typ));
    }
    hash
}

fn path_selection_signature(path: &Path, selection: &crate::editing::Selection) -> u64 {
    let points = path.points().as_slice();
    let mut hash = hash_outline_part(0xcbf29ce484222325u64, points.len() as u64);
    for point in points {
        if selection.contains(&point.id) {
            hash = hash_outline_part(hash, hash_entity_id(point.id));
        }
    }
    hash
}

fn hash_entity_id(id: EntityId) -> u64 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

fn point_type_signature(point_type: PointType) -> u64 {
    match point_type {
        PointType::OffCurve { auto } => u64::from(auto),
        PointType::OnCurve { smooth } => 0x100 | u64::from(smooth),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_grid_level_paths(
    view: Affine,
    spacing: f64,
    coarse_n: u32,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    origin_x: f64,
    origin_y: f64,
) -> (BezPath, BezPath) {
    let mut fine_path = BezPath::new();
    let mut coarse_path = BezPath::new();
    let start_x = ((min_x - origin_x) / spacing).floor() as i64;
    let end_x = ((max_x - origin_x) / spacing).ceil() as i64;
    let start_y = ((min_y - origin_y) / spacing).floor() as i64;
    let end_y = ((max_y - origin_y) / spacing).ceil() as i64;

    for ix in start_x..=end_x {
        let x = origin_x + ix as f64 * spacing;
        let is_coarse = coarse_n > 0 && (ix.unsigned_abs() % coarse_n as u64 == 0);
        let path = if is_coarse {
            &mut coarse_path
        } else {
            &mut fine_path
        };
        let p0 = view * Point::new(x, min_y);
        let p1 = view * Point::new(x, max_y);
        path.move_to(p0);
        path.line_to(p1);
    }

    for iy in start_y..=end_y {
        let y = origin_y + iy as f64 * spacing;
        let is_coarse = coarse_n > 0 && (iy.unsigned_abs() % coarse_n as u64 == 0);
        let path = if is_coarse {
            &mut coarse_path
        } else {
            &mut fine_path
        };
        let p0 = view * Point::new(min_x, y);
        let p1 = view * Point::new(max_x, y);
        path.move_to(p0);
        path.line_to(p1);
    }

    (fine_path, coarse_path)
}

fn append_rect_path(path: &mut BezPath, rect: Rect) {
    path.move_to((rect.x0, rect.y0));
    path.line_to((rect.x1, rect.y0));
    path.line_to((rect.x1, rect.y1));
    path.line_to((rect.x0, rect.y1));
    path.close_path();
}

fn append_screen_line(path: &mut BezPath, view: Affine, p0: Point, p1: Point) {
    let p0 = view * p0;
    let p1 = view * p1;
    path.move_to(p0);
    path.line_to(p1);
}

fn append_circle_path(path: &mut BezPath, center: Point, radius: f64) {
    const KAPPA: f64 = 0.552_284_749_830_793_6;
    let control = radius * KAPPA;
    path.move_to((center.x + radius, center.y));
    path.curve_to(
        (center.x + radius, center.y + control),
        (center.x + control, center.y + radius),
        (center.x, center.y + radius),
    );
    path.curve_to(
        (center.x - control, center.y + radius),
        (center.x - radius, center.y + control),
        (center.x - radius, center.y),
    );
    path.curve_to(
        (center.x - radius, center.y - control),
        (center.x - control, center.y - radius),
        (center.x, center.y - radius),
    );
    path.curve_to(
        (center.x + control, center.y - radius),
        (center.x + radius, center.y - control),
        (center.x + radius, center.y),
    );
    path.close_path();
}

fn append_bezpath(target: &mut BezPath, source: &BezPath) {
    for element in source.elements() {
        target.push(*element);
    }
}
