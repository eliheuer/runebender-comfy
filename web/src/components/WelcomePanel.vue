<script setup lang="ts">
// Empty-state panel shown when no font is loaded. Mirrors
// runebender-xilem's `views/welcome.rs` — a centered prompt with the
// drop affordance. In dev the auto-loader usually means you never
// see this, but it's the first impression for new users and for any
// future production deploy.
</script>

<template>
  <div class="welcome">
    <div class="title">Runebender</div>
    <div class="subtitle">A font editor</div>
    <div class="prompt">
      Drop a <code>.ufo</code> folder or <code>.designspace</code> file
      anywhere in this window to begin.
    </div>
    <div class="note">
      On macOS, if a font tool is installed the picker may treat
      <code>.ufo</code> as a single file you can't enter — drag-and-drop
      from Finder bypasses that.
    </div>
  </div>
</template>

<style scoped>
.welcome {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  pointer-events: none;
  text-align: center;
  padding: 32px;
  color: #707070;
}

.title {
  font: 28px ui-sans-serif, system-ui, sans-serif;
  color: #909090;
  letter-spacing: 0.02em;
}
.subtitle {
  font: 13px ui-sans-serif, system-ui, sans-serif;
  color: #606060;
  margin-bottom: 14px;
}
.prompt {
  font: 14px ui-sans-serif, system-ui, sans-serif;
  color: #909090;
  max-width: 520px;
  line-height: 1.55;
}
.note {
  margin-top: 14px;
  font: 12px ui-sans-serif, system-ui, sans-serif;
  color: #606060;
  max-width: 520px;
  line-height: 1.55;
}
code {
  font: 12px ui-monospace, monospace;
  color: #66ee88;
  background: rgba(28, 28, 28, 0.7);
  padding: 1px 5px;
  border-radius: 3px;
}
</style>
