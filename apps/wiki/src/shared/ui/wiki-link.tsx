import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import type { ReactNode } from "react";

const WikiLink = ({
  section,
  name,
  children,
}: Readonly<{ section: string; name: string; children: ReactNode }>): ReactNode => {
  const params = useMemo(() => ({ section, name }), [section, name]);
  return (
    <Link to="/wiki/$section/$name" params={params} className="underline">
      {children}
    </Link>
  );
};

export { WikiLink };
