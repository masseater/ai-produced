import data from "#/pages/mou-ikkai/generated/mou-ikkai.json";

export type Mora = Readonly<{
  text: string;
  kind: string;
  start: number;
  length: number;
  hook: number;
}>;

export type Word = Readonly<{ text: string; morae: readonly number[] }>;

export type Line = Readonly<{
  section: string;
  index: number;
  text: string;
  note: string;
  words: readonly Word[];
  morae: readonly Mora[];
}>;

export type Section = Readonly<{ name: string; startBar: number; bars: number }>;

export type Song = Readonly<{
  bpm: number;
  beatsPerBar: number;
  bars: number;
  hooks: number;
  sections: readonly Section[];
  lines: readonly Line[];
}>;

export const song: Song = data;

export const FPS = 48;
export const SIXTEENTHS_PER_BEAT = 4;
export const SIXTEENTHS_PER_BAR = SIXTEENTHS_PER_BEAT * song.beatsPerBar;
const SECONDS_PER_MINUTE = 60;
const FRAMES_PER_SIXTEENTH = (FPS * SECONDS_PER_MINUTE) / song.bpm / SIXTEENTHS_PER_BEAT;

export const frameOf = (sixteenth: number): number => Math.round(sixteenth * FRAMES_PER_SIXTEENTH);

export const DURATION_IN_FRAMES = frameOf(song.bars * SIXTEENTHS_PER_BAR);
