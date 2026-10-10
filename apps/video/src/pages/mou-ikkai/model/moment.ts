import { Array as Arr, Option } from "effect";

import { DURATION_IN_FRAMES, frameOf, song } from "#/pages/mou-ikkai/model/song";
import type { Line } from "#/pages/mou-ikkai/model/song";

type Moment = Readonly<{
  line: Line;
  mora: number;
  lineFrom: number;
  sounding: boolean;
}>;

type Entry = Readonly<{
  line: Line;
  mora: number;
  lineFrom: number;
  from: number;
  to: number;
}>;

const NEXT = 1;

const entries: readonly Entry[] = song.lines.flatMap((line) => {
  const lineFrom = Arr.head(line.morae).pipe(
    Option.map((mora) => frameOf(mora.start)),
    Option.getOrElse(() => DURATION_IN_FRAMES),
  );
  return line.morae.map((mora, index) => ({
    line,
    mora: index,
    lineFrom,
    from: frameOf(mora.start),
    to: frameOf(mora.start + mora.length),
  }));
});

const entryByFrame: readonly Entry[] = entries.flatMap((entry, index) => {
  const next = Arr.get(entries, index + NEXT).pipe(
    Option.map((following) => following.from),
    Option.getOrElse(() => DURATION_IN_FRAMES),
  );
  return Array.from({ length: next - entry.from }, () => entry);
});

const firstFrom = Arr.head(entries).pipe(
  Option.map((entry) => entry.from),
  Option.getOrElse(() => DURATION_IN_FRAMES),
);

const momentAt = (frame: number): Option.Option<Moment> =>
  Arr.get(entryByFrame, frame - firstFrom).pipe(
    Option.map((entry) => ({
      line: entry.line,
      mora: entry.mora,
      lineFrom: entry.lineFrom,
      sounding: frame < entry.to,
    })),
  );

export { momentAt };
export type { Moment };
