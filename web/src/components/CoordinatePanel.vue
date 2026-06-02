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
  if (value === undefined || !Number.isFinite(value)) return "";
  const rounded = Math.round(value);
  if (Math.abs(value - rounded) < 0.001) return rounded.toString();
  return value.toFixed(2).replace(/\.?0+$/, "");
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
      <label class="coord-field">
        <span>X</span>
        <input
          class="coord-input"
          :value="displayNumber(value?.x)"
          aria-label="X"
          inputmode="decimal"
          :readonly="selectionCount === 0"
          @change="commitCoordinate('x', $event)"
          @keydown.enter="blurOnEnter"
        />
      </label>
      <label class="coord-field">
        <span>Y</span>
        <input
          class="coord-input"
          :value="displayNumber(value?.y)"
          aria-label="Y"
          inputmode="decimal"
          :readonly="selectionCount === 0"
          @change="commitCoordinate('y', $event)"
          @keydown.enter="blurOnEnter"
        />
      </label>
      <label class="coord-field">
        <span>W</span>
        <input
          class="coord-input"
          :value="displayDimension(value?.width)"
          aria-label="Width"
          inputmode="decimal"
          :readonly="selectionCount <= 1"
          @change="commitCoordinate('width', $event)"
          @keydown.enter="blurOnEnter"
        />
      </label>
      <label class="coord-field">
        <span>H</span>
        <input
          class="coord-input"
          :value="displayDimension(value?.height)"
          aria-label="Height"
          inputmode="decimal"
          :readonly="selectionCount <= 1"
          @change="commitCoordinate('height', $event)"
          @keydown.enter="blurOnEnter"
        />
      </label>
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
  width: auto;
  height: auto;
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 8px 8px 8px 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 14px;
  pointer-events: auto;
}

/* 64x64 box with a 2x2 cell grid (outer border + center cross) and a
   dot at each of the 9 intersections. Each dot is positioned directly
   at its intersection (0% / 50% / 100% of the box) and centered with a
   translate, so the 3x3 grid stays even — the previous cell+::after
   scheme pushed the middle column/row off-center. */
.quadrant-picker {
  width: 58px;
  height: 58px;
  box-sizing: border-box;
  position: relative;
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
}

.quadrant-picker::before,
.quadrant-picker::after {
  content: "";
  position: absolute;
  z-index: 0;
  background: var(--rb-panel-outline, #606060);
  pointer-events: none;
}

.quadrant-picker::before {
  width: var(--rb-stroke-width, 1px);
  top: 0;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
}

.quadrant-picker::after {
  height: var(--rb-stroke-width, 1px);
  left: 0;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
}

.quadrant-dot {
  appearance: none;
  position: absolute;
  z-index: 1;
  width: 12px;
  height: 12px;
  margin: 0;
  padding: 0;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  background: var(--rb-control-background, #303030);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
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
  background: #808080;
}
.quadrant-dot:hover {
  border-color: var(--rb-accent, #18b86f);
}

.fields {
  display: grid;
  grid-template-columns: repeat(2, 66px);
  column-gap: 6px;
  row-gap: 6px;
}

.coord-field {
  width: 66px;
  height: 30px;
  box-sizing: border-box;
  margin: 0;
  padding: 0 8px;
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  background: var(--rb-app-background, #101010);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-button-radius, 8px);
}
.coord-field span {
  color: var(--rb-muted-text, #808080);
  font: 10px ui-sans-serif, system-ui, sans-serif;
  letter-spacing: 0;
  text-transform: uppercase;
}

.coord-input {
  appearance: textfield;
  width: 100%;
  height: 100%;
  min-width: 0;
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  background: transparent;
  border: 0;
  color: var(--rb-primary-text, #909090);
  font: 13px ui-monospace, monospace;
  text-align: right;
  outline: none;
}
.coord-input::-webkit-outer-spin-button,
.coord-input::-webkit-inner-spin-button {
  appearance: none;
  margin: 0;
}
.coord-input::placeholder {
  color: var(--rb-subdued-text, #505050);
}
.coord-field:focus-within {
  border-color: var(--rb-accent, #18b86f);
}
</style>
