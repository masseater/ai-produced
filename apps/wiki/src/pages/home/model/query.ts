import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { Atom } from "effect/reactivity";

export const NO_QUERY = "";

const searchQueryAtom = Atom.make(NO_QUERY);

export const useSearchQuery = (): string => useAtomValue(searchQueryAtom);

export const useSetSearchQuery = (): ((query: string) => void) => useAtomSet(searchQueryAtom);
