import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      dev: { command: "alchemy dev", cache: false },
      deploy: { command: "alchemy deploy", cache: false },
      destroy: { command: "alchemy destroy", cache: false },
    },
  },
  resolve: { tsconfigPaths: true },
  plugins: [
    tailwindcss(),
    tanstackStart({
      srcDirectory: "src/app",
      router: { generatedRouteTree: "generated/routeTree.gen.ts" },
    }),
    viteReact(),
  ],
});
