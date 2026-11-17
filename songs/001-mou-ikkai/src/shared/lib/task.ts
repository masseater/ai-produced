import { runMain } from "@effect/platform-node/NodeRuntime";
import { layer as nodeServicesLayer } from "@effect/platform-node/NodeServices";
import {
  Array as Arr,
  Config,
  Context,
  Effect,
  FileSystem,
  Layer,
  Option,
  Path,
  Predicate,
  Schema,
  Stream,
  String as Str,
  pipe,
} from "effect";
import type { Scope } from "effect";
import { ChildProcess, ChildProcessSpawner } from "effect/process";

import { TaskFailed } from "#/shared/lib/task-failed.ts";

const EXIT_SUCCESS = 0;

type Command = Readonly<{
  command: string;
  args: readonly string[];
  cwd: string;
  env?: Readonly<Record<string, string>>;
}>;

type Output = Readonly<{ stdout: string; stderr: string }>;

class Machine extends Context.Service<
  Machine,
  {
    readonly run: (command: Command) => Effect.Effect<void, TaskFailed>;
    readonly output: (command: Command) => Effect.Effect<Output, TaskFailed>;
    readonly succeeds: (command: Command) => Effect.Effect<boolean>;
    readonly background: (command: Command) => Effect.Effect<void, TaskFailed, Scope.Scope>;
    readonly read: (file: string) => Effect.Effect<string, TaskFailed>;
    readonly write: (file: string, text: string) => Effect.Effect<void, TaskFailed>;
    readonly exists: (file: string) => Effect.Effect<boolean>;
    readonly remove: (file: string) => Effect.Effect<void, TaskFailed>;
    readonly list: (directory: string) => Effect.Effect<readonly string[], TaskFailed>;
    readonly makeDirectory: (directory: string) => Effect.Effect<void, TaskFailed>;
    readonly copy: (from: string, to: string) => Effect.Effect<void, TaskFailed>;
    readonly temporaryDirectory: Effect.Effect<string, TaskFailed, Scope.Scope>;
    readonly join: (...segments: readonly string[]) => string;
    readonly resolve: (...segments: readonly string[]) => string;
    readonly basename: (file: string, suffix: string) => string;
    readonly windows: boolean;
    readonly home: string;
  }
>()("@workspace/mou-ikkai/task/Machine") {}

type ProcessOps = Pick<Machine["Service"], "run" | "output" | "succeeds" | "background">;

type FileOps = Pick<
  Machine["Service"],
  "read" | "write" | "exists" | "remove" | "list" | "makeDirectory" | "copy" | "temporaryDirectory"
>;

const isTaskFailed = Schema.is(TaskFailed);

const failed = (failure: unknown): TaskFailed => {
  if (isTaskFailed(failure)) {
    return failure;
  }
  if (Predicate.isError(failure)) {
    return TaskFailed.make({ message: failure.message });
  }
  return TaskFailed.make({ message: String(failure) });
};

const processOf = (
  command: Command,
  stdout: "inherit" | "pipe" | "ignore",
): ChildProcess.StandardCommand =>
  ChildProcess.make(command.command, command.args, {
    cwd: command.cwd,
    env: { ...command.env },
    extendEnv: true,
    stdout,
    stderr: stdout,
  });

const exitedCleanly =
  (command: Command) =>
  (code: number): Effect.Effect<void, TaskFailed> => {
    if (code === EXIT_SUCCESS) {
      return Effect.void;
    }
    return Effect.fail(
      TaskFailed.make({ message: `${command.command} exited with ${String(code)}` }),
    );
  };

const processes = Effect.gen(function* processes() {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;
  const collect = (command: Command): Effect.Effect<Output, TaskFailed> =>
    Effect.scoped(
      Effect.gen(function* collected() {
        const handle = yield* spawner.spawn(processOf(command, "pipe"));
        const [stdout, stderr, code] = yield* Effect.all(
          [
            handle.stdout.pipe(Stream.decodeText, Stream.mkString),
            handle.stderr.pipe(Stream.decodeText, Stream.mkString),
            handle.exitCode,
          ],
          { concurrency: "unbounded" },
        );
        yield* exitedCleanly(command)(code);
        return { stdout, stderr } satisfies Output;
      }),
    ).pipe(Effect.mapError(failed));
  const ops: ProcessOps = {
    run: (command) =>
      spawner
        .exitCode(processOf(command, "inherit"))
        .pipe(Effect.mapError(failed), Effect.flatMap(exitedCleanly(command))),
    output: collect,
    succeeds: (command) =>
      spawner.exitCode(processOf(command, "ignore")).pipe(
        Effect.map((code) => code === EXIT_SUCCESS),
        Effect.orElseSucceed(() => false),
      ),
    background: (command) =>
      spawner.spawn(processOf(command, "ignore")).pipe(
        Effect.tap((handle) => Effect.addFinalizer(() => Effect.ignore(handle.kill()))),
        Effect.mapError(failed),
        Effect.asVoid,
      ),
  };
  return ops;
});

const files = Effect.gen(function* files() {
  const fileSystem = yield* FileSystem.FileSystem;
  const ops: FileOps = {
    read: (file) => Effect.mapError(fileSystem.readFileString(file), failed),
    write: (file, text) => Effect.mapError(fileSystem.writeFileString(file, text), failed),
    exists: (file) => Effect.orElseSucceed(fileSystem.exists(file), () => false),
    remove: (file) => Effect.mapError(fileSystem.remove(file, { force: true }), failed),
    list: (directory) => Effect.mapError(fileSystem.readDirectory(directory), failed),
    makeDirectory: (directory) =>
      Effect.mapError(fileSystem.makeDirectory(directory, { recursive: true }), failed),
    copy: (from, to) => Effect.mapError(fileSystem.copyFile(from, to), failed),
    temporaryDirectory: Effect.mapError(fileSystem.makeTempDirectoryScoped(), failed),
  };
  return ops;
});

const home = Config.String("HOME").pipe(
  Config.orElse(() => Config.String("USERPROFILE")),
  Config.withDefault("."),
);

const machine = Layer.effect(
  Machine,
  Effect.gen(function* machine() {
    const path = yield* Path.Path;
    const processOps = yield* processes;
    const fileOps = yield* files;
    const homeDirectory = yield* home;
    return Machine.of({
      ...processOps,
      ...fileOps,
      join: (...segments) => path.join(...segments),
      resolve: (...segments) => path.resolve(...segments),
      basename: (file, suffix) => path.basename(file, suffix),
      windows: path.sep === "\\",
      home: homeDirectory,
    });
  }),
).pipe(Layer.provide(nodeServicesLayer));

const rows = (text: string): readonly (readonly string[])[] =>
  pipe(
    Str.linesIterator(text),
    Arr.fromIterable,
    Arr.filter(Str.isNonEmpty),
    Arr.map(Str.split("\t")),
  );

const field =
  (index: number) =>
  (row: readonly string[]): string =>
    pipe(
      Arr.get(row, index),
      Option.getOrElse(() => ""),
    );

const runTask = (program: Effect.Effect<void, TaskFailed, Machine>): void => {
  runMain(Effect.provide(program, machine));
};

export type { Command };
export { Machine, field, rows, runTask };
