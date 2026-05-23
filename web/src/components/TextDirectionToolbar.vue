<script setup lang="ts">
// Sub-toolbar peer for runebender-xilem's
// `components/text_direction_toolbar.rs`. Appears when Text is active
// and chooses the direction used by the upcoming text buffer work.

export type TextDirection = "ltr" | "rtl";

defineProps<{
  active: TextDirection;
}>();

defineEmits<{
  (e: "select", direction: TextDirection): void;
}>();

const directions = [
  ["ltr", "Left to Right"],
  ["rtl", "Right to Left"],
] as const;
</script>

<template>
  <div
    class="text-direction-toolbar"
    role="toolbar"
    aria-label="Text direction toolbar"
  >
    <button
      v-for="[id, label] in directions"
      :key="id"
      type="button"
      class="direction-btn"
      :class="{ active: id === active }"
      :title="label"
      :aria-label="label"
      :aria-pressed="id === active"
      @click="$emit('select', id)"
    >
      <svg
        class="direction-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <template v-if="id === 'ltr'">
          <path d="M5 6h10" />
          <path d="M12 3l3 3-3 3" />
          <path d="M5 13h13" />
          <path d="M5 18h9" />
        </template>
        <template v-else>
          <path d="M9 6h10" />
          <path d="M12 3 9 6l3 3" />
          <path d="M6 13h13" />
          <path d="M10 18h9" />
        </template>
      </svg>
    </button>
  </div>
</template>

<style scoped>
.text-direction-toolbar {
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 8px;
  display: flex;
  flex-direction: row;
  gap: 6px;
  width: fit-content;
  flex-shrink: 0;
}

.direction-btn {
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

.direction-btn:hover,
.direction-btn.active {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}

.direction-icon {
  width: 32px;
  height: 32px;
  display: block;
}
</style>
