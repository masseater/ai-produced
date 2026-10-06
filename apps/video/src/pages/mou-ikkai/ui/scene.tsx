import type { ReactNode } from "react";
import { AbsoluteFill } from "remotion";

import { contentOf, hookOf, NOT_HOOK } from "#/pages/mou-ikkai/model/content";
import type { Moment } from "#/pages/mou-ikkai/model/moment";
import type { LyricsShot, PaletteName } from "#/pages/mou-ikkai/model/storyboard";
import { Body } from "#/pages/mou-ikkai/ui/body";
import { Counter } from "#/pages/mou-ikkai/ui/counter";
import { inverted, layouts, palettes, upright } from "#/pages/mou-ikkai/ui/look";
import { Tape } from "#/pages/mou-ikkai/ui/tape";

const SPEECH = "語り";

const paletteOf = (shot: LyricsShot, moment: Moment): PaletteName => {
  if (shot.treatment === "loop" && moment.line.note.startsWith(SPEECH)) {
    return "ash";
  }
  return shot.palette;
};

const polarityOf = (shot: LyricsShot, frame: number): string => {
  if (frame < shot.from + shot.lead) {
    return inverted;
  }
  return upright;
};

const layoutOf = (shot: LyricsShot, moment: Moment): string => {
  if (contentOf({ shot, moment }) === "hook") {
    return layouts[shot.hookLayout];
  }
  return layouts[shot.layout];
};

type SceneProps = Readonly<{ shot: LyricsShot; moment: Moment; frame: number }>;

const Scene = ({ shot, moment, frame }: SceneProps): ReactNode => {
  const hook = hookOf(moment);
  return (
    <AbsoluteFill className={`${palettes[paletteOf(shot, moment)]} ${polarityOf(shot, frame)}`}>
      <AbsoluteFill className={layoutOf(shot, moment)}>
        <Body shot={shot} moment={moment} />
      </AbsoluteFill>
      {hook > NOT_HOOK && <Counter hook={hook} />}
      {shot.treatment === "loop" && <Tape elapsed={frame - moment.lineFrom} />}
    </AbsoluteFill>
  );
};

export { Scene };
