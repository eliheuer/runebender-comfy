<script setup lang="ts">
import GeneratedIcon from "./GeneratedIcon.vue";

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

const SHAPE_ICONS: Record<ShapeKind, string> = {
  rectangle: "shape-rectangle",
  ellipse: "shape-ellipse",
};
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
      <GeneratedIcon :name="SHAPE_ICONS[id]" />
    </button>
  </div>
</template>

<style scoped>
.shapes-toolbar {
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 6px;
  display: flex;
  flex-direction: row;
  gap: 6px;
  width: fit-content;
  flex-shrink: 0;
}

.shape-btn {
  appearance: none;
  font: inherit;
  width: 48px;
  height: 48px;
  background: var(--rb-button-background, #181818);
  color: var(--rb-glyph-preview, #808080);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-button-radius, 8px);
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

</style>
