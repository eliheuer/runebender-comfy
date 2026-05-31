<script setup lang="ts">
// Empty-state panel shown when no font is loaded. Mirrors
// runebender-xilem's `views/welcome.rs`: compact upper-left welcome
// UI layered over an interactive demo glyph canvas.

defineEmits<{
  (e: "openUfo"): void;
}>();
</script>

<template>
  <div class="welcome">
    <div class="title">Runebender</div>
    <div class="subtitle">A font editor</div>
    <div class="prompt">
      Drop a <code>.ufo</code> folder or <code>.designspace</code> file.
    </div>
    <button type="button" @click="$emit('openUfo')">Open UFO...</button>
  </div>
</template>

<style scoped>
.welcome {
  position: absolute;
  left: 16px;
  top: 16px;
  width: 220px;
  height: 200px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: flex-start;
  gap: 8px;
  text-align: left;
  color: var(--rb-secondary-text, #707070);
}

.title {
  font: 48px ui-sans-serif, system-ui, sans-serif;
  color: var(--rb-primary-text, #909090);
  line-height: 1;
}
.subtitle {
  font: 13px ui-sans-serif, system-ui, sans-serif;
  color: var(--rb-panel-outline, #606060);
}
.prompt {
  font: 14px ui-sans-serif, system-ui, sans-serif;
  color: var(--rb-primary-text, #909090);
  line-height: 1.4;
}

button {
  width: 200px;
  height: 32px;
  margin-top: 8px;
  color: var(--rb-primary-text, #909090);
  background: var(--rb-panel-background, #1c1c1c);
  border: var(--rb-stroke-width, 1px) solid var(--rb-panel-outline, #606060);
  border-radius: var(--rb-button-radius, 8px);
  font: 14px ui-sans-serif, system-ui, sans-serif;
  text-align: center;
  cursor: pointer;
}
button:hover {
  color: var(--rb-accent, #66ee88);
  border-color: var(--rb-accent, #66ee88);
}
code {
  font: 12px ui-monospace, monospace;
  color: var(--rb-accent, #66ee88);
  background: color-mix(in srgb, var(--rb-panel-background, #1c1c1c) 70%, transparent);
  padding: 1px 5px;
  border-radius: 3px;
}
</style>
