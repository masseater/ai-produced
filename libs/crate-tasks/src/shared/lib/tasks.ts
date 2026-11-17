import type { UserConfig } from "vite-plus";

type Task = NonNullable<NonNullable<UserConfig["run"]>["tasks"]>[string];

const rustupToolchain = (root: string): string => `RUSTUP_TOOLCHAIN=${root}/.pnpm/rust`;

const cargoCommands = (crate: string, rust: string): string[] => [
  `${rust} cargo fmt -p ${crate} --check`,
  `${rust} cargo clippy -p ${crate} --all-targets --locked -- -D warnings`,
  `${rust} cargo test -p ${crate} --locked`,
];

const cargoTask = ({ crate, root }: Readonly<{ crate: string; root: string }>): Task => ({
  command: cargoCommands(crate, rustupToolchain(root)),
  cache: {
    input: [{ auto: true }, { pattern: "!target/**", base: "workspace" }],
    output: [],
  },
});

export { cargoTask, rustupToolchain };
export type { Task };
