/* The issue tracker's half of LosOS Git's API, /git/api/v1/tracker/
 * (backend/src/forge/tracker/api.rs). An issue is named by its key
 * (`LOS-12`) and a project by its key (`LOS`) everywhere. */

import { call } from "../api";

export type Category = "backlog" | "unstarted" | "started" | "done" | "canceled";

/** 0 none, 1 urgent, 2 high, 3 medium, 4 low. */
export type Priority = 0 | 1 | 2 | 3 | 4;

export type Role = "read" | "write" | "admin";
export type Permission = "none" | Role;

export interface WorkflowState {
  id: number;
  projectId: number;
  name: string;
  category: Category;
  color: string;
  position: number;
}

export interface Label {
  id: number;
  name: string;
  color: string;
  description: string;
}

export interface Cycle {
  id: number;
  projectId: number;
  number: number;
  name: string;
  starts: string;
  ends: string;
}

export interface Project {
  id: number;
  key: string;
  name: string;
  description: string;
  color: string;
  lead: string | null;
  visibility: "private" | "internal";
  archived: boolean;
  created: string;
  updated: string;
  repos: { owner: string; name: string; defaultBranch: string }[];
  permission: Permission;
  states: WorkflowState[];
  labels: Label[];
  cycles: Cycle[];
  counts?: { total: number; open: number; started: number };
  members?: { login: string; role: Role }[];
}

export interface Issue {
  id: number;
  projectId: number;
  key: string;
  number: number;
  title: string;
  body: string;
  state: WorkflowState;
  priority: Priority;
  estimate: number | null;
  assignee: string | null;
  creator: string;
  cycleId: number | null;
  parentId: number | null;
  parentKey: string | null;
  due: string | null;
  sort: number;
  repo: { owner: string; name: string; number: number } | null;
  labels: Label[];
  comments: number;
  children: number;
  childrenClosed: number;
  created: string;
  updated: string;
  startedAt: string | null;
  completedAt: string | null;
  canceledAt: string | null;
  url: string;
}

export interface Comment {
  id: number;
  author: string;
  body: string;
  created: string;
  updated: string;
}

export interface Activity {
  id: number;
  actor: string;
  kind: string;
  detail: Record<string, unknown>;
  created: string;
}

export type RelationKind = "blocks" | "blocked_by" | "relates" | "duplicates" | "duplicated_by";

export interface Summary {
  key: string;
  title: string;
  state: WorkflowState;
  priority: Priority;
  assignee: string | null;
  url: string;
}

export interface Relation {
  id: number;
  kind: RelationKind;
  issue: Summary;
}

export interface Link {
  id: number;
  kind: "commit" | "pull";
  owner: string;
  repo: string;
  target: string;
  title: string;
  actor: string;
  closes: boolean;
  created: string;
  url: string;
}

/** One issue with everything around it. Where a list row has counts of
 *  comments and sub-issues, this has the lists. */
export interface IssueDetail extends Omit<Issue, "comments" | "children"> {
  project: Project;
  comments: Comment[];
  children: Issue[];
  activity: Activity[];
  relations: Relation[];
  links: Link[];
  parent?: Summary;
  subscribers: string[];
  subscribed: boolean;
  permission: Permission;
}

/** What a list shows; every field narrows it (backend/src/forge/tracker/query.rs). */
export interface Filter {
  project?: string[];
  category?: string[];
  state?: number[];
  assignee?: string[];
  creator?: string[];
  priority?: number[];
  label?: string[];
  cycle?: string[];
  parent?: string;
  due?: string;
  q?: string;
  sort?: "manual" | "priority" | "updated" | "created" | "due";
}

export type Layout = "list" | "board";
export type GroupBy = "state" | "assignee" | "priority" | "project" | "cycle" | "none";

export interface View {
  id: number;
  owner: string;
  name: string;
  query: Filter;
  layout: Layout;
  groupBy: GroupBy;
  shared: boolean;
  created: string;
}

export interface InboxItem {
  id: number;
  ticketId: number;
  kind: "assigned" | "mention" | "comment" | "state" | "link";
  actor: string;
  detail: Record<string, unknown>;
  created: string;
  read: boolean;
  issue: Issue;
}

export interface Home {
  me: string | null;
  projects: Project[];
  views: View[];
  unread: number;
  assigned: number;
  people: { login: string; fullName: string }[];
  ai: boolean;
  today: string;
}

