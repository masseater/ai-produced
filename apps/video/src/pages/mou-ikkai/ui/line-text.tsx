import { Array as Arr, Option } from "effect";
import type { ReactNode } from "react";

import type { Line, Word } from "#/pages/mou-ikkai/model/song";
import type { Size, Tone } from "#/pages/mou-ikkai/ui/glyphs";
import { Glyphs } from "#/pages/mou-ikkai/ui/glyphs";

const NO_CHARS = 0;

type Fit = Readonly<{ maxChars: number; size: Size }>;

const fits: Readonly<Record<"across" | "down", readonly Fit[]>> = {
  across: [
    { maxChars: 9, size: "line-xl" },
    { maxChars: 12, size: "line-lg" },
    { maxChars: 15, size: "line-md" },
  ],
  down: [
    { maxChars: 5, size: "line-xl" },
    { maxChars: 6, size: "line-lg" },
    { maxChars: 8, size: "line-md" },
  ],
};

const directions = {
  across: "flex max-w-420 flex-wrap gap-x-12",
  down: "flex max-h-215 flex-wrap gap-y-12 [writing-mode:vertical-rl]",
} as const;

const toneOf = (word: Word, mora: number): Tone => {
  if (word.morae.includes(mora)) {
    return "now";
  }
  const first = Arr.head(word.morae).pipe(Option.getOrElse(() => mora));
  if (first < mora) {
    return "sung";
  }
  return "ahead";
};

const sizeOf = (line: Line, direction: keyof typeof directions): Size => {
  const chars = line.words.reduce((sum, word) => sum + word.text.length, NO_CHARS);
  return Arr.findFirst(fits[direction], (fit) => chars <= fit.maxChars).pipe(
    Option.map((fit) => fit.size),
    Option.getOrElse((): Size => "line-sm"),
  );
};

type LineTextProps = Readonly<{ line: Line; mora: number; direction: keyof typeof directions }>;

const LineText = ({ line, mora, direction }: LineTextProps): ReactNode => (
  <div className={directions[direction]}>
    {line.words.map((word) => (
      <Glyphs
        key={`${word.text}${word.morae.join(",")}`}
        text={word.text}
        tone={toneOf(word, mora)}
        size={sizeOf(line, direction)}
      />
    ))}
  </div>
);

export { LineText };
