<script setup lang="ts">
// Editor-view tool palette. Mirrors runebender-xilem's
// `components/edit_mode_toolbar.rs` — same eight tools, same order,
// same identifier strings as xilem's `tools::ToolId` (shared via
// `./toolIds.ts`).
//
// The host routes each selected id into the Rust editor core and shows
// companion sub-toolbars for tools that need them.

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
      <svg
        class="tool-icon"
        viewBox="0 0 24 24"
        fill="currentColor"
        aria-hidden="true"
      >
        <template v-if="id === 'Select'">
          <path d="M7 2.5 19.4 14.2c.7.7.3 1.8-.7 1.8h-4.1c-.5 0-.8.3-.6.8l2.4 4.1c.3.6.1 1.2-.5 1.5l-2.4 1.1c-.6.3-1.2 0-1.5-.6l-2-4.8c-.2-.5-.6-.6-1-.2l-2.2 2.2c-.7.7-1.8.2-1.8-.7L5 3.4c0-1 .8-1.5 2-.9Z" />
        </template>
        <template v-else-if="id === 'Pen'">
          <path d="M8.1 2.5h7.8c.6 0 .8.2.8.8v2.2c0 .7-.2.8-.8.8H8.1c-.6 0-.8-.1-.8-.8V3.3c0-.6.2-.8.8-.8Zm0 5.2h7.8c.7 0 .9.1 1.5.9l2.3 3.1c.3.4.4.7.4 1.1v1.2c0 .2-.1.7-.3 1.1l-3.9 8.3c-.3.7-.6.7-1.1.7h-1.2c-.5 0-.8-.3-.8-.9v-7c0-.6.1-.8.3-.9.7-.4 1.3-1.1 1.3-2.1 0-1.3-1.1-2.4-2.4-2.4s-2.4 1.1-2.4 2.4c0 1 .6 1.7 1.3 2.1.2.1.3.3.3.9v7c0 .6-.3.9-.8.9H9.2c-.5 0-.8 0-1.1-.7l-3.9-8.3c-.2-.4-.3-.9-.3-1.1v-1.2c0-.4.1-.7.4-1.1l2.3-3.1c.6-.8.9-.9 1.5-.9Z" />
        </template>
        <template v-else-if="id === 'HyperPen'">
          <path d="M10.2 14.1c-1.6 0-4.2-1.2-4.2-3.5 0-2.6 1.5-4.6 5.1-4.6 3.9 0 6.8 2.8 6.8 7.5 0 2.7-2.7 6.3-7.8 6.3-7 0-10.3-4-10.3-9.1C-.2 4.4 4.1 0 11.9 0c6.5 0 12.7 3.8 12.7 12.8 0 6.6-3.4 13-14.2 13-4.2 0-9.2-.9-9.2-2.9 0-1.1.7-1.7 1.8-1.7 1.6 0 2.5 1.3 7.4 1.3 7.3 0 10.8-4.1 10.8-9.8 0-4.7-4.1-9.7-9.3-9.7-6.4 0-8.7 3.9-8.7 7.6 0 3.8 3.1 6.2 7.1 6.2 3.8 0 4.4-2.1 4.4-4.4 0-2.2-1.6-3.5-3.9-3.5-1.3 0-1.9.7-1.9 1.7 0 1.3 2.7.9 2.7 2.2 0 .8-.4 1.3-1.4 1.3Z" />
        </template>
        <template v-else-if="id === 'Preview'">
          <path d="M8.1 0h7.1c.5.3.7.8.7 1.3v1.6c0 .4-.2.8-.8 1.4-2.7 2.7-4 7.4-4 13.8 0 2.3.8 3.4 1.6 3.4.5 0 .7-.3.7-.8 0-.6-.1-1.1-.1-2.4 0-.9.5-3.1.9-4 .1-.3.4-.4.6-.4s.4.1.4.4c0 .4-.4 1.7-.4 2.9 0 3.4 1 8 2.2 8s1.1-.8 1.1-1.2c0-.6-.4-1.5-.4-4.3 0-2.3.1-3.1.2-3.4.1-.3.4-.5.5-.5.2 0 .5.2.5.5 0 5 1.6 8.6 2.4 9.8.5.7 1 .9 1.7.9.4 0 .7-.4.7-1 0-.8-1.8-2.9-1.8-8 0-.9 0-1.6.1-1.8.1-.2.2-.3.4-.3s.4.1.5.3c.9 4.2 2.5 6.5 3.6 7.5.5.5.9.6 1.3.6.5 0 .6-.3.6-.9 0-.7-2.9-5.7-2.9-10.2 0-1.9 1.2-2.8 2-2.8 1 0 1.8 1.6 2.6 2.9.9 1.5 1.6 2.2 2.4 2.2.5 0 .8-.4.8-1 0-1.3-2.9-10.1-7.9-12.3-.6-.3-.9-.6-.9-1V1.3c0-.5-.2-1-.7-1.3H8.1Z" transform="translate(-4.5 -1.5) scale(.9)" />
        </template>
        <template v-else-if="id === 'Knife'">
          <path d="M22.6 11.7c.8-.8.8-2.1 0-2.9s-2.1-.8-2.9 0c-.5.5-.7 1.9-1.2 2.4l-1.4 1.4c-.3.3-.5.2-.7 0L7.1 1.6c-.2-.2-.3-.5-.5-.7C6.4.7 6.1.6 5.8.6L.9 0C.5 0 .3.1.2.2S0 .6 0 .9l.5 4.8c0 .5.2.8.6 1.1l11 9.2c.3.2.3.4 0 .7l-1.8 1.8c-.5.5-1.9.6-2.4 1.1-.8.8-.8 2.1 0 2.9s2.1.8 2.9 0c.5-.5.7-1.9 1.2-2.4l1.9-1.9c.3-.3.5-.3.8 0l3.2 3.2c.7.7 2.1 0 3-1s1.7-2.3 1-3l-3.2-3.2c-.3-.3-.3-.5 0-.8l1.4-1.4c.5-.5 1.9-.7 2.4-1.2Z" />
        </template>
        <template v-else-if="id === 'Measure'">
          <path d="M17.1 1.3 23 7.2c.5.5.5 1 0 1.5L8.7 23c-.5.5-1 .5-1.5 0L1.3 17.1c-.5-.5-.5-1 0-1.5L15.6 1.3c.5-.5 1-.5 1.5 0Zm-12 14.3-1.3 1.3 3.2 3.2 1.3-1.3-3.2-3.2Zm2.8-2.8-1.3 1.3 4.8 4.8 1.3-1.3-4.8-4.8Zm2.8-2.8-1.3 1.3 3.2 3.2 1.3-1.3-3.2-3.2Zm2.8-2.8-1.3 1.3 4.8 4.8 1.3-1.3-4.8-4.8Zm2.8-2.8L15 5.7l3.2 3.2 1.3-1.3-3.2-3.2Z" />
        </template>
        <template v-else-if="id === 'Shapes'">
          <path d="M1.4 1.4h12.4c.8 0 1.4.6 1.4 1.4v3.7h1.4c4.1 0 7.4 3.3 7.4 7.4s-3.3 7.4-7.4 7.4c-3.8 0-6.9-2.8-7.4-6.5H2.8c-.8 0-1.4-.6-1.4-1.4V1.4Zm3.3 3.3v6.8h4.8c.9-2.7 3.3-4.7 6.2-5V4.7H4.7Zm11.9 5.1a4.1 4.1 0 1 0 0 8.2 4.1 4.1 0 0 0 0-8.2Z" />
        </template>
        <template v-else-if="id === 'Text'">
          <path d="M2.1 2h19.8c.7 0 1.1.4 1.1 1.1v5.8c0 .5-.4.9-.9.9h-.4c-.6 0-.9-.4-1.1-.9-.7-1.8-1.3-3.1-3-3.1h-1.2c-1.4 0-1.8.5-1.8 2v9.3c0 2.8.6 4.4 3.7 4.4.7 0 .9.2.9.9v.9c0 .6-.2.8-.9.8H5.7c-.7 0-.9-.2-.9-.8v-.9c0-.7.2-.9.9-.9 3.1 0 3.7-1.6 3.7-4.4V7.8c0-1.5-.4-2-1.8-2H6.4c-1.7 0-2.3 1.3-3 3.1-.2.5-.5.9-1.1.9h-.4c-.5 0-.9-.4-.9-.9V3.1C1 2.4 1.4 2 2.1 2Z" />
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
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 6px;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 64px;
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
