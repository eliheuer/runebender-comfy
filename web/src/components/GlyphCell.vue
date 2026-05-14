<script setup lang="ts">
// One tile in the glyph grid. Mirrors runebender-xilem's
// `views/glyph_grid/glyph_cell.rs` layout: big glyph area on top,
// glyph name + Unicode codepoint stacked underneath.
//
// Colors come from xilem's theme.rs:
//   GRID_CELL_BACKGROUND          #1C1C1C
//   GRID_CELL_OUTLINE / BASE_F    #606060
//   GRID_CELL_SELECTED_OUTLINE    #66EE88
//   GRID_GLYPH_COLOR / BASE_J     #A0A0A0
//   GRID_CELL_TEXT / BASE_H       #808080

import { computed } from "vue";

const props = defineProps<{
  name: string;
  /** Uppercase hex, no "U+" prefix. Empty when the glyph has no codepoint. */
  unicode?: string;
  /** Inline SVG markup (output of `glifToSvg`). */
  svg?: string;
  /** Highlights this cell as the currently-selected glyph. */
  selected?: boolean;
  /** UFO `public.markColor` "r,g,b,a" with 0–1 floats. When set,
   *  the cell background tints to this color. Selection outline
   *  overlays on top so the mark stays visible. */
  markColor?: string;
}>();

defineEmits<{
  (e: "click"): void;
  (e: "dblclick"): void;
}>();

const cellStyle = computed(() => {
  if (!props.markColor) return undefined;
  const parts = props.markColor.split(",").map(Number);
  if (parts.length !== 4 || parts.some((n) => !Number.isFinite(n))) return undefined;
  const [r, g, b] = parts;
  // Tint at ~35% — enough to see the color without drowning the glyph.
  const rgba = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, 0.35)`;
  return { background: rgba };
});
</script>

<template>
  <button
    type="button"
    class="cell"
    :class="{ selected }"
    :style="cellStyle"
    :title="name"
    @click="$emit('click')"
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
  height: 170px;
  background: #1c1c1c;
  border: 1px solid #606060;
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
  transition:
    background-color 0.08s,
    border-color 0.08s;
}
.cell:hover {
  border-color: #66ee88;
}
.cell.selected {
  border-color: #66ee88;
  /* Mark color (from inline style) stays visible underneath. */
}

.cell-glyph {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #a0a0a0;
  padding: 12px;
  min-height: 0;
}
.cell-glyph :deep(svg) {
  max-width: 100%;
  max-height: 100%;
  display: block;
}

.cell-labels {
  padding: 4px 6px 6px;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.cell-name {
  font: 11px ui-sans-serif, system-ui, sans-serif;
  color: #909090;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cell.selected .cell-name {
  color: #66ee88;
}
.cell-unicode {
  font: 10px ui-monospace, monospace;
  color: #707070;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cell.selected .cell-unicode {
  color: #66ee88;
}
</style>
