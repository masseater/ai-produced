import { Array as Arr, Config, Console, Effect, Option, String as Str, pipe } from "effect";

import { TaskFailed, expect } from "#/shared/lib/task-failed.ts";
import { Machine, field, rows } from "#/shared/lib/task.ts";
import type { Command } from "#/shared/lib/task.ts";

import { dotnet, pick } from "./dotnet.ts";

const OPENUTAU_COMMIT = "799ba8de27c19fb305b1d3c989f1823ad4a8e605";
const OPENUTAU_REPOSITORY = "https://github.com/stakira/OpenUtau.git";
const COLUMN = {
  singer: { id: 0, name: 1, type: 2, colors: 3 },
  vocalKind: 5,
  phoneme: { lyric: 2, phoneme: 4 },
} as const;
const singerColumn = {
  id: field(COLUMN.singer.id),
  name: field(COLUMN.singer.name),
  type: field(COLUMN.singer.type),
  colors: field(COLUMN.singer.colors),
};
const vocalKind = field(COLUMN.vocalKind);
const phonemeColumn = {
  lyric: field(COLUMN.phoneme.lyric),
  phoneme: field(COLUMN.phoneme.phoneme),
};
const NO_CLOSURES = 0;

const here = import.meta.dirname;

type Settings = Readonly<{
  singerPattern: string;
  singerType: string;
  pitch: string;
  colors: Option.Option<string>;
  phonemizer: string;
}>;

const settings: Config.Config<Settings> = Config.all({
  singerPattern: Config.String("SINGER_PATTERN").pipe(Config.withDefault("Nishiren")),
  singerType: Config.String("SINGER_TYPE").pipe(Config.withDefault("DiffSinger")),
  pitch: Config.String("PITCH").pipe(Config.withDefault("1")),
  colors: Config.option(Config.String("COLORS")),
  phonemizer: Config.String("PHONEMIZER").pipe(Config.withDefault("")),
});

type Paths = Readonly<{
  song: string;
  repo: string;
  tool: string;
  source: string;
  out: string;
  vocal: string;
  ustx: string;
  phonemes: string;
  dll: string;
}>;

const optional = (flag: string, value: string): readonly string[] => {
  if (Str.isNonEmpty(value)) {
    return [flag, value];
  }
  return [];
};

const pathsOf = Effect.gen(function* pathsOf() {
  const machine = yield* Machine;
  const song = machine.resolve(here, "..", "..", "..", "..");
  const repo = machine.resolve(song, "..", "..");
  const tool = machine.join(repo, "tools", "openutau-render");
  const out = machine.join(song, "03-vocal", "render");
  yield* machine.makeDirectory(out);
  return {
    song,
    repo,
    tool,
    source: machine.join(machine.home, ".cache", "ai-produced", "openutau"),
    out,
    vocal: machine.join(song, "02-composition", "generated", "vocal.tsv"),
    ustx: machine.join(out, "mou-ikkai.ustx"),
    phonemes: machine.join(out, "phonemes.tsv"),
    dll: machine.join(tool, "bin", "Release", "net10.0", "openutau-render.dll"),
  } satisfies Paths;
});

const build = Effect.fn("build")(function* build(paths: Paths, options: Command) {
  const machine = yield* Machine;
  const exists = yield* machine.exists(paths.source);
  if (!exists) {
    yield* machine.run({
      ...options,
      command: "git",
      args: ["clone", "--filter=blob:none", OPENUTAU_REPOSITORY, paths.source],
    });
  }
  yield* machine.run({
    ...options,
    command: "git",
    args: ["-C", paths.source, "fetch", "--quiet", "origin", OPENUTAU_COMMIT],
  });
  yield* machine.run({
    ...options,
    command: "git",
    args: ["-C", paths.source, "checkout", "--quiet", OPENUTAU_COMMIT],
  });
  yield* machine.run({
    ...options,
    args: [
      "build",
      paths.tool,
      "-c",
      "Release",
      "--nologo",
      "-v",
      "quiet",
      "-clp:ErrorsOnly",
      `-p:OpenUtauDir=${paths.source}`,
    ],
  });
});

