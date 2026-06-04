<script setup lang="ts">
export type AnchorPanelValue = {
  name?: string | null;
  x: number;
  y: number;
};

const props = defineProps<{
  value: AnchorPanelValue;
}>();

const emit = defineEmits<{
  (e: "change-anchor", field: "name" | "x" | "y", value: string | number): void;
}>();

function displayNumber(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return "";
  const rounded = Math.round(value);
  if (Math.abs(value - rounded) < 0.001) return rounded.toString();
  return value.toFixed(2).replace(/\.?0+$/, "");
}

function commitName(event: Event) {
  emit("change-anchor", "name", (event.target as HTMLInputElement).value);
}

function commitNumber(field: "x" | "y", event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  if (!Number.isFinite(value)) {
    input.value = displayNumber(props.value[field]);
    return;
  }
  emit("change-anchor", field, value);
}

function blurOnEnter(event: KeyboardEvent) {
  (event.target as HTMLInputElement).blur();
}
</script>

<template>
  <section class="anchor-panel" aria-label="Anchor">
    <label class="anchor-field name-field">
      <span>Name</span>
      <input
        type="text"
        :value="value.name ?? ''"
        aria-label="Anchor name"
        @change="commitName"
        @keydown.enter.prevent="blurOnEnter"
      />
    </label>
    <label class="anchor-field">
      <span>X</span>
      <input
        :value="displayNumber(value.x)"
        aria-label="Anchor X"
        inputmode="decimal"
        @change="commitNumber('x', $event)"
        @keydown.enter.prevent="blurOnEnter"
      />
    </label>
    <label class="anchor-field">
      <span>Y</span>
      <input
        :value="displayNumber(value.y)"
        aria-label="Anchor Y"
        inputmode="decimal"
        @change="commitNumber('y', $event)"
        @keydown.enter.prevent="blurOnEnter"
      />
    </label>
  </section>
</template>

<style scoped>
.anchor-panel {
  box-sizing: border-box;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 8px;
  display: grid;
  grid-template-columns: 126px 66px 66px;
  gap: 6px;
  pointer-events: auto;
}

.anchor-field {
  height: 30px;
  box-sizing: border-box;
  margin: 0;
  padding: 0 8px;
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  background: var(--rb-app-background, #101010);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-button-radius, 8px);
}

.anchor-field:not(.name-field) {
  grid-template-columns: 14px minmax(0, 1fr);
}

.anchor-field span {
  color: var(--rb-muted-text, #808080);
  font: 10px ui-sans-serif, system-ui, sans-serif;
  letter-spacing: 0;
  text-transform: uppercase;
}

.anchor-field input {
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
  outline: none;
}

.anchor-field:not(.name-field) input {
  text-align: right;
}

.anchor-field input::-webkit-outer-spin-button,
.anchor-field input::-webkit-inner-spin-button {
  appearance: none;
  margin: 0;
}

.anchor-field:focus-within {
  border-color: var(--rb-accent, #18b86f);
}
</style>
