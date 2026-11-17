import { defineConfig } from "vite-plus";

export default defineConfig({
  msch: {
    lint: {
      overrides: [
        {
          files: ["src/shared/ui/**"],
          rules: { "shadcn/no-restyle": "off", "react/jsx-props-no-spreading": "off" },
        },
      ],
    },
  },
});
