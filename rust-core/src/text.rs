//! Text buffer state for the Text tool.
//!
//! This is the wasm-core counterpart to runebender-xilem's `sort`
//! buffer. It intentionally starts small: Vue still owns glyph lookup
//! and preview rendering today, but cursor movement, line breaks, and
//! active sort selection now have a Rust-side home we can migrate to.

use runebender_core::{model::kerning::lookup_kerning as lookup_xilem_kerning, shaping};
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSortActivation {
    pub index: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct TextKerningModel {
    #[serde(default)]
    groups: HashMap<String, Vec<String>>,
    #[serde(default, rename = "leftGroups")]
    left_groups: HashMap<String, String>,
    #[serde(default, rename = "rightGroups")]
    right_groups: HashMap<String, String>,
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

impl TextGlyphInventory {
    fn has_glyph(&self, name: &str) -> bool {
        self.widths.contains_key(name) || self.outlines.contains_key(name)
    }
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
        *glyph_name = name.into();
        *glyph_codepoint = codepoint;
        *glyph_advance_width = advance_width;
        true
    }

    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    pub fn set_direction(&mut self, direction: TextDirection) {
        self.direction = direction;
    }

    pub fn set_kerning_model(&mut self, kerning: TextKerningModel) {
        self.kerning = kerning;
    }

    pub fn kerning_model(&self) -> &TextKerningModel {
        &self.kerning
    }

    pub fn set_glyph_inventory(&mut self, glyph_inventory: TextGlyphInventory) {
        self.glyph_inventory = glyph_inventory;
    }

    pub fn iter(&self) -> impl Iterator<Item = &TextSort> {
        self.sorts.iter()
    }

    pub fn insert_character(&mut self, char: char) -> bool {
        self.insert_character_with_active_advance(char, None)
    }

    pub fn insert_character_with_active_advance(
        &mut self,
        char: char,
        active_advance_width: Option<f64>,
    ) -> bool {
        let Some(glyph_name) = self.glyph_inventory.unicode.get(&(char as u32)).cloned() else {
            return false;
        };
        let use_active_advance =
            self.direction != TextDirection::RightToLeft || !shaping::is_arabic(char);
        let advance_width = active_advance_width
            .filter(|_| use_active_advance)
            .or_else(|| self.glyph_inventory.widths.get(&glyph_name).copied())
            .unwrap_or(500.0);
        let position = self.cursor;
        self.insert_inactive_glyph(glyph_name, Some(char), advance_width);
        self.shape_arabic_around_if_rtl(position);
        true
    }

    pub fn layout(&self, line_height: f64) -> TextLayout {
        let mut items = Vec::new();
        let mut cursor_x = 0.0;
        let mut cursor_y = 0.0;
        let mut line_start = 0;
        let rtl_line_start_x = self.rtl_line_start_x();

        while line_start <= self.sorts.len() {
            let line_end = self.next_line_end(line_start);
            let mut x = match self.direction {
                TextDirection::LeftToRight => 0.0,
                // Match xilem: every RTL line starts from the full buffer's
                // sum of advances, not the current line's width.
                // Kerning is applied as each following sort is positioned,
                // but it does not shift the whole run before layout begins.
                TextDirection::RightToLeft => rtl_line_start_x,
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
                cursor_x = match self.direction {
                    TextDirection::LeftToRight => 0.0,
                    TextDirection::RightToLeft => rtl_line_start_x,
                };
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

    /// Glyph positions for the bottom Text preview strip.
    ///
    /// Xilem renders the bottom preview separately from the editable canvas
    /// text layout. Line breaks only break kerning context there; they do not
    /// create stacked preview lines or reset the strip position.
    pub fn preview_layout(&self) -> Vec<TextLayoutItem> {
        let total_width = match self.direction {
            TextDirection::LeftToRight => 0.0,
            TextDirection::RightToLeft => (0..self.sorts.len())
                .filter(|index| !matches!(self.sorts[*index].kind, TextSortKind::LineBreak))
                .map(|index| self.sort_advance(index))
                .sum(),
        };
        let mut items = Vec::new();
        let mut x = total_width;
        let mut previous_glyph_name: Option<&str> = None;

        for index in 0..self.sorts.len() {
            let advance_width = self.sort_advance(index);
            let Some(glyph_name) = self.sort_glyph_name(index) else {
                previous_glyph_name = None;
                continue;
            };

            match self.direction {
                TextDirection::LeftToRight => {
                    if let Some(kern) = previous_glyph_name
                        .zip(Some(glyph_name))
                        .map(|(left, right)| self.lookup_kerning(left, right))
                    {
                        x += kern;
                    }
                    items.push(TextLayoutItem {
                        index,
                        x,
                        y: 0.0,
                        advance_width,
                    });
                    x += advance_width;
                }
                TextDirection::RightToLeft => {
                    x -= advance_width;
                    if let Some(kern) = previous_glyph_name
                        .zip(Some(glyph_name))
                        .map(|(left, right)| self.lookup_kerning(left, right))
                    {
                        x -= kern;
                    }
                    items.push(TextLayoutItem {
                        index,
                        x,
                        y: 0.0,
                        advance_width,
                    });
                }
            }

            previous_glyph_name = Some(glyph_name);
        }

        items
    }

    pub fn hit_test(
        &self,
        x: f64,
        y: f64,
        line_height: f64,
        ascender: f64,
        descender: f64,
    ) -> TextHit {
        let layout = self.layout(line_height);
        self.hit_test_with_layout(x, y, line_height, ascender, descender, &layout)
    }

    fn hit_test_with_layout(
        &self,
        x: f64,
        y: f64,
        line_height: f64,
        ascender: f64,
        descender: f64,
        layout: &TextLayout,
    ) -> TextHit {
        if self.sorts.is_empty() {
            return TextHit {
                cursor: 0,
                active_sort: None,
            };
        }

        let line_height = line_height.max(1.0);
        let target_line = self.line_number_for_y(y, line_height, ascender, descender);
        let (line_start, line_end) = self.line_range_for_number(target_line);
        let nearest_cursor = self.nearest_cursor_for_line(x, line_start, line_end, layout);

        for item in layout
            .items
            .iter()
            .filter(|item| (line_start..line_end).contains(&item.index))
        {
            // Match xilem's `kurbo::Rect::contains` sort hit-test:
            // min edges inclusive, max edges exclusive.
            let within_x = x >= item.x && x < item.x + item.advance_width;
            let within_y = y >= item.y + descender && y < item.y + ascender;
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
        self.direction = TextDirection::default();
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

    pub fn insert_inactive_glyph(
        &mut self,
        name: impl Into<String>,
        codepoint: Option<char>,
        advance_width: f64,
    ) {
        self.insert_inactive_glyph_at_cursor(name, codepoint, advance_width);
    }

    pub fn insert_line_break(&mut self) {
        self.manual_kerning = None;
        let index = self.cursor;
        self.sorts.insert(self.cursor, TextSort::line_break());
        self.cursor += 1;
        if let Some(active) = self.active_sort
            && active >= index
        {
            self.active_sort = Some(active + 1);
        }
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
        true
    }

    pub fn activate_sort_at(
        &mut self,
        x: f64,
        y: f64,
        line_height: f64,
        ascender: f64,
        descender: f64,
    ) -> Option<TextSortActivation> {
        let layout = self.layout(line_height);
        let hit = self.hit_test_with_layout(x, y, line_height, ascender, descender, &layout);
        let index = hit.active_sort?;
        self.activate_sort(index).then(|| {
            let item = layout
                .items
                .iter()
                .find(|item| item.index == index)
                .expect("hit active sort has a layout item");
            TextSortActivation {
                index,
                x: item.x,
                y: item.y,
            }
        })
    }

    pub fn begin_manual_kerning(&mut self, sort_index: usize, start_x: f64) -> bool {
        if sort_index == 0
            || !matches!(
                self.sorts.get(sort_index).map(|sort| &sort.kind),
                Some(TextSortKind::Glyph { .. })
            )
        {
            return false;
        }
        let original_value = self
            .glyph_pair_names(sort_index)
            .map(|(left, right)| self.lookup_kerning(&left, &right))
            .unwrap_or(0.0);
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
        let current_offset = current_x - session.start_x;
        self.manual_kerning = Some(ManualKerningSession {
            current_offset,
            ..session
        });
        let (left, right) = self.glyph_pair_names(session.sort_index)?;
        let value = session.original_value + current_offset;
        self.set_direct_kerning(&left, &right, value);
        Some(value)
    }

    pub fn end_manual_kerning(&mut self) -> bool {
        self.manual_kerning.take().is_some()
    }

    pub fn shape_arabic(&mut self) -> bool {
        let chars = self.glyph_chars();
        let mut updates = Vec::new();

        for index in 0..self.sorts.len() {
            let Some(char) = self.sort_codepoint(index) else {
                continue;
            };
            let char_index = self.char_index_for_sort_index(index);
            let name = self.shaped_glyph_name_for_character(char, &chars, char_index, index);
            let advance_width = self
                .glyph_inventory
                .widths
                .get(&name)
                .copied()
                .unwrap_or_else(|| self.sort_advance(index));
            updates.push((index, name, advance_width));
        }

        self.apply_shape_updates(updates)
    }

    pub fn shape_arabic_if_rtl(&mut self) -> bool {
        if self.direction != TextDirection::RightToLeft {
            return false;
        }
        self.shape_arabic()
    }

    pub fn shape_arabic_around_if_rtl(&mut self, position: usize) -> bool {
        if self.direction != TextDirection::RightToLeft {
            return false;
        }
        self.shape_arabic_around(position)
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

    fn insert_inactive_glyph_at_cursor(
        &mut self,
        name: impl Into<String>,
        codepoint: Option<char>,
        advance_width: f64,
    ) {
        self.manual_kerning = None;
        let index = self.cursor;
        self.sorts
            .insert(index, TextSort::glyph(name, codepoint, advance_width));
        self.cursor += 1;
        if let Some(active) = self.active_sort
            && active >= index
        {
            self.active_sort = Some(active + 1);
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

    fn shape_arabic_around(&mut self, position: usize) -> bool {
        if self.sorts.is_empty() {
            return false;
        }

        let indices = self.arabic_shape_indices_around(position);
        if indices.is_empty() {
            return false;
        }

        let chars = self.glyph_chars();
        let mut updates = Vec::new();

        for index in indices {
            let Some(char) = self.sort_codepoint(index) else {
                continue;
            };
            if !shaping::is_arabic(char) {
                continue;
            }
            let char_index = self.char_index_for_sort_index(index);
            let name = self.shaped_glyph_name_for_character(char, &chars, char_index, index);
            let advance_width = self
                .glyph_inventory
                .widths
                .get(&name)
                .copied()
                .unwrap_or_else(|| self.sort_advance(index));
            updates.push((index, name, advance_width));
        }

        self.apply_shape_updates(updates)
    }

    fn arabic_shape_indices_around(&self, position: usize) -> Vec<usize> {
        let mut indices = Vec::new();

        if let Some(index) = self.previous_nontransparent_arabic_sort(position) {
            indices.push(index);
        }

        if let Some(index) = self.next_nontransparent_arabic_sort(position) {
            indices.push(index);
            if let Some(next_index) = self.next_nontransparent_arabic_sort(index + 1) {
                indices.push(next_index);
            }
        }

        indices.dedup();
        indices
    }

    fn previous_nontransparent_arabic_sort(&self, position: usize) -> Option<usize> {
        let end = position.min(self.sorts.len());
        (0..end)
            .rev()
            .find(|index| self.is_nontransparent_arabic_sort(*index))
    }

    fn next_nontransparent_arabic_sort(&self, position: usize) -> Option<usize> {
        (position..self.sorts.len()).find(|index| self.is_nontransparent_arabic_sort(*index))
    }

    fn is_nontransparent_arabic_sort(&self, index: usize) -> bool {
        self.sort_codepoint(index).is_some_and(|char| {
            shaping::is_arabic(char) && !shaping::arabic_joining_type(char).is_transparent()
        })
    }

    fn glyph_chars(&self) -> Vec<char> {
        self.sorts
            .iter()
            .filter_map(|sort| match sort.kind {
                TextSortKind::Glyph {
                    codepoint: Some(char),
                    ..
                } => Some(char),
                _ => None,
            })
            .collect()
    }

    fn char_index_for_sort_index(&self, sort_index: usize) -> usize {
        self.sorts[..sort_index]
            .iter()
            .filter(|sort| {
                matches!(
                    sort.kind,
                    TextSortKind::Glyph {
                        codepoint: Some(_),
                        ..
                    }
                )
            })
            .count()
    }

    fn apply_shape_updates(&mut self, updates: Vec<(usize, String, f64)>) -> bool {
        let mut changed = false;
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

        changed
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
        if !suffix.is_empty() && self.glyph_inventory.has_glyph(&shaped_name) {
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

    fn line_number_for_y(&self, y: f64, line_height: f64, ascender: f64, descender: f64) -> usize {
        let mut start = 0;
        let mut line_number = 0;
        let mut nearest_line = 0;
        let mut nearest_distance = f64::INFINITY;
        while start <= self.sorts.len() {
            let baseline = -line_height * line_number as f64;
            let top = baseline + ascender;
            let bottom = baseline + descender;
            if y >= bottom && y <= top {
                return line_number;
            }
            let distance = if y > top { y - top } else { bottom - y };
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_line = line_number;
            }

            let end = self.next_line_end(start);
            if end >= self.sorts.len() {
                break;
            }
            start = end + 1;
            line_number += 1;
        }
        nearest_line
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
        let line_start_x = match self.direction {
            TextDirection::LeftToRight => self.line_width(line_start, line_end),
            TextDirection::RightToLeft => self.rtl_line_start_x(),
        };

        for candidate in line_start..=line_end {
            let cursor_x = if candidate == line_start {
                match self.direction {
                    TextDirection::LeftToRight => 0.0,
                    TextDirection::RightToLeft => line_start_x,
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

    fn rtl_line_start_x(&self) -> f64 {
        (0..self.sorts.len())
            .filter(|index| !matches!(self.sorts[*index].kind, TextSortKind::LineBreak))
            .map(|index| self.sort_advance(index))
            .sum()
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

    fn glyph_pair_names(&self, sort_index: usize) -> Option<(String, String)> {
        let left = self.sort_glyph_name(sort_index.checked_sub(1)?)?;
        let right = self.sort_glyph_name(sort_index)?;
        Some((left.to_string(), right.to_string()))
    }

    fn lookup_kerning(&self, left: &str, right: &str) -> f64 {
        lookup_xilem_kerning(
            &self.kerning.kerning,
            &self.kerning.groups,
            left,
            self.kerning.right_groups.get(left).map(String::as_str),
            right,
            self.kerning.left_groups.get(right).map(String::as_str),
        )
    }

    fn set_direct_kerning(&mut self, left: &str, right: &str, value: f64) {
        if value == 0.0 {
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
    fn insert_inactive_glyph_preserves_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.set_cursor(0);
        buffer.insert_inactive_glyph("B", Some('B'), 610.0);

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer.active_sort(), Some(1));
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("B"));
        assert_eq!(buffer.sort(1).and_then(TextSort::glyph_name), Some("A"));
    }

    #[test]
    fn activate_sort_preserves_cursor_position() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);
        buffer.set_cursor(0);

        assert!(buffer.activate_sort(1));

        assert_eq!(buffer.active_sort(), Some(1));
        assert_eq!(buffer.cursor(), 0);
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
        assert_eq!(buffer.active_sort(), None);
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
    fn insert_character_missing_width_uses_xilem_shaper_fallback() {
        let mut buffer = TextBuffer::new();
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A" },
                    "outlines": { "A": "M0,0 L10,0" }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('A'));

        let TextSortKind::Glyph { advance_width, .. } = &buffer.sort(0).expect("sort exists").kind
        else {
            panic!("expected glyph sort");
        };
        assert_eq!(*advance_width, 500.0);
    }

    #[test]
    fn clear_resets_direction_like_fresh_xilem_session() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 600.0);

        buffer.clear();

        assert_eq!(buffer.direction(), TextDirection::LeftToRight);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.cursor(), 0);
        assert_eq!(buffer.active_sort(), None);
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
    fn rtl_arabic_shaping_context_crosses_line_breaks_like_xilem() {
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
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        buffer.insert_line_break();
        assert!(buffer.insert_character('\u{0647}'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar")
        );
        assert!(matches!(
            buffer.sort(1).map(|sort| &sort.kind),
            Some(TextSortKind::LineBreak)
        ));
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn rtl_arabic_insert_after_transparent_mark_reshapes_previous_letter() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1614": "fatha-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "fatha-ar": 0,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{064e}'));
        assert!(buffer.insert_character('\u{0647}'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("fatha-ar")
        );
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn rtl_arabic_tatweel_joins_neighbors_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1600": "tatweel-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "tatweel-ar": 250,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0640}'));
        assert!(buffer.insert_character('\u{0647}'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("tatweel-ar")
        );
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
    }

    #[test]
    fn rtl_arabic_positional_glyph_can_exist_without_width_like_xilem() {
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
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    },
                    "outlines": {
                        "beh-ar.init": "M0,0 L10,0"
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0647}'));

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
    fn rtl_arabic_delete_transparent_mark_repairs_joining_neighbors() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1614": "fatha-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "fatha-ar": 0,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        buffer.insert_glyph("beh-ar", Some('\u{0628}'), 500.0);
        buffer.insert_glyph("fatha-ar", Some('\u{064e}'), 0.0);
        buffer.insert_glyph("heh-ar", Some('\u{0647}'), 510.0);
        buffer.set_cursor(2);

        assert!(buffer.delete_before_cursor().is_some());
        assert!(buffer.shape_arabic_around_if_rtl(buffer.cursor()));

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
    fn rtl_arabic_insert_right_joining_sort_reshapes_next_letter() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "1576": "beh-ar",
                        "1575": "alef-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "alef-ar": 450,
                        "alef-ar.fina": 430,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0647}'));
        buffer.set_cursor(1);
        assert!(buffer.insert_character('\u{0627}'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("alef-ar.fina")
        );
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("heh-ar")
        );
    }

    #[test]
    fn rtl_arabic_insert_latin_separator_breaks_joining_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "65": "A",
                        "1576": "beh-ar",
                        "1605": "meem-ar"
                    },
                    "widths": {
                        "A": 700,
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

        buffer.set_cursor(1);
        assert!(buffer.insert_character('A'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar")
        );
        assert_eq!(buffer.sort(1).and_then(TextSort::glyph_name), Some("A"));
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("meem-ar")
        );
    }

    #[test]
    fn rtl_arabic_delete_latin_separator_repairs_joining_neighbors() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "65": "A",
                        "1576": "beh-ar",
                        "1605": "meem-ar"
                    },
                    "widths": {
                        "A": 700,
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
        assert!(buffer.insert_character('A'));
        assert!(buffer.insert_character('\u{0645}'));
        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar")
        );
        assert_eq!(
            buffer.sort(2).and_then(TextSort::glyph_name),
            Some("meem-ar")
        );

        buffer.set_cursor(2);
        assert!(buffer.delete_before_cursor().is_some());
        assert!(buffer.shape_arabic_around_if_rtl(buffer.cursor()));

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
    fn insert_character_ltr_preserves_existing_shaped_forms() {
        let mut buffer = TextBuffer::new();
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": {
                        "65": "A",
                        "1576": "beh-ar",
                        "1607": "heh-ar"
                    },
                    "widths": {
                        "A": 700,
                        "beh-ar": 500,
                        "beh-ar.init": 480,
                        "heh-ar": 510,
                        "heh-ar.fina": 490
                    }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        buffer.set_direction(TextDirection::RightToLeft);
        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0647}'));
        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );

        buffer.set_direction(TextDirection::LeftToRight);
        assert!(buffer.insert_character('A'));

        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );
        assert_eq!(buffer.sort(2).and_then(TextSort::glyph_name), Some("A"));
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
    fn delete_before_cursor_clears_deleted_active_sort_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);
        buffer.insert_glyph("C", Some('C'), 620.0);
        buffer.activate_sort(1);
        buffer.set_cursor(2);

        let deleted = buffer.delete_before_cursor();

        assert_eq!(deleted.as_ref().and_then(TextSort::glyph_name), Some("B"));
        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer.active_sort(), None);
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("A"));
        assert_eq!(buffer.sort(1).and_then(TextSort::glyph_name), Some("C"));
        assert!(!buffer.iter().any(|sort| sort.active));
    }

    #[test]
    fn delete_after_cursor_clears_deleted_active_sort_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);
        buffer.insert_glyph("C", Some('C'), 620.0);
        buffer.activate_sort(1);
        buffer.set_cursor(1);

        let deleted = buffer.delete_after_cursor();

        assert_eq!(deleted.as_ref().and_then(TextSort::glyph_name), Some("B"));
        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer.active_sort(), None);
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("A"));
        assert_eq!(buffer.sort(1).and_then(TextSort::glyph_name), Some("C"));
        assert!(!buffer.iter().any(|sort| sort.active));
    }

    #[test]
    fn line_break_preserves_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_line_break();

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.cursor(), 2);
        assert_eq!(buffer.active_sort(), Some(0));
    }

    #[test]
    fn line_break_before_active_shifts_active_sort_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 600.0);
        buffer.insert_glyph("B", Some('B'), 610.0);
        buffer.activate_sort(1);
        buffer.set_cursor(1);

        buffer.insert_line_break();

        assert_eq!(buffer.cursor(), 2);
        assert_eq!(buffer.active_sort(), Some(2));
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("A"));
        assert!(matches!(
            buffer.sort(1).map(|sort| &sort.kind),
            Some(TextSortKind::LineBreak)
        ));
        assert_eq!(buffer.sort(2).and_then(TextSort::glyph_name), Some("B"));
        assert!(buffer.sort(2).is_some_and(|sort| sort.active));
    }

    #[test]
    fn typed_sort_before_active_shifts_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A", "66": "B" },
                    "widths": { "A": 640, "B": 650 }
                }"#,
            )
            .expect("valid glyph inventory"),
        );
        buffer.insert_glyph("B", Some('B'), 650.0);
        buffer.set_cursor(0);

        assert!(buffer.insert_character('A'));

        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer.active_sort(), Some(1));
        assert_eq!(buffer.sort(0).and_then(TextSort::glyph_name), Some("A"));
        assert_eq!(buffer.sort(1).and_then(TextSort::glyph_name), Some("B"));
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
    fn hit_test_activates_clicked_ltr_sort() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(650.0, 200.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, Some(1));
        assert_eq!(hit.cursor, 2);
    }

    #[test]
    fn hit_test_rejects_sort_above_ascender() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(650.0, 900.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, None);
        assert_eq!(hit.cursor, 1);
    }

    #[test]
    fn hit_test_places_ltr_cursor_nearest_boundary() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(20.0, 1200.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, None);
        assert_eq!(hit.cursor, 0);
    }

    #[test]
    fn hit_test_uses_xilem_exclusive_sort_max_edges() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 300.0);

        let boundary = buffer.hit_test(500.0, 100.0, 1000.0, 800.0, -200.0);
        assert_eq!(boundary.active_sort, Some(1));
        assert_eq!(boundary.cursor, 2);

        let top_edge = buffer.hit_test(250.0, 800.0, 1000.0, 800.0, -200.0);
        assert_eq!(top_edge.active_sort, None);
        assert_eq!(top_edge.cursor, 0);
    }

    #[test]
    fn hit_test_uses_metric_box_for_ltr_line_selection() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(250.0, -300.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, Some(2));
        assert_eq!(hit.cursor, 3);
    }

    #[test]
    fn hit_test_uses_rtl_visual_cursor_positions() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(980.0, -1200.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, None);
        assert_eq!(hit.cursor, 0);
    }

    #[test]
    fn hit_test_uses_metric_box_for_rtl_line_selection() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 500.0);

        let hit = buffer.hit_test(750.0, -300.0, 1000.0, 800.0, -200.0);

        assert_eq!(hit.active_sort, Some(2));
        assert_eq!(hit.cursor, 3);
    }

    #[test]
    fn activate_sort_at_returns_layout_origin_for_active_sort() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 300.0);

        let activation = buffer
            .activate_sort_at(750.0, -300.0, 1000.0, 800.0, -200.0)
            .expect("sort activates");

        assert_eq!(activation.index, 2);
        assert_eq!(activation.x, 500.0);
        assert_eq!(activation.y, -1000.0);
        assert_eq!(buffer.active_sort(), Some(2));
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
    fn set_direction_does_not_reshape_existing_sorts_like_xilem() {
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

        assert!(buffer.insert_character('\u{0628}'));
        assert!(buffer.insert_character('\u{0647}'));
        assert_eq!(
            buffer.sort(0).and_then(TextSort::glyph_name),
            Some("beh-ar.init")
        );
        assert_eq!(
            buffer.sort(1).and_then(TextSort::glyph_name),
            Some("heh-ar.fina")
        );

        buffer.set_direction(TextDirection::LeftToRight);

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
    fn set_direction_only_changes_direction_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 500.0));
        buffer.set_direction(TextDirection::RightToLeft);

        assert_eq!(buffer.direction(), TextDirection::RightToLeft);
        assert_eq!(buffer.manual_kerning_sort(), Some(1));
    }

    #[test]
    fn set_kerning_model_keeps_manual_kerning_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 500.0));
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

        assert_eq!(buffer.manual_kerning_sort(), Some(1));
    }

    #[test]
    fn set_glyph_inventory_keeps_manual_kerning_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 500.0));
        buffer.set_glyph_inventory(
            serde_json::from_str(
                r#"{
                    "unicode": { "65": "A", "86": "V" },
                    "widths": { "A": 500, "V": 500 },
                    "outlines": {}
                }"#,
            )
            .expect("valid glyph inventory"),
        );

        assert_eq!(buffer.manual_kerning_sort(), Some(1));
    }

    #[test]
    fn update_glyph_keeps_manual_kerning_like_xilem_width_edit() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 500.0));
        assert!(buffer.update_glyph(1, "V", Some('V'), 520.0));

        assert_eq!(buffer.manual_kerning_sort(), Some(1));
        let TextSortKind::Glyph { advance_width, .. } = &buffer.sort(1).expect("sort exists").kind
        else {
            panic!("expected glyph sort");
        };
        assert_eq!(*advance_width, 520.0);
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
    fn layout_places_cursor_on_empty_line_after_trailing_line_break_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 300.0);
        buffer.insert_line_break();

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items.len(), 1);
        assert_eq!(layout.cursor_x, 0.0);
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
    fn manual_kerning_drag_preserves_tiny_nonzero_pair_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(buffer.begin_manual_kerning(1, 0.0));
        assert_eq!(buffer.drag_manual_kerning(f64::EPSILON), Some(f64::EPSILON));
        assert_eq!(
            buffer
                .kerning_model()
                .kerning
                .get("A")
                .and_then(|pairs| pairs.get("V"))
                .copied(),
            Some(f64::EPSILON)
        );
    }

    #[test]
    fn manual_kerning_enters_noop_session_after_line_break_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("V", Some('V'), 500.0);

        assert!(!buffer.begin_manual_kerning(0, 0.0));
        assert!(buffer.begin_manual_kerning(2, 0.0));
        assert_eq!(buffer.manual_kerning_sort(), Some(2));
        assert_eq!(buffer.active_sort(), Some(2));
        assert_eq!(buffer.drag_manual_kerning(30.0), None);
        assert!(buffer.end_manual_kerning());
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
    fn layout_applies_raw_xilem_group_names_without_public_prefix() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "groups": {
                        "leftRaw": ["A"],
                        "rightRaw": ["V"]
                    },
                    "kerning": {
                        "leftRaw": { "rightRaw": -80 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items[1].x, 420.0);
        assert_eq!(layout.cursor_x, 920.0);
    }

    #[test]
    fn layout_prioritizes_xilem_glyph_group_hints_before_other_memberships() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.set_kerning_model(
            serde_json::from_str(
                r#"{
                    "groups": {
                        "firstLeft": ["A"],
                        "hintLeft": ["A"],
                        "firstRight": ["V"],
                        "hintRight": ["V"]
                    },
                    "leftGroups": { "V": "hintRight" },
                    "rightGroups": { "A": "hintLeft" },
                    "kerning": {
                        "firstLeft": { "firstRight": -20 },
                        "hintLeft": { "hintRight": -70 }
                    }
                }"#,
            )
            .expect("valid kerning model"),
        );

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items[1].x, 430.0);
        assert_eq!(layout.cursor_x, 930.0);
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
    fn activate_sort_at_uses_rtl_kerned_layout_origin_like_xilem() {
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

        let activation = buffer
            .activate_sort_at(100.0, 0.0, 1000.0, 800.0, -200.0)
            .expect("kerned RTL sort activates");

        assert_eq!(activation.index, 1);
        assert_eq!(activation.x, 80.0);
        assert_eq!(activation.y, 0.0);
        assert_eq!(buffer.active_sort(), Some(1));
    }

    #[test]
    fn rtl_layout_places_cursor_on_empty_line_after_trailing_line_break_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 300.0);
        buffer.insert_line_break();

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items.len(), 1);
        assert_eq!(layout.cursor_x, 300.0);
        assert_eq!(layout.cursor_y, -1000.0);
    }

    #[test]
    fn layout_positions_rtl_lines_from_buffer_width() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("B", Some('B'), 300.0);

        let layout = buffer.layout(1000.0);

        assert_eq!(layout.items.len(), 2);
        assert_eq!(layout.items[0].x, 300.0);
        assert_eq!(layout.items[0].y, 0.0);
        assert_eq!(layout.items[1].x, 500.0);
        assert_eq!(layout.items[1].y, -1000.0);
        assert_eq!(layout.cursor_x, 500.0);
        assert_eq!(layout.cursor_y, -1000.0);
    }

    #[test]
    fn layout_applies_rtl_kerning_without_shifting_line_start() {
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

        assert_eq!(layout.items[0].x, 500.0);
        assert_eq!(layout.items[1].x, 80.0);
        assert_eq!(layout.cursor_x, 80.0);
    }

    #[test]
    fn rtl_multiline_layout_resets_kerning_but_keeps_raw_buffer_width_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_glyph("V", Some('V'), 500.0);
        buffer.insert_line_break();
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

        assert_eq!(layout.items.len(), 3);
        assert_eq!(layout.items[0].x, 1000.0);
        assert_eq!(layout.items[0].y, 0.0);
        assert_eq!(layout.items[1].x, 580.0);
        assert_eq!(layout.items[1].y, 0.0);
        assert_eq!(layout.items[2].x, 1000.0);
        assert_eq!(layout.items[2].y, -1000.0);
        assert_eq!(layout.cursor_x, 1000.0);
        assert_eq!(layout.cursor_y, -1000.0);
    }

    #[test]
    fn preview_layout_keeps_line_breaks_in_one_strip() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("V", Some('V'), 300.0);

        let preview = buffer.preview_layout();

        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].x, 0.0);
        assert_eq!(preview[0].y, 0.0);
        assert_eq!(preview[1].x, 500.0);
        assert_eq!(preview[1].y, 0.0);

        let canvas = buffer.layout(1000.0);
        assert_eq!(canvas.items[1].x, 0.0);
        assert_eq!(canvas.items[1].y, -1000.0);
    }

    #[test]
    fn preview_layout_breaks_kerning_across_line_breaks() {
        let mut buffer = TextBuffer::new();
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
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

        let preview = buffer.preview_layout();

        assert_eq!(preview[1].x, 500.0);
    }

    #[test]
    fn rtl_preview_layout_keeps_line_breaks_in_one_strip_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
        buffer.insert_glyph("V", Some('V'), 300.0);

        let preview = buffer.preview_layout();

        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].x, 300.0);
        assert_eq!(preview[0].y, 0.0);
        assert_eq!(preview[1].x, 0.0);
        assert_eq!(preview[1].y, 0.0);

        let canvas = buffer.layout(1000.0);
        assert_eq!(canvas.items[0].x, 300.0);
        assert_eq!(canvas.items[0].y, 0.0);
        assert_eq!(canvas.items[1].x, 500.0);
        assert_eq!(canvas.items[1].y, -1000.0);
    }

    #[test]
    fn rtl_preview_layout_breaks_kerning_across_line_breaks_like_xilem() {
        let mut buffer = TextBuffer::new();
        buffer.set_direction(TextDirection::RightToLeft);
        buffer.insert_glyph("A", Some('A'), 500.0);
        buffer.insert_line_break();
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

        let preview = buffer.preview_layout();

        assert_eq!(preview[0].x, 500.0);
        assert_eq!(preview[1].x, 0.0);
    }
}
