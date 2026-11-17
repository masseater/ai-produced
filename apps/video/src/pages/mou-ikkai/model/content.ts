import { Array as Arr, Option, pipe } from "effect";

import type { Moment } from "#/pages/mou-ikkai/model/moment";
import type { Line } from "#/pages/mou-ikkai/model/song";
import type { LyricsShot } from "#/pages/mou-ikkai/model/storyboard";

export type Content = "hook" | "chop" | "line";

export const NOT_HOOK = 0;

export const hookOf = (moment: Moment): number =>
  Arr.get(moment.line.morae, moment.mora).pipe(
    Option.map((mora) => mora.hook),
    Option.getOrElse(() => NOT_HOOK),
  );

const isHookLine = (line: Line): boolean =>
  pipe(
    line.morae,
    Arr.every((mora) => mora.hook > NOT_HOOK),
  );

const showsHook = (shot: LyricsShot, moment: Moment): boolean =>
  hookOf(moment) > NOT_HOOK && (shot.treatment === "chop" || isHookLine(moment.line));

export const contentOf = ({
  shot,
  moment,
}: Readonly<{ shot: LyricsShot; moment: Moment }>): Content => {
  if (showsHook(shot, moment)) {
    return "hook";
  }
  if (shot.treatment === "chop") {
    return "chop";
  }
  return "line";
};

export const sungInHook = (moment: Moment): number => {
  const hook = hookOf(moment);
  return pipe(
    moment.line.morae,
    Arr.filter((mora, index) => mora.hook === hook && index <= moment.mora),
    Arr.length,
  );
};

export const moraText = (moment: Moment): string =>
  Arr.get(moment.line.morae, moment.mora).pipe(
    Option.map((mora) => mora.text),
    Option.getOrElse(() => ""),
  );
