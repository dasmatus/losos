import * as React from "react";
import { useFaces } from "../store";
import type { Home, Issue, Project } from "./api";

/* What every tracker page shares: the home answer (projects with their
 * workflows, labels and cycles, the people one may give an issue to, the
 * saved views, the unread count), and a way to read it again after a
 * change that moves the counts. */

export interface TrackerState {
  home: Home | null;
  reloadHome: () => void;
}

export const TrackerContext = React.createContext<TrackerState>({ home: null, reloadHome: () => {} });

export function useTracker(): TrackerState {
  return React.useContext(TrackerContext);
}

/** The project an issue is in, from the home answer. */
export function projectOf(home: Home | null, issue: Pick<Issue, "projectId">): Project | undefined {
  return home?.projects.find((p) => p.id === issue.projectId);
}

/** A person's name: their profile's, else their LosOS Git account's full
 *  name, else the login. */
export function useNameOf(): (login: string) => string {
  const { home } = useTracker();
  const faces = useFaces();
  return React.useCallback(
    (login: string) => {
      const face = faces(login).displayName;
      if (face !== "" && face !== login) return face;
      return home?.people.find((p) => p.login === login)?.fullName || login;
    },
    [faces, home],
  );
}

/** Every project key, for linking `LOS-12` in Markdown. */
export function useIssueKeys(): ReadonlySet<string> {
  const { home } = useTracker();
  return React.useMemo(() => new Set((home?.projects ?? []).map((p) => p.key)), [home]);
}

/** Typing in a text box, a menu or a dialog: the page's shortcuts stay out
 *  of the way. */
export function typingIn(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  return target.closest('[role="menu"], [role="listbox"], dialog[open]') !== null;
}
