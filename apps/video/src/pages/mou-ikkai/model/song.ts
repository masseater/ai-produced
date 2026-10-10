import data from "#/pages/mou-ikkai/generated/mou-ikkai.json";

type Mora = Readonly<{
  text: string;
  kind: string;
  start: number;
  length: number;
  hook: number;
}>;

type Word = Readonly<{ text: string; morae: readonly number[] }>;

type Line = Readonly<{
  section: string;
  index: number;
  text: string;
  note: string;
  words: readonly Word[];
  morae: readonly Mora[];
}>;

type Section = Readonly<{ name: string; startBar: number; bars: number }>;

type Song = Readonly<{
  bpm: number;
  beatsPerBar: number;
  bars: number;
  hooks: number;
  sections: readonly Section[];
  lines: readonly Line[];
}>;

const song: Song = data;

const FPS = 48;
const SIXTEENTHS_PER_BEAT = 4;
const SIXTEENTHS_PER_BAR = SIXTEENTHS_PER_BEAT * song.beatsPerBar;
const SECONDS_PER_MINUTE = 60;
const FRAMES_PER_SIXTEENTH = (FPS * SECONDS_PER_MINUTE) / song.bpm / SIXTEENTHS_PER_BEAT;

const frameOf = (sixteenth: number): number => Math.round(sixteenth * FRAMES_PER_SIXTEENTH);

const DURATION_IN_FRAMES = frameOf(song.bars * SIXTEENTHS_PER_BAR);

export { DURATION_IN_FRAMES, FPS, frameOf, SIXTEENTHS_PER_BAR, SIXTEENTHS_PER_BEAT, song };
export type { Line, Mora, Section, Song, Word };
