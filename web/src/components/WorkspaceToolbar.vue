<script setup lang="ts">
// Editor workspace navigation toolbar. This mirrors
// runebender-xilem's `components/workspace_toolbar.rs`: a compact
// floating toolbar with a glyph-grid button that closes the editor.

const emit = defineEmits<{
  (e: "glyph-grid"): void;
}>();
</script>

<template>
  <div class="workspace-toolbar" role="toolbar" aria-label="Workspace toolbar">
    <button
      type="button"
      class="workspace-btn"
      title="Glyph Grid"
      aria-label="Glyph Grid"
      @click="emit('glyph-grid')"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="4" y="4" width="4" height="4" rx="0.5" />
        <rect x="10" y="4" width="4" height="4" rx="0.5" />
        <rect x="16" y="4" width="4" height="4" rx="0.5" />
        <rect x="4" y="10" width="4" height="4" rx="0.5" />
        <rect x="10" y="10" width="4" height="4" rx="0.5" />
        <rect x="16" y="10" width="4" height="4" rx="0.5" />
        <rect x="4" y="16" width="4" height="4" rx="0.5" />
        <rect x="10" y="16" width="4" height="4" rx="0.5" />
        <rect x="16" y="16" width="4" height="4" rx="0.5" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.workspace-toolbar {
  height: 64px;
  box-sizing: border-box;
  padding: 8px;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  display: flex;
  align-items: center;
  gap: 6px;
  pointer-events: auto;
}

.workspace-btn {
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

.workspace-btn:hover {
  color: var(--rb-accent, #66ee88);
}

svg {
  width: 32px;
  height: 32px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linejoin: round;
}
</style>
