import { createIsomorphicFn } from "@tanstack/react-start";
import { browserClient, createQueries, serverClient } from "@workspace/start/api-client";
import { Effect } from "effect";

import type { WikiPageRoutes } from "./routes.server";

const loadWikiPageRoutes = (): Promise<WikiPageRoutes> =>
  Effect.runPromise(
    Effect.promise(() => import("./routes.server")).pipe(
      Effect.map((module) => module.wikiPageRoutes),
    ),
  );

const client = createIsomorphicFn()
  .server(() => serverClient<WikiPageRoutes>(loadWikiPageRoutes))
  .client(() => browserClient<WikiPageRoutes>());

const wikiPageQueries = createQueries<WikiPageRoutes>(client());

export { wikiPageQueries };
