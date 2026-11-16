import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      verify: { command: [], dependsOn: ["cargo"] },
      cargo: {
        command: [
          "cargo fmt -p transcribe --check",
          "cargo clippy -p transcribe --all-targets --locked -- -D warnings",
          "cargo test -p transcribe --locked",
        ],
        cache: {
          input: [{ auto: true }, { pattern: "!target/**", base: "workspace" }],
          output: [],
        },
      },
    },
  },
});
