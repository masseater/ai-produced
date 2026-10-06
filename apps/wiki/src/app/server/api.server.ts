import { createRouter } from "@workspace/start/api-router";

import { homeRoutes } from "#/pages/home/index.server";
import { wikiPageRoutes } from "#/pages/wiki-page/index.server";

export const api = createRouter("").use(homeRoutes).use(wikiPageRoutes);
