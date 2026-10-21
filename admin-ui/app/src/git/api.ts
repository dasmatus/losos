/* The web app's half of LosOS Git's API at /git/api/v1/
 * (backend/src/forge/{api,pulls,code,worktrees}.rs).
 *
 * nginx signs the person in with LosOS Login before the page loads and
 * names them to LosOS Git on every call, so a call carries no token: the
 * browser's session cookie does it. A call that comes back 401 means the
 * session ran out; the page then says so and offers to sign in again. */

export interface User {
  id: number;
  login: string;
  full_name: string;
  email: string;
  is_admin: boolean;
  is_bot: boolean;
}

export type Permission = "none" | "read" | "write" | "admin";

export interface Repo {
  id: number;
  owner: { login: string };
  name: string;
  full_name: string;
  description: string;
  private: boolean;
  default_branch: string;
  empty: boolean;
  fork: boolean;
  clone_url: string;
  created: string;
  updated: string;
  permissions: { admin: boolean; push: boolean; pull: boolean };
}

export interface Branch {
  name: string;
  default: boolean;
  commit: { sha: string; subject: string | null; date: string | null; author: string | null };
}

export interface Commit {
  sha: string;
  author: { name: string; email: string };
  date: string;
  subject: string;
  body: string;
}

export interface TreeEntry {
  name: string;
  path: string;
  type: "dir" | "file" | "symlink" | "submodule";
  sha: string;
  size: number;
}

export interface Dir {
  type: "dir";
  path: string;
  commit: string | null;
  entries: TreeEntry[];
}

export interface File {
  type: "file" | "symlink";
  name: string;
  path: string;
  sha: string;
  commit: string;
  size: number;
  binary: boolean;
  truncated: boolean;
  text: string | null;
}

export interface Login {
  login: string;
  /** Set on review and line comment authors: LosOS Reviewer, the box's bot. */
  is_bot?: boolean;
}

export interface Issue {
  number: number;
  title: string;
  body: string;
  user: Login;
  state: "open" | "closed";
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  html_url: string;
  pull_request?: { head: string | null; base: string | null; merged: boolean; merge_commit_sha: string | null };
}

export type ReviewState = "approved" | "changes_requested" | "commented";

export interface Reviewer {
  login: string;
  /** `requested` while they have been asked and not answered since. */
  state: ReviewState | "requested";
  requested: boolean;
  is_bot?: boolean;
  commit_id?: string;
  previous?: ReviewState | null;
}

export interface Worktree {
  id: number;
  owner: Login;
  computer: string;
  name: string;
  branch?: string;
  head: string;
  branch_exists?: boolean;
  behind_branch?: boolean;
  updated_at: string;
}

export interface Pull extends Issue {
  merged: boolean;
  merged_by: Login | null;
  merged_at: string | null;
  merge_commit_sha: string | null;
  head: { ref: string; sha: string | null };
  base: { ref: string; sha: string | null };
  merge_base: string | null;
  ahead_by: number;
  behind_by: number;
  mergeable: boolean;
  conflicts: string[];
  fast_forward: boolean;
  reviewers: Reviewer[];
  worktree: Worktree | null;
  comments: number;
  review_comments: number;
  can_merge: boolean;
  permission: Permission;
}

export interface Comment {
  id: number;
  user: Login;
  body: string;
  created_at: string;
  updated_at: string;
}

export interface Review {
  id: number;
  user: Login;
  state: ReviewState;
  body: string;
  commit_id: string;
  submitted_at: string;
}

export type Side = "old" | "new";

export interface LineComment {
  id: number;
  review_id: number | null;
  in_reply_to: number | null;
  user: Login;
  path: string;
  line: number;
  side: Side;
  commit_id: string;
  body: string;
  resolved: boolean;
  resolved_by: Login | null;
  created_at: string;
}

export interface FileDiff {
  filename: string;
  previous_filename: string | null;
  status: "added" | "modified" | "deleted" | "renamed" | "copied" | "changed";
  additions: number;
  deletions: number;
  binary: boolean;
  patch: string | null;
  too_large: boolean;
}

export interface Compare {
  base: { ref: string; sha: string };
  head: { ref: string; sha: string };
  merge_base: string | null;
  ahead_by: number;
  behind_by: number;
  mergeable: boolean;
  commits: Commit[];
  files: FileDiff[];
}

