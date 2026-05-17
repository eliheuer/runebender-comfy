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
      <div class="field-row">
        <label>
          <span>X</span>
          <input
            :value="displayNumber(value?.x)"
            placeholder="X"
            inputmode="decimal"
            :readonly="selectionCount === 0"
            @change="commitCoordinate('x', $event)"
            @keydown.enter="blurOnEnter"
          />
        </label>
        <label>
          <span>Y</span>
          <input
            :value="displayNumber(value?.y)"
            placeholder="Y"
            inputmode="decimal"
            :readonly="selectionCount === 0"
            @change="commitCoordinate('y', $event)"
            @keydown.enter="blurOnEnter"
          />
        </label>
      </div>

      <div class="field-row">
        <label>
          <span>W</span>
          <input
            :value="displayDimension(value?.width)"
            placeholder="W"
            inputmode="decimal"
            :readonly="selectionCount <= 1"
            @change="commitCoordinate('width', $event)"
            @keydown.enter="blurOnEnter"
          />
        </label>
        <label>
          <span>H</span>
          <input
            :value="displayDimension(value?.height)"
            placeholder="H"
            inputmode="decimal"
            :readonly="selectionCount <= 1"
            @change="commitCoordinate('height', $event)"
            @keydown.enter="blurOnEnter"
          />
        </label>
      </div>
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

.quadrant-picker {
  width: 80px;
  height: 80px;
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
  width: 33.333%;
  height: 33.333%;
  background: transparent;
  border: 0;
  padding: 0;
  cursor: pointer;
}
.quadrant-dot::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  background: var(--rb-control-background, #303030);
  border: 1px solid var(--rb-primary-text, #909090);
  box-sizing: border-box;
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
  left: 66.667%;
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
  top: 66.667%;
}
.quadrant-dot.tl::after,
.quadrant-dot.cl::after,
.quadrant-dot.bl::after {
  left: 0;
}
.quadrant-dot.tc::after,
.quadrant-dot.cc::after,
.quadrant-dot.bc::after {
  left: 50%;
}
.quadrant-dot.tr::after,
.quadrant-dot.cr::after,
.quadrant-dot.br::after {
  left: 100%;
}
.quadrant-dot.tl::after,
.quadrant-dot.tc::after,
.quadrant-dot.tr::after {
  top: 0;
}
.quadrant-dot.cl::after,
.quadrant-dot.cc::after,
.quadrant-dot.cr::after {
  top: 50%;
}
.quadrant-dot.bl::after,
.quadrant-dot.bc::after,
.quadrant-dot.br::after {
  top: 100%;
}
.quadrant-dot.active::after {
  background: var(--rb-muted-text, #808080);
}
.quadrant-dot:hover::after {
  border-color: var(--rb-accent, #66ee88);
}

.fields {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-row {
  display: flex;
  gap: 4px;
}

label {
  display: grid;
  gap: 2px;
}

label span {
  color: var(--rb-secondary-text, #707070);
  font: 10px ui-sans-serif, system-ui, sans-serif;
  text-align: center;
}

input {
  width: 48px;
  height: 28px;
  box-sizing: border-box;
  background: var(--rb-app-background, #101010);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-primary-text, #909090);
  font: 12px ui-monospace, monospace;
  text-align: center;
  padding: 0 4px;
}
input::placeholder {
  color: var(--rb-subdued-text, #505050);
}
</style>
