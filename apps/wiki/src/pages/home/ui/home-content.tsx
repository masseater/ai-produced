import { useAtomValue } from "@effect/atom-react";
import { Match } from "effect";
import type { ReactNode } from "react";

import { NO_QUERY, searchQueryAtom } from "#/pages/home/model/query";

import { PageList } from "./page-list";
import { SearchResults } from "./search-results";

const HomeContent = (): ReactNode =>
  Match.value(useAtomValue(searchQueryAtom)).pipe(
    Match.when(NO_QUERY, () => <PageList />),
    Match.orElse((query) => <SearchResults query={query} />),
  );

export { HomeContent };
