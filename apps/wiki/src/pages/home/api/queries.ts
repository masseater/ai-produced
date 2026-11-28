import { createIsomorphicFn } from "@tanstack/react-start";
import { browserClient, createQueries, serverClient } from "@workspace/start/api-client";
import { Effect } from "effect";

import type { HomeRoutes } from "./routes.server.ts";

const loadHomeRoutes = (): Promise<HomeRoutes> =>
  Effect.runPromise(
    Effect.promise(() => import("./routes.server.ts")).pipe(
      Effect.map((module) => module.homeRoutes),
    ),
  );

const client = createIsomorphicFn()
  .server(() => serverClient<HomeRoutes>(loadHomeRoutes))
  .client(() => browserClient<HomeRoutes>());

export const homeQueries = createQueries<HomeRoutes>(client());
