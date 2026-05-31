<script setup lang="ts">
// Editor-view tool palette. Mirrors runebender-xilem's
// `components/edit_mode_toolbar.rs` — same eight tools, same order,
// same identifier strings as xilem's `tools::ToolId` (shared via
// `./toolIds.ts`).
//
// The host routes each selected id into the Rust editor core and shows
// companion sub-toolbars for tools that need them.

import { TOOL_IDS, TOOL_LABELS, type ToolId } from "./toolIds";
import { TOOLBAR_ICONS, type ToolbarIcon } from "./generatedToolbarIcons";

defineProps<{
  active: ToolId;
}>();

defineEmits<{
  (e: "select", id: ToolId): void;
}>();

// Each tool's icon is a glyph in assets/runebender-icons.ufo. Design or
// edit them in Runebender, then regenerate generatedToolbarIcons.ts with
// scripts/build_toolbar_icons.py. Each icon carries its own tight
// bounding box as the viewBox, so the SVG's xMidYMid-meet scaling
// reproduces runebender-xilem's `paint_icon`: fit the larger dimension
// to the button, centered — uniform sizing regardless of the glyph's
// proportions or where it sits in the em.
const TOOL_ICON_GLYPH: Record<ToolId, string> = {
  Select: "select",
  Pen: "pen",
  HyperPen: "hyperpen",
  Knife: "knife",
  Measure: "measure",
  Shapes: "shapes",
  Preview: "preview",
  Text: "text",
};

const FALLBACK_ICON: ToolbarIcon = { viewBox: "0 0 1 1", d: "" };

function iconFor(id: ToolId): ToolbarIcon {
  return TOOLBAR_ICONS[TOOL_ICON_GLYPH[id]] ?? FALLBACK_ICON;
}
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
      <svg
        class="tool-icon"
        :viewBox="iconFor(id).viewBox"
        preserveAspectRatio="xMidYMid meet"
        fill="currentColor"
        aria-hidden="true"
      >
        <path :d="iconFor(id).d" />
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
 *   TOOLBAR_PADDING                 6 px
 *   TOOLBAR_ICON_PADDING             8 px  (icon target = 48 − 16 = 32)
 *   TOOLBAR_BUTTON_RADIUS           6 px
 *   TOOLBAR_BORDER_WIDTH            1.5 px
 */

.edit-mode-toolbar {
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
  padding: 6px;
  display: flex;
  flex-direction: row;
  gap: 6px;
  width: fit-content;
  flex-shrink: 0;
}

.tool-btn {
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
.tool-btn:hover {
  color: var(--rb-accent, #66ee88);
}
.tool-btn.active {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}

.tool-icon {
  width: 32px;
  height: 32px;
  display: block;
}
</style>
