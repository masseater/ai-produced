import { Array as Arr, Config, Effect, Schedule, String as Str, pipe } from "effect";

import { TaskFailed, expect } from "#/shared/lib/task-failed.ts";
import { Machine } from "#/shared/lib/task.ts";

const STARTUP = "10 seconds";
const POLL = "1 second";
const DONE = "ok";
const MAC = "/Applications/REAPER.app/Contents/MacOS/REAPER";
const WINDOWS = String.raw`C:\Program Files\REAPER (x64)\reaper.exe`;

export type Script = Readonly<{ lua: string; done: string; seconds: number; cwd: string }>;

const reaperPath = Effect.gen(function* findReaper() {
  const machine = yield* Machine;
  if (machine.windows) {
    return yield* Config.String("REAPER").pipe(Config.withDefault(WINDOWS));
  }
  return yield* Config.String("REAPER").pipe(Config.withDefault(MAC));
}).pipe(Effect.mapError((failure) => TaskFailed.make({ message: String(failure) })));

const running = Effect.fn("running")(function* isRunning(cwd: string) {
  const machine = yield* Machine;
  if (!machine.windows) {
    return yield* machine.succeeds({ command: "pgrep", args: ["-x", "REAPER"], cwd });
  }
  const { stdout } = yield* machine.output({
    command: "tasklist",
    args: ["/FI", "IMAGENAME eq reaper.exe", "/NH"],
    cwd,
  });
  return pipe(stdout, Str.toLowerCase, Str.includes("reaper.exe"));
});

export const ensureClosed = Effect.fn("ensureClosed")(function* checkClosed(
  task: Readonly<{ cwd: string; doing: string }>,
) {
  const open = yield* running(task.cwd);
  yield* expect({
    holds: !open,
    message: `REAPER is already running; close it before ${task.doing}`,
  });
});

const session = Effect.fn("session")(function* openSession(script: Script) {
  const machine = yield* Machine;
  const reaper = yield* reaperPath;
  yield* machine.background({
    command: reaper,
    args: ["-nosplash", "-ignoreerrors", "-new"],
    cwd: script.cwd,
  });
  yield* Effect.sleep(STARTUP);
  yield* machine.output({ command: reaper, args: ["-nonewinst", script.lua], cwd: script.cwd });
  yield* machine
    .exists(script.done)
    .pipe(
      Effect.repeat({ schedule: Schedule.spaced(POLL), until: Boolean, times: script.seconds }),
    );
}, Effect.scoped);

const unfinished = (script: Script, log: string): string => {
  if (Str.isNonEmpty(log)) {
    return log;
  }
  return `${script.lua} did not finish`;
};

export const runScript = Effect.fn("runScript")(function* runReaperScript(script: Script) {
  const machine = yield* Machine;
  yield* machine.remove(script.done);
  yield* session(script);
  const log = yield* machine.read(script.done).pipe(Effect.orElseSucceed(() => ""));
  const finished = pipe(Str.linesIterator(log), Arr.fromIterable, Arr.contains(DONE));
  yield* expect({ holds: finished, message: unfinished(script, log) });
});
