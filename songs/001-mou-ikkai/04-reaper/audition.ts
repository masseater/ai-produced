import { Array as Arr, Console, Effect, String as Str, pipe } from "effect";

import { ensureClosed, runScript } from "#/04-reaper/reaper.ts";
import { Machine } from "#/task.ts";

const TIMEOUT_SECONDS = 1800;
const WAV = ".wav";

const here = import.meta.dirname;

const auditionWavs = Effect.gen(function* auditionWavs() {
  const machine = yield* Machine;
  const out = machine.join(here, "render", "audition");
  const names = yield* machine.list(out);
  return pipe(
    names,
    Arr.filter(Str.endsWith(WAV)),
    Arr.sortBy(Str.Order),
    Arr.map((name) => machine.join(out, name)),
  );
});

const metricsOf = Effect.fn("metricsOf")(function* metricsOf(wav: string) {
  const machine = yield* Machine;
  const { stdout } = yield* machine.output({
    command: "cargo",
    args: ["run", "-q", "--release", "-p", "audio", "--bin", "mix-metrics", "--", wav],
    cwd: machine.resolve(here, "..", "..", ".."),
  });
  return `${machine.basename(wav, WAV)}\t${Str.trim(stdout)}\n`;
});

const clean = Effect.gen(function* clean() {
  const machine = yield* Machine;
  yield* machine.makeDirectory(machine.join(here, "render", "audition"));
  const stale = yield* auditionWavs;
  yield* pipe(stale, Effect.forEach(machine.remove));
});

const audition = Effect.gen(function* audition() {
  const machine = yield* Machine;
  yield* clean;
  yield* ensureClosed({ cwd: here, doing: "auditioning" });
  yield* runScript({
    lua: machine.join(here, "audition.lua"),
    done: machine.join(here, "render", "audition-done.txt"),
    seconds: TIMEOUT_SECONDS,
    cwd: here,
  });
  const wavs = yield* auditionWavs;
  const lines = yield* pipe(wavs, Effect.forEach(metricsOf));
  const table = Arr.join(lines, "");
  yield* Console.log(Str.trimEnd(table));
  yield* machine.write(machine.join(here, "render", "audition", "metrics.tsv"), table);
});

export { audition };
