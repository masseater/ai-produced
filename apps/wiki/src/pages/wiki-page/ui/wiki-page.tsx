import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import { useMemo } from "react";
import type { ReactNode } from "react";
import Markdown from "react-markdown";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { wikiPageQueries } from "#/pages/wiki-page/api/queries";

import { MarkdownLink } from "./markdown-link";

const REMARK_PLUGINS = [remarkGfm];
const BACK_LABEL = "一覧に戻る";
const ANCHOR = "a";

const componentsFor = (slug: string): Components => ({
  [ANCHOR]: ({ href = "", children }) => (
    <MarkdownLink href={href} slug={slug}>
      {children}
    </MarkdownLink>
  ),
});

const WikiPage = (): ReactNode => {
  const { section, name } = useParams({ from: "/wiki/$section/$name" });
  const slug = `${section}/${name}`;
  const { data } = useSuspenseQuery(wikiPageQueries.api.page.get.queryOptions({ slug }));
  const components = useMemo(() => componentsFor(slug), [slug]);
  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-4 p-8">
      <Link to="/" className="text-sm underline">
        {BACK_LABEL}
      </Link>
      <article className="prose max-w-none">
        <Markdown remarkPlugins={REMARK_PLUGINS} components={components}>
          {data.body}
        </Markdown>
      </article>
    </main>
  );
};

export { WikiPage };
