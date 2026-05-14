<script setup lang="ts">
// Right-side info panel. Mirrors runebender-xilem's
// `components/glyph_info_panel.rs` field-for-field:
//   Master           — current master name
//   Glyph Name       — UFO glyph identifier
//   Width            — advance width in design units
//   Kerning Groups   — Left / Right (stubbed "(empty)" until
//                      groups.plist plumbing lands)
//   Unicode          — first codepoint as 4-digit hex
//   Contours         — number of contours in the active glyph
//
// Label rows are green; values are gray. Empty / unset values are
// shown as a dim "(empty)" or em-dash.

defineProps<{
  master: string;
  /** Empty when no glyph is selected. */
  name: string;
  /** Uppercase hex, no "U+" prefix. */
  unicode?: string;
  /** Design units. -1 means "no glyph open" (sidebar shows em-dash). */
  width?: number;
  contours?: number;
  /** Kerning group names. Stubbed until groups.plist parsing arrives. */
  leftGroup?: string;
  rightGroup?: string;
}>();
</script>

<template>
  <aside class="info-sidebar">
    <div class="row">
      <div class="label">Master</div>
      <div class="value">{{ master || "—" }}</div>
    </div>

    <div class="row">
      <div class="label">Glyph Name</div>
      <div class="value">{{ name || "—" }}</div>
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
          <span class="kerning-val">{{ leftGroup || "(empty)" }}</span>
        </div>
        <div class="kerning-row">
          <span class="kerning-side">Right</span>
          <span class="kerning-val">{{ rightGroup || "(empty)" }}</span>
        </div>
      </div>
    </div>

    <div class="row">
      <div class="label">Unicode</div>
      <div class="value mono">{{ unicode || "—" }}</div>
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
 *   GRID_CELL_SELECTED_OUTLINE     #66EE88 (used for labels)
 *
 * Width matches xilem's GLYPH_INFO_PANEL_WIDTH (240px).
 */

.info-sidebar {
  width: 240px;
  flex-shrink: 0;
  background: #1c1c1c;
  border: 1.5px solid #606060;
  border-radius: 6px;
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
  color: #66ee88;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-weight: 500;
}
.value {
  color: #909090;
  font: 13px ui-sans-serif, system-ui, sans-serif;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.value.mono {
  font-family: ui-monospace, monospace;
  font-size: 12px;
}

.kerning-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}
.kerning-side {
  color: #707070;
  font: 12px ui-sans-serif, system-ui, sans-serif;
}
.kerning-val {
  color: #909090;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-style: italic;
}
</style>
