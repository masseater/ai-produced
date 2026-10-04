import { Option, Schema } from "effect";
import frontMatter from "front-matter";

import type { WikiPage } from "./types";

const Attributes = Schema.Struct({
  title: Schema.optional(Schema.String),
  artist: Schema.optional(Schema.String),
  tags: Schema.optional(Schema.Array(Schema.String)),
});

const decodeAttributes = Schema.decodeUnknownOption(Attributes);

const sources = import.meta.glob<string>("../../../../../docs/wiki/{works,topics}/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
});

const SOURCE_PATTERN = /\/docs\/wiki\/(?<section>works|topics)\/(?<name>[^/]+)\.md$/u;
const HEADING_PATTERN = /^# (?<heading>.+)$/mu;

const sourceGroup = (path: string, name: string): Option.Option<string> =>
  Option.fromNullishOr(SOURCE_PATTERN.exec(path)).pipe(
    Option.flatMapNullishOr((match) => match.groups),
    Option.flatMapNullishOr((groups) => groups[name]),
  );

const headingOf = (body: string): Option.Option<string> =>
  Option.fromNullishOr(HEADING_PATTERN.exec(body)).pipe(
    Option.flatMapNullishOr((match) => match.groups),
    Option.flatMapNullishOr((groups) => groups["heading"]),
  );

const isSection = Schema.is(Schema.Literals(["works", "topics"]));

const toPage = ([path, raw]: readonly [string, string]): Option.Option<WikiPage> =>
  Option.all({
    section: sourceGroup(path, "section").pipe(Option.filter(isSection)),
    name: sourceGroup(path, "name"),
  }).pipe(
    Option.map(({ section, name }) => {
      const { attributes, body } = frontMatter(raw);
      const decoded = decodeAttributes(attributes);
      return {
        slug: `${section}/${name}`,
        section,
        name,
        title: decoded.pipe(
          Option.flatMapNullishOr((value) => value.title),
          Option.orElse(() => headingOf(body)),
          Option.getOrElse(() => name),
        ),
        artist: decoded.pipe(
          Option.flatMapNullishOr((value) => value.artist),
          Option.getOrElse(() => ""),
        ),
        tags: decoded.pipe(
          Option.flatMapNullishOr((value) => value.tags),
          Option.getOrElse(() => []),
        ),
        body,
      };
    }),
  );

const wikiPages: readonly WikiPage[] = Object.entries(sources).flatMap((entry) =>
  Option.toArray(toPage(entry)),
);

const findPage = (slug: string): Option.Option<WikiPage> =>
  Option.fromNullishOr(wikiPages.find((page) => page.slug === slug));

export { findPage, wikiPages };
