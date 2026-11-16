import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      verify: { command: [], dependsOn: ["cargo"] },
      cargo: {
        command: [
          "cargo fmt -p song --check",
          "cargo clippy -p song --all-targets --locked -- -D warnings",
          "cargo test -p song --locked",
        ],
        cache: {
          input: [{ auto: true }, { pattern: "!target/**", base: "workspace" }],
          output: [],
        },
      },
    },
  },
});
