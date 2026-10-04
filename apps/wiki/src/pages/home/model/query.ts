import { make } from "effect/unstable/reactivity/Atom";

const NO_QUERY = "";

const searchQueryAtom = make(NO_QUERY);

export { NO_QUERY, searchQueryAtom };
