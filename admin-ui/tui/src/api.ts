/* The routes the panes read and write, over any transport. The shapes are the
 * web UI's (admin-ui/app/src/lib/api.ts, transcribed from backend/schema.json);
 * only the way a request travels differs: every one is an envelope. */

import { call, errorText, type Answer, type Session } from "./envelope";

export interface Transport {
  request(method: string, path: string, body?: unknown): Promise<Answer>;
}

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export function envelopeTransport(session: Session): Transport {
  return { request: (method, path, body) => call(session, method, path, body) };
}

/** A route's answer, or an ApiError carrying the box's own sentence. */
export async function request<T>(transport: Transport, method: string, path: string, body?: unknown): Promise<T> {
  const answer = await transport.request(method, path, body);
  if (answer.status >= 400) throw new ApiError(answer.status, errorText(answer));
  return answer.body as T;
}

export const get = <T>(transport: Transport, path: string): Promise<T> => request<T>(transport, "GET", path);
export const post = <T>(transport: Transport, path: string, body?: unknown): Promise<T> =>
  request<T>(transport, "POST", path, body);

export function describeError(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return String(error);
}
