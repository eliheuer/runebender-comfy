// Ported from runebender-xilem/src/model/glyph_renderer.rs (Apache-2.0).

//! Glyph rendering — converts glyph contours to Kurbo paths.

use super::workspace::{Contour, ContourPoint, Glyph, PointType, Workspace};
use kurbo::{Affine, BezPath, Point};

/// Convert a `Glyph` to a `BezPath` (contours only, no components).
pub fn glyph_to_bezpath(glyph: &Glyph) -> BezPath {
    let mut path = BezPath::new();
    for contour in &glyph.contours {
        append_contour_to_path(&mut path, contour);
    }
    path
}

/// Convert a `Glyph` to a `BezPath` including components. Recursively
/// resolves component references and applies their transforms.
pub fn glyph_to_bezpath_with_components(
    glyph: &Glyph,
    workspace: &Workspace,
) -> BezPath {
    let mut path = BezPath::new();

    for contour in &glyph.contours {
        append_contour_to_path(&mut path, contour);
    }

    append_components_to_path(&mut path, glyph, workspace, Affine::IDENTITY);

    path
}

fn append_components_to_path(
    path: &mut BezPath,
    glyph: &Glyph,
    workspace: &Workspace,
    parent_transform: Affine,
) {
    for component in &glyph.components {
        // Skip components whose base glyph is missing.
        let base_glyph = match workspace.glyphs.get(&component.base) {
            Some(g) => g,
            None => continue,
        };

        let combined_transform = parent_transform * component.transform;

        for contour in &base_glyph.contours {
            let mut contour_path = BezPath::new();
            append_contour_to_path(&mut contour_path, contour);
            let transformed = combined_transform * &contour_path;
            path.extend(transformed.elements().iter().cloned());
        }

        append_components_to_path(path, base_glyph, workspace, combined_transform);
    }
}

fn append_contour_to_path(path: &mut BezPath, contour: &Contour) {
    let points = &contour.points;
    if points.is_empty() {
        return;
    }

    let is_hyperbezier = points
        .iter()
        .any(|pt| matches!(pt.point_type, PointType::Hyper | PointType::HyperCorner));

    if is_hyperbezier {
        append_hyperbezier_contour(path, contour);
        return;
    }

    let start_idx = points
        .iter()
        .position(|p| {
            matches!(
                p.point_type,
                PointType::Move | PointType::Line | PointType::Curve
            )
        })
        .unwrap_or(0);

    let rotated: Vec<_> = points[start_idx..]
        .iter()
        .chain(points[..start_idx].iter())
        .collect();

    if rotated.is_empty() {
        return;
    }

    let first = rotated[0];
    path.move_to(point_to_kurbo(first));

    let mut i = 1;
    while i < rotated.len() {
        let pt = rotated[i];

        match pt.point_type {
            PointType::Move => {
                path.move_to(point_to_kurbo(pt));
                i += 1;
            }
            PointType::Line => {
                path.line_to(point_to_kurbo(pt));
                i += 1;
            }
            PointType::Curve => {
                let off_curve_points = collect_preceding_off_curve_points(&rotated, i);
                add_curve_segment(path, &off_curve_points, pt);
                i += 1;
            }
            PointType::OffCurve => {
                // Handled when we hit the following on-curve point.
                i += 1;
            }
            PointType::QCurve => {
                if i > 0 && rotated[i - 1].point_type == PointType::OffCurve {
                    let cp = point_to_kurbo(rotated[i - 1]);
                    let end = point_to_kurbo(pt);
                    path.quad_to(cp, end);
                } else {
                    path.line_to(point_to_kurbo(pt));
                }
                i += 1;
            }
            PointType::Hyper | PointType::HyperCorner => {
                // Hyperbezier points should not appear here — they
                // should be routed through `Path::Hyper`.
                path.line_to(point_to_kurbo(pt));
                i += 1;
            }
        }
    }

    handle_trailing_off_curve_points(path, &rotated);
}

fn point_to_kurbo(pt: &ContourPoint) -> Point {
    Point::new(pt.x, pt.y)
}

