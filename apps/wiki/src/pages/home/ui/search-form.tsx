import { useAtomSet } from "@effect/atom-react";
import { Button } from "@workspace/ui/components/button";
import { Option } from "effect";
import { useCallback } from "react";
import type { ReactNode } from "react";

import { NO_QUERY, searchQueryAtom } from "#/pages/home/model/query";

const FIELD = "text";
const LABEL = "wikiを検索";
const PLACEHOLDER = "例: 低域が厚くて音圧が高い曲";
const SUBMIT = "検索";

const textOf = (formData: Readonly<FormData>): string =>
  Option.fromNullishOr(formData.get(FIELD)).pipe(
    Option.filter((value) => typeof value === "string"),
    Option.map((value) => value.trim()),
    Option.getOrElse(() => NO_QUERY),
  );

const SearchForm = (): ReactNode => {
  const setQuery = useAtomSet(searchQueryAtom);
  const submit = useCallback(
    (formData: Readonly<FormData>) => {
      setQuery(textOf(formData));
    },
    [setQuery],
  );
  return (
    <search>
      <form className="flex gap-2" action={submit}>
        <input
          name={FIELD}
          type="search"
          aria-label={LABEL}
          placeholder={PLACEHOLDER}
          className="bg-background h-9 flex-1 rounded-md border px-3 text-sm"
        />
        <Button type="submit">{SUBMIT}</Button>
      </form>
    </search>
  );
};

export { SearchForm };
