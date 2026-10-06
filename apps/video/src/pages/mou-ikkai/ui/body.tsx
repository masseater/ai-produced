import { Match } from "effect";
import type { ReactNode } from "react";
import { random } from "remotion";

import { contentOf, moraText, sungInHook } from "#/pages/mou-ikkai/model/content";
import type { Moment } from "#/pages/mou-ikkai/model/moment";
import type { LyricsShot } from "#/pages/mou-ikkai/model/storyboard";
import { Glyphs } from "#/pages/mou-ikkai/ui/glyphs";
import { HookCard } from "#/pages/mou-ikkai/ui/hook-card";
import { LineText } from "#/pages/mou-ikkai/ui/line-text";

const MASK = "■";
const MASKED_SHARE = 0.3;
const MASKED_SECTION = "Break";

const chopText = (shot: LyricsShot, moment: Moment): string => {
  if (
    shot.section === MASKED_SECTION &&
    random(`${shot.section}-${String(shot.ordinal)}`) < MASKED_SHARE
  ) {
    return MASK;
  }
  return moraText(moment);
};

const directionOf = (shot: LyricsShot): "across" | "down" => {
  if (shot.layout === "vertical") {
    return "down";
  }
  return "across";
};

type BodyProps = Readonly<{ shot: LyricsShot; moment: Moment }>;

const Body = ({ shot, moment }: BodyProps): ReactNode =>
  Match.value(contentOf({ shot, moment })).pipe(
    Match.when("hook", () => (
      <HookCard sung={sungInHook(moment)} tiled={shot.hookLayout === "tile"} />
    )),
    Match.when("chop", () => <Glyphs text={chopText(shot, moment)} tone="sung" size="chop" />),
    Match.when("line", () => (
      <LineText line={moment.line} mora={moment.mora} direction={directionOf(shot)} />
    )),
    Match.exhaustive,
  );

export { Body };
