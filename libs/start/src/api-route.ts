type RequestHandler = (context: Readonly<{ request: Request }>) => Promise<Response>;

type ApiHandlers = Readonly<Record<"GET" | "POST" | "PUT" | "PATCH" | "DELETE", RequestHandler>>;

const apiHandlers = (
  app: Readonly<{ handle: (request: Request) => Promise<Response> }>,
): ApiHandlers => {
  const handle: RequestHandler = ({ request }) => app.handle(request);
  return { GET: handle, POST: handle, PUT: handle, PATCH: handle, DELETE: handle };
};

export type { ApiHandlers, RequestHandler };
export { apiHandlers };
