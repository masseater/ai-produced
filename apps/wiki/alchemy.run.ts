import { Website, Workers, providers, state } from "@alchemy.run/cloudflare";
import type { InferEnv } from "@alchemy.run/cloudflare";
import { Stack } from "alchemy";
import { Effect } from "effect";

const wiki = Website.Vite("Wiki", {
  env: { AI: Workers.AI() },
  observability: { enabled: true },
});

type WikiEnv = InferEnv<typeof wiki>;

export type { WikiEnv };
export default Stack(
  "ai-produced-wiki",
  { providers: providers(), state: state() },
  Effect.gen(function* stack() {
    const { url } = yield* wiki;
    return { url };
  }),
);
