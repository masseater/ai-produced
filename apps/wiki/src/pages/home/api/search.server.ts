import { env } from "cloudflare:workers";
import { Array as Arr, Context, Effect, Layer, ManagedRuntime, Number as Num, Order } from "effect";

import type { PageSummary } from "#/shared/content";
import { wikiPages } from "#/shared/content/index.server";

const MODEL = "@cf/google/embeddinggemma-300m";
const BATCH_SIZE = 50;
const DOCUMENT_PREFIX = "title: ";
const QUERY_PREFIX = "task: search result | query: ";
const RESULT_LIMIT = 10;
const EMPTY = 0;
const SECTION_PATTERN = /^## /mu;

type Passage = Readonly<{
  section: PageSummary["section"];
  name: string;
  title: string;
  text: string;
}>;
type SearchHit = Passage & Readonly<{ score: number }>;
type Vector = readonly number[];

const passagesOf = (page: PageSummary & Readonly<{ body: string }>): readonly Passage[] =>
  page.body
    .split(SECTION_PATTERN)
    .map((part) => part.trim())
    .filter((part) => part.length > EMPTY)
    .map((text) => ({ section: page.section, name: page.name, title: page.title, text }));

const passages: readonly Passage[] = wikiPages.flatMap((page) => passagesOf(page));

const runEmbedding = (batch: readonly string[]): Effect.Effect<readonly Vector[]> =>
  Effect.promise(() => env.AI.run(MODEL, { text: [...batch] })).pipe(
    Effect.map((output) => output.data),
  );

const embed = (texts: readonly string[]): Effect.Effect<readonly Vector[]> =>
  Effect.forEach(Arr.chunksOf(texts, BATCH_SIZE), runEmbedding).pipe(
    Effect.map((outputs) => outputs.flat()),
  );

class PassageIndex extends Context.Service<PassageIndex, { readonly vectors: readonly Vector[] }>()(
  "wiki/pages/home/api/search.server/PassageIndex",
) {}

const passageIndexLive = Layer.effect(
  PassageIndex,
  embed(
    passages.map((passage) => `${DOCUMENT_PREFIX}${passage.title} | text: ${passage.text}`),
  ).pipe(Effect.map((vectors) => PassageIndex.of({ vectors }))),
);

const runtime = ManagedRuntime.make(passageIndexLive);

const dot = (left: Vector, right: Vector): number =>
  Num.sumAll(Arr.zipWith(left, right, (leftValue, rightValue) => leftValue * rightValue));

const cosine = (left: Vector, right: Vector): number =>
  dot(left, right) / Math.sqrt(dot(left, left) * dot(right, right));

const byScore = Order.flip(Order.mapInput(Num.Order, (hit: SearchHit) => hit.score));

const rank = (queryVector: Vector, vectors: readonly Vector[]): readonly SearchHit[] =>
  Arr.take(
    Arr.sort(
      Arr.zipWith(passages, vectors, (passage, vector) => ({
        ...passage,
        score: cosine(queryVector, vector),
      })),
      byScore,
    ),
    RESULT_LIMIT,
  );

const searchPages = (query: string): Promise<readonly SearchHit[]> =>
  runtime.runPromise(
    Effect.gen(function* search() {
      const { vectors } = yield* PassageIndex;
      const [queryVector = []] = yield* embed([`${QUERY_PREFIX}${query}`]);
      return rank(queryVector, vectors);
    }).pipe(Effect.withSpan("wiki.search")),
  );

const toSummary = (page: PageSummary): PageSummary => ({
  slug: page.slug,
  section: page.section,
  name: page.name,
  title: page.title,
  artist: page.artist,
  tags: page.tags,
});

const pageSummaries: readonly PageSummary[] = wikiPages.map((page) => toSummary(page));

export type { SearchHit };
export { pageSummaries, searchPages };
