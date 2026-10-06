import { Audio } from "@remotion/media";
import { Option } from "effect";
import type { ReactNode } from "react";
import { AbsoluteFill, staticFile, useCurrentFrame } from "remotion";

import { momentAt } from "#/pages/mou-ikkai/model/moment";
import { shotAt } from "#/pages/mou-ikkai/model/storyboard";
import { Scene } from "#/pages/mou-ikkai/ui/scene";

type MouIkkaiProps = Readonly<{ audio: string }>;

const MouIkkai = ({ audio }: MouIkkaiProps): ReactNode => {
  const frame = useCurrentFrame();
  const shot = shotAt(frame);
  const moment = momentAt(frame);
  return (
    <AbsoluteFill className="bg-void">
      <Audio src={staticFile(audio)} />
      {shot.kind === "lyrics" && Option.isSome(moment) && (
        <Scene shot={shot} moment={moment.value} frame={frame} />
      )}
    </AbsoluteFill>
  );
};

export { MouIkkai };
export type { MouIkkaiProps };
