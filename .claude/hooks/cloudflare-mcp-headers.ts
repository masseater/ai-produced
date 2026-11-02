import { CloudflareApiLive, Credentials } from "@alchemy.run/cloudflare";
import { NodeServices } from "@effect/platform-node";
import { AuthProviders } from "alchemy";
import { Effect, Layer, Match, Redacted, Schema } from "effect";
import { FetchHttpClient } from "effect/http";

import { runHookWith, writeJson } from "./env.ts";

const Headers = Schema.Record(Schema.String, Schema.String);

const cloudflareApi = CloudflareApiLive().pipe(
  Layer.provide(Layer.succeed(AuthProviders, {})),
  Layer.provide(NodeServices.layer),
  Layer.provide(FetchHttpClient.layer),
);

runHookWith(cloudflareApi)(
  Effect.gen(function* cloudflareMcpHeaders() {
    const resolve = yield* Credentials;
    const credentials = yield* resolve;
    yield* writeJson(
      Headers,
      Match.value(credentials).pipe(
        Match.when({ type: "apiKey" }, ({ apiKey, email }) => ({
          "X-Auth-Key": Redacted.value(apiKey),
          "X-Auth-Email": email,
        })),
        Match.when({ type: "apiToken" }, ({ apiToken }) => ({
          Authorization: `Bearer ${Redacted.value(apiToken)}`,
        })),
        Match.when({ type: "oauth" }, ({ accessToken }) => ({
          Authorization: `Bearer ${Redacted.value(accessToken)}`,
        })),
        Match.exhaustive,
      ),
    );
  }),
);
