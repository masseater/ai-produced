import { Array as Arr, Console, Effect, Option, String as Str, pipe } from "effect";

import { ensureClosed, runScript } from "#/04-reaper/reaper.ts";
import { TaskFailed } from "#/task-failed.ts";
import { Machine, field, rows } from "#/task.ts";

const TIMEOUT_SECONDS = 900;
const ROUNDS = 4;
const FIRST_ROUND = 1;
const TOLERANCE_LU = 0.25;
const FIRST_THRESHOLD = "-6";
const DECIMALS = 2;
const TOM_SWEEP = 2;
const NOISE = "(2*random(0)-1)";
const WIDE = "(2*random(1)-1)";
const TOMS = { low: 90, mid: 130, high: 180 } as const;
const COLUMN = { key: 0, value: 1 } as const;
const LOUDNESS_WORD = 1;
const keyOf = field(COLUMN.key);
const valueOf = field(COLUMN.value);

type Drum = Readonly<{ name: string; expression: string; seconds: number; filter: string }>;

const stereo = (envelope: string): string => `${NOISE}*${envelope}|${WIDE}*${envelope}`;

const tom = (name: string, hz: number): Drum => ({
  name,
  expression: `sin(2*PI*(${String(hz)}*t+${String(hz * TOM_SWEEP)}/20*(1-exp(-20*t))))*exp(-8*t)`,
  seconds: 0.45,
  filter: "anull",
});

const drums: readonly Drum[] = [
  {
    name: "kick",
    expression: "sin(2*PI*(50*t+6*(1-exp(-25*t))))*exp(-5*t)",
    seconds: 0.45,
    filter: "anull",
  },
  {
    name: "stick",
    expression: `0.6*sin(2*PI*1700*t)*exp(-80*t)+0.5*${NOISE}*exp(-120*t)`,
    seconds: 0.08,
    filter: "anull",
  },
  {
    name: "snare",
    expression: `0.5*sin(2*PI*190*t)*exp(-25*t)+0.6*${NOISE}*exp(-14*t)|0.5*sin(2*PI*190*t)*exp(-25*t)+0.6*${WIDE}*exp(-14*t)`,
    seconds: 0.3,
    filter: "highpass=f=150",
  },
  {
    name: "clap",
    expression: stereo("exp(-15*t)"),
    seconds: 0.25,
    filter: "bandpass=f=1200:w=900",
  },
  { name: "hat-closed", expression: stereo("exp(-45*t)"), seconds: 0.1, filter: "highpass=f=7000" },
  { name: "hat-open", expression: stereo("exp(-7*t)"), seconds: 0.5, filter: "highpass=f=7000" },
  { name: "crash", expression: stereo("exp(-2.5*t)"), seconds: 2, filter: "highpass=f=4000" },
  tom("tom-low", TOMS.low),
  tom("tom-mid", TOMS.mid),
  tom("tom-high", TOMS.high),
];

const here = import.meta.dirname;

const synthesize = Effect.fn("synthesize")(function* synthesize(drum: Drum) {
  const machine = yield* Machine;
  yield* machine.output({
    command: "ffmpeg",
    args: [
      "-hide_banner",
      "-loglevel",
      "error",
      "-y",
      "-f",
      "lavfi",
      "-i",
      `aevalsrc=${drum.expression}:s=48000:d=${String(drum.seconds)}`,
      "-af",
      drum.filter,
      "-c:a",
      "pcm_s24le",
      machine.join(here, "samples", `${drum.name}.wav`),
    ],
    cwd: here,
  });
});

const integratedLoudness = (report: string): Option.Option<string> =>
  pipe(
    Str.linesIterator(report),
    Arr.fromIterable,
    Arr.map((line) => pipe(line, Str.trim, Str.split(/\s+/u))),
    Arr.filter(Arr.contains("I:")),
    Arr.last,
    Option.flatMap(Arr.get(LOUDNESS_WORD)),
  );

const lufs = Effect.fn("lufs")(function* lufs(file: string) {
  const machine = yield* Machine;
  const { stderr } = yield* machine.output({
    command: "ffmpeg",
    args: [
      "-nostdin",
      "-hide_banner",
      "-nostats",
      "-i",
      file,
      "-af",
      "ebur128=peak=true:framelog=quiet",
      "-f",
      "null",
      "-",
    ],
    cwd: here,
  });
  return yield* Effect.fromOption(integratedLoudness(stderr)).pipe(
    Effect.mapError(() => TaskFailed.make({ message: `ffmpeg found no loudness in ${file}` })),
  );
});

