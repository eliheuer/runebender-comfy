<script setup lang="ts">
// Right-side info panel. Mirrors runebender-xilem's
// `components/glyph_info_panel.rs` field-for-field:
//   Master           — current master name
//   Glyph Name       — UFO glyph identifier
//   Width            — advance width in design units
//   Kerning Groups   — Left / Right from groups.plist
//   Unicode          — first codepoint as 4-digit hex
//   Contours         — number of contours in the active glyph
//
// Label rows are green; values are gray. Empty kerning groups show
// "(empty)" and empty glyph fields show "No Selection", matching xilem.

defineProps<{
  master: string;
  /** Empty when no glyph is selected. */
  name: string;
  /** Uppercase hex, no "U+" prefix. */
  unicode?: string;
  /** Design units. -1 means "no glyph open" (sidebar shows em-dash). */
  width?: number;
  contours?: number;
  /** Full UFO kerning group names, e.g. "public.kern1.O". */
  leftGroup?: string;
  rightGroup?: string;
}>();

function displayGroup(group: string | undefined, prefix: string): string {
  return group ? group.replace(prefix, "") : "(empty)";
}
</script>

<template>
  <aside class="info-sidebar">
    <div class="row">
      <div class="label">Master</div>
      <div class="value">{{ master || "(single UFO)" }}</div>
    </div>

    <div class="row">
      <div class="label">Glyph Name</div>
      <div class="value">{{ name || "No Selection" }}</div>
    </div>

    <div class="row">
      <div class="label">Width</div>
      <div class="value mono">
        {{ width !== undefined && width >= 0 ? Math.round(width) : "—" }}
      </div>
    </div>

    <div class="row group">
      <div class="label">Kerning Groups</div>
      <div class="kerning">
        <div class="kerning-row">
          <span class="kerning-side">Left</span>
          <span class="kerning-val">
            {{ displayGroup(leftGroup, "public.kern1.") }}
          </span>
        </div>
        <div class="kerning-row">
          <span class="kerning-side">Right</span>
          <span class="kerning-val">
            {{ displayGroup(rightGroup, "public.kern2.") }}
          </span>
        </div>
      </div>
    </div>

    <div class="row">
      <div class="label">Unicode</div>
      <div class="value mono">{{ unicode || "No Selection" }}</div>
    </div>

    <div class="row">
      <div class="label">Contours</div>
      <div class="value mono">
        {{ contours !== undefined ? contours : "—" }}
      </div>
    </div>
  </aside>
</template>

<style scoped>
/*
 * Colors from xilem/src/theme.rs:
 *   PANEL_BACKGROUND               #1C1C1C
 *   PANEL_OUTLINE / BASE_F         #606060
 *   PRIMARY_UI_TEXT / BASE_I       #909090
 *   SECONDARY_UI_TEXT / BASE_G     #707070
 *   GRID_CELL_SELECTED_OUTLINE     #18B86F (used for labels)
 *
 * Width matches xilem's GLYPH_INFO_PANEL_WIDTH (220px).
 */

.info-sidebar {
  width: 220px;
  flex-shrink: 0;
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-panel-radius, 12px);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
}

.row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.row.group .kerning {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.label {
  color: var(--rb-accent, #18b86f);
  font: 16px ui-sans-serif, system-ui, sans-serif;
}
.value {
  color: var(--rb-primary-text, #909090);
  font: 16px ui-sans-serif, system-ui, sans-serif;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.value.mono {
  font: 16px ui-sans-serif, system-ui, sans-serif;
}

.kerning-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}
.kerning-side {
  color: var(--rb-secondary-text, #707070);
  font: 16px ui-sans-serif, system-ui, sans-serif;
}
.kerning-val {
  color: var(--rb-primary-text, #909090);
  font: 16px ui-sans-serif, system-ui, sans-serif;
}
</style>
