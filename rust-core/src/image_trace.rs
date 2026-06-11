// Copyright 2026 the Runebender Authors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local browser/WASM image tracing adapter.
//!
//! Keep `img2bez` details isolated here so updating the sibling tracer crate
//! does not require changes throughout the Vue host.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceImageConfig {
    glyph: String,
    unicode: Option<String>,
    width: Option<f64>,
    target_height: Option<f64>,
    x_offset: Option<f64>,
    y_offset: Option<f64>,
    grid: Option<i32>,
    accuracy: Option<f64>,
    smooth: Option<usize>,
    alphamax: Option<f64>,
    global_fit: Option<bool>,
    invert: Option<bool>,
    threshold: Option<u8>,
}

pub fn trace_image_to_glif(image_bytes: &[u8], config_json: &str) -> Result<String, String> {
    trace_image(image_bytes, config_json).map(|trace| trace.glif)
}

pub fn trace_image_to_glif_report(image_bytes: &[u8], config_json: &str) -> Result<String, String> {
    let trace = trace_image(image_bytes, config_json)?;
    serde_json::to_string(&trace).map_err(|e| format!("serialize trace report: {e}"))
}

fn trace_image(image_bytes: &[u8], config_json: &str) -> Result<TraceImageReport, String> {
    if image_bytes.is_empty() {
        return Err("image bytes are empty".to_string());
    }

    let config: TraceImageConfig =
        serde_json::from_str(config_json).map_err(|e| format!("parse trace config: {e}"))?;
    if config.glyph.trim().is_empty() {
        return Err("glyph name is required".to_string());
    }

    let grid = config.grid.unwrap_or(2).max(0);
    let snap_grid = grid.max(1) as f64;
    let x_offset = config.x_offset.unwrap_or(64.0);

    let mut tracing_config = img2bez::TracingConfig {
        advance_width: Some(config.width.unwrap_or(600.0).max(1.0)),
        target_height: config.target_height.unwrap_or(1088.0).max(1.0),
        y_offset: config.y_offset.unwrap_or(-256.0),
        lsb: (x_offset / snap_grid).round() * snap_grid,
        grid,
        fit_accuracy: config.accuracy.unwrap_or(4.0).max(0.1),
        smooth_iterations: config.smooth.unwrap_or(0),
        alphamax: config.alphamax.unwrap_or(0.35),
        global_fit: config.global_fit.unwrap_or(false),
        invert: config.invert.unwrap_or(false),
        threshold: config
            .threshold
            .map_or(img2bez::ThresholdMethod::Otsu, img2bez::ThresholdMethod::Fixed),
        ..img2bez::TracingConfig::default()
    };
    tracing_config.codepoints = parse_codepoints(config.unicode.as_deref())?;

    let result = img2bez::trace_image_bytes(image_bytes, &tracing_config)
        .map_err(|e| format!("img2bez trace failed: {e}"))?;
    let glif = img2bez::glif::to_glif(&config.glyph, &result, &tracing_config)
        .map_err(|e| format!("img2bez glif serialize failed: {e}"))?;
    let (curves, lines, on_curves, off_curves) = count_path_geometry(&result.paths);
    let diagnostics = result.diagnostics;

    Ok(TraceImageReport {
        glif,
        contours: result.paths.len(),
        curves,
        lines,
        on_curves,
        off_curves,
        advance_width: result.advance_width,
        reposition_shift_x: result.reposition_shift.0,
        reposition_shift_y: result.reposition_shift.1,
        diagnostics: TraceImageDiagnostics {
            missed_extrema_fixed: diagnostics.missed_extrema_fixed,
            high_deviation_splits: diagnostics.high_deviation_splits,
            strong_tangent_overrides: diagnostics.strong_tangent_overrides,
            clean_tangent_overrides: diagnostics.clean_tangent_overrides,
            visible_tangent_overrides: diagnostics.visible_tangent_overrides,
            rejected_tangent_near_misses: diagnostics.rejected_tangent_near_misses,
            oversegmented_splits_removed: diagnostics.oversegmented_splits_removed,
            final_outline_divergences: diagnostics.final_outline_divergences,
            final_outline_repairs: diagnostics.final_outline_repairs,
        },
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceImageReport {
    glif: String,
    contours: usize,
    curves: usize,
    lines: usize,
    on_curves: usize,
    off_curves: usize,
    advance_width: f64,
    reposition_shift_x: f64,
    reposition_shift_y: f64,
    diagnostics: TraceImageDiagnostics,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceImageDiagnostics {
    missed_extrema_fixed: usize,
    high_deviation_splits: usize,
    strong_tangent_overrides: usize,
    clean_tangent_overrides: usize,
    visible_tangent_overrides: usize,
    rejected_tangent_near_misses: usize,
    oversegmented_splits_removed: usize,
    final_outline_divergences: usize,
    final_outline_repairs: usize,
}

fn count_path_geometry(paths: &[img2bez::kurbo::BezPath]) -> (usize, usize, usize, usize) {
    let mut curves = 0usize;
    let mut lines = 0usize;
    let mut on_curves = 0usize;
    let mut off_curves = 0usize;
    for path in paths {
        for element in path.elements() {
            match element {
                img2bez::kurbo::PathEl::MoveTo(_) => {
                    on_curves += 1;
                }
                img2bez::kurbo::PathEl::LineTo(_) => {
                    lines += 1;
                    on_curves += 1;
                }
                img2bez::kurbo::PathEl::CurveTo(..) => {
                    curves += 1;
                    on_curves += 1;
                    off_curves += 2;
                }
                img2bez::kurbo::PathEl::QuadTo(..) => {
                    on_curves += 1;
                    off_curves += 1;
                }
                img2bez::kurbo::PathEl::ClosePath => {}
            }
        }
    }
    (curves, lines, on_curves, off_curves)
}

fn parse_codepoints(value: Option<&str>) -> Result<Vec<char>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(Vec::new());
    }

    let mut codepoints = Vec::new();
    for raw in value.split(|c: char| c == ',' || c.is_whitespace()) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let hex = raw
            .strip_prefix("U+")
            .or_else(|| raw.strip_prefix("u+"))
            .unwrap_or(raw);
        let codepoint =
            u32::from_str_radix(hex, 16).map_err(|_| format!("invalid unicode hex: {raw}"))?;
        let ch = char::from_u32(codepoint)
            .ok_or_else(|| format!("invalid unicode scalar value: {raw}"))?;
        codepoints.push(ch);
    }
    Ok(codepoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_unicode_forms() {
        assert_eq!(parse_codepoints(None).expect("none parses"), Vec::<char>::new());
        assert_eq!(
            parse_codepoints(Some("0023, U+0026")).expect("hex parses"),
            vec!['#', '&']
        );
    }

    #[test]
    fn rejects_invalid_unicode() {
        let err = parse_codepoints(Some("not-hex")).expect_err("invalid hex rejected");
        assert!(err.contains("invalid unicode hex"));
    }

    #[test]
    fn trace_report_rejects_empty_image_bytes() {
        let err = trace_image_to_glif_report(&[], r#"{"glyph":"a"}"#)
            .expect_err("empty image bytes rejected");
        assert!(err.contains("image bytes are empty"));
    }
}
