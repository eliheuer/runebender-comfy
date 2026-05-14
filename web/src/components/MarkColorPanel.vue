<script setup lang="ts">
// Bottom-left mark-color picker. Mirrors runebender-xilem's
// `components/mark_color_panel.rs` — seven preset swatches in the
// same order as xilem's `theme::mark::COLORS`, plus a clear (X)
// button. The shared palette + RGBA-to-CSS conversion live in
// `markColors.ts` (Vue's <script setup> can't have `export`).
//
// Clicking a swatch tags the *selected* glyph (not the one in the
// editor — the cell that's highlighted in the grid). The active
// swatch is the one that matches the selected glyph's current
// mark color.

import { MARK_COLORS, rgbaToCss } from "./markColors";

defineProps<{
  /** Currently-applied mark color on the selected glyph, or empty
   *  when no glyph is selected / glyph is unmarked. */
  active?: string;
  /** False when no glyph is selected — swatches are visible but
   *  inert (a click would have nothing to apply to). */
  enabled?: boolean;
}>();

defineEmits<{
  /** Empty string means "clear the mark." */
  (e: "set", rgba: string): void;
}>();
</script>

<template>
  <div class="mark-color-panel" :class="{ disabled: !enabled }">
    <div class="header">Colors</div>
    <div class="swatches">
      <button
        v-for="c in MARK_COLORS"
        :key="c.rgba"
        type="button"
        class="swatch"
        :class="{ active: c.rgba === active }"
        :style="{ background: rgbaToCss(c.rgba) }"
        :title="c.name"
        :aria-label="`Set mark color: ${c.name}`"
        :disabled="!enabled"
        @click="$emit('set', c.rgba)"
      />
      <button
        type="button"
        class="swatch clear"
        :class="{ active: !active && enabled }"
        title="Clear mark color"
        aria-label="Clear mark color"
        :disabled="!enabled"
        @click="$emit('set', '')"
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <line x1="4" y1="4" x2="12" y2="12" />
          <line x1="12" y1="4" x2="4" y2="12" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
/*
 * Colors from xilem theme.rs:
 *   PANEL_BACKGROUND               #1C1C1C
 *   PANEL_OUTLINE / BASE_F         #606060
 *   GRID_CELL_SELECTED_OUTLINE     #66EE88 (active swatch ring)
 *   SECONDARY_UI_TEXT / BASE_G     #707070
 */

.mark-color-panel {
  background: #1c1c1c;
  border: 1.5px solid #606060;
  border-radius: 6px;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}
.mark-color-panel.disabled {
  opacity: 0.5;
}

.header {
  color: #66ee88;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-weight: 500;
}

.swatches {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.swatch {
  appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1.5px solid transparent;
  cursor: pointer;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.swatch:hover:not(:disabled) {
  /* Subtle ring on hover. Active swatch already has a ring. */
  outline: 2px solid #606060;
  outline-offset: 1px;
}
.swatch.active {
  outline: 2px solid #66ee88;
  outline-offset: 1px;
}
.swatch:disabled {
  cursor: default;
}

.swatch.clear {
  background: #1c1c1c;
  border-color: #606060;
}
.swatch.clear svg {
  width: 12px;
  height: 12px;
  stroke: #707070;
  stroke-width: 1.5;
}
.swatch.clear:hover:not(:disabled) svg {
  stroke: #66ee88;
}
</style>
