// Copyright 2026 the Runebender Authors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local browser/WASM image tracing adapter.
//!
//! Keep `img2bez` details isolated here so updating the sibling tracer crate
//! does not require changes throughout the Vue host.

use serde::Deserialize;

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

    img2bez::trace_image_bytes_to_glif(image_bytes, &config.glyph, &tracing_config)
        .map_err(|e| format!("img2bez trace failed: {e}"))
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
}
