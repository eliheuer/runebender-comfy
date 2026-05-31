<script setup lang="ts">
import GeneratedIcon from "./GeneratedIcon.vue";

// Right-side transform panel. Mirrors runebender-xilem's
// `components/transform_panel.rs` as a compact 2-column action grid.
// Bounds display lives in CoordinatePanel, matching xilem's split
// between coordinate editing and transform actions.

type SelectionBounds = {
  count: number;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type TransformActionId =
  | "flip-h"
  | "flip-v"
  | "rot-cw"
  | "rot-ccw"
  | "duplicate"
  | "duplicate-repeat"
  | "union"
  | "subtract"
  | "intersect"
  | "exclude";

defineProps<{
  bounds?: SelectionBounds;
  contourCount: number;
}>();

const emit = defineEmits<{
  (e: "transform", action: TransformActionId): void;
}>();

const actions = [
  ["Flip Horizontal", "flip-h"],
  ["Flip Vertical", "flip-v"],
  ["Rotate 90 CW", "rot-cw"],
  ["Rotate 90 CCW", "rot-ccw"],
  ["Duplicate", "duplicate"],
  ["Dup + Repeat", "duplicate-repeat"],
  ["Union", "union"],
  ["Subtract", "subtract"],
  ["Intersect", "intersect"],
  ["Exclude (XOR)", "exclude"],
] as const;

const ACTION_ICONS: Record<TransformActionId, string> = {
  "flip-h": "flip-h",
  "flip-v": "flip-v",
  "rot-cw": "rot-cw",
  "rot-ccw": "rot-ccw",
  duplicate: "duplicate",
  "duplicate-repeat": "duplicate-repeat",
  union: "union",
  subtract: "subtract",
  intersect: "intersect",
  exclude: "exclude",
};

function actionEnabled(id: string, hasSelection: boolean, contourCount: number): boolean {
  if (["union", "subtract", "intersect", "exclude"].includes(id)) {
    return contourCount >= 2;
  }
  return hasSelection;
}

function actionImplemented(id: string): boolean {
  return (
    id === "flip-h" ||
    id === "flip-v" ||
    id === "rot-cw" ||
    id === "rot-ccw" ||
    id === "duplicate" ||
    id === "duplicate-repeat" ||
    id === "union" ||
    id === "subtract" ||
    id === "intersect" ||
    id === "exclude"
  );
}

function actionAvailable(
  id: string,
  hasSelection: boolean,
  contourCount: number,
): boolean {
  return actionImplemented(id) && actionEnabled(id, hasSelection, contourCount);
}

function runAction(
  id: TransformActionId,
  hasSelection: boolean,
  contourCount: number,
) {
  if (!actionAvailable(id, hasSelection, contourCount)) return;
  emit("transform", id);
}
</script>

<template>
  <section class="transform-panel" aria-label="Selection transforms">
    <div class="actions">
      <button
        v-for="[label, id] in actions"
        :key="id"
        type="button"
        class="action-btn"
        :class="{ disabled: !actionAvailable(id, !!bounds, contourCount) }"
        :title="label"
        :aria-label="label"
        :disabled="!actionAvailable(id, !!bounds, contourCount)"
        @click="runAction(id, !!bounds, contourCount)"
      >
        <GeneratedIcon :name="ACTION_ICONS[id]" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.transform-panel {
  width: 117px;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
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
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  /* Neutral gray by default, green only on hover — same as the tool
     palette and xilem's transform buttons. Enabled buttons should not
     glow green just for being usable. */
  color: var(--rb-panel-outline, #606060);
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.action-btn:not(.disabled):hover {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}
.action-btn.disabled {
  color: var(--rb-panel-outline, #606060);
  opacity: 0.55;
}

</style>
