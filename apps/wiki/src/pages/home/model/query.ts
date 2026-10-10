import { useAtomSet, useAtomValue } from "@effect/atom-react";
import { Atom } from "effect/reactivity";

const NO_QUERY = "";

const searchQueryAtom = Atom.make(NO_QUERY);

const useSearchQuery = (): string => useAtomValue(searchQueryAtom);

const useSetSearchQuery = (): ((query: string) => void) => useAtomSet(searchQueryAtom);

export { NO_QUERY, useSearchQuery, useSetSearchQuery };
