import { RegistryProvider } from "@effect/atom-react";
import { HeadContent, Scripts } from "@tanstack/react-router";
import type { ReactNode } from "react";

const RootDocument = ({ children }: Readonly<{ children: ReactNode }>): ReactNode => (
  <html lang="ja">
    <head>
      <HeadContent />
    </head>
    <body>
      <RegistryProvider>{children}</RegistryProvider>
      <Scripts />
    </body>
  </html>
);

export { RootDocument };
