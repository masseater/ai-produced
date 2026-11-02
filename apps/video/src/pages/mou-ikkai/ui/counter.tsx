import { String as Str, pipe } from "effect";
import type { ReactNode } from "react";

const DIGITS = 2;

type CounterProps = Readonly<{ hook: number }>;

const Counter = ({ hook }: CounterProps): ReactNode => (
  <div className="absolute bottom-14 left-18 font-sans text-5xl font-normal text-(--fg) tabular-nums">
    {`#${pipe(String(hook), Str.padStart(DIGITS, "0"))}`}
  </div>
);

export { Counter };
