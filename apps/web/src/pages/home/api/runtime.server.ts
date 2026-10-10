import { waitUntil } from "cloudflare:workers";
import { Effect, Layer, ManagedRuntime } from "effect";
import { OtlpExporter } from "effect/observability";

import { featureFlagsLive } from "#/shared/flags/index.server";
import { telemetryLive } from "#/shared/telemetry/index.server";

const servicesLive = Layer.mergeAll(featureFlagsLive, telemetryLive);

const runtime = ManagedRuntime.make(Layer.mergeAll(servicesLive, OtlpExporter.layerFlusher));

type Services = Layer.Success<typeof servicesLive>;

const flushTelemetry = Effect.gen(function* flushTelemetry() {
  const flusher = yield* OtlpExporter.Flusher;
  yield* flusher.flush;
});

const scheduleFlush = Effect.sync(() => {
  waitUntil(runtime.runPromise(flushTelemetry));
});

const runRequest = <Success>(effect: Effect.Effect<Success, never, Services>): Promise<Success> =>
  runtime.runPromise(effect.pipe(Effect.ensuring(scheduleFlush)));

export { runRequest };
