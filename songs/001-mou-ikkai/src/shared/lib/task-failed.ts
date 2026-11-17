import { Effect, Schema } from "effect";

const taggedError = Schema.TaggedError;

export class TaskFailed extends taggedError<TaskFailed, { readonly brand: unique symbol }>()(
  "TaskFailed",
  { message: Schema.String },
) {}

export type Expectation = Readonly<{ holds: boolean; message: string }>;

export const expect = (expectation: Expectation): Effect.Effect<void, TaskFailed> => {
  if (expectation.holds) {
    return Effect.void;
  }
  return Effect.fail(TaskFailed.make({ message: expectation.message }));
};
