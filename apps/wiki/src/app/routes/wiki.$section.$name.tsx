import { createFileRoute } from "@tanstack/react-router";

import { WikiPage, loadWikiPage } from "#/pages/wiki-page";

const Route = createFileRoute("/wiki/$section/$name")({
  loader: ({ context, params }) =>
    loadWikiPage({ queryClient: context.queryClient, slug: `${params.section}/${params.name}` }),
  component: WikiPage,
});

export { Route };
