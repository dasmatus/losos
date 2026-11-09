/* The panes' read plumbing, the port of Panes.kt's Read/ReadRoute pair.
 *
 * One read of one route, fresh when the pane opens (the terminal's `r`): a
 * wait, the box's own error word, or the document itself. The Compose app
 * keyed the LaunchedEffect on path/host/token/anonymous/refresh; here the
 * hook's effect does exactly that keying, and the caller re-arms a refresh
 * counter after an action (Panes.kt did the same with mutableIntStateOf).
 */

import * as React from "react";

import { BoxApi, failWord, type JsonBody } from "../api/api";

export type Read =
  | { kind: "loading" }
  | { kind: "failed"; reason: string }
  | { kind: "done"; data: JsonBody };

/** The box API for `config`, with the SPA's eviction: a 401 drops the
 *  token in the config store, and the app shell then shows the connect
 *  screen again (MainActivity's needsConnect flowed on exactly this).
 *  Holds on host+token, as `remember` did. */
export function useBoxApi(host: string, token: string, onUnauthorized: () => void): BoxApi {
  const apiRef = React.useRef<BoxApi | null>(null);
  const key = `${host}\u0000${token}`;
  if (apiRef.current === null || (apiRef.current as unknown as { key?: string }).key !== key) {
    const api = new BoxApi(host, token, onUnauthorized);
    (api as unknown as { key?: string }).key = key;
    apiRef.current = api;
  }
  return apiRef.current as BoxApi;
}

/** The read a card shows, once per key like the Compose LaunchedEffect's
 *  remember of the Read. */
export function useRead(
  api: BoxApi,
  path: string,
  { anonymous = false, refresh = 0 }: { anonymous?: boolean; refresh?: number } = {},
): Read {
  const [read, setRead] = React.useState<Read>({ kind: "loading" });
  React.useEffect(() => {
    let live = true;
    setRead({ kind: "loading" });
    api
      .get(path, { anonymous })
      .then((data) => {
        if (live) setRead({ kind: "done", data });
      })
      .catch((e: unknown) => {
        if (live) setRead({ kind: "failed", reason: failWord(e) });
      });
    return () => {
      live = false;
    };
  }, [api, path, anonymous, refresh]);
  return read;
}

/** ReadRoute as a component: the cards that draw straight out of one read
 *  (the read carried its own Read.Loading / Read.Failed bodies under
 *  Compose; here the caller chooses). */
export function ReadSlot({
  read,
  loadingWord = "Asking the box…",
  children,
}: {
  read: Read;
  loadingWord?: string;
  children: (body: JsonBody) => React.ReactNode;
}) {
  if (read.kind === "loading") return null;
  if (read.kind === "failed") return null;
  return <>{children(read.data)}</>;
}
