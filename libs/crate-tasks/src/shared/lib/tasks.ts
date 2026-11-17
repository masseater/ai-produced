import type { UserConfig } from "vite-plus";

export type Task = NonNullable<NonNullable<UserConfig["run"]>["tasks"]>[string];

export const rustupToolchain = (root: string): string => `RUSTUP_TOOLCHAIN=${root}/.pnpm/rust`;

const cargoCommands = (crate: string, rust: string): string[] => [
  `${rust} cargo fmt -p ${crate} --check`,
  `${rust} cargo clippy -p ${crate} --all-targets --locked -- -D warnings`,
  `${rust} cargo test -p ${crate} --locked`,
];

export const cargoTask = ({ crate, root }: Readonly<{ crate: string; root: string }>): Task => ({
  command: cargoCommands(crate, rustupToolchain(root)),
  cache: {
    input: [{ auto: true }, { pattern: "!target/**", base: "workspace" }],
    output: [],
  },
});
