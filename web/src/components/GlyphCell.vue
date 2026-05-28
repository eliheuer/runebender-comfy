<script setup lang="ts">
// One tile in the glyph grid. Mirrors runebender-xilem's
// `views/glyph_grid/glyph_cell.rs` layout: big glyph area on top,
// glyph name + Unicode codepoint stacked underneath.
//
// Colors come from xilem's theme.rs:
//   GRID_CELL_BACKGROUND          #1C1C1C
//   GRID_CELL_OUTLINE / BASE_F    #606060
//   GRID_CELL_SELECTED_OUTLINE    #FFFFFF
//   GRID_CELL_TEXT / BASE_H       #808080

import { computed } from "vue";
import { rgbaToCss } from "./markColors";

const props = defineProps<{
  name: string;
  /** Uppercase hex, no "U+" prefix. Empty when the glyph has no codepoint. */
  unicode?: string;
  /** Inline SVG markup (output of `glifToSvg`). */
  svg?: string;
  /** Highlights this cell as the currently-selected glyph. */
  selected?: boolean;
  /** Number of xilem grid columns this cell should span. */
  columnSpan?: number;
  /** UFO `public.markColor` "r,g,b,a" with 0–1 floats. */
  markColor?: string;
}>();

defineEmits<{
  (e: "click", event: MouseEvent): void;
  (e: "dblclick"): void;
}>();

const cellStyle = computed(() => {
  const style: Record<string, string> = {
    gridColumn: `span ${Math.max(1, props.columnSpan ?? 1)}`,
  };
  if (!props.markColor) return style;
  const parts = props.markColor.split(",").map(Number);
  if (parts.length !== 4 || parts.some((n) => !Number.isFinite(n))) return style;
  style["--mark-color"] = rgbaToCss(props.markColor);
  return style;
});
</script>

<template>
  <button
    type="button"
    class="cell"
    :class="{ selected, marked: !!markColor }"
    :style="cellStyle"
    :title="name"
    @click="$emit('click', $event)"
    @dblclick="$emit('dblclick')"
  >
    <div class="cell-glyph" v-html="svg ?? ''" />
    <div class="cell-labels">
      <div class="cell-name">{{ name }}</div>
      <div class="cell-unicode">{{ unicode ? `U+${unicode}` : "" }}</div>
    </div>
  </button>
</template>

<style scoped>
.cell {
  appearance: none;
  font: inherit;
  text-align: left;
  margin: 0;

  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  cursor: pointer;
  overflow: hidden;
  scroll-snap-align: start;
  transition:
    background-color 0.08s,
    border-color 0.08s;
}
.cell:hover {
  border-color: var(--rb-accent, #66ee88);
}
.cell:focus {
  outline: none;
}
.cell:focus-visible {
  border-color: var(--rb-grid-selected, #ffffff);
}
.cell.marked:not(.selected):not(:hover) {
  border-color: var(--mark-color);
}
.cell.selected {
  border-color: var(--rb-grid-selected, #ffffff);
}

.cell-glyph {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--rb-muted-text, #808080);
  padding: 8px;
  min-height: 0;
}
.cell.marked:not(.selected):not(:hover) .cell-glyph {
  color: var(--mark-color);
}
.cell.selected .cell-glyph {
  color: var(--rb-grid-selected, #ffffff);
}
/* The grid SVG carries a constant em-based viewBox (same vertical
   extent for every glyph) and a per-glyph horizontal extent. Scaling
   to a fixed HEIGHT therefore renders every glyph at the same scale —
   a period stays a small dot, an M stays tall — and lets the width
   vary so each glyph is centered on its own advance. max-width guards
   against unusually wide glyphs overflowing the cell. */
.cell-glyph :deep(svg) {
  height: 100%;
  width: auto;
  max-width: 100%;
  display: block;
}

.cell-labels {
  min-height: 56px;
  box-sizing: border-box;
  padding: 5px 8px 8px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 2px;
}
.cell-name {
  font: 16px ui-sans-serif, system-ui, sans-serif;
  color: var(--rb-muted-text, #808080);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cell.selected .cell-name,
.cell:hover .cell-name {
  color: var(--rb-accent, #66ee88);
}
.cell.selected .cell-name {
  color: var(--rb-grid-selected, #ffffff);
}
.cell-unicode {
  font: 16px ui-sans-serif, system-ui, sans-serif;
  color: var(--rb-muted-text, #808080);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cell.marked:not(.selected):not(:hover) .cell-name,
.cell.marked:not(.selected):not(:hover) .cell-unicode {
  color: var(--mark-color);
}
.cell.selected .cell-unicode,
.cell:hover .cell-unicode {
  color: var(--rb-accent, #66ee88);
}
.cell.selected .cell-unicode {
  color: var(--rb-grid-selected, #ffffff);
}
</style>
