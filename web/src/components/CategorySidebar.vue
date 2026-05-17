<script setup lang="ts">
// Glyph-category filter sidebar. Mirrors runebender-xilem's
// `components/category_panel.rs` — same eight categories, same
// labels, same default selection ("All").
//
// The list is presented as plain left-aligned rows; the active row
// gets a green outline highlight, matching xilem's selected-row rect.

export type Category =
  | "All"
  | "Letter"
  | "Number"
  | "Punctuation"
  | "Symbol"
  | "Mark"
  | "Separator"
  | "Other";

const CATEGORIES: Category[] = [
  "All",
  "Letter",
  "Number",
  "Punctuation",
  "Symbol",
  "Mark",
  "Separator",
  "Other",
];

defineProps<{
  selected: Category;
  /** Number of glyphs in each bucket, keyed by category name. The
   *  "All" key carries the total. */
  counts?: Record<string, number>;
}>();

defineEmits<{
  (e: "select", category: Category): void;
}>();
</script>

<template>
  <aside class="category-sidebar">
    <div class="header">Categories</div>
    <ul class="list">
      <li v-for="cat in CATEGORIES" :key="cat">
        <button
          type="button"
          class="row"
          :class="{ active: cat === selected }"
          @click="$emit('select', cat)"
        >
          <span class="row-name">{{ cat }}</span>
        </button>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
/*
 * Colors / sizes from xilem/src/{theme,components}.rs:
 *   PANEL_BACKGROUND               #1C1C1C
 *   PANEL_OUTLINE / BASE_F         #606060
 *   PRIMARY_UI_TEXT / BASE_I       #909090
 *   SECONDARY_UI_TEXT / BASE_G     #707070
 *   GRID_CELL_SELECTED_OUTLINE     #66EE88
 *   CATEGORY_PANEL_WIDTH           220 px
 *   ROW_HEIGHT                     24 px
 *   TEXT_INSET                     12 px
 */

.category-sidebar {
  width: 220px;
  flex-shrink: 0;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.header {
  padding: 10px 12px 6px;
  color: var(--rb-muted-text, #808080);
  font: 16px ui-sans-serif, system-ui, sans-serif;
  font-weight: 400;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}

.row {
  appearance: none;
  font: inherit;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  width: calc(100% - 8px);
  height: 24px;
  padding: 0 12px;
  margin: 0 4px;
  background: transparent;
  border: 1.5px solid transparent;
  border-radius: 4px;
  color: var(--rb-primary-text, #909090);
  cursor: pointer;
  font: 16px ui-sans-serif, system-ui, sans-serif;
  text-align: left;
}
.row.active {
  background: transparent;
  border-color: var(--rb-accent, #66ee88);
  color: var(--rb-accent, #66ee88);
}
</style>
