<script setup lang="ts">
import GeneratedIcon from "./GeneratedIcon.vue";

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
      <GeneratedIcon name="glyph-grid" />
    </button>
  </div>
</template>

<style scoped>
.workspace-toolbar {
  box-sizing: border-box;
  padding: 6px;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
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

</style>
