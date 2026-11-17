import type { QueryClient } from "@tanstack/react-query";

import { wikiPageQueries } from "./queries";

export const loadWikiPage = ({
  queryClient,
  slug,
}: Readonly<{ queryClient: QueryClient; slug: string }>): Promise<unknown> =>
  queryClient.query({
    ...wikiPageQueries.api.page.get.queryOptions({ slug }),
    staleTime: "static",
  });
