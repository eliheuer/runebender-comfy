// ComfyUI extension entry — registers the Runebender full-screen widget.
//
// ComfyUI exposes its app singleton at /scripts/app.js. We attach our
// widget to the Runebender node class on register.

// @ts-expect-error — provided by ComfyUI host at runtime.
import { app } from "/scripts/app.js";

import { createApp } from "vue";
import Runebender from "./Runebender.vue";

app.registerExtension({
  name: "runebender-comfy-nodes.Runebender",
  async beforeRegisterNodeDef(nodeType: any, nodeData: any) {
    if (nodeData.name !== "Runebender") return;

    const onCreated = nodeType.prototype.onNodeCreated;
    nodeType.prototype.onNodeCreated = function () {
      onCreated?.apply(this, arguments);
      const container = document.createElement("div");
      container.style.width = "100%";
      container.style.height = "100%";
      this.addDOMWidget("runebender-canvas", "div", container, {});
      createApp(Runebender).mount(container);
    };
  },
});
