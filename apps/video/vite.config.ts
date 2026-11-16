import { defineConfig } from "vite-plus";

export default defineConfig({
  msch: {
    lint: {
      overrides: [
        { files: ["**"], use: ["remotion"] },
        {
          files: ["src/app/root.tsx"],
          rules: { "import/no-unassigned-import": ["error", { allow: ["**/*.css"] }] },
        },
      ],
    },
  },
  run: {
    tasks: {
      studio: {
        command:
          "remotion studio src/app/index.ts --public-dir ../../songs/001-mou-ikkai/04-reaper/render",
        cache: false,
      },
      render: {
        command:
          "remotion render src/app/index.ts MouIkkai --public-dir ../../songs/001-mou-ikkai/04-reaper/render",
        cache: false,
      },
      timing:
        "cargo run -q -p song --bin lyric-timing -- ../../songs/001-mou-ikkai/02-composition/score.toml ../../songs/001-mou-ikkai/02-composition/generated/vocal.tsv src/pages/mou-ikkai/generated/mou-ikkai.json",
    },
  },
});
