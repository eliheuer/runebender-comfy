// Standalone dev entry — `pnpm dev` mounts the Runebender widget
// directly into index.html so we can test outside of ComfyUI. The
// production build (vite build) goes through extension.ts instead.

import { createApp } from "vue";
import { readDevTestFontFiles } from "./devTestFont";
import { runebenderHostKey } from "./host/runebenderHost";
import { browserHost } from "./hosts/browser/browserHost";
import Runebender from "./Runebender.vue";

createApp(Runebender, {
  initialFiles: readDevTestFontFiles,
})
  .provide(runebenderHostKey, browserHost)
  .mount("#app");
