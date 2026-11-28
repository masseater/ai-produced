import type { ReactNode } from "react";

import { HomeContent } from "./home-content.tsx";
import { SearchForm } from "./search-form.tsx";

const TITLE = "分析wiki";

export const HomePage = (): ReactNode => (
  <main className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
    <h1 className="text-2xl font-bold">{TITLE}</h1>
    <SearchForm />
    <HomeContent />
  </main>
);
