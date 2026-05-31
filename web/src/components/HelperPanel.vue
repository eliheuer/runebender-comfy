<script setup lang="ts">
import GeneratedIcon from "./GeneratedIcon.vue";

// Left-side helper panel. This is the counterpart to TransformPanel for
// miscellaneous tools that do not belong in the main tool palette.

const helpers = [
  ["Snap Helpers", "glyph-grid"],
  ["Measure Helpers", "measure"],
  ["Preview Helpers", "preview"],
  ["Shape Helpers", "shapes"],
  ["Text Helpers", "text"],
  ["Assist Helpers", "hyperpen"],
  ["Select Helpers", "select"],
  ["Pen Helpers", "pen"],
  ["Save Helpers", "save"],
  ["Source Helpers", "save-as"],
] as const;
</script>

<template>
  <section class="helper-panel" aria-label="Helper tools">
    <div class="actions">
      <button
        v-for="[label, icon] in helpers"
        :key="label"
        type="button"
        class="action-btn disabled"
        :title="`${label} placeholder`"
        :aria-label="`${label} placeholder`"
        disabled
      >
        <GeneratedIcon :name="icon" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.helper-panel {
  width: 117px;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 6px;
  pointer-events: auto;
}

.actions {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 6px;
}

.action-btn {
  appearance: none;
  width: 48px;
  height: 48px;
  background: var(--rb-button-background, #181818);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-button-radius, 8px);
  color: var(--rb-glyph-preview, #808080);
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.action-btn.disabled {
  color: var(--rb-glyph-preview, #808080);
  opacity: 0.55;
}
</style>