const pass = Effect.fn("pass")(function* pass(table: string) {
  const machine = yield* Machine;
  yield* machine.write(machine.join(here, "render", "pass.tsv"), table);
  yield* runScript({
    lua: machine.join(here, "mix.lua"),
    done: machine.join(here, "render", "mix-done.txt"),
    seconds: TIMEOUT_SECONDS,
    cwd: here,
  });
});

const prepare = Effect.gen(function* prepare() {
  const machine = yield* Machine;
  const stems = machine.join(here, "render", "stems");
  yield* machine.makeDirectory(machine.join(here, "samples"));
  yield* machine.makeDirectory(stems);
  yield* Effect.forEach(drums, synthesize, { concurrency: 1 });
  yield* ensureClosed({ cwd: here, doing: "mixing" });
  const names = yield* machine.list(stems);
  const stale = pipe(
    names,
    Arr.filter(Str.endsWith(".wav")),
    Arr.map((name) => machine.join(stems, name)),
  );
  yield* Effect.forEach(stale, machine.remove, { concurrency: 1 });
});

const gainOf = Effect.fn("gainOf")(function* gainOf(row: readonly string[], vocal: number) {
  const machine = yield* Machine;
  const track = keyOf(row);
  const stem = yield* lufs(machine.join(here, "render", "stems", `${track}.wav`));
  return `${track}\t${(Number(valueOf(row)) + vocal - Number(stem)).toFixed(DECIMALS)}\n`;
});

const balance = Effect.gen(function* balance() {
  const machine = yield* Machine;
  yield* pass("pass\tstems\n");
  const vocalLufs = yield* lufs(machine.join(here, "render", "stems", "Vocal.wav"));
  const vocal = Number(vocalLufs);
  const levels = yield* machine.read(machine.join(here, "levels.tsv"));
  const gains = yield* Effect.forEach(rows(levels), (row) => gainOf(row, vocal), {
    concurrency: 1,
  });
  const table = Arr.join(gains, "");
  yield* Console.log(Str.trimEnd(table));
  yield* machine.write(machine.join(here, "render", "gains.tsv"), table);
});

type Master = Readonly<{ lufs: number; lowShelf: string; highShelf: string }>;

type Round = Readonly<{ done: boolean; threshold: string }>;

const settingOf =
  (settings: readonly (readonly string[])[]) =>
  (key: string): string =>
    pipe(
      settings,
      Arr.findFirst((row) => keyOf(row) === key),
      Option.map(valueOf),
      Option.getOrElse(() => ""),
    );

const master = Effect.gen(function* readMaster() {
  const machine = yield* Machine;
  const text = yield* machine.read(machine.join(here, "master.tsv"));
  const setting = settingOf(rows(text));
  return {
    lufs: Number(setting("lufs")),
    lowShelf: setting("low_shelf"),
    highShelf: setting("high_shelf"),
  } satisfies Master;
});

const tune = Effect.fn("tune")(function* tune(settings: Master, round: number, threshold: string) {
  const machine = yield* Machine;
  const output = machine.join(here, "render", "mou-ikkai-mix.wav");
  yield* machine.remove(output);
  yield* pass(
    `pass\tmix\nthreshold\t${threshold}\nlow_shelf\t${settings.lowShelf}\nhigh_shelf\t${settings.highShelf}\n`,
  );
  const measured = yield* lufs(output);
  yield* Console.log(`round ${String(round)}: threshold ${threshold} -> ${measured} LUFS`);
  const miss = settings.lufs - Number(measured);
  return {
    done: Math.abs(miss) < TOLERANCE_LU,
    threshold: (Number(threshold) - miss).toFixed(DECIMALS),
  } satisfies Round;
});

const keepTuning =
  (settings: Master) =>
  (state: Round, round: number): Effect.Effect<Round, TaskFailed, Machine> => {
    if (state.done) {
      return Effect.succeed(state);
    }
    return tune(settings, round, state.threshold);
  };

const mix = Effect.gen(function* mix() {
  const machine = yield* Machine;
  yield* prepare;
  yield* balance;
  const settings = yield* master;
  yield* Effect.reduce(
    Arr.range(FIRST_ROUND, ROUNDS),
    (): Round => ({ done: false, threshold: FIRST_THRESHOLD }),
    keepTuning(settings),
  );
  yield* machine.copy(
    machine.join(here, "render", "pass.tsv"),
    machine.join(here, "render", "mou-ikkai-mix.settings.tsv"),
  );
});

export { mix };
