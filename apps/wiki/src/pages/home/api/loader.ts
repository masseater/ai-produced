import type { QueryClient } from "@tanstack/react-query";

import { homeQueries } from "./queries";

export const loadHomePage = (queryClient: QueryClient): Promise<unknown> =>
  queryClient.query({ ...homeQueries.api.pages.get.queryOptions(), staleTime: "static" });
