import { Array as Arr, Effect, String as Str, pipe } from "effect";

import { Machine, field, rows } from "#/task.ts";
import type { Command } from "#/task.ts";

const DOTNET_CHANNEL = "10.0";
const DOTNET_MAJOR = "10.";
const DOTNET_INSTALL = "https://builds.dotnet.microsoft.com/dotnet/scripts/v1/dotnet-install";
const SDK_VERSION_COLUMN = 0;
const sdkVersion = field(SDK_VERSION_COLUMN);

type Sdk = Readonly<{ command: string; env: Readonly<Record<string, string>> }>;
type Choice = Readonly<{ when: boolean; value: string; otherwise: string }>;

const pick = (choice: Choice): string => {
  if (choice.when) {
    return choice.value;
  }
  return choice.otherwise;
};

const installerCommand = (
  machine: Machine["Service"],
  script: string,
  directory: string,
): Omit<Command, "cwd"> => {
  if (machine.windows) {
    return {
      command: "powershell",
      args: [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        script,
        "-Channel",
        DOTNET_CHANNEL,
        "-InstallDir",
        directory,
      ],
    };
  }
  return {
    command: "bash",
    args: [script, "--channel", DOTNET_CHANNEL, "--install-dir", directory],
  };
};

const installDotnet = Effect.fn("installDotnet")(function* installDotnet(cwd: string) {
  const machine = yield* Machine;
  const directory = machine.join(machine.home, ".dotnet");
  const extension = pick({ when: machine.windows, value: ".ps1", otherwise: ".sh" });
  const temporary = yield* machine.temporaryDirectory;
  const script = machine.join(temporary, `dotnet-install${extension}`);
  yield* machine.run({
    command: "curl",
    args: ["-sSL", `${DOTNET_INSTALL}${extension}`, "-o", script],
    cwd,
  });
  yield* machine.run({ ...installerCommand(machine, script, directory), cwd });
  const executable = pick({ when: machine.windows, value: "dotnet.exe", otherwise: "dotnet" });
  return {
    command: machine.join(directory, executable),
    env: { DOTNET_ROOT: directory },
  } satisfies Sdk;
}, Effect.scoped);

const dotnet = Effect.fn("dotnet")(function* dotnet(cwd: string) {
  const machine = yield* Machine;
  const sdks = yield* machine
    .output({ command: "dotnet", args: ["--list-sdks"], cwd })
    .pipe(Effect.orElseSucceed(() => ({ stdout: "", stderr: "" })));
  const ready = pipe(
    rows(sdks.stdout),
    Arr.some((row) => Str.startsWith(DOTNET_MAJOR)(sdkVersion(row))),
  );
  if (ready) {
    return { command: "dotnet", env: {} } satisfies Sdk;
  }
  return yield* installDotnet(cwd);
});

export type { Choice };
export { dotnet, pick };
