import { createFileRoute } from "@tanstack/react-router";
import { apiHandlers } from "@workspace/start/api-route";

import { api } from "#/app/server/api.server";

export const Route = createFileRoute("/api/$")({
  server: { handlers: apiHandlers(api) },
});
