import { createIsomorphicFn } from "@tanstack/react-start";
import { browserClient, createQueries, serverClient } from "@workspace/start/api-client";
import { Effect } from "effect";

import type { TodoRoutes } from "./routes.server";

const loadTodoRoutes = (): Promise<TodoRoutes> =>
  Effect.runPromise(
    Effect.promise(() => import("./routes.server")).pipe(Effect.map((module) => module.todoRoutes)),
  );

const client = createIsomorphicFn()
  .server(() => serverClient<TodoRoutes>(loadTodoRoutes))
  .client(() => browserClient<TodoRoutes>());

const todoQueries = createQueries<TodoRoutes>(client());

export { todoQueries };
