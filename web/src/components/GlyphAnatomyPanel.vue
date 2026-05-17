<script setup lang="ts">
// Bottom of the right sidebar. Mirrors xilem's
// `components/glyph_anatomy_panel` — an untitled "x-ray" preview of
// the selected glyph at a larger size than the grid cell.
//
// xilem's version renders outline + control points + handle lines.
// This panel now uses a dedicated anatomy SVG so it can show the
// x-ray overlay without duplicating editor state in Vue.

defineProps<{
  /** Inline SVG for the selected glyph. Empty when no glyph is
   *  selected. */
  svg?: string;
  /** Glyph name retained for parity-friendly accessibility labels. */
  name?: string;
}>();
</script>

<template>
  <div class="anatomy" :aria-label="name ? `${name} anatomy` : 'Glyph anatomy'">
    <div v-if="svg" class="canvas" v-html="svg" />
  </div>
</template>

<style scoped>
.anatomy {
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.canvas {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--rb-glyph-preview, #a0a0a0);
}
.canvas :deep(svg) {
  max-width: 100%;
  max-height: 100%;
  display: block;
}
</style>
