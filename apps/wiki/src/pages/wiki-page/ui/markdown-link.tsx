import { Match } from "effect";
import type { ReactNode } from "react";

import { resolveLink } from "#/pages/wiki-page/model/links";
import { WikiLink } from "#/shared/ui/wiki-link";

const LINK_CLASS = "underline";
const NEW_TAB_LABEL = "（新しいタブで開く）";

const MarkdownLink = ({
  href,
  source,
  children,
}: Readonly<{ href: string; source: string; children: ReactNode }>): ReactNode =>
  Match.value(resolveLink({ href, currentPath: source })).pipe(
    Match.discriminatorsExhaustive("kind")({
      page: (link) => (
        <WikiLink section={link.section} name={link.name}>
          {children}
        </WikiLink>
      ),
      external: (link) => (
        <a href={link.href} className={LINK_CLASS} rel="noreferrer" target="_blank">
          {children}
          <span className="sr-only">{NEW_TAB_LABEL}</span>
        </a>
      ),
    }),
  );

export { MarkdownLink };