const singerOf = Effect.fn("singerOf")(function* singerOf(
  paths: Paths,
  options: Command,
  config: Settings,
) {
  const machine = yield* Machine;
  const { stdout } = yield* machine.output({ ...options, args: [paths.dll, "singers"] });
  yield* Console.log(Str.trimEnd(stdout));
  yield* machine.write(machine.join(paths.out, "singers.tsv"), stdout);
  const wanted = Str.toLowerCase(config.singerPattern);
  const matches = (row: readonly string[]): boolean =>
    pipe(
      `${singerColumn.id(row)}${singerColumn.name(row)}`,
      Str.toLowerCase,
      Str.includes(wanted),
    ) && singerColumn.type(row) === config.singerType;
  return yield* pipe(
    rows(stdout),
    Arr.findFirst(matches),
    Effect.fromOption,
    Effect.mapError(() =>
      TaskFailed.make({
        message: `no ${config.singerType} singer matching ${config.singerPattern}`,
      }),
    ),
  );
});

const score = Effect.fn("score")(function* score(paths: Paths, options: Command, config: Settings) {
  const machine = yield* Machine;
  const singer = yield* singerOf(paths, options, config);
  const colors = Option.getOrElse(config.colors, () => singerColumn.colors(singer));
  const phonemizer = pick({
    when: Str.isNonEmpty(config.phonemizer),
    value: config.phonemizer,
    otherwise: "default",
  });
  yield* Console.log(`singer=${singerColumn.id(singer)} colors=${colors} phonemizer=${phonemizer}`);
  yield* machine.run({
    ...options,
    command: "cargo",
    args: [
      "run",
      "-q",
      "-p",
      "song",
      "--bin",
      "ustx-from-vocal",
      "--",
      paths.vocal,
      "--score",
      machine.join(paths.song, "02-composition", "score.toml"),
      "--out",
      paths.ustx,
      "--tuning",
      machine.join(paths.song, "03-vocal", "tuning.toml"),
      "--singer",
      singerColumn.id(singer),
      ...optional("--colors", colors),
      ...optional("--phonemizer", config.phonemizer),
    ],
  });
});

const sing = Effect.fn("sing")(function* sing(paths: Paths, options: Command, config: Settings) {
  const machine = yield* Machine;
  const pitch = Arr.filter(["--pitch"], () => Str.isNonEmpty(config.pitch));
  yield* machine.run({
    ...options,
    args: [
      paths.dll,
      "render",
      paths.ustx,
      ...pitch,
      "--save",
      paths.ustx,
      "--phonemes",
      paths.phonemes,
      "--out",
      paths.out,
    ],
  });
});

const checkClosures = Effect.fn("checkClosures")(function* checkClosures(paths: Paths) {
  const machine = yield* Machine;
  const vocal = yield* machine.read(paths.vocal);
  const phonemes = yield* machine.read(paths.phonemes);
  const closures = Arr.filter(rows(vocal), (row) => vocalKind(row) === "closure").length;
  const sung = Arr.filter(
    rows(phonemes),
    (row) => phonemeColumn.lyric(row) === "っ" && phonemeColumn.phoneme(row) === "cl",
  ).length;
  yield* Console.log(`closures: ${String(closures)}, sung as cl: ${String(sung)}`);
  yield* expect({
    holds: closures > NO_CLOSURES && closures === sung,
    message: "every closure in vocal.tsv must be sung as cl",
  });
});

const loudness = Effect.fn("loudness")(function* loudness(paths: Paths, options: Command) {
  const machine = yield* Machine;
  const { stderr } = yield* machine.output({
    ...options,
    command: "ffmpeg",
    args: [
      "-hide_banner",
      "-nostats",
      "-i",
      machine.join(paths.out, "mou-ikkai_Vocal.wav"),
      "-af",
      "ebur128=framelog=quiet",
      "-f",
      "null",
      "-",
    ],
  });
  const summary = pipe(
    Str.indexOf("Summary")(stderr),
    Option.map((start) => Str.slice(start)(stderr)),
    Option.getOrElse(() => ""),
  );
  yield* Console.log(Str.trimEnd(summary));
  yield* machine.write(machine.join(paths.out, "loudness.txt"), summary);
});

const render = Effect.gen(function* render() {
  const config = yield* settings.pipe(
    Effect.mapError((failure) => TaskFailed.make({ message: String(failure) })),
  );
  const paths = yield* pathsOf;
  const sdk = yield* dotnet(paths.repo);
  const options = {
    command: sdk.command,
    args: [],
    cwd: paths.repo,
    env: { ...sdk.env, DOTNET_CLI_TELEMETRY_OPTOUT: "1", DOTNET_NOLOGO: "1" },
  } satisfies Command;
  yield* build(paths, options);
  yield* score(paths, options, config);
  yield* sing(paths, options, config);
  yield* checkClosures(paths);
  yield* loudness(paths, options);
});

export { render };
