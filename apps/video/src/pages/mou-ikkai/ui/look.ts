import { loadFont } from "@remotion/google-fonts/NotoSansJP";

import type { LayoutName, PaletteName } from "#/pages/mou-ikkai/model/storyboard";

loadFont("normal", { weights: ["400", "900"], subsets: ["japanese", "latin"] });

const palettes: Readonly<Record<PaletteName, string>> = {
  ink: "[--base-bg:#0b0b0b] [--base-fg:#f4f4f0] [--dim:#2a2a2a] [--accent:#ff3b2f]",
  paper: "[--base-bg:#f4f4f0] [--base-fg:#0b0b0b] [--dim:#d4d4ce] [--accent:#ff3b2f]",
  rec: "[--base-bg:#ff3b2f] [--base-fg:#0b0b0b] [--dim:#d03027] [--accent:#f4f4f0]",
  signal: "[--base-bg:#ffe14d] [--base-fg:#0b0b0b] [--dim:#d8be40] [--accent:#ff3b2f]",
  ash: "[--base-bg:#6e6e6e] [--base-fg:#f4f4f0] [--dim:#5c5c5c] [--accent:#0b0b0b]",
};

const upright = "[--bg:var(--base-bg)] [--fg:var(--base-fg)] bg-(--bg)";
const inverted = "[--bg:var(--base-fg)] [--fg:var(--base-bg)] bg-(--bg)";

const layouts: Readonly<Record<LayoutName, string>> = {
  center: "items-center justify-center",
  topLeft: "items-start justify-start p-30",
  bottomRight: "items-end justify-end p-30",
  vertical: "items-end justify-center p-30",
  tile: "items-center justify-center",
};

export { inverted, layouts, palettes, upright };
