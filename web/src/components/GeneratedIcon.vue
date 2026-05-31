<script setup lang="ts">
import { computed } from "vue";
import { TOOLBAR_ICONS, type ToolbarIcon } from "./generatedToolbarIcons";

const props = withDefaults(
  defineProps<{
    name: string;
    size?: number;
    strokeWidth?: number;
  }>(),
  {
    size: 32,
    strokeWidth: 1.5,
  },
);

const icon = computed<ToolbarIcon | undefined>(() => TOOLBAR_ICONS[props.name]);
const mode = computed(() => icon.value?.mode ?? "fill");
</script>

<template>
  <svg
    v-if="icon"
    class="generated-icon"
    :width="size"
    :height="size"
    :viewBox="icon.viewBox"
    preserveAspectRatio="xMidYMid meet"
    aria-hidden="true"
  >
    <path
      :d="icon.d"
      :fill="mode === 'fill' ? 'currentColor' : 'none'"
      :stroke="mode === 'stroke' ? 'currentColor' : 'none'"
      :stroke-width="mode === 'stroke' ? strokeWidth : undefined"
      stroke-linecap="round"
      stroke-linejoin="round"
      vector-effect="non-scaling-stroke"
    />
  </svg>
</template>

<style scoped>
.generated-icon {
  display: block;
  overflow: visible;
  color: currentColor;
}
</style>
