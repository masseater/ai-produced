type Section = "works" | "topics";

type PageSummary = Readonly<{
  slug: string;
  section: Section;
  name: string;
  title: string;
  artist: string;
  tags: readonly string[];
}>;

type WikiPage = PageSummary & Readonly<{ source: string; body: string }>;

export type { PageSummary, Section, WikiPage };
