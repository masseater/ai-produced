import type { ReactNode } from "react";

import type { Size, Tone } from "#/pages/mou-ikkai/ui/glyphs";
import { Glyphs } from "#/pages/mou-ikkai/ui/glyphs";

const HOOK = [
  { id: "mo", char: "も" },
  { id: "u", char: "う" },
  { id: "i1", char: "い" },
  { id: "q", char: "っ" },
  { id: "ka", char: "か" },
  { id: "i2", char: "い" },
] as const;

const NOW = 1;

const toneOf = (position: number, sung: number): Tone => {
  if (position < sung - NOW) {
    return "sung";
  }
  if (position === sung - NOW) {
    return "now";
  }
  return "ahead";
};

type HookRowProps = Readonly<{ sung: number; size: Size }>;

const HookRow = ({ sung, size }: HookRowProps): ReactNode => (
  <div className="flex">
    {HOOK.map(({ id, char }, position) => (
      <Glyphs key={id} text={char} tone={toneOf(position, sung)} size={size} />
    ))}
  </div>
);

export { HookRow };
