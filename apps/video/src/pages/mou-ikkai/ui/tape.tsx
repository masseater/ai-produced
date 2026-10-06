import type { ReactNode } from "react";

import { FPS, frameOf, SIXTEENTHS_PER_BAR } from "#/shared/song";

const LOOP_BARS = 2;
const LOOP_FRAMES = frameOf(SIXTEENTHS_PER_BAR * LOOP_BARS);
const EIGHTHS = 16;
const TENTHS = 10;
const DECIMALS = 1;
const START = 0;
const SEGMENTS = Array.from({ length: EIGHTHS }, (_slot, eighth) => ({
  id: `e${String(eighth)}`,
  until: frameOf((SIXTEENTHS_PER_BAR * LOOP_BARS * eighth) / EIGHTHS),
}));

const clock = (frames: number): string =>
  `0:0${(Math.floor((frames * TENTHS) / FPS) / TENTHS).toFixed(DECIMALS)}`;

const fillOf = (until: number, played: number): string => {
  if (until < played) {
    return "bg-(--fg)";
  }
  return "bg-(--dim)";
};

type TapeProps = Readonly<{ elapsed: number }>;

const Tape = ({ elapsed }: TapeProps): ReactNode => {
  const played = Math.min(Math.max(elapsed, START), LOOP_FRAMES);
  return (
    <>
      <div className="absolute bottom-36 left-30 font-sans text-4xl font-normal text-(--fg) tabular-nums">
        {`▶ ${clock(played)} / ${clock(LOOP_FRAMES)}`}
      </div>
      <div className="absolute bottom-30 left-30 flex w-420 gap-1">
        {SEGMENTS.map(({ id, until }) => (
          <div key={id} className={`h-1 flex-1 ${fillOf(until, played)}`} />
        ))}
      </div>
    </>
  );
};

export { Tape };