fn collect_preceding_off_curve_points<'a>(
    rotated: &'a [&'a ContourPoint],
    current_idx: usize,
) -> Vec<&'a ContourPoint> {
    let mut off_curve_points = Vec::new();
    let mut j = current_idx.saturating_sub(1);

    while j > 0 && rotated[j].point_type == PointType::OffCurve {
        off_curve_points.insert(0, rotated[j]);
        j -= 1;
    }

    off_curve_points
}

fn add_curve_segment(
    path: &mut BezPath,
    off_curve_points: &[&ContourPoint],
    end_point: &ContourPoint,
) {
    match off_curve_points.len() {
        0 => {
            path.line_to(point_to_kurbo(end_point));
        }
        1 => {
            let cp = point_to_kurbo(off_curve_points[0]);
            let end = point_to_kurbo(end_point);
            path.quad_to(cp, end);
        }
        2 => {
            let cp1 = point_to_kurbo(off_curve_points[0]);
            let cp2 = point_to_kurbo(off_curve_points[1]);
            let end = point_to_kurbo(end_point);
            path.curve_to(cp1, cp2, end);
        }
        _ => {
            let len = off_curve_points.len();
            let cp1 = point_to_kurbo(off_curve_points[len - 2]);
            let cp2 = point_to_kurbo(off_curve_points[len - 1]);
            let end = point_to_kurbo(end_point);
            path.curve_to(cp1, cp2, end);
        }
    }
}

fn handle_trailing_off_curve_points(path: &mut BezPath, rotated: &[&ContourPoint]) {
    let trailing_off_curve = collect_trailing_off_curve_points(rotated);

    if trailing_off_curve.is_empty() {
        path.close_path();
        return;
    }

    let first_pt = rotated[0];
    add_closing_curve(path, &trailing_off_curve, first_pt);
}

fn collect_trailing_off_curve_points<'a>(
    rotated: &'a [&'a ContourPoint],
) -> Vec<&'a ContourPoint> {
    let mut trailing_off_curve = Vec::new();
    let mut j = rotated.len().saturating_sub(1);

    while j > 0 && rotated[j].point_type == PointType::OffCurve {
        trailing_off_curve.insert(0, rotated[j]);
        j -= 1;
    }

    trailing_off_curve
}

fn add_closing_curve(
    path: &mut BezPath,
    trailing_off_curve: &[&ContourPoint],
    first_pt: &ContourPoint,
) {
    match first_pt.point_type {
        PointType::Curve => {
            add_curve_segment(path, trailing_off_curve, first_pt);
        }
        PointType::QCurve => {
            if !trailing_off_curve.is_empty() {
                let cp = point_to_kurbo(trailing_off_curve[0]);
                let end = point_to_kurbo(first_pt);
                path.quad_to(cp, end);
            } else {
                path.close_path();
            }
        }
        _ => {
            path.close_path();
        }
    }
}

/// Append a hyperbezier contour using the spline solver.
fn append_hyperbezier_contour(path: &mut BezPath, contour: &Contour) {
    use super::entity_id::EntityId;
    use crate::path::HyperPath;
    use crate::path::PathPoints;
    use crate::path::{PathPoint, PointType as PathPointType};

    let path_points: Vec<PathPoint> = contour
        .points
        .iter()
        .map(|pt| PathPoint {
            id: EntityId::next(),
            point: Point::new(pt.x, pt.y),
            typ: match pt.point_type {
                PointType::Hyper => PathPointType::OnCurve { smooth: true },
                PointType::HyperCorner => PathPointType::OnCurve { smooth: false },
                _ => PathPointType::OnCurve { smooth: true },
            },
        })
        .collect();

    let closed = !matches!(
        contour.points.first().map(|p| p.point_type),
        Some(PointType::Move)
    );

    let hyper_path = HyperPath::from_points(PathPoints::from_vec(path_points), closed);
    let bezier = hyper_path.to_bezpath();

    for el in bezier.elements() {
        match el {
            kurbo::PathEl::MoveTo(p) => path.move_to(*p),
            kurbo::PathEl::LineTo(p) => path.line_to(*p),
            kurbo::PathEl::QuadTo(p1, p2) => path.quad_to(*p1, *p2),
            kurbo::PathEl::CurveTo(p1, p2, p3) => path.curve_to(*p1, *p2, *p3),
            kurbo::PathEl::ClosePath => path.close_path(),
        }
    }
}
