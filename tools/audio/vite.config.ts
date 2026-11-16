import { cargoTask } from "@workspace/crate-tasks/tasks";
import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      verify: { command: [], dependsOn: ["cargo"] },
      cargo: cargoTask({ crate: "audio", root: `${import.meta.dirname}/../..` }),
    },
  },
});