/** A new issue or a change: a field left out stays, `null` clears it. */
export interface Fields {
  project?: string;
  title?: string;
  body?: string;
  state?: number;
  category?: Category;
  priority?: number;
  estimate?: number | null;
  assignee?: string | null;
  cycle?: number | null;
  parent?: string | null;
  due?: string | null;
  sort?: number;
  labels?: string[];
  addLabels?: string[];
  removeLabels?: string[];
  repo?: string;
}

const e = encodeURIComponent;

function query(f: Filter): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(f)) {
    if (v === undefined || v === "") continue;
    if (Array.isArray(v)) {
      if (v.length > 0) q.set(k, v.join(","));
    } else q.set(k, String(v));
  }
  const s = q.toString();
  return s === "" ? "" : `?${s}`;
}

export const tracker = {
  home: (signal?: AbortSignal) => call<Home>("GET", "/tracker", undefined, signal),
  issues: (filter: Filter, signal?: AbortSignal) =>
    call<{ issues: Issue[] }>("GET", `/tracker/issues${query(filter)}`, undefined, signal).then((r) => r.issues),
  issue: (key: string, signal?: AbortSignal) => call<IssueDetail>("GET", `/tracker/issues/${e(key)}`, undefined, signal),
  create: (fields: Fields) => call<IssueDetail>("POST", "/tracker/issues", fields),
  edit: (key: string, fields: Fields) => call<IssueDetail>("PATCH", `/tracker/issues/${e(key)}`, fields),
  bulk: (keys: string[], change: Fields) => call<{ changed: string[] }>("POST", "/tracker/issues/bulk", { keys, change }),
  remove: (key: string) => call<null>("DELETE", `/tracker/issues/${e(key)}`),
  comment: (key: string, body: string) => call<Comment>("POST", `/tracker/issues/${e(key)}/comments`, { body }),
  editComment: (id: number, body: string) => call<null>("PATCH", `/tracker/comments/${id}`, { body }),
  removeComment: (id: number) => call<null>("DELETE", `/tracker/comments/${id}`),
  relate: (key: string, kind: RelationKind, other: string) =>
    call<null>("POST", `/tracker/issues/${e(key)}/relations`, { kind, other }),
  unrelate: (id: number) => call<null>("DELETE", `/tracker/relations/${id}`),
  follow: (key: string, on: boolean) => call<null>(on ? "PUT" : "DELETE", `/tracker/issues/${e(key)}/subscription`),
  ai: (key: string, task: "breakdown" | "summary") =>
    call<{ text: string; items: string[] }>("POST", `/tracker/issues/${e(key)}/ai`, { task }),
  project: (key: string, signal?: AbortSignal) => call<Project>("GET", `/tracker/projects/${e(key)}`, undefined, signal),
  repoProject: (owner: string, repo: string, signal?: AbortSignal) =>
    call<Project>("GET", `/tracker/repos/${e(owner)}/${e(repo)}`, undefined, signal),
  createProject: (fields: { name: string; key?: string; description?: string; visibility?: string; repo?: string }) =>
    call<Project>("POST", "/tracker/projects", fields),
  editProject: (key: string, fields: { name?: string; description?: string; visibility?: string; archived?: boolean }) =>
    call<Project>("PATCH", `/tracker/projects/${e(key)}`, fields),
  addCycle: (key: string, cycle: { name: string; starts: string; ends: string }) =>
    call<Cycle>("POST", `/tracker/projects/${e(key)}/cycles`, cycle),
  saveView: (view: { name: string; query: Filter; layout: Layout; groupBy: GroupBy; shared?: boolean }) =>
    call<View>("POST", "/tracker/views", view),
  removeView: (id: number) => call<null>("DELETE", `/tracker/views/${id}`),
  inbox: (signal?: AbortSignal) =>
    call<{ items: InboxItem[]; unread: number }>("GET", "/tracker/inbox", undefined, signal),
  markRead: (ids?: number[], read = true) => call<{ unread: number }>("POST", "/tracker/inbox/read", { ids, read }),
  /** A calendar file of one issue's due date, or of everything given to me. */
  calendarUrl: (key?: string) =>
    key === undefined ? "/git/api/v1/tracker/calendar.ics" : `/git/api/v1/tracker/issues/${e(key)}/calendar.ics`,
};

export const CATEGORY_ORDER: Category[] = ["backlog", "unstarted", "started", "done", "canceled"];

export function closed(c: Category): boolean {
  return c === "done" || c === "canceled";
}

/** Urgent first, none last, as the server sorts. */
export function priorityRank(p: number): number {
  return p === 0 ? 5 : p;
}
