import type { Router } from "@tanstack/react-router";
import { createQueryRouter } from "@workspace/start/router";

import { routeTree } from "./generated/routeTree.gen";

export type AppRouter = Router<typeof routeTree>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}

export const getRouter = (): AppRouter => createQueryRouter(routeTree);
