/* Issues of LosOS Git's tracker for the parts of the admin page that list
 * them: the command palette and the pin picker.
 *
 * They are asked of LosOS Git itself, as the person signed in through
 * LosOS Login, because the tracker decides who may read which project and
 * lososd holds no such list. Signed in with the spare key there is nobody
 * to ask as (401), and a box without LosOS Git answers 404 or with the
 * admin page itself: all of them are `blocked`, which hides the group. */

/** An issue of LosOS Git's tracker. */
export interface TrackerIssue {
  key: string;
  title: string;
  /** Its workflow state's name. */
  state: string;
  closed: boolean;
}

const KEY = /^[A-Z][A-Z0-9]{1,5}-[1-9][0-9]*$/;

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value : null;
}

/** Nothing typed: the open issues given to the person, last touched first. */
export async function searchIssues(
  query: string,
  signal: AbortSignal,
): Promise<{ items: readonly TrackerIssue[] } | { blocked: "noGit" }> {
  const q = new URLSearchParams({ limit: "6", sort: "updated" });
  if (query === "") {
    q.set("assignee", "me");
    q.set("category", "active");
  } else q.set("q", query);
  const res = await fetch(`/git/api/v1/tracker/issues?${q.toString()}`, {
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (res.status === 401 || res.status === 403 || res.status === 404) return { blocked: "noGit" };
  if (!res.ok) throw new Error(`LosOS Git answered ${res.status}`);
  if (!(res.headers.get("content-type") ?? "").includes("json")) return { blocked: "noGit" };
  const body = record(await res.json());
  const items: TrackerIssue[] = [];
  for (const entry of Array.isArray(body?.["issues"]) ? (body["issues"] as unknown[]) : []) {
    const row = record(entry);
    const key = text(row?.["key"]);
    const title = text(row?.["title"]);
    if (row === null || key === null || title === null || !KEY.test(key)) continue;
    const state = record(row["state"]) ?? {};
    const category = state["category"];
    items.push({ key, title, state: text(state["name"]) ?? "", closed: category === "done" || category === "canceled" });
  }
  return { items };
}

/** An issue in LosOS Git's tracker. */
export function issueHref(issue: { key: string }): string {
  return `/git/issues/${encodeURIComponent(issue.key)}`;
}