export type CheckState = "pending" | "success" | "failure" | "error";

export interface Checks {
  state: CheckState;
  statuses: {
    context: string;
    state: CheckState;
    description: string;
    targetUrl: string | null;
    creator: string;
    created: string;
    updated: string;
  }[];
}

/** The AI reviewer's progress on a pull request, where the box runs one. */
export interface AiReview {
  state: "unavailable" | "off" | "idle" | "queued" | "reviewing" | "done" | "failed";
  filesDone: number;
  filesTotal: number;
  ahead: number;
  error?: string | null;
}

/** Whether a repository asks the AI reviewer (its own thread's API). */
export interface AiReviewSetting {
  enabled: boolean;
  available: boolean;
  /** The box's code model, while Build with AI is on. */
  model?: string | null;
  reviewer: { login: string; full_name: string };
  canChange?: boolean;
}

export type MergeStyle = "merge" | "squash" | "fast-forward";

export interface NewLine {
  path: string;
  line: number;
  side: Side;
  body: string;
}

export type ReviewEvent = "APPROVE" | "REQUEST_CHANGES" | "COMMENT";

export class GitError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

const BASE = "/git/api/v1";

/** Told when a call finds the session gone, so the shell can say so. */
let onSignedOut: () => void = () => {};
export function whenSignedOut(f: () => void): void {
  onSignedOut = f;
}

async function call<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const init: RequestInit = { method, credentials: "same-origin" };
  if (signal !== undefined) init.signal = signal;
  if (body !== undefined) {
    init.headers = { "Content-Type": "application/json" };
    init.body = JSON.stringify(body);
  }
  let res: Response;
  try {
    res = await fetch(BASE + path, init);
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new GitError(0, "network");
  }
  if (res.status === 401) onSignedOut();
  const text = await res.text();
  let data: unknown = null;
  try {
    data = text.length > 0 ? JSON.parse(text) : null;
  } catch {
    data = null;
  }
  if (!res.ok) {
    const err = (data ?? {}) as { message?: string };
    throw new GitError(res.status, err.message ?? res.statusText);
  }
  return data as T;
}

const e = encodeURIComponent;
const repoPath = (owner: string, name: string) => `/repos/${e(owner)}/${e(name)}`;
/** A path inside a repository, each part escaped and the slashes kept. */
const filePath = (path: string) => path.split("/").map(e).join("/");

