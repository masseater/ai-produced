import { Atom } from "effect/reactivity";

const NO_QUERY = "";

const searchQueryAtom = Atom.make(NO_QUERY);

export { NO_QUERY, searchQueryAtom };
