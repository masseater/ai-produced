import { useSuspenseQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { homeQueries } from "#/pages/home/api/queries";
import { WikiLink } from "#/shared/ui/wiki-link";

const PageList = (): ReactNode => {
  const { data } = useSuspenseQuery(homeQueries.api.pages.get.queryOptions());
  return (
    <ul className="flex flex-col gap-1">
      {data.map((page) => (
        <li key={page.slug} className="flex gap-2">
          <WikiLink section={page.section} name={page.name}>
            {page.title}
          </WikiLink>
          <span className="text-muted-foreground text-sm">{page.artist}</span>
        </li>
      ))}
    </ul>
  );
};

export { PageList };
