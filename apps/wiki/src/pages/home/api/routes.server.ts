import { createRouter } from "@workspace/start/api-router";
import { t } from "elysia";

import { pageSummaries, searchPages } from "./search.server";

const homeRoutes = createRouter("/api/pages")
  .get("/", () => pageSummaries)
  .get("/search", ({ query }) => searchPages(query.text), {
    query: t.Object({ text: t.String({ minLength: 1 }) }),
  });

type HomeRoutes = typeof homeRoutes;

export type { HomeRoutes };
export { homeRoutes };
