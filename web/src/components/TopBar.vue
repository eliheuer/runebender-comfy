<script setup lang="ts">
// Top file-info bar. Mirrors runebender-xilem's
//   views/glyph_grid/mod.rs `file_info_panel`
// + components/master_toolbar.rs
// + components/system_toolbar.rs
//
// Layout: font label + save status stretches left; master switcher
// in the middle-right; system buttons on the far right. All three
// are individual panel tiles, separated by
// BENTO_GAP (6px), matching xilem's bento layout.

import SystemToolbar from "./SystemToolbar.vue";

defineProps<{
  /** Display label for the open font (UFO folder name, designspace
   *  path, or empty string when nothing's loaded). */
  fontLabel: string;
  /** True when an in-memory edit has not been serialized back. */
  unsaved?: boolean;
  /** Last successful save time, e.g. "03:42 PM". */
  lastSaved?: string | null;
  /** Source destination summary, e.g. linked disk root. */
  sourceLabel?: string | null;
  /** Names of available masters. Stubbed to a single entry until
   *  designspace loading lands (Phase 7). */
  masters?: string[];
  /** Index of the active master. */
  activeMaster?: number;
  /** Rendered preview glyphs for each master, usually lowercase n. */
  masterPreviews?: Array<string | undefined>;
  /** False when there is no loaded font/workspace to persist. */
  saveEnabled?: boolean;
  /** False when there is no loaded workspace to export. */
  saveAsEnabled?: boolean;
  /** True when the host wants a Close button next to Save (e.g. when
   *  embedded as a ComfyUI overlay). */
  closeEnabled?: boolean;
}>();

function masterLabel(name: string): string {
  return name.trim().slice(0, 1).toLowerCase() || "?";
}

defineEmits<{
  (e: "selectMaster", index: number): void;
  (e: "save"): void;
  (e: "saveAs"): void;
  (e: "close"): void;
}>();
</script>

<template>
  <div class="top-bar">
    <!-- File info: stretches to fill -->
    <div class="panel file-info">
      <div class="file-path">
        {{ fontLabel || "No font loaded" }}
      </div>
      <div
        v-if="fontLabel"
        class="save-status"
        :class="{ saved: !unsaved && lastSaved }"
      >
        {{ !unsaved && lastSaved ? `Saved ${lastSaved}` : "Not saved" }}
        <span v-if="sourceLabel" class="source-label" :title="sourceLabel"> · {{ sourceLabel }}</span>
      </div>
    </div>

    <!-- Master switcher -->
    <div v-if="masters && masters.length > 1" class="panel masters">
      <button
        v-for="(name, i) in masters"
        :key="name"
        type="button"
        class="master-btn"
        :class="{ active: i === activeMaster }"
        :title="name"
        @click="$emit('selectMaster', i)"
      >
        <span
          v-if="masterPreviews?.[i]"
          class="master-preview"
          aria-hidden="true"
          v-html="masterPreviews[i]"
        />
        <span v-else>{{ masterLabel(name) }}</span>
      </button>
    </div>

    <SystemToolbar
      :save-enabled="saveEnabled"
      :save-as-enabled="saveAsEnabled"
      :close-enabled="closeEnabled"
      @save="$emit('save')"
      @save-as="$emit('saveAs')"
      @close="$emit('close')"
    />
  </div>
</template>

<style scoped>
/*
 * Colors from xilem/src/theme.rs:
 *   PANEL_BACKGROUND       #1C1C1C
 *   PANEL_OUTLINE / BASE_F #606060
 *   PRIMARY_UI_TEXT / BASE_I #909090
 *   SECONDARY_UI_TEXT / BASE_G #707070
 *   GRID_CELL_SELECTED_OUTLINE / TOOLBAR_ICON_HOVERED #66EE88
 *   MARK_YELLOW (Not saved) #FFDD33
 *
 * Sizes:
 *   TOOLBAR_BUTTON_RADIUS  6px
 *   TOOLBAR_BORDER_WIDTH   1.5px
 *   BENTO_GAP              6px (parent grid)
 */

.top-bar {
  display: flex;
  gap: 6px;
  height: 64px; /* 48px toolbar item + 8px panel padding on each side */
  flex-shrink: 0;
}

.panel {
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
  display: flex;
  align-items: center;
}

.file-info {
  flex: 1;
  padding: 6px 12px;
  gap: 2px;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  min-width: 0;
}
.file-path {
  color: var(--rb-muted-text, #808080);
  font: 16px ui-sans-serif, system-ui, sans-serif;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  width: 100%;
  min-width: 0;
}
.save-status {
  color: var(--rb-warning, #ffdd33);
  font: 16px ui-sans-serif, system-ui, sans-serif;
  flex-shrink: 0;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.save-status.saved {
  color: var(--rb-accent, #66ee88);
}
.source-label {
  color: var(--rb-secondary-text, #707070);
}

.masters {
  padding: 6px;
  gap: 6px;
}
.master-btn {
  appearance: none;
  font: inherit;
  background: var(--rb-panel-background, #1c1c1c);
  color: var(--rb-panel-outline, #606060);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  width: 48px;
  height: 48px;
  cursor: pointer;
  font: 14px ui-sans-serif, system-ui, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
}
.master-btn:hover {
  color: var(--rb-accent, #66ee88);
}
.master-btn.active {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}

.master-preview {
  width: 32px;
  height: 32px;
  color: currentColor;
  display: flex;
  align-items: center;
  justify-content: center;
}
.master-preview :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
  overflow: visible;
}

</style>
