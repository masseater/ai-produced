import type { ReactNode } from "react";

type Tone = "sung" | "now" | "ahead";
type Size = "chop" | "card" | "tile" | "line-xl" | "line-lg" | "line-md" | "line-sm";

const tones: Readonly<Record<Tone, string>> = {
  sung: "text-(--fg)",
  now: "text-(--accent)",
  ahead: "text-(--dim)",
};

const sizes: Readonly<Record<Size, string>> = {
  chop: "text-chop",
  card: "text-card",
  tile: "text-tile",
  "line-xl": "text-line-xl",
  "line-lg": "text-line-lg",
  "line-md": "text-line-md",
  "line-sm": "text-line-sm",
};

type GlyphsProps = Readonly<{ text: string; tone: Tone; size: Size }>;

const Glyphs = ({ text, tone, size }: GlyphsProps): ReactNode => (
  <span
    className={`font-sans leading-tight font-black whitespace-pre ${sizes[size]} ${tones[tone]}`}
  >
    {text}
  </span>
);

export { Glyphs };
export type { Size, Tone };
