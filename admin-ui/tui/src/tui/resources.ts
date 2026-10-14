/* The reads a pane needs, fetched when the pane opens and again on `r`. */

import * as React from "react";
import { describeError, get, type Transport } from "../api";

export type Resource<T> =
  | { state: "loading" }
  | { state: "ok"; data: T }
  | { state: "error"; message: string; status?: number };

export interface Resources {
  get<T>(path: string): Resource<T>;
  /** Fetch every path not yet fetched. */
  want(paths: readonly string[]): void;
  /** Fetch these again. */
  reload(paths: readonly string[]): void;
}

export function useResources(transport: Transport): Resources {
  const [store, setStore] = React.useState<Record<string, Resource<unknown>>>({});
  const asked = React.useRef(new Set<string>());
  const live = React.useRef(true);
  React.useEffect(
    () => () => {
      live.current = false;
    },
    [],
  );

  const fetchOne = React.useCallback(
    (path: string) => {
      asked.current.add(path);
      void (async () => {
        try {
          const data = await get<unknown>(transport, path);
          if (live.current) setStore((s) => ({ ...s, [path]: { state: "ok", data } }));
        } catch (error) {
          const status = (error as { status?: unknown }).status;
          if (live.current) {
            setStore((s) => ({
              ...s,
              [path]: {
                state: "error",
                message: describeError(error),
                ...(typeof status === "number" ? { status } : {}),
              },
            }));
          }
        }
      })();
    },
    [transport],
  );

  return React.useMemo<Resources>(
    () => ({
      get<T>(path: string): Resource<T> {
        return (store[path] as Resource<T> | undefined) ?? { state: "loading" };
      },
      want(paths) {
        for (const path of paths) if (!asked.current.has(path)) fetchOne(path);
      },
      reload(paths) {
        for (const path of paths) fetchOne(path);
      },
    }),
    [store, fetchOne],
  );
}
