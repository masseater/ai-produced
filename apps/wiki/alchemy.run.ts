import { Website, Workers, providers, state } from "@alchemy.run/cloudflare";
import type { InferEnv } from "@alchemy.run/cloudflare";
import { Stack } from "alchemy";
import { Effect } from "effect";

import { APP_NAME, requireAppPrefix } from "./alchemy.app-name.ts";

const wiki = Website.Vite("Wiki", {
  env: { AI: Workers.AI() },
  observability: { enabled: true },
  logpush: true,
  limits: { cpuMs: 1000 },
});

export type WikiEnv = InferEnv<typeof wiki>;

export default Stack(
  APP_NAME,
  { providers: providers(), state: state() },
  Effect.gen(function* stack() {
    const { url } = yield* wiki;
    yield* requireAppPrefix;
    return { url };
  }),
);
