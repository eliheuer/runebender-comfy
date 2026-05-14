<script setup lang="ts">
// Bottom of the right sidebar. Mirrors xilem's
// `components/glyph_anatomy_panel` — an "x-ray" preview of the
// selected glyph at a larger size than the grid cell.
//
// xilem's version renders outline + control points + handle lines.
// For now we just render the BezPath silhouette (the same inline
// SVG the cells use) larger. Point + handle overlay matching
// xilem's `paint` is a future polish pass.

defineProps<{
  /** Inline SVG for the selected glyph. Empty when no glyph is
   *  selected. */
  svg?: string;
  /** Glyph name for the empty-state label. */
  name?: string;
}>();
</script>

<template>
  <div class="anatomy">
    <div class="header">Anatomy</div>
    <div v-if="svg" class="canvas" v-html="svg" />
    <div v-else class="empty">
      {{ name ? "(no contours)" : "Select a glyph" }}
    </div>
  </div>
</template>

<style scoped>
.anatomy {
  background: #1c1c1c;
  border: 1.5px solid #606060;
  border-radius: 6px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  flex: 1;
}

.header {
  color: #66ee88;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-weight: 500;
  flex-shrink: 0;
}

.canvas {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #a0a0a0;
  padding: 8px;
}
.canvas :deep(svg) {
  max-width: 100%;
  max-height: 100%;
  display: block;
}

.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #606060;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-style: italic;
}
</style>
