<script setup lang="ts">
// Editor-view tool palette. Mirrors runebender-xilem's
// `components/edit_mode_toolbar.rs` — same eight tools, same order,
// same identifier strings as xilem's `tools::ToolId` (shared via
// `./toolIds.ts`).
//
// Only Select is wired up in the editor; the other tools set the
// active id but the editor currently ignores them. Visual presence
// first; functionality lands per-tool later.

import { TOOL_IDS, TOOL_LABELS, type ToolId } from "./toolIds";

defineProps<{
  active: ToolId;
}>();

defineEmits<{
  (e: "select", id: ToolId): void;
}>();
</script>

<template>
  <div class="edit-mode-toolbar">
    <button
      v-for="id in TOOL_IDS"
      :key="id"
      type="button"
      class="tool-btn"
      :class="{ active: id === active }"
      :title="TOOL_LABELS[id]"
      :aria-label="TOOL_LABELS[id]"
      @click="$emit('select', id)"
    >
      <!-- Simple, recognisable glyphs per tool. Not pixel-matching
           xilem's exact icons; can be polished later. -->
      <svg
        class="tool-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <template v-if="id === 'Select'">
          <path d="M5 3 L5 19 L9 15 L11 20 L14 19 L12 14 L18 14 Z" />
        </template>
        <template v-else-if="id === 'Pen'">
          <path d="M16 4 L20 8 L9 19 L4 20 L5 15 Z" />
          <line x1="14" y1="6" x2="18" y2="10" />
        </template>
        <template v-else-if="id === 'HyperPen'">
          <path d="M16 4 L20 8 L9 19 L4 20 L5 15 Z" />
          <path d="M9 12 q 1.5 -2 3 0 t 3 0" />
        </template>
        <template v-else-if="id === 'Preview'">
          <path d="M2 12 c 3 -6 8 -6 10 -6 s 7 0 10 6 c -3 6 -8 6 -10 6 s -7 0 -10 -6 Z" />
          <circle cx="12" cy="12" r="3" />
        </template>
        <template v-else-if="id === 'Knife'">
          <path d="M19 4 L21 6 L9 18 L7 18 L7 16 Z" />
          <line x1="6" y1="20" x2="11" y2="15" />
        </template>
        <template v-else-if="id === 'Measure'">
          <rect x="3" y="9" width="18" height="6" rx="1" />
          <line x1="7" y1="9" x2="7" y2="12" />
          <line x1="11" y1="9" x2="11" y2="13" />
          <line x1="15" y1="9" x2="15" y2="12" />
          <line x1="19" y1="9" x2="19" y2="13" />
        </template>
        <template v-else-if="id === 'Shapes'">
          <rect x="3" y="3" width="9" height="9" />
          <circle cx="16" cy="16" r="5" />
        </template>
        <template v-else-if="id === 'Text'">
          <line x1="6" y1="6" x2="18" y2="6" />
          <line x1="12" y1="6" x2="12" y2="20" />
          <line x1="9" y1="20" x2="15" y2="20" />
        </template>
      </svg>
    </button>
  </div>
</template>

<style scoped>
/*
 * Colors / sizes from xilem theme.rs:
 *   PANEL_BACKGROUND               #1C1C1C
 *   TOOLBAR_BUTTON_OUTLINE / BASE_F #606060
 *   TOOLBAR_ICON_UNSELECTED         #606060
 *   TOOLBAR_ICON_HOVERED            #66EE88
 *   TOOLBAR_ICON_SELECTED           #66EE88
 *   TOOLBAR_ITEM_SIZE               48 px
 *   TOOLBAR_ITEM_SPACING            6 px
 *   TOOLBAR_PADDING                 8 px
 *   TOOLBAR_BUTTON_RADIUS           6 px
 *   TOOLBAR_BORDER_WIDTH            1.5 px
 */

.edit-mode-toolbar {
  background: #1c1c1c;
  border: 1.5px solid #606060;
  border-radius: 6px;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 48px;
  flex-shrink: 0;
}

.tool-btn {
  appearance: none;
  font: inherit;
  width: 32px;
  height: 32px;
  background: #1c1c1c;
  color: #606060;
  border: 1.5px solid #606060;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}
.tool-btn:hover {
  color: #66ee88;
}
.tool-btn.active {
  color: #66ee88;
  border-color: #66ee88;
}

.tool-icon {
  width: 20px;
  height: 20px;
  display: block;
}
</style>
