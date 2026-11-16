import { defineConfig } from "vite-plus";

export default defineConfig({
  msch: {
    lint: {
      overrides: [
        {
          files: ["src/components/**"],
          rules: { "shadcn/no-restyle": "off", "react/jsx-props-no-spreading": "off" },
        },
      ],
    },
  },
});
