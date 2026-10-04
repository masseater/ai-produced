import { createRouter } from "@workspace/start/api-router";
import { Option } from "effect";
import { t } from "elysia";

import { findPage } from "#/shared/content/index.server";

const NOT_FOUND = 404;

const wikiPageRoutes = createRouter("/api/page").get(
  "/",
  ({ query, status }) =>
    Option.match(findPage(query.slug), {
      onNone: () => status(NOT_FOUND, "not found"),
      onSome: (page) => page,
    }),
  { query: t.Object({ slug: t.String() }) },
);

type WikiPageRoutes = typeof wikiPageRoutes;

export type { WikiPageRoutes };
export { wikiPageRoutes };
