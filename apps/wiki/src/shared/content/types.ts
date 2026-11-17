export type Section = "works" | "topics";

export type PageSummary = Readonly<{
  slug: string;
  section: Section;
  name: string;
  title: string;
  artist: string;
  tags: readonly string[];
}>;

export type WikiPage = PageSummary & Readonly<{ source: string; body: string }>;
