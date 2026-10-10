import { Array as Arr, Option } from "effect";

import type { Moment } from "#/pages/mou-ikkai/model/moment";
import type { Line } from "#/pages/mou-ikkai/model/song";
import type { LyricsShot } from "#/pages/mou-ikkai/model/storyboard";

type Content = "hook" | "chop" | "line";

const NOT_HOOK = 0;

const hookOf = (moment: Moment): number =>
  Arr.get(moment.line.morae, moment.mora).pipe(
    Option.map((mora) => mora.hook),
    Option.getOrElse(() => NOT_HOOK),
  );

const isHookLine = (line: Line): boolean => line.morae.every((mora) => mora.hook > NOT_HOOK);

const showsHook = (shot: LyricsShot, moment: Moment): boolean =>
  hookOf(moment) > NOT_HOOK && (shot.treatment === "chop" || isHookLine(moment.line));

const contentOf = ({ shot, moment }: Readonly<{ shot: LyricsShot; moment: Moment }>): Content => {
  if (showsHook(shot, moment)) {
    return "hook";
  }
  if (shot.treatment === "chop") {
    return "chop";
  }
  return "line";
};

const sungInHook = (moment: Moment): number => {
  const hook = hookOf(moment);
  return moment.line.morae.filter((mora, index) => mora.hook === hook && index <= moment.mora)
    .length;
};

const moraText = (moment: Moment): string =>
  Arr.get(moment.line.morae, moment.mora).pipe(
    Option.map((mora) => mora.text),
    Option.getOrElse(() => ""),
  );

export { contentOf, hookOf, moraText, NOT_HOOK, sungInHook };
export type { Content };
