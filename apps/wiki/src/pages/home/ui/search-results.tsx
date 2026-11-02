import { useQuery } from "@tanstack/react-query";
import { Array as Arr, Match, String as Str, pipe } from "effect";
import type { ReactNode } from "react";

import { homeQueries } from "#/pages/home/api/queries";
import { WikiLink } from "#/shared/ui/wiki-link";

const START = 0;
const EXCERPT_LENGTH = 160;
const PENDING = "検索しています";
const FAILED = "検索に失敗しました";

const SearchResults = ({ query }: Readonly<{ query: string }>): ReactNode =>
  Match.value(useQuery(homeQueries.api.pages.search.get.queryOptions({ text: query }))).pipe(
    Match.discriminatorsExhaustive("status")({
      pending: () => <p className="text-muted-foreground text-sm">{PENDING}</p>,
      error: () => <p className="text-destructive text-sm">{FAILED}</p>,
      success: ({ data }) => (
        <ol className="flex flex-col gap-3">
          {pipe(
            data,
            Arr.map((hit) => {
              const excerpt = Str.slice(START, EXCERPT_LENGTH)(hit.text);
              return (
                <li key={hit.id}>
                  <WikiLink section={hit.section} name={hit.name}>
                    {hit.title}
                  </WikiLink>
                  <p className="text-muted-foreground text-sm">{excerpt}</p>
                </li>
              );
            }),
          )}
        </ol>
      ),
    }),
  );

export { SearchResults };
