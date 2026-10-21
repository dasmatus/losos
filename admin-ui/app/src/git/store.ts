import * as React from "react";
import type { PersonLike } from "@/components/person";
import { getProfiles, type Profile } from "@/lib/api";
import { git, GitError, type User } from "./api";

/* What every page of the app shares: who is signed in, the faces of the
 * people a page names, and a loader that refetches when its inputs change. */

export const MeContext = React.createContext<User | null>(null);

export function useMe(): User | null {
  return React.useContext(MeContext);
}

export interface Loaded<T> {
  data: T | null;
  error: unknown;
  loading: boolean;
  reload: () => void;
}

/** Calls `load` when `deps` change or `reload` is called; the last answer
 *  stays on screen while the next one is on its way. */
export function useLoad<T>(load: (signal: AbortSignal) => Promise<T>, deps: React.DependencyList): Loaded<T> {
  const [data, setData] = React.useState<T | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [loading, setLoading] = React.useState(true);
  const [round, setRound] = React.useState(0);
  React.useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    load(controller.signal)
      .then((value) => {
        setData(value);
        setError(null);
      })
      .catch((e: unknown) => {
        if (controller.signal.aborted) return;
        setError(e);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [...deps, round]);
  const reload = React.useCallback(() => setRound((r) => r + 1), []);
  return { data, error, loading, reload };
}

export function notFound(error: unknown): boolean {
  return error instanceof GitError && error.status === 404;
}

/** The person each LosOS Git account is: their LosOS profile when the box
 *  has one by that name (the owner's account is `notshared`, found by the
 *  name LosOS Login gave it), their initials otherwise. */
export type Faces = (login: string) => PersonLike;

const FacesContext = React.createContext<Faces>((login) => ({ name: login, displayName: login }));

export function useFaces(): Faces {
  return React.useContext(FacesContext);
}

export const FacesProvider = FacesContext.Provider;

export function useFaceBook(me: User | null): Faces {
  const [profiles, setProfiles] = React.useState<readonly Profile[]>([]);
  React.useEffect(() => {
    const controller = new AbortController();
    void getProfiles({ signal: controller.signal })
      .then(({ people }) => setProfiles(people))
      .catch(() => {
        /* initials, then: the profiles answer only on the LAN */
      });
    return () => controller.abort();
  }, []);
  return React.useCallback(
    (login: string) => {
      const byName = profiles.find((p) => p.name === login);
      if (byName !== undefined) return byName;
      if (me !== null && login === me.login && me.full_name !== "") {
        const byDisplay = profiles.find((p) => p.displayName === me.full_name);
        if (byDisplay !== undefined) return byDisplay;
        return { name: login, displayName: me.full_name };
      }
      return { name: login, displayName: login };
    },
    [profiles, me],
  );
}

/** The repository a route names, loaded once per route. */
export function useRepo(owner: string, name: string) {
  return useLoad((signal) => git.repo(owner, name, signal), [owner, name]);
}
