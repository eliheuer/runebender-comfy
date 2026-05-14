<script setup lang="ts">
// Top file-info bar. Mirrors runebender-xilem's
//   views/glyph_grid/mod.rs `file_info_panel`
// + components/master_toolbar.rs
// + components/system_toolbar.rs (Save button)
//
// Layout: font label + "Not saved" stretches left; master switcher
// in the middle-right; system buttons (currently just Save) on the
// far right. All three are individual panel tiles, separated by
// BENTO_GAP (6px), matching xilem's bento layout.

defineProps<{
  /** Display label for the open font (UFO folder name, designspace
   *  path, or empty string when nothing's loaded). */
  fontLabel: string;
  /** Set to true once we wire dirty-tracking; for now always true
   *  if a font is loaded so the indicator is visible. */
  unsaved?: boolean;
  /** Names of available masters. Stubbed to a single entry until
   *  designspace loading lands (Phase 7). */
  masters?: string[];
  /** Index of the active master. */
  activeMaster?: number;
}>();

defineEmits<{
  (e: "selectMaster", index: number): void;
  (e: "save"): void;
}>();
</script>

<template>
  <div class="top-bar">
    <!-- File info: stretches to fill -->
    <div class="panel file-info">
      <div class="file-path">
        {{ fontLabel || "No font loaded" }}
      </div>
      <div v-if="unsaved && fontLabel" class="not-saved">Not saved</div>
    </div>

    <!-- Master switcher -->
    <div v-if="masters && masters.length" class="panel masters">
      <button
        v-for="(name, i) in masters"
        :key="name"
        type="button"
        class="master-btn"
        :class="{ active: i === activeMaster }"
        :title="name"
        @click="$emit('selectMaster', i)"
      >
        {{ name.slice(0, 1).toLowerCase() }}
      </button>
    </div>

    <!-- System toolbar: Save (stub) -->
    <div class="panel system">
      <button
        type="button"
        class="sys-btn"
        title="Save (not yet wired)"
        @click="$emit('save')"
      >
        Save
      </button>
    </div>
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
  height: 48px; /* matches TOOLBAR_ITEM_SIZE */
  flex-shrink: 0;
}

.panel {
  background: #1c1c1c;
  border: 1.5px solid #606060;
  border-radius: 6px;
  display: flex;
  align-items: center;
}

.file-info {
  flex: 1;
  padding: 0 12px;
  gap: 12px;
  min-width: 0;
}
.file-path {
  color: #909090;
  font: 12px ui-monospace, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
}
.not-saved {
  color: #ffdd33; /* MARK_YELLOW */
  font: 11px ui-sans-serif, system-ui, sans-serif;
  flex-shrink: 0;
}

.masters {
  padding: 6px;
  gap: 6px;
}
.master-btn {
  appearance: none;
  font: inherit;
  background: #1c1c1c;
  color: #606060;
  border: 1.5px solid #606060;
  border-radius: 6px;
  width: 32px;
  height: 32px;
  cursor: pointer;
  font: 14px ui-sans-serif, system-ui, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
}
.master-btn:hover {
  color: #66ee88;
}
.master-btn.active {
  color: #66ee88;
  border-color: #66ee88;
}

.system {
  padding: 6px;
  gap: 6px;
}
.sys-btn {
  appearance: none;
  font: inherit;
  background: #1c1c1c;
  color: #606060;
  border: 1.5px solid #606060;
  border-radius: 6px;
  padding: 0 12px;
  height: 32px;
  cursor: pointer;
  font: 11px ui-sans-serif, system-ui, sans-serif;
}
.sys-btn:hover {
  color: #66ee88;
}
</style>
