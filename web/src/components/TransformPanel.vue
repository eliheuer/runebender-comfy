<script setup lang="ts">
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
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <template v-if="id === 'flip-h'">
            <path d="M12 4v16M5 7l5 5-5 5V7Zm14 0-5 5 5 5V7Z" />
          </template>
          <template v-else-if="id === 'flip-v'">
            <path d="M4 12h16M7 5l5 5 5-5H7Zm0 14 5-5 5 5H7Z" />
          </template>
          <template v-else-if="id === 'rot-cw'">
            <path d="M7 7a7 7 0 1 0 10 0M17 7v5h-5" />
          </template>
          <template v-else-if="id === 'rot-ccw'">
            <path d="M17 7a7 7 0 1 1-10 0M7 7v5h5" />
          </template>
          <template v-else-if="id === 'duplicate'">
            <rect x="8" y="8" width="10" height="10" />
            <path d="M6 16H4V4h12v2" />
          </template>
          <template v-else-if="id === 'duplicate-repeat'">
            <rect x="9" y="9" width="9" height="9" />
            <path d="M6 15H4V4h11v2M17 4l3 3-3 3" />
          </template>
          <template v-else-if="id === 'union'">
            <circle cx="10" cy="12" r="5" />
            <circle cx="14" cy="12" r="5" />
          </template>
          <template v-else-if="id === 'subtract'">
            <circle cx="10" cy="12" r="5" />
            <path d="M14 7a5 5 0 0 1 0 10" />
          </template>
          <template v-else-if="id === 'intersect'">
            <path d="M12 8a5 5 0 0 1 0 8 5 5 0 0 1 0-8Z" />
          </template>
          <template v-else>
            <circle cx="9" cy="12" r="4" />
            <circle cx="15" cy="12" r="4" />
            <path d="M9 8l6 8M15 8l-6 8" />
          </template>
        </svg>
      </button>
    </div>
  </section>
</template>

<style scoped>
.transform-panel {
  width: 118px;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 8px;
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

svg {
  width: 24px;
  height: 24px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
