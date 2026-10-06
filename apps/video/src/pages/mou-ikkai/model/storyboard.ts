import { Array as Arr, Option, Order } from "effect";

import {
  DURATION_IN_FRAMES,
  frameOf,
  SIXTEENTHS_PER_BAR,
  SIXTEENTHS_PER_BEAT,
  song,
} from "#/shared/song";
import type { Line, Section } from "#/shared/song";

type Treatment = "loop" | "verse" | "build" | "chorus" | "chop" | "bridge";
type PaletteName = "ink" | "paper" | "rec" | "signal" | "ash";
type LayoutName = "center" | "topLeft" | "bottomRight" | "vertical" | "tile";
type LayoutBy = "cut" | "bar" | "line";
type Silence = "none" | "lastBeat" | "lastBar";

type Plan = Readonly<{
  treatment: Treatment;
  palettes: readonly PaletteName[];
  layouts: readonly LayoutName[];
  layoutBy: LayoutBy;
  grid: readonly (readonly [fromBar: number, step: number])[];
  lineCuts: boolean;
  voiceCuts: boolean;
  leadEvery: number;
  silence: Silence;
  tileFromBar: number;
}>;

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

const FIRST = 0;
const NEXT = 1;
const NO_LEAD = 0;
const LEAD_FRAMES = 2;
const SIXTEENTH = 1;
const EIGHTH = 2;
const HALF_NOTE = EIGHTH * SIXTEENTHS_PER_BEAT;
const BUILD_HALVES_FROM_BAR = 2;
const BUILD_BEATS_FROM_BAR = 4;
const BREAK_SIXTEENTHS_FROM_BAR = 4;
const EXTENSION_FROM_BAR = 16;
const NEVER = Number.POSITIVE_INFINITY;
const CLOSURE = "closure";

const still: Omit<Plan, "treatment" | "palettes"> = {
  layouts: ["center"],
  layoutBy: "cut",
  grid: [],
  lineCuts: true,
  voiceCuts: false,
  leadEvery: NO_LEAD,
  silence: "none",
  tileFromBar: NEVER,
};

const verse: Plan = {
  ...still,
  treatment: "verse",
  palettes: ["ink"],
  layouts: ["topLeft", "center", "bottomRight", "vertical"],
  layoutBy: "line",
};

const build: Plan = {
  ...still,
  treatment: "build",
  palettes: ["ink", "paper"],
  layouts: ["center", "topLeft", "bottomRight"],
  grid: [
    [FIRST, SIXTEENTHS_PER_BAR],
    [BUILD_HALVES_FROM_BAR, HALF_NOTE],
    [BUILD_BEATS_FROM_BAR, SIXTEENTHS_PER_BEAT],
  ],
  silence: "lastBeat",
};

const chorus: Plan = {
  ...still,
  treatment: "chorus",
  palettes: ["ink", "paper", "rec", "paper"],
  layouts: ["center", "vertical", "topLeft", "bottomRight"],
  layoutBy: "bar",
  grid: [[FIRST, SIXTEENTHS_PER_BEAT]],
  lineCuts: false,
  leadEvery: SIXTEENTHS_PER_BAR,
};

const chop: Plan = {
  ...still,
  treatment: "chop",
  palettes: ["rec", "ink"],
  layouts: ["center", "topLeft", "bottomRight", "vertical"],
  grid: [[FIRST, EIGHTH]],
  lineCuts: false,
  voiceCuts: true,
  leadEvery: SIXTEENTHS_PER_BEAT,
};

const plans: Readonly<Record<string, Plan>> = {
  Intro: { ...still, treatment: "loop", palettes: ["ink"] },
  A1: verse,
  B1: build,
  Chorus1: chorus,
  Drop1: chop,
  A2: verse,
  B2: build,
  Chorus2: { ...chorus, palettes: ["ink", "paper", "rec", "signal"] },
  Bridge: { ...still, treatment: "bridge", palettes: ["ink"] },
  Break: {
    ...chop,
    palettes: ["rec", "ink", "paper"],
    grid: [
      [FIRST, EIGHTH],
      [BREAK_SIXTEENTHS_FROM_BAR, SIXTEENTH],
    ],
    silence: "lastBar",
  },
  LastChorus: {
    ...chorus,
    palettes: ["ink", "paper", "rec", "signal"],
    grid: [
      [FIRST, SIXTEENTHS_PER_BEAT],
      [EXTENSION_FROM_BAR, EIGHTH],
    ],
    tileFromBar: EXTENSION_FROM_BAR,
  },
  Outro: { ...still, treatment: "loop", palettes: ["ink"] },
};

const silenceLengths: Readonly<Record<Silence, number>> = {
  none: 0,
  lastBeat: SIXTEENTHS_PER_BEAT,
  lastBar: SIXTEENTHS_PER_BAR,
};

const pick = <Item>(items: readonly Item[], index: number): Item =>
  Arr.get(items, index % items.length).pipe(Option.getOrThrow);

const planOf = (section: Section): Plan =>
  Option.fromNullishOr(plans[section.name]).pipe(Option.getOrThrow);

const firstStart = (line: Line, fallback: number): number =>
  Arr.head(line.morae).pipe(
    Option.map((mora) => mora.start),
    Option.getOrElse(() => fallback),
  );

const spanOf = (section: Section): Span => {
  const plan = planOf(section);
  const origin = section.startBar * SIXTEENTHS_PER_BAR;
  const length = section.bars * SIXTEENTHS_PER_BAR;
  const lines = song.lines.filter((line) => line.section === section.name);
  return {
    section,
    plan,
    origin,
    length,
    quiet: length - silenceLengths[plan.silence],
    lines,
    lineStarts: lines.map((line) => firstStart(line, origin) - origin),
  };
};

const gridCuts = (plan: Plan, end: number): readonly number[] =>
  plan.grid.flatMap(([fromBar, step], index) => {
    const start = fromBar * SIXTEENTHS_PER_BAR;
    const stop = Arr.get(plan.grid, index + NEXT).pipe(
      Option.map(([nextBar]) => nextBar * SIXTEENTHS_PER_BAR),
      Option.getOrElse(() => end),
    );
    return Array.from(
      { length: Math.ceil((stop - start) / step) },
      (_slot, count) => start + count * step,
    );
  });

const voiceCuts = (span: Span): readonly number[] =>
  span.lines.flatMap((line) =>
    line.morae.filter((mora) => mora.kind !== CLOSURE).map((mora) => mora.start - span.origin),
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
    candidates.filter((cut) => cut >= FIRST && cut < span.quiet),
    Order.Number,
  );
};

const layoutIndexOf = (span: Span, cut: number, ordinal: number): number =>
  ({
    cut: ordinal,
    bar: Math.floor(cut / SIXTEENTHS_PER_BAR),
    line: span.lineStarts.filter((start) => start <= cut).length,
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
  return cuts.map((cut, ordinal) => ({
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
  }));
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

const storyboard: readonly Shot[] = song.sections
  .map(spanOf)
  .flatMap((span) => Arr.appendAll(lyricsShots(span), silenceShots(span)));

const shotByFrame: readonly Shot[] = storyboard.flatMap((shot) =>
  Array.from({ length: shot.to - shot.from }, () => shot),
);

const LAST_FRAME = DURATION_IN_FRAMES - NEXT;

const shotAt = (frame: number): Shot =>
  Arr.get(shotByFrame, Math.min(Math.max(frame, FIRST), LAST_FRAME)).pipe(Option.getOrThrow);

export { shotAt };
export type { LayoutName, LyricsShot, PaletteName, Shot, Treatment };
