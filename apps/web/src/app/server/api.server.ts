import { createRouter } from "@workspace/start/api-router";

import { todoRoutes } from "#/pages/home/index.server";
import { auth } from "#/shared/auth/index.server";

export const api = createRouter("").mount(auth.handler).use(todoRoutes);
