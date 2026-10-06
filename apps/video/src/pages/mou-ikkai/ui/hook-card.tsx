import type { ReactNode } from "react";

import { HookRow } from "#/pages/mou-ikkai/ui/hook-row";

const TILES = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"] as const;

type HookCardProps = Readonly<{ sung: number; tiled: boolean }>;

const HookCard = ({ sung, tiled }: HookCardProps): ReactNode => {
  if (!tiled) {
    return <HookRow sung={sung} size="card" />;
  }
  return (
    <div className="grid grid-cols-3 gap-x-24 gap-y-12">
      {TILES.map((tile) => (
        <HookRow key={tile} sung={sung} size="tile" />
      ))}
    </div>
  );
};

export { HookCard };