export const git = {
  me: (signal?: AbortSignal) => call<User>("GET", "/user", undefined, signal),
  repos: (signal?: AbortSignal) => call<Repo[]>("GET", "/user/repos", undefined, signal),
  createRepo: (name: string, description: string, isPrivate: boolean) =>
    call<Repo>("POST", "/user/repos", { name, description, private: isPrivate, auto_init: true }),
  repo: (o: string, r: string, signal?: AbortSignal) => call<Repo>("GET", repoPath(o, r), undefined, signal),
  editRepo: (o: string, r: string, change: { name?: string; description?: string; private?: boolean; default_branch?: string }) =>
    call<Repo>("PATCH", repoPath(o, r), change),
  collaborators: (o: string, r: string, signal?: AbortSignal) =>
    call<(User & { permission: Permission })[]>("GET", `${repoPath(o, r)}/collaborators`, undefined, signal),
  branches: (o: string, r: string, signal?: AbortSignal) =>
    call<Branch[]>("GET", `${repoPath(o, r)}/branches`, undefined, signal),
  contents: (o: string, r: string, path: string, ref: string, signal?: AbortSignal) =>
    call<Dir | File>(
      "GET",
      `${repoPath(o, r)}/contents${path === "" ? "" : `/${filePath(path)}`}?ref=${e(ref)}`,
      undefined,
      signal,
    ),
  readme: (o: string, r: string, ref: string, signal?: AbortSignal) =>
    call<File>("GET", `${repoPath(o, r)}/readme?ref=${e(ref)}`, undefined, signal),
  commits: (o: string, r: string, ref: string, signal?: AbortSignal) =>
    call<Commit[]>("GET", `${repoPath(o, r)}/commits?sha=${e(ref)}&limit=50`, undefined, signal),
  worktrees: (o: string, r: string, signal?: AbortSignal) =>
    call<Worktree[]>("GET", `${repoPath(o, r)}/worktrees`, undefined, signal),
  forgetWorktree: (o: string, r: string, id: number) =>
    call<unknown>("DELETE", `${repoPath(o, r)}/worktrees/${id}`),
  pulls: (o: string, r: string, state: "open" | "closed" | "all", signal?: AbortSignal) =>
    call<Issue[]>("GET", `${repoPath(o, r)}/pulls?state=${state}`, undefined, signal),
  pull: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<Pull>("GET", `${repoPath(o, r)}/pulls/${n}`, undefined, signal),
  createPull: (o: string, r: string, pr: { title: string; body: string; head: string; base: string }) =>
    call<Pull>("POST", `${repoPath(o, r)}/pulls`, pr),
  editPull: (o: string, r: string, n: number, change: { title?: string; body?: string; state?: "open" | "closed" }) =>
    call<Pull>("PATCH", `${repoPath(o, r)}/pulls/${n}`, change),
  pullCommits: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<Commit[]>("GET", `${repoPath(o, r)}/pulls/${n}/commits`, undefined, signal),
  pullFiles: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<FileDiff[]>("GET", `${repoPath(o, r)}/pulls/${n}/files`, undefined, signal),
  comments: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<Comment[]>("GET", `${repoPath(o, r)}/issues/${n}/comments`, undefined, signal),
  comment: (o: string, r: string, n: number, body: string) =>
    call<Comment>("POST", `${repoPath(o, r)}/issues/${n}/comments`, { body }),
  reviews: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<Review[]>("GET", `${repoPath(o, r)}/pulls/${n}/reviews`, undefined, signal),
  review: (
    o: string,
    r: string,
    n: number,
    review: { event: ReviewEvent; body: string; commit_id: string | null; comments: NewLine[] },
  ) => call<Review>("POST", `${repoPath(o, r)}/pulls/${n}/reviews`, review),
  lineComments: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<LineComment[]>("GET", `${repoPath(o, r)}/pulls/${n}/comments`, undefined, signal),
  reply: (o: string, r: string, n: number, inReplyTo: number, body: string) =>
    call<LineComment>("POST", `${repoPath(o, r)}/pulls/${n}/comments`, { in_reply_to: inReplyTo, body }),
  resolve: (o: string, r: string, id: number, resolved: boolean) =>
    call<LineComment>("PATCH", `${repoPath(o, r)}/pulls/comments/${id}`, { resolved }),
  requestReviewers: (o: string, r: string, n: number, reviewers: string[]) =>
    call<Pull>("POST", `${repoPath(o, r)}/pulls/${n}/requested_reviewers`, { reviewers }),
  dropReviewer: (o: string, r: string, n: number, login: string) =>
    call<Pull>("DELETE", `${repoPath(o, r)}/pulls/${n}/requested_reviewers/${e(login)}`),
  merge: (
    o: string,
    r: string,
    n: number,
    merge: { style: MergeStyle; message?: string; head_commit_id: string | null; delete_branch: boolean },
  ) => call<unknown>("POST", `${repoPath(o, r)}/pulls/${n}/merge`, merge),
  compare: (o: string, r: string, base: string, head: string, signal?: AbortSignal) =>
    call<Compare>("GET", `${repoPath(o, r)}/compare/${filePath(base)}...${filePath(head)}`, undefined, signal),
  checks: (o: string, r: string, sha: string, signal?: AbortSignal) =>
    call<Checks>("GET", `${repoPath(o, r)}/commits/${e(sha)}/statuses`, undefined, signal),
  aiReviewSetting: (o: string, r: string, signal?: AbortSignal) =>
    call<AiReviewSetting>("GET", `${repoPath(o, r)}/ai-review`, undefined, signal),
  setAiReview: (o: string, r: string, enabled: boolean) => call<AiReviewSetting>("PUT", `${repoPath(o, r)}/ai-review`, { enabled }),
  aiReview: (o: string, r: string, n: number, signal?: AbortSignal) =>
    call<AiReview>("GET", `${repoPath(o, r)}/pulls/${n}/ai-review`, undefined, signal),
};
