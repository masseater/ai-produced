import { Array as Arr, Option, Order, pipe } from "effect";

import {
  CLOSURE,
  FIRST,
  LEAD_FRAMES,
  NEXT,
  NO_LEAD,
  planOf,
  silenceLengths,
} from "#/pages/mou-ikkai/model/plan";
import type { LayoutName, PaletteName, Plan, Treatment } from "#/pages/mou-ikkai/model/plan";
import {
  DURATION_IN_FRAMES,
  frameOf,
  SIXTEENTHS_PER_BAR,
  song,
} from "#/pages/mou-ikkai/model/song";
import type { Line, Section } from "#/pages/mou-ikkai/model/song";

type LyricsShot = Readonly<{
  kind: "lyrics";
  from: number;
  to: number;
  section: string;
  treatment: Treatment;
  palette: PaletteName;
  layout: LayoutName;
  hookLayout: LayoutName;
  lead: number;
  ordinal: number;
}>;

type SilenceShot = Readonly<{ kind: "silence"; from: number; to: number; section: string }>;

type Shot = LyricsShot | SilenceShot;

type Span = Readonly<{
  section: Section;
  plan: Plan;
  origin: number;
  quiet: number;
  length: number;
  lines: readonly Line[];
  lineStarts: readonly number[];
}>;

const pick = <Item>(items: readonly Item[], index: number): Item =>
  Arr.get(items, index % items.length).pipe(Option.getOrThrow);

const firstStart = (line: Line, fallback: number): number =>
  Arr.head(line.morae).pipe(
    Option.map((mora) => mora.start),
    Option.getOrElse(() => fallback),
  );

const spanOf = (section: Section): Span => {
  const plan = planOf(section);
  const origin = section.startBar * SIXTEENTHS_PER_BAR;
  const length = section.bars * SIXTEENTHS_PER_BAR;
  const lines = pipe(
    song.lines,
    Arr.filter((line) => line.section === section.name),
  );
  return {
    section,
    plan,
    origin,
    length,
    quiet: length - silenceLengths[plan.silence],
    lines,
    lineStarts: pipe(
      lines,
      Arr.map((line) => firstStart(line, origin) - origin),
    ),
  };
};

const gridCuts = (plan: Plan, end: number): readonly number[] =>
  pipe(
    plan.grid,
    Arr.flatMap(([fromBar, step], index) => {
      const start = fromBar * SIXTEENTHS_PER_BAR;
      const stop = Arr.get(plan.grid, index + NEXT).pipe(
        Option.map(([nextBar]) => nextBar * SIXTEENTHS_PER_BAR),
        Option.getOrElse(() => end),
      );
      return Array.from(
        { length: Math.ceil((stop - start) / step) },
        (_slot, count) => start + count * step,
      );
    }),
  );

const voiceCuts = (span: Span): readonly number[] =>
  pipe(
    span.lines,
    Arr.flatMap((line) =>
      pipe(
        line.morae,
        Arr.filter((mora) => mora.kind !== CLOSURE),
        Arr.map((mora) => mora.start - span.origin),
      ),
    ),
  );

const optional = (enabled: boolean, cuts: readonly number[]): readonly number[] => {
  if (enabled) {
    return cuts;
  }
  return [];
};

const cutsOf = (span: Span): readonly number[] => {
  const lineCuts = optional(span.plan.lineCuts, span.lineStarts);
  const sungCuts = optional(span.plan.voiceCuts, voiceCuts(span));
  const candidates = Arr.dedupe([
    FIRST,
    ...gridCuts(span.plan, span.quiet),
    ...lineCuts,
    ...sungCuts,
  ]);
  return Arr.sort(
    pipe(
      candidates,
      Arr.filter((cut) => cut >= FIRST && cut < span.quiet),
    ),
    Order.Number,
  );
};

const layoutIndexOf = (span: Span, cut: number, ordinal: number): number =>
  ({
    cut: ordinal,
    bar: Math.floor(cut / SIXTEENTHS_PER_BAR),
    line: pipe(
      span.lineStarts,
      Arr.filter((start) => start <= cut),
      Arr.length,
    ),
  })[span.plan.layoutBy];

const leadOf = (plan: Plan, cut: number): number => {
  if (plan.leadEvery > NO_LEAD && cut > FIRST && cut % plan.leadEvery === FIRST) {
    return LEAD_FRAMES;
  }
  return NO_LEAD;
};

const hookLayoutOf = (plan: Plan, cut: number): LayoutName => {
  if (cut >= plan.tileFromBar * SIXTEENTHS_PER_BAR) {
    return "tile";
  }
  return "center";
};

const lyricsShots = (span: Span): readonly LyricsShot[] => {
  const cuts = cutsOf(span);
  const { plan, origin, section } = span;
  return pipe(
    cuts,
    Arr.map((cut, ordinal) => ({
      kind: "lyrics",
      from: frameOf(origin + cut),
      to: frameOf(origin + Arr.get(cuts, ordinal + NEXT).pipe(Option.getOrElse(() => span.quiet))),
      section: section.name,
      treatment: plan.treatment,
      palette: pick(plan.palettes, ordinal),
      layout: pick(plan.layouts, layoutIndexOf(span, cut, ordinal)),
      hookLayout: hookLayoutOf(plan, cut),
      lead: leadOf(plan, cut),
      ordinal,
    })),
  );
};

const silenceShots = ({ section, origin, quiet, length }: Span): readonly SilenceShot[] => {
  if (quiet >= length) {
    return [];
  }
  return [
    {
      kind: "silence",
      from: frameOf(origin + quiet),
      to: frameOf(origin + length),
      section: section.name,
    },
  ];
};

const storyboard: readonly Shot[] = pipe(
  song.sections,
  Arr.map(spanOf),
  Arr.flatMap((span) => Arr.appendAll(lyricsShots(span), silenceShots(span))),
);

const shotByFrame: readonly Shot[] = pipe(
  storyboard,
  Arr.flatMap((shot) => Array.from({ length: shot.to - shot.from }, () => shot)),
);

const LAST_FRAME = DURATION_IN_FRAMES - NEXT;

const shotAt = (frame: number): Shot =>
  Arr.get(shotByFrame, Math.min(Math.max(frame, FIRST), LAST_FRAME)).pipe(Option.getOrThrow);

export { shotAt };
export type { LyricsShot, Shot };
