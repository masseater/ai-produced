import { Effect, Schema } from "effect";

const taggedError = Schema.TaggedError;

class TaskFailed extends taggedError<TaskFailed, { readonly brand: unique symbol }>()(
  "TaskFailed",
  { message: Schema.String },
) {}

type Expectation = Readonly<{ holds: boolean; message: string }>;

const expect = (expectation: Expectation): Effect.Effect<void, TaskFailed> => {
  if (expectation.holds) {
    return Effect.void;
  }
  return Effect.fail(TaskFailed.make({ message: expectation.message }));
};

export type { Expectation };
export { TaskFailed, expect };
