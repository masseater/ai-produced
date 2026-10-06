import type { Router } from "@tanstack/react-router";
import { createQueryRouter } from "@workspace/start/router";

import { routeTree } from "./generated/routeTree.gen";

type AppRouter = Router<typeof routeTree>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}

const getRouter = (): AppRouter => createQueryRouter(routeTree);

export type { AppRouter };
export { getRouter };
