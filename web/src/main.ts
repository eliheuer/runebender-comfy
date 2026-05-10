// Standalone dev entry — `pnpm dev` mounts the Runebender widget
// directly into index.html so we can test outside of ComfyUI. The
// production build (vite build) goes through extension.ts instead.

import { createApp } from "vue";
import Runebender from "./Runebender.vue";

createApp(Runebender).mount("#app");
