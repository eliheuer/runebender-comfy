<script setup lang="ts">
// Sub-toolbar peer for runebender-xilem's `shapes_toolbar.rs`.
// Appears when Shapes is active and chooses the primitive created by
// drag gestures in the editor canvas.

export type ShapeKind = "rectangle" | "ellipse";

defineProps<{
  active: ShapeKind;
}>();

defineEmits<{
  (e: "select", shape: ShapeKind): void;
}>();

const shapes = [
  ["rectangle", "Rectangle"],
  ["ellipse", "Ellipse"],
] as const;
</script>

<template>
  <div class="shapes-toolbar" role="toolbar" aria-label="Shapes toolbar">
    <button
      v-for="[id, label] in shapes"
      :key="id"
      type="button"
      class="shape-btn"
      :class="{ active: id === active }"
      :title="label"
      :aria-label="label"
      :aria-pressed="id === active"
      @click="$emit('select', id)"
    >
      <svg
        class="shape-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        aria-hidden="true"
      >
        <rect v-if="id === 'rectangle'" x="5" y="6" width="14" height="12" />
        <ellipse v-else cx="12" cy="12" rx="7" ry="5.5" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.shapes-toolbar {
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 64px;
  flex-shrink: 0;
}

.shape-btn {
  appearance: none;
  font: inherit;
  width: 48px;
  height: 48px;
  background: var(--rb-panel-background, #1c1c1c);
  color: var(--rb-panel-outline, #606060);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}

.shape-btn:hover,
.shape-btn.active {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}

.shape-icon {
  width: 32px;
  height: 32px;
  display: block;
}
</style>
