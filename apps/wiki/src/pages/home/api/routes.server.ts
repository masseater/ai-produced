import { createRouter } from "@workspace/start/api-router";
import { t } from "elysia";

import { pageSummaries, searchPages } from "./search.server";

const MAX_QUERY_LENGTH = 200;

const homeRoutes = createRouter("/api/pages")
  .get("/", () => pageSummaries)
  .get("/search", ({ query }) => searchPages(query.text), {
    query: t.Object({ text: t.String({ minLength: 1, maxLength: MAX_QUERY_LENGTH }) }),
  });

type HomeRoutes = typeof homeRoutes;

export type { HomeRoutes };
export { homeRoutes };
