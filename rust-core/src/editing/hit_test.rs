// Ported from runebender-xilem/src/editing/hit_test.rs (Apache-2.0).

//! Hit testing for finding points and segments under the cursor.
//!
//! Given a screen-space click position, `find_closest` iterates
//! candidate points and returns the nearest one within a threshold.
//! On-curve points receive a small distance penalty so that nearby
//! off-curve handles are easier to grab.

use crate::model::EntityId;
use kurbo::Point;

/// Default maximum distance for clicking on a point (in screen pixels).
pub const MIN_CLICK_DISTANCE: f64 = 10.0;

/// Penalty added to on-curve points to favor off-curve handle
/// selection when they overlap.
pub const ON_CURVE_PENALTY: f64 = 5.0;

#[derive(Debug, Clone, Copy)]
pub struct HitTestResult {
    pub entity: EntityId,
    pub distance: f64,
}

/// Find the closest entity to `point` within `max_dist`. The
/// `is_on_curve` flag drives the on-curve penalty.
pub fn find_closest(
    point: Point,
    candidates: impl Iterator<Item = (EntityId, Point, bool)>,
    max_dist: f64,
) -> Option<HitTestResult> {
    let mut best: Option<HitTestResult> = None;
    let mut best_score = f64::MAX;

    for (entity, candidate_pos, is_on_curve) in candidates {
        let dx = point.x - candidate_pos.x;
        let dy = point.y - candidate_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();

        let score = if is_on_curve {
            distance + ON_CURVE_PENALTY
        } else {
            distance
        };

        if distance <= max_dist && score < best_score {
            best_score = score;
            best = Some(HitTestResult { entity, distance });
        }
    }
    best
}
