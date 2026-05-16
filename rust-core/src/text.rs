//! Text buffer state for the Text tool.
//!
//! This is the wasm-core counterpart to runebender-xilem's `sort`
//! buffer. It intentionally starts small: Vue still owns glyph lookup
//! and preview rendering today, but cursor movement, line breaks, and
//! active sort selection now have a Rust-side home we can migrate to.

use runebender_core::shaping;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextSortKind {
    Glyph {
        name: String,
        codepoint: Option<char>,
        advance_width: f64,
    },
    LineBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSort {
    pub kind: TextSortKind,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub items: Vec<TextLayoutItem>,
    pub cursor_x: f64,
    pub cursor_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLayoutItem {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub advance_width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextHit {
    pub cursor: usize,
    pub active_sort: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct TextKerningModel {
    #[serde(default)]
    groups: HashMap<String, Vec<String>>,
    #[serde(default)]
    kerning: HashMap<String, HashMap<String, f64>>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TextGlyphInventory {
    #[serde(default)]
    unicode: HashMap<u32, String>,
    #[serde(default)]
    widths: HashMap<String, f64>,
    #[serde(default)]
    outlines: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ManualKerningSession {
    sort_index: usize,
    start_x: f64,
    original_value: f64,
    current_offset: f64,
}

impl TextSort {
    pub fn glyph(name: impl Into<String>, codepoint: Option<char>, advance_width: f64) -> Self {
        Self {
            kind: TextSortKind::Glyph {
                name: name.into(),
                codepoint,
                advance_width,
            },
            active: false,
        }
    }

    pub fn line_break() -> Self {
        Self {
            kind: TextSortKind::LineBreak,
            active: false,
        }
    }

    pub fn glyph_name(&self) -> Option<&str> {
        match &self.kind {
            TextSortKind::Glyph { name, .. } => Some(name),
            TextSortKind::LineBreak => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextBuffer {
    sorts: Vec<TextSort>,
    cursor: usize,
    active_sort: Option<usize>,
    direction: TextDirection,
    kerning: TextKerningModel,
    glyph_inventory: TextGlyphInventory,
    manual_kerning: Option<ManualKerningSession>,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sorts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sorts.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn active_sort(&self) -> Option<usize> {
        self.active_sort
    }

    pub fn manual_kerning_sort(&self) -> Option<usize> {
        self.manual_kerning.map(|session| session.sort_index)
    }

    pub fn sort(&self, index: usize) -> Option<&TextSort> {
        self.sorts.get(index)
    }

    pub fn glyph_outline_svg(&self, glyph_name: &str) -> Option<&str> {
        self.glyph_inventory
            .outlines
            .get(glyph_name)
            .map(String::as_str)
    }

    pub fn update_glyph(
        &mut self,
        index: usize,
        name: impl Into<String>,
        codepoint: Option<char>,
        advance_width: f64,
    ) -> bool {
        let Some(sort) = self.sorts.get_mut(index) else {
            return false;
        };
        let TextSortKind::Glyph {
            name: glyph_name,
            codepoint: glyph_codepoint,
            advance_width: glyph_advance_width,
        } = &mut sort.kind
        else {
            return false;
        };
        self.manual_kerning = None;
        *glyph_name = name.into();
        *glyph_codepoint = codepoint;
        *glyph_advance_width = advance_width;
        true
    }

    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    pub fn set_direction(&mut self, direction: TextDirection) {
        self.manual_kerning = None;
        self.direction = direction;
    }

    pub fn set_kerning_model(&mut self, kerning: TextKerningModel) {
        self.kerning = kerning;
        self.manual_kerning = None;
    }

    pub fn kerning_model(&self) -> &TextKerningModel {
        &self.kerning
    }

    pub fn set_glyph_inventory(&mut self, glyph_inventory: TextGlyphInventory) {
        self.manual_kerning = None;
        self.glyph_inventory = glyph_inventory;
    }

    pub fn iter(&self) -> impl Iterator<Item = &TextSort> {
        self.sorts.iter()
    }

    pub fn insert_character(&mut self, char: char) -> bool {
        let Some(glyph_name) = self.glyph_inventory.unicode.get(&(char as u32)).cloned() else {
            return false;
        };
        let advance_width = self
            .glyph_inventory
            .widths
            .get(&glyph_name)
            .copied()
            .unwrap_or(600.0);
        self.insert_glyph(glyph_name, Some(char), advance_width);
        self.shape_arabic();
        true
    }

    pub fn layout(&self, line_height: f64) -> TextLayout {
        let mut items = Vec::new();
        let mut cursor_x = 0.0;
        let mut cursor_y = 0.0;
        let mut line_start = 0;

        while line_start <= self.sorts.len() {
            let line_end = self.next_line_end(line_start);
            let line_width = self.line_width(line_start, line_end);
            let mut x = match self.direction {
                TextDirection::LeftToRight => 0.0,
                TextDirection::RightToLeft => line_width,
            };
            let mut previous_glyph_name: Option<&str> = None;
            let y = -line_height * self.line_number_for_index(line_start) as f64;

            if self.cursor == line_start {
                cursor_x = x;
                cursor_y = y;
            }

            for index in line_start..line_end {
                let advance_width = self.sort_advance(index);
                let glyph_name = self.sort_glyph_name(index);
                let kern = previous_glyph_name
                    .zip(glyph_name)
                    .map(|(left, right)| self.lookup_kerning(left, right))
                    .unwrap_or(0.0);
                match self.direction {
                    TextDirection::LeftToRight => {
                        x += kern;
                        items.push(TextLayoutItem {
                            index,
                            x,
                            y,
                            advance_width,
                        });
                        x += advance_width;
                    }
                    TextDirection::RightToLeft => {
                        x -= advance_width + kern;
                        items.push(TextLayoutItem {
                            index,
                            x,
                            y,
                            advance_width,
                        });
                    }
                }

                previous_glyph_name = glyph_name;
                if self.cursor == index + 1 {
                    cursor_x = x;
                    cursor_y = y;
                }
            }

            if line_end >= self.sorts.len() {
                break;
            }

            // Skip the line-break sort.
            if self.cursor == line_end + 1 {
                cursor_x = 0.0;
                cursor_y = -line_height * (self.line_number_for_index(line_end) + 1) as f64;
            }
            line_start = line_end + 1;
        }

        TextLayout {
            items,
            cursor_x,
            cursor_y,
        }
    }

    pub fn hit_test(&self, x: f64, y: f64, line_height: f64) -> TextHit {
        if self.sorts.is_empty() {
            return TextHit {
                cursor: 0,
                active_sort: None,
            };
        }

        let line_height = line_height.max(1.0);
        let target_line = ((-y / line_height).round().max(0.0)) as usize;
        let (line_start, line_end) = self.line_range_for_number(target_line);
        let layout = self.layout(line_height);
        let nearest_cursor = self.nearest_cursor_for_line(x, line_start, line_end, &layout);

        for item in layout
            .items
            .iter()
            .filter(|item| (line_start..line_end).contains(&item.index))
        {
            let within_x = x >= item.x && x <= item.x + item.advance_width;
            let within_y = y >= item.y - line_height * 0.25 && y <= item.y + line_height;
            if within_x && within_y {
                return TextHit {
                    cursor: item.index + 1,
                    active_sort: Some(item.index),
                };
            }
        }

        TextHit {
            cursor: nearest_cursor,
            active_sort: None,
        }
    }

    pub fn clear(&mut self) {
        self.sorts.clear();
        self.cursor = 0;
        self.active_sort = None;
        self.manual_kerning = None;
    }

    pub fn insert_glyph(
        &mut self,
        name: impl Into<String>,
        codepoint: Option<char>,
        advance_width: f64,
    ) {
        self.manual_kerning = None;
        let index = self.cursor;
        self.sorts
            .insert(index, TextSort::glyph(name, codepoint, advance_width));
        self.set_active_sort(Some(index));
        self.cursor += 1;
    }

    pub fn insert_line_break(&mut self) {
        self.manual_kerning = None;
        self.sorts.insert(self.cursor, TextSort::line_break());
        self.cursor += 1;
        self.set_active_sort(None);
    }

    pub fn delete_before_cursor(&mut self) -> Option<TextSort> {
        if self.cursor == 0 {
            return None;
        }
        self.manual_kerning = None;
        let deleted_index = self.cursor - 1;
        let deleted = self.sorts.remove(deleted_index);
        self.cursor -= 1;
        self.adjust_active_after_delete(deleted_index);
        Some(deleted)
    }

    pub fn delete_after_cursor(&mut self) -> Option<TextSort> {
        if self.cursor >= self.sorts.len() {
            return None;
        }
        self.manual_kerning = None;
        let deleted = self.sorts.remove(self.cursor);
        self.adjust_active_after_delete(self.cursor);
        Some(deleted)
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.sorts.len());
    }

    pub fn move_cursor_visual_left(&mut self) {
        match self.direction {
            TextDirection::LeftToRight => self.move_cursor_left(),
            TextDirection::RightToLeft => self.move_cursor_right(),
        }
    }

    pub fn move_cursor_visual_right(&mut self) {
        match self.direction {
            TextDirection::LeftToRight => self.move_cursor_right(),
            TextDirection::RightToLeft => self.move_cursor_left(),
        }
    }

    pub fn move_cursor_visual_up(&mut self, line_height: f64) {
        self.move_cursor_vertical(line_height, -1);
    }

    pub fn move_cursor_visual_down(&mut self, line_height: f64) {
        self.move_cursor_vertical(line_height, 1);
    }

    pub fn move_cursor_line_start(&mut self) {
        let line_number = self.line_number_for_index(self.cursor);
        let (line_start, _) = self.line_range_for_number(line_number);
        self.cursor = line_start;
    }

    pub fn move_cursor_line_end(&mut self) {
        let line_number = self.line_number_for_index(self.cursor);
        let (_, line_end) = self.line_range_for_number(line_number);
        self.cursor = line_end;
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.sorts.len());
    }

    pub fn activate_sort(&mut self, index: usize) -> bool {
        if !matches!(
            self.sorts.get(index).map(|sort| &sort.kind),
            Some(TextSortKind::Glyph { .. })
        ) {
            return false;
        }
        self.set_active_sort(Some(index));
        self.cursor = index + 1;
        true
    }

    pub fn begin_manual_kerning(&mut self, sort_index: usize, start_x: f64) -> bool {
        if sort_index == 0 || !self.sort_pair_is_glyph_pair(sort_index) {
            return false;
        }
        let Some((left, right)) = self.glyph_pair_names(sort_index) else {
            return false;
        };
        let original_value = self.lookup_kerning(&left, &right);
        self.manual_kerning = Some(ManualKerningSession {
            sort_index,
            start_x,
            original_value,
            current_offset: 0.0,
        });
        self.activate_sort(sort_index);
        true
    }

    pub fn drag_manual_kerning(&mut self, current_x: f64) -> Option<f64> {
        let session = self.manual_kerning?;
        let (left, right) = self.glyph_pair_names(session.sort_index)?;
        let current_offset = current_x - session.start_x;
        let value = session.original_value + current_offset;
        self.set_direct_kerning(&left, &right, value);
        self.manual_kerning = Some(ManualKerningSession {
            current_offset,
            ..session
        });
        Some(value)
    }

    pub fn end_manual_kerning(&mut self) -> bool {
        self.manual_kerning.take().is_some()
    }

    pub fn shape_arabic(&mut self) -> bool {
        let mut changed = false;
        let mut line_start = 0;

        while line_start <= self.sorts.len() {
            let line_end = self.next_line_end(line_start);
            let mut updates = Vec::new();
            self.shape_line(line_start, line_end, &mut updates);

            for (index, name, advance_width) in updates {
                let Some(sort) = self.sorts.get_mut(index) else {
                    continue;
                };
                let TextSortKind::Glyph {
                    name: glyph_name,
                    advance_width: glyph_advance_width,
                    ..
                } = &mut sort.kind
                else {
                    continue;
                };
                if *glyph_name != name || *glyph_advance_width != advance_width {
                    *glyph_name = name;
                    *glyph_advance_width = advance_width;
                    changed = true;
                }
            }

            if line_end >= self.sorts.len() {
                break;
            }
            line_start = line_end + 1;
        }

        changed
    }

    fn set_active_sort(&mut self, active: Option<usize>) {
        for sort in &mut self.sorts {
            sort.active = false;
        }
        if let Some(index) = active
            && let Some(sort) = self.sorts.get_mut(index)
        {
            sort.active = true;
            self.active_sort = Some(index);
        } else {
            self.active_sort = None;
        }
    }

    fn adjust_active_after_delete(&mut self, deleted_index: usize) {
        let Some(active) = self.active_sort else {
            return;
        };
        if active == deleted_index {
            self.set_active_sort(None);
        } else if active > deleted_index {
            self.active_sort = Some(active - 1);
        }
    }

    fn shape_line(
        &self,
        line_start: usize,
        line_end: usize,
        updates: &mut Vec<(usize, String, f64)>,
    ) {
        let chars = self.sorts[line_start..line_end]
            .iter()
            .filter_map(|sort| match sort.kind {
                TextSortKind::Glyph {
                    codepoint: Some(char),
                    ..
                } => Some(char),
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut char_index = 0;
        for index in line_start..line_end {
            let Some(char) = self.sort_codepoint(index) else {
                continue;
            };
            let name = self.shaped_glyph_name_for_character(char, &chars, char_index, index);
            let advance_width = self
                .glyph_inventory
                .widths
                .get(&name)
                .copied()
                .unwrap_or_else(|| self.sort_advance(index));
            updates.push((index, name, advance_width));
            char_index += 1;
        }
    }

    fn shaped_glyph_name_for_character(
        &self,
        char: char,
        line_chars: &[char],
        char_index: usize,
        sort_index: usize,
    ) -> String {
        let base_name = self
            .glyph_inventory
            .unicode
            .get(&(char as u32))
            .cloned()
            .or_else(|| self.sort_glyph_name(sort_index).map(ToOwned::to_owned))
            .unwrap_or_else(|| ".notdef".to_string());
        if self.direction != TextDirection::RightToLeft || !shaping::is_arabic(char) {
            return base_name;
        }

        let suffix = shaping::arabic_positional_form(line_chars, char_index).suffix();
        let shaped_name = format!("{base_name}{suffix}");
        if !suffix.is_empty() && self.glyph_inventory.widths.contains_key(&shaped_name) {
            shaped_name
        } else {
            base_name
        }
    }

    fn next_line_end(&self, start: usize) -> usize {
        self.sorts[start..]
            .iter()
            .position(|sort| matches!(sort.kind, TextSortKind::LineBreak))
            .map(|offset| start + offset)
            .unwrap_or(self.sorts.len())
    }

    fn line_range_for_number(&self, line_number: usize) -> (usize, usize) {
        let mut start = 0;
        let mut current_line = 0;
        while start <= self.sorts.len() {
            let end = self.next_line_end(start);
            if current_line == line_number || end >= self.sorts.len() {
                return (start, end);
            }
            start = end + 1;
            current_line += 1;
        }
        (self.sorts.len(), self.sorts.len())
    }

    fn line_width(&self, start: usize, end: usize) -> f64 {
        let mut width = 0.0;
        let mut previous_glyph_name: Option<&str> = None;
        for index in start..end {
            let glyph_name = self.sort_glyph_name(index);
            if let Some((left, right)) = previous_glyph_name.zip(glyph_name) {
                width += self.lookup_kerning(left, right);
            }
            width += self.sort_advance(index);
            previous_glyph_name = glyph_name;
        }
        width
    }

    fn nearest_cursor_for_line(
        &self,
        x: f64,
        line_start: usize,
        line_end: usize,
        layout: &TextLayout,
    ) -> usize {
        let mut nearest_cursor = line_start;
        let mut nearest_distance = f64::INFINITY;
        let line_width = self.line_width(line_start, line_end);

        for candidate in line_start..=line_end {
            let cursor_x = if candidate == line_start {
                match self.direction {
                    TextDirection::LeftToRight => 0.0,
                    TextDirection::RightToLeft => line_width,
                }
            } else {
                layout
                    .items
                    .iter()
                    .find(|item| item.index + 1 == candidate)
                    .map(|item| match self.direction {
                        TextDirection::LeftToRight => item.x + item.advance_width,
                        TextDirection::RightToLeft => item.x,
                    })
                    .unwrap_or(0.0)
            };
            let distance = (x - cursor_x).abs();
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_cursor = candidate;
            }
        }

        nearest_cursor
    }

    fn move_cursor_vertical(&mut self, line_height: f64, line_delta: isize) {
        if self.sorts.is_empty() {
            return;
        }
        let line_height = line_height.max(1.0);
        let layout = self.layout(line_height);
        let current_line = ((-layout.cursor_y / line_height).round().max(0.0)) as isize;
        let target_line = (current_line + line_delta).max(0) as usize;
        let (line_start, line_end) = self.line_range_for_number(target_line);
        self.cursor = self.nearest_cursor_for_line(layout.cursor_x, line_start, line_end, &layout);
    }

    fn sort_advance(&self, index: usize) -> f64 {
        match &self.sorts[index].kind {
            TextSortKind::Glyph { advance_width, .. } => *advance_width,
            TextSortKind::LineBreak => 0.0,
        }
    }

    fn sort_glyph_name(&self, index: usize) -> Option<&str> {
        match &self.sorts[index].kind {
            TextSortKind::Glyph { name, .. } => Some(name),
            TextSortKind::LineBreak => None,
        }
    }

    fn sort_codepoint(&self, index: usize) -> Option<char> {
        match &self.sorts[index].kind {
            TextSortKind::Glyph { codepoint, .. } => *codepoint,
            TextSortKind::LineBreak => None,
        }
    }

    fn sort_pair_is_glyph_pair(&self, sort_index: usize) -> bool {
        matches!(
            (self.sorts.get(sort_index - 1), self.sorts.get(sort_index)),
            (
                Some(TextSort {
                    kind: TextSortKind::Glyph { .. },
                    ..
                }),
                Some(TextSort {
                    kind: TextSortKind::Glyph { .. },
                    ..
                })
            )
        )
    }

    fn glyph_pair_names(&self, sort_index: usize) -> Option<(String, String)> {
        let left = self.sort_glyph_name(sort_index.checked_sub(1)?)?;
        let right = self.sort_glyph_name(sort_index)?;
        Some((left.to_string(), right.to_string()))
    }

    fn lookup_kerning(&self, left: &str, right: &str) -> f64 {
        let left_groups = self.groups_for_glyph(left, Some("public.kern1."));
        let right_groups = self.groups_for_glyph(right, Some("public.kern2."));

        if let Some(value) = self.kerning_value(left, right) {
            return value;
        }
        for right_group in &right_groups {
            if let Some(value) = self.kerning_value(left, right_group) {
                return value;
            }
        }
        for left_group in &left_groups {
            if let Some(value) = self.kerning_value(left_group, right) {
                return value;
            }
        }
        for left_group in &left_groups {
            for right_group in &right_groups {
                if let Some(value) = self.kerning_value(left_group, right_group) {
                    return value;
                }
            }
        }
        0.0
    }

    fn set_direct_kerning(&mut self, left: &str, right: &str, value: f64) {
        if value.abs() < f64::EPSILON {
            if let Some(pairs) = self.kerning.kerning.get_mut(left) {
                pairs.remove(right);
                if pairs.is_empty() {
                    self.kerning.kerning.remove(left);
                }
            }
            return;
        }
        self.kerning
            .kerning
            .entry(left.to_string())
            .or_default()
            .insert(right.to_string(), value);
    }

    fn kerning_value(&self, left: &str, right: &str) -> Option<f64> {
        self.kerning
            .kerning
            .get(left)
            .and_then(|pairs| pairs.get(right))
            .copied()
    }

    fn groups_for_glyph(&self, glyph_name: &str, prefix: Option<&str>) -> Vec<&str> {
        self.kerning
            .groups
            .iter()
            .filter_map(|(group_name, members)| {
                if prefix.is_some_and(|prefix| !group_name.starts_with(prefix)) {
                    return None;
                }
                members
                    .iter()
                    .any(|member| member == glyph_name)
                    .then_some(group_name.as_str())
            })
            .collect()
    }

    fn line_number_for_index(&self, index: usize) -> usize {
        self.sorts[..index]
            .iter()
            .filter(|sort| matches!(sort.kind, TextSortKind::LineBreak))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_glyph_moves_cursor_and_sets_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.cursor(), 2);
        assert_eq!(buffer.active_sort(), Some(1));
        assert_eq!(
            buffer.iter().last().and_then(TextSort::glyph_name),
            Some("B")
        );
    }

    #[test]
    fn insert_character_uses_glyph_inventory() {
        let mut buffer = TextBuffer::new();
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A" },
                    "widths": { "A": 640 }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('A'));
        assert!(!buffer.insert_character('Z'));

        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("A"));
        let TextSortKind::Glyph {
            codepoint,
            advance_width,
            ..
        } = &buffer.sort(0).expect("sort exists").kind
        else {
            panic!("expected glyph sort");
        };
        assert_eq!(*codepoint, Some('A'));
        assert_eq!(*advance_width, 640.0);
    }

    #[test]
    fn insert_character_shapes_rtl_arabic_neighbors() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1605": "meem-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "meem-ar": 520,
                        "meem-ar.fina": 500
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0645}'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("meem-ar.fina")
        );
    }

    #[test]
    fn delete_before_cursor_updates_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);
        buffer.activate_sort(1);
        buffer.set_cursor(1);

        let deleted = buffer.delete_before_cursor();

        assert_eq!(deleted.as_ref().and_then(TextSort::glyph_name), Some("A"));
        assert_eq!(buffer.cursor(), 0);
        assert_eq!(buffer.active_sort(), Some(0));
    }

    #[test]
    fn line_break_clears_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_line_break();

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.cursor(), 2);
        assert_eq!(buffer.active_sort(), None);
    }

    #[test]
    fn visual_cursor_movement_respects_direction() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 600.0);

        buffer.move_cursor_visual_left();
        assert_eq!(buffer.cursor(), 1);

        buffer.set_direction(TextDirection::RightToLeft);
        buffer.move_cursor_visual_left();
        assert_eq!(buffer.cursor(), 2);
        buffer.move_cursor_visual_right();
        assert_eq!(buffer.cursor(), 1);
    }

    #[test]
    fn vertical_cursor_movement_preserves_visual_x_across_lines() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 300.0);
        buffer.insert_glyph("B", Some('B'), 300.0);
        buffer.insert_line_break();
        buffer.insert_glyph("C", Some('C'), 250.0);
        buffer.insert_glyph("D", Some('D'), 250.0);
        buffer.insert_glyph("E", Some('E'), 250.0);
        buffer.set_cursor(2);

        buffer.move_cursor_visual_down(1000.0);
        assert_eq!(buffer.cursor(), 5);

        buffer.move_cursor_visual_up(1000.0);
        assert_eq!(buffer.cursor(), 2);
    }

    #[test]
    fn vertical_cursor_movement_clamps_at_first_and_last_line() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 300.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 300.0);
        buffer.set_cursor(0);

        buffer.move_cursor_visual_up(1000.0);
        assert_eq!(buffer.cursor(), 0);

        buffer.set_cursor(3);
        buffer.move_cursor_visual_down(1000.0);
        assert_eq!(buffer.cursor(), 3);
    }

    #[test]
    fn line_start_and_end_cursor_movement_stays_on_current_line() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 300.0);
        buffer.insert_glyph("B", Some('B'), 300.0);
        buffer.insert_line_break();
        buffer.insert_glyph("C", Some('C'), 300.0);
        buffer.insert_glyph("D", Some('D'), 300.0);
        buffer.set_cursor(4);

        buffer.move_cursor_line_start();
        assert_eq!(buffer.cursor(), 3);

        buffer.move_cursor_line_end();
        assert_eq!(buffer.cursor(), 5);
    }

    #[test]
    fn hit_test_activates_clicked_ltr_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(650.0, 200.0, 1000.0);

        assert_eq!(hit.active_sort, Some(1));
        assert_eq!(hit.cursor, 2);
    }

    #[test]
    fn hit_test_places_ltr_cursor_nearest_boundary() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(20.0, 1200.0, 1000.0);

        assert_eq!(hit.active_sort, None);
        assert_eq!(hit.cursor, 0);
    }

    #[test]
    fn hit_test_uses_rtl_visual_cursor_positions() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(980.0, -1200.0, 1000.0);

        assert_eq!(hit.active_sort, None);
        assert_eq!(hit.cursor, 0);
    }

    #[test]
    fn update_glyph_changes_existing_sort_metadata() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("beh-ar", Some('\u{0628}'), 500.0);

        assert!(buffer.update_glyph(0, "beh-ar.init", Some('\u{0628}'), 480.0));
        let sort = buffer.sort(0).expect("sort exists");
        assert_eq!(sort.glyph_name(), Some("beh-ar.init"));
        let TextSortKind::Glyph { advance_width, .. } = sort.kind else {
            panic!("expected glyph sort");
        };
        assert_eq!(advance_width, 480.0);
    }

    #[test]
    fn shape_arabic_uses_positional_forms_when_available() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        buffer.insert_glyph("beh-ar", Some('\u{0628}'), 500.0);
        buffer.insert_glyph("heh-ar", Some('\u{0647}'), 510.0);

        assert!(buffer.shape_arabic());

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn shape_arabic_resets_to_base_forms_in_ltr() {
        let mut buffer = TextBuffer::new();
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        buffer.insert_glyph("beh-ar.init", Some('\u{0628}'), 480.0);

        assert!(buffer.shape_arabic());

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar")
        );
    }

    #[test]
    fn layout_positions_ltr_lines_and_cursor() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 300.0);

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items.len(), 2);
        assert_eq!(layout.items[0].x, 0.0);
        assert_eq!(layout.items[0].y, 0.0);
        assert_eq!(layout.items[1].x, 0.0);
        assert_eq!(layout.items[1].y, -1000.0);
        assert_eq!(layout.cursor_x, 300.0);
        assert_eq!(layout.cursor_y, -1000.0);
    }

    #[test]
    fn layout_applies_direct_kerning_pairs() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "kerning": {
                        "A": { "V": -80 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items[0].x, 0.0);
        assert_eq!(layout.items[1].x, 420.0);
        assert_eq!(layout.cursor_x, 920.0);
    }

    #[test]
    fn manual_kerning_drag_updates_direct_pair() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "kerning": {
                        "A": { "V": -80 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        assert!(buffer.begin_manual_kerning(1, 500.0));
        assert_eq!(buffer.manual_kerning_sort(), Some(1));
        assert_eq!(buffer.drag_manual_kerning(530.0), Some(-50.0));

        let layout = buffer.layout(1000.0);
        assert_eq!(layout.items[1].x, 450.0);
        assert_eq!(layout.cursor_x, 950.0);
        assert!(buffer.end_manual_kerning());
        assert_eq!(buffer.manual_kerning_sort(), None);
    }

    #[test]
    fn manual_kerning_rejects_first_or_line_break_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(!buffer.begin_manual_kerning(0, 0.0));
        assert!(!buffer.begin_manual_kerning(2, 0.0));
        assert_eq!(buffer.manual_kerning_sort(), None);
    }

    #[test]
    fn structural_text_edits_cancel_manual_kerning() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 500.0));
        assert_eq!(buffer.manual_kerning_sort(), Some(1));
        buffer.set_cursor(1);
        assert!(buffer.delete_after_cursor().is_some());
        assert_eq!(buffer.manual_kerning_sort(), None);

        buffer.insert_glyph("V", Some('V'), 500.0);
        assert!(buffer.begin_manual_kerning(1, 500.0));
        buffer.clear();
        assert_eq!(buffer.manual_kerning_sort(), None);
    }

    #[test]
    fn layout_applies_group_kerning_pairs() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "groups": {
                        "public.kern1.A": ["A"],
                        "public.kern2.V": ["V"]
                    },
                    "kerning": {
                        "public.kern1.A": { "public.kern2.V": -90 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items[1].x, 410.0);
        assert_eq!(layout.cursor_x, 910.0);
    }

    #[test]
    fn layout_positions_rtl_from_line_width() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 300.0);

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items.len(), 2);
        assert_eq!(layout.items[0].x, 300.0);
        assert_eq!(layout.items[1].x, 0.0);
        assert_eq!(layout.cursor_x, 0.0);
        assert_eq!(layout.cursor_y, 0.0);
    }

    #[test]
    fn layout_applies_rtl_kerning_to_line_width_and_positions() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "kerning": {
                        "A": { "V": -80 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items[0].x, 420.0);
        assert_eq!(layout.items[1].x, 0.0);
        assert_eq!(layout.cursor_x, 0.0);
    }
}
