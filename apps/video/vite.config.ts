import { defineConfig } from "vite-plus";

export default defineConfig({
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
        "uv run --project ../../tools/lyric-timing ../../tools/lyric-timing/lyric_timing.py ../../songs/001-mou-ikkai/02-composition/score.toml ../../songs/001-mou-ikkai/02-composition/generated/vocal.tsv src/pages/mou-ikkai/generated/mou-ikkai.json",
    },
  },
});
