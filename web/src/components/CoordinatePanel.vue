<script setup lang="ts">
// Bottom-right coordinate panel. Mirrors runebender-xilem's
// `components/coordinate_panel.rs` visually: a 3x3 reference picker
// plus X/Y and W/H fields. The picker changes which selection-bounds
// anchor is used for X/Y display and transform operations. Committed
// X/Y edits move the selection by that reference point; committed W/H
// edits scale from it.

type CoordinateValue = {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
};

const props = defineProps<{
  value?: CoordinateValue;
  selectionCount: number;
  quadrant: CoordinateQuadrant;
}>();

const emit = defineEmits<{
  (e: "select-quadrant", quadrant: CoordinateQuadrant): void;
  (
    e: "change-coordinate",
    axis: "x" | "y" | "width" | "height",
    value: number,
  ): void;
}>();

const quadrants = ["tl", "tc", "tr", "cl", "cc", "cr", "bl", "bc", "br"] as const;
export type CoordinateQuadrant = (typeof quadrants)[number];

function displayNumber(value: number | undefined): string {
  return value === undefined || !Number.isFinite(value) ? "" : Math.round(value).toString();
}

function displayDimension(value: number | undefined): string {
  return props.selectionCount > 1 ? displayNumber(value) : "";
}

function commitCoordinate(axis: "x" | "y" | "width" | "height", event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  if (!Number.isFinite(value)) {
    input.value = displayNumber(props.value?.[axis]);
    return;
  }
  emit("change-coordinate", axis, value);
}

function blurOnEnter(event: KeyboardEvent) {
  (event.target as HTMLInputElement).blur();
}
</script>

<template>
  <section class="coordinate-panel" aria-label="Selection coordinates">
    <div class="quadrant-picker" aria-label="Coordinate reference point">
      <button
        v-for="q in quadrants"
        :key="q"
        type="button"
        class="quadrant-dot"
        :class="[q, { active: q === quadrant }]"
        :aria-label="`Use ${q} as coordinate reference`"
        :aria-pressed="q === quadrant"
        @click="emit('select-quadrant', q)"
      />
    </div>

    <div class="fields">
      <input
        class="coord-input"
        :value="displayNumber(value?.x)"
        placeholder="X"
        aria-label="X"
        inputmode="decimal"
        :readonly="selectionCount === 0"
        @change="commitCoordinate('x', $event)"
        @keydown.enter="blurOnEnter"
      />
      <input
        class="coord-input"
        :value="displayNumber(value?.y)"
        placeholder="Y"
        aria-label="Y"
        inputmode="decimal"
        :readonly="selectionCount === 0"
        @change="commitCoordinate('y', $event)"
        @keydown.enter="blurOnEnter"
      />
      <input
        class="coord-input"
        :value="displayDimension(value?.width)"
        placeholder="W"
        aria-label="Width"
        inputmode="decimal"
        :readonly="selectionCount <= 1"
        @change="commitCoordinate('width', $event)"
        @keydown.enter="blurOnEnter"
      />
      <input
        class="coord-input"
        :value="displayDimension(value?.height)"
        placeholder="H"
        aria-label="Height"
        inputmode="decimal"
        :readonly="selectionCount <= 1"
        @change="commitCoordinate('height', $event)"
        @keydown.enter="blurOnEnter"
      />
    </div>
  </section>
</template>

<style scoped>
/*
 * Colors / sizes mirror xilem/src/theme.rs coordinate_panel:
 *   PANEL_BACKGROUND       #1C1C1C
 *   PANEL_OUTLINE / BASE_F #606060
 *   PANEL_LINE / BASE_I    #909090
 *   DOT_SELECTED_INNER     #808080
 *   DOT_UNSELECTED_INNER   #303030
 */

.coordinate-panel {
  width: 240px;
  height: 140px;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
  padding: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  pointer-events: auto;
}

/* 64x64 box with a 2x2 cell grid (outer border + center cross) and a
   dot at each of the 9 intersections. Each dot is positioned directly
   at its intersection (0% / 50% / 100% of the box) and centered with a
   translate, so the 3x3 grid stays even — the previous cell+::after
   scheme pushed the middle column/row off-center. */
.quadrant-picker {
  width: 64px;
  height: 64px;
  box-sizing: border-box;
  position: relative;
  border: 1px solid var(--rb-primary-text, #909090);
  background:
    linear-gradient(var(--rb-primary-text, #909090), var(--rb-primary-text, #909090)) 50% 0 / 1px 100% no-repeat,
    linear-gradient(var(--rb-primary-text, #909090), var(--rb-primary-text, #909090)) 0 50% / 100% 1px no-repeat;
}

.quadrant-dot {
  appearance: none;
  position: absolute;
  width: 14px;
  height: 14px;
  margin: 0;
  padding: 0;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  background: var(--rb-control-background, #303030);
  border: 1px solid var(--rb-primary-text, #909090);
  box-sizing: border-box;
  cursor: pointer;
}
.quadrant-dot.tl,
.quadrant-dot.cl,
.quadrant-dot.bl {
  left: 0;
}
.quadrant-dot.tc,
.quadrant-dot.cc,
.quadrant-dot.bc {
  left: 50%;
}
.quadrant-dot.tr,
.quadrant-dot.cr,
.quadrant-dot.br {
  left: 100%;
}
.quadrant-dot.tl,
.quadrant-dot.tc,
.quadrant-dot.tr {
  top: 0;
}
.quadrant-dot.cl,
.quadrant-dot.cc,
.quadrant-dot.cr {
  top: 50%;
}
.quadrant-dot.bl,
.quadrant-dot.bc,
.quadrant-dot.br {
  top: 100%;
}
.quadrant-dot.active {
  background: var(--rb-muted-text, #808080);
}
.quadrant-dot:hover {
  border-color: var(--rb-accent, #66ee88);
}

.fields {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.coord-input {
  width: 58px;
  height: 34px;
  box-sizing: border-box;
  background: var(--rb-app-background, #101010);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-primary-text, #909090);
  font: 12px ui-monospace, monospace;
  text-align: center;
  padding: 0 4px;
}
.coord-input::placeholder {
  color: var(--rb-subdued-text, #505050);
}
</style>
