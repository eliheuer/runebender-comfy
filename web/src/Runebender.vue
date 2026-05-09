<script setup lang="ts">
import { onMounted, ref } from "vue";

const canvas = ref<HTMLCanvasElement | null>(null);

onMounted(async () => {
  // TODO: import init, GlyphEditor from "/wasm/runebender_comfy_core.js"
  // and render into canvas.value via Vello.
});

function enterFullscreen() {
  canvas.value?.requestFullscreen?.();
}
</script>

<template>
  <div class="runebender-host">
    <canvas ref="canvas" class="runebender-canvas" />
    <button class="fs-btn" @click="enterFullscreen">Full screen</button>
  </div>
</template>

<style scoped>
.runebender-host {
  position: relative;
  width: 100%;
  height: 100%;
  background: #1f1a14; /* warm dark — matches campfire palette */
}
.runebender-canvas {
  width: 100%;
  height: 100%;
  display: block;
}
.fs-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 4px 8px;
  background: #3a2f24;
  color: #f0e6d2;
  border: 1px solid #5a4a38;
  border-radius: 4px;
  cursor: pointer;
}
</style>
