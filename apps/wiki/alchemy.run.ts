import { Stack } from "alchemy";
import { Website, Workers, providers, state } from "alchemy/Cloudflare";
import type { InferEnv } from "alchemy/Cloudflare";
import { Effect } from "effect";

const wiki = Website.Vite("Wiki", {
  env: { AI: Workers.AI() },
  observability: { enabled: true },
});

type WikiEnv = InferEnv<typeof wiki>;

export type { WikiEnv };
export default Stack(
  "wiki",
  { providers: providers(), state: state() },
  Effect.gen(function* stack() {
    const { url } = yield* wiki;
    return { url };
  }),
);
