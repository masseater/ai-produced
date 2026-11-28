# ai-produced

Follow `masseater-ecosystem`, a skill in masseater/skills that says which skills to load in each situation. `vp install` puts the skills in `.agents/skills/`. This repository is based on https://github.com/masseater/typescript-template, and the structure and policies that come from the template follow the template.

An AI makes songs here with the same steps a Vocaloid producer takes. Each piece lives in `songs/<number>-<name>/` with the products of each step and a log.md, and `docs/songs.md` describes the layout.

Each workspace has its own AGENTS.md describing it and the rules that apply only there. Read it before changing that workspace.

## Tech stack

The base stack is TypeScript 7, Effect, Vite+ and Rust.

## Commands

- `vp run verify` runs every check in this repository, including rustfmt, clippy and the crate tests. The pre-push hook runs the same checks.
- Run a Rust tool with `cargo run -p <crate> --bin <tool> -- <args>`.

## Rules

- Do not operate the screen (screenshots, clicks, key presses). Operate tools through MCP, scripts and project files only.
- Take each piece through the steps in `docs/songs.md` without skipping any step a human producer takes. After each step, write the decisions and the reasons for any redo in that piece's log.md.
- Judge rendered sound by analysis results such as loudness, spectrogram and pitch, and record the numbers in log.md.
- Wait for the owner's instruction before deploying to production or publishing anything outside the repository.
- This repository is public. Do not add material derived from other people's songs or videos.
- Steps that need something on the Windows PC, such as REAPER or OpenUtau singers, run in a Remote Control session on that PC.
- The only workflow that runs on `pull_request` is `up-to-date`, which checks nothing and lets the ruleset refuse to merge a pull request that has not taken in the latest main.
