import { Option } from "effect";

const ORIGIN = "https://wiki.invalid";
const WIKI_ROOT = "/docs/wiki/";
const PAGE_PATTERN = /^\/docs\/wiki\/(?<section>works|topics)\/(?<name>[^/]+?)(?:\/index)?\.md$/u;
const REPOSITORY_BLOB = "https://github.com/masseater/ai-produced/blob/main";

type ResolvedLink =
  | Readonly<{ kind: "page"; section: string; name: string }>
  | Readonly<{ kind: "external"; href: string }>;

const pageOf = (pathname: string): Option.Option<ResolvedLink> =>
  Option.liftThrowable(decodeURIComponent)(pathname).pipe(
    Option.flatMapNullishOr((decoded) => PAGE_PATTERN.exec(decoded)),
    Option.flatMapNullishOr((match) => match.groups),
    Option.flatMap((groups) =>
      Option.all({
        section: Option.fromNullishOr(groups["section"]),
        name: Option.fromNullishOr(groups["name"]),
      }),
    ),
    Option.map(({ section, name }) => ({ kind: "page", section, name }) as const),
  );

const resolveLink = ({
  href,
  currentPath,
}: Readonly<{ href: string; currentPath: string }>): ResolvedLink => {
  const url = new URL(href, `${ORIGIN}${WIKI_ROOT}${currentPath}`);
  return Option.some(url).pipe(
    Option.filter((candidate) => candidate.origin === ORIGIN),
    Option.match({
      onNone: (): ResolvedLink => ({ kind: "external", href }),
      onSome: (local) =>
        pageOf(local.pathname).pipe(
          Option.getOrElse((): ResolvedLink => ({
            kind: "external",
            href: `${REPOSITORY_BLOB}${local.pathname}`,
          })),
        ),
    }),
  );
};

export type { ResolvedLink };
export { resolveLink };
