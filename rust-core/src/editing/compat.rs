// Ported from runebender-xilem/src/editing/compat.rs (Apache-2.0).
//
// Comfy keeps UFO/designspace data in the browser and hands individual
// `.glif` files through wasm, so this checker works directly on parsed
// `norad::Glyph` values instead of xilem's Workspace locks.

//! Interpolation compatibility checking across designspace masters.

use serde::Serialize;

/// A single interpolation compatibility error.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatError {
    pub kind: CompatErrorKind,
    pub master_name: String,
    pub message: String,
    pub contour_index: Option<usize>,
    pub point_index: Option<usize>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatErrorKind {
    MissingGlyph,
    ContourCountMismatch,
    PointCountMismatch,
    PointTypeMismatch,
}

/// Check interpolation compatibility for a glyph across other masters.
///
/// `reference_glyph` is the glyph from the current active master.
/// Other masters are compared against it in order.
pub fn check_compat(
    glyph_name: &str,
    reference_glyph: &norad::Glyph,
    masters: &[(String, Option<norad::Glyph>)],
) -> Vec<CompatError> {
    let mut errors = Vec::new();
    let ref_contours = &reference_glyph.contours;

    for (master_name, other_glyph) in masters {
        let Some(other_glyph) = other_glyph else {
            errors.push(CompatError {
                kind: CompatErrorKind::MissingGlyph,
                master_name: master_name.clone(),
                message: format!("Glyph '{glyph_name}' missing from '{master_name}'"),
                contour_index: None,
                point_index: None,
                x: None,
                y: None,
                expected: None,
                actual: None,
            });
            continue;
        };

        let other_contours = &other_glyph.contours;
        if ref_contours.len() != other_contours.len() {
            errors.push(CompatError {
                kind: CompatErrorKind::ContourCountMismatch,
                master_name: master_name.clone(),
                message: format!(
                    "Contour count: expected {}, got {} in '{}'",
                    ref_contours.len(),
                    other_contours.len(),
                    master_name
                ),
                contour_index: None,
                point_index: None,
                x: None,
                y: None,
                expected: Some(ref_contours.len().to_string()),
                actual: Some(other_contours.len().to_string()),
            });
            continue;
        }

        for (contour_index, (ref_contour, other_contour)) in
            ref_contours.iter().zip(other_contours.iter()).enumerate()
        {
            if ref_contour.points.len() != other_contour.points.len() {
                let anchor = ref_contour.points.first();
                errors.push(CompatError {
                    kind: CompatErrorKind::PointCountMismatch,
                    master_name: master_name.clone(),
                    message: format!(
                        "Contour {contour_index}: expected {} points, got {} in '{}'",
                        ref_contour.points.len(),
                        other_contour.points.len(),
                        master_name
                    ),
                    contour_index: Some(contour_index),
                    point_index: None,
                    x: anchor.map(|pt| pt.x),
                    y: anchor.map(|pt| pt.y),
                    expected: Some(ref_contour.points.len().to_string()),
                    actual: Some(other_contour.points.len().to_string()),
                });
                continue;
            }

            for (point_index, (ref_point, other_point)) in ref_contour
                .points
                .iter()
                .zip(other_contour.points.iter())
                .enumerate()
            {
                if !point_types_compatible(&ref_point.typ, &other_point.typ) {
                    errors.push(CompatError {
                        kind: CompatErrorKind::PointTypeMismatch,
                        master_name: master_name.clone(),
                        message: format!(
                            "Contour {contour_index}, point {point_index}: expected {:?}, got {:?} in '{}'",
                            ref_point.typ, other_point.typ, master_name
                        ),
                        contour_index: Some(contour_index),
                        point_index: Some(point_index),
                        x: Some(ref_point.x),
                        y: Some(ref_point.y),
                        expected: Some(format!("{:?}", ref_point.typ)),
                        actual: Some(format!("{:?}", other_point.typ)),
                    });
                }
            }
        }
    }

    errors
}

fn point_types_compatible(a: &norad::PointType, b: &norad::PointType) -> bool {
    use norad::PointType;

    match (a, b) {
        (x, y) if x == y => true,
        (PointType::Curve, PointType::Line) | (PointType::Line, PointType::Curve) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph_with_point_types(types: &[norad::PointType]) -> norad::Glyph {
        let points = types
            .iter()
            .enumerate()
            .map(|(index, typ)| {
                norad::ContourPoint::new(
                    index as f64 * 100.0,
                    0.0,
                    typ.clone(),
                    false,
                    None,
                    None,
                    None,
                )
            })
            .collect();
        let contour = norad::Contour::new(points, None, None);
        let mut glyph = norad::Glyph::new("A");
        glyph.contours.push(contour);
        glyph
    }

    #[test]
    fn compatible_matching_glyphs_have_no_errors() {
        let reference = glyph_with_point_types(&[norad::PointType::Move, norad::PointType::Line]);
        let other = glyph_with_point_types(&[norad::PointType::Move, norad::PointType::Curve]);

        let errors = check_compat("A", &reference, &[("Bold".to_string(), Some(other))]);

        assert!(errors.is_empty());
    }

    #[test]
    fn reports_point_type_mismatch_with_reference_position() {
        let reference = glyph_with_point_types(&[norad::PointType::Move, norad::PointType::Line]);
        let other = glyph_with_point_types(&[norad::PointType::Move, norad::PointType::OffCurve]);

        let errors = check_compat("A", &reference, &[("Bold".to_string(), Some(other))]);

        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0].kind, CompatErrorKind::PointTypeMismatch));
        assert_eq!(errors[0].contour_index, Some(0));
        assert_eq!(errors[0].point_index, Some(1));
        assert_eq!(errors[0].x, Some(100.0));
    }

    #[test]
    fn reports_missing_glyph() {
        let reference = glyph_with_point_types(&[norad::PointType::Move]);

        let errors = check_compat("A", &reference, &[("Bold".to_string(), None)]);

        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0].kind, CompatErrorKind::MissingGlyph));
    }
}
