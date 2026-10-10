import { Match } from "effect";
import type { ReactNode } from "react";

import { NO_QUERY, useSearchQuery } from "#/pages/home/model/query";

import { PageList } from "./page-list";
import { SearchResults } from "./search-results";

const HomeContent = (): ReactNode =>
  Match.value(useSearchQuery()).pipe(
    Match.when(NO_QUERY, () => <PageList />),
    Match.orElse((query) => <SearchResults query={query} />),
  );

export { HomeContent };
