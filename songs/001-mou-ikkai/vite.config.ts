import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      vocal: { command: "node bin/vocal.ts", cache: false },
      audition: { command: "node bin/audition.ts", cache: false },
      mix: { command: "node bin/mix.ts", cache: false },
      video: { command: [], dependsOn: ["video#render"], cache: false },
    },
  },
});
