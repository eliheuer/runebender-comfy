<script setup lang="ts">
// File-operation toolbar peer for runebender-xilem's
// `components/system_toolbar.rs`. Save is wired through the host
// and stays disabled only when there is no loaded font/workspace.
// Close is shown only when the host provides somewhere to close to
// (i.e. running embedded inside another app like ComfyUI).

withDefaults(
  defineProps<{
    saveEnabled?: boolean;
    closeEnabled?: boolean;
  }>(),
  {
    saveEnabled: false,
    closeEnabled: false,
  },
);

const emit = defineEmits<{
  (e: "save"): void;
  (e: "close"): void;
}>();

function onSave(enabled: boolean) {
  if (!enabled) return;
  emit("save");
}
</script>

<template>
  <div class="system-toolbar" role="toolbar" aria-label="System toolbar">
    <button
      type="button"
      class="system-btn"
      :class="{ disabled: !saveEnabled }"
      :disabled="!saveEnabled"
      :aria-disabled="!saveEnabled"
      :title="saveEnabled ? 'Save' : 'Save unavailable'"
      aria-label="Save"
      @click="onSave(saveEnabled)"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M5 3h12l2 2v16H5V3Z" />
        <path d="M8 3v6h8V3M8 21v-7h8v7" />
        <path d="M10 6h4" />
      </svg>
    </button>
    <button
      v-if="closeEnabled"
      type="button"
      class="system-btn close-btn"
      title="Close editor"
      aria-label="Close editor"
      @click="emit('close')"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M6 6l12 12M18 6l-12 12" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.system-toolbar {
  height: 64px;
  box-sizing: border-box;
  padding: 8px;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.system-btn {
  appearance: none;
  width: 48px;
  height: 48px;
  box-sizing: border-box;
  padding: 0;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  color: var(--rb-panel-outline, #606060);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.system-btn:not(:disabled):hover {
  color: var(--rb-accent, #66ee88);
}

.close-btn:not(:disabled):hover {
  color: var(--rb-warning, #ffdd33);
  border-color: var(--rb-warning, #ffdd33);
}

.system-btn.disabled {
  cursor: default;
  opacity: 0.55;
}

svg {
  width: 32px;
  height: 32px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
