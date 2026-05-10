// Ported from runebender-xilem/src/editing/viewport.rs (Apache-2.0).

//! Viewport transformation between design space and screen space.
//!
//! Font coordinates use Y-up (origin at baseline); screen coordinates
//! use Y-down (origin at top-left). `ViewPort` stores an offset and
//! zoom level and provides `to_screen` / `screen_to_design`
//! conversions that handle the Y-flip, scaling, and translation.

#[derive(Debug, Clone)]
pub struct ViewPort {
    /// Scroll offset in screen space.
    pub offset: kurbo::Vec2,
    /// Zoom level (screen pixels per design unit).
    pub zoom: f64,
}

impl ViewPort {
    pub fn new() -> Self {
        Self {
            offset: kurbo::Vec2::ZERO,
            zoom: 1.0,
        }
    }

    /// Convert a point from design space to screen space (Y-flip,
    /// scale, translate).
    pub fn to_screen(&self, point: kurbo::Point) -> kurbo::Point {
        kurbo::Point::new(
            point.x * self.zoom + self.offset.x,
            -point.y * self.zoom + self.offset.y,
        )
    }

    /// Convert a point from screen space to design space.
    pub fn screen_to_design(&self, point: kurbo::Point) -> kurbo::Point {
        kurbo::Point::new(
            (point.x - self.offset.x) / self.zoom,
            -(point.y - self.offset.y) / self.zoom,
        )
    }

    /// Affine transform from design to screen (scale, Y-flip, translate).
    pub fn affine(&self) -> kurbo::Affine {
        kurbo::Affine::new([
            self.zoom,
            0.0,
            0.0,
            -self.zoom,
            self.offset.x,
            self.offset.y,
        ])
    }
}

impl Default for ViewPort {
    fn default() -> Self {
        Self::new()
    }
}
