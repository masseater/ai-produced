import { Option } from "effect";

import { SIXTEENTHS_PER_BAR, SIXTEENTHS_PER_BEAT } from "#/pages/mou-ikkai/model/song";
import type { Section } from "#/pages/mou-ikkai/model/song";

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

const planOf = (section: Section): Plan =>
  Option.fromNullishOr(plans[section.name]).pipe(Option.getOrThrow);

export { CLOSURE, FIRST, LEAD_FRAMES, NEXT, NO_LEAD, planOf, silenceLengths };
export type { LayoutName, PaletteName, Plan, Treatment };
