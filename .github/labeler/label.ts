/* Label an issue from its title and body, and a pull request from those
 * plus its commits.
 *
 * Commit messages are read in full. The paths those commits change are
 * matched too, which is what the language and dependency labels mean.
 * The diff is not: a word inside a patch would mark almost every change.
 *
 * Labels are only added. Nothing is removed, so a label a person set
 * stays. A label named in rules.ts is created the first time it is
 * needed, with the colour and description written there.
 *
 * Run by .github/workflows/label.yml with `bun label.ts`. The workflow
 * checks out the default branch, never the pull request, and this file
 * is the one from that checkout.
 */
import { appendFileSync, readFileSync } from "node:fs";

import { rules } from "./rules";

declare global {
  interface ImportMeta {
    readonly main: boolean;
  }
}

export type LabelRule = {
  readonly description: string;
  readonly color: string;
  readonly text?: readonly string[];
  readonly paths?: readonly string[];
};

export type LabelRules = Readonly<Record<string, LabelRule>>;

export type LabelInput = {
  readonly title: string;
  readonly body: string;
  readonly commits?: readonly string[];
  readonly files?: readonly string[];
};

export type Match = {
  readonly label: string;
  readonly reason: string;
};

type Compiled = {
  readonly label: string;
  readonly description: string;
  readonly color: string;
  readonly text: readonly RegExp[];
  readonly paths: readonly { readonly glob: string; readonly re: RegExp }[];
};

/* `**` crosses directories, `*` stays inside one, `{a,b}` is either.
 * Anything else is literal, so `*.ts` does not match `file.tsx`. */
export function pathMatches(glob: string, path: string): boolean {
  return expandBraces(glob).some((expanded) => globToRegExp(expanded).test(path));
}

export function compileRules(input: LabelRules): Compiled[] {
  return Object.entries(input).map(([label, rule]) => {
    if (!/^[0-9a-fA-F]{6}$/.test(rule.color)) {
      throw new Error(`${label}: color must be six hex digits`);
    }
    if (rule.description.trim() === "") {
      throw new Error(`${label}: description is empty`);
    }
    const text = (rule.text ?? []).map((pattern) => {
      try {
        return new RegExp(pattern, "im");
      } catch (error) {
        const detail = error instanceof Error ? error.message : String(error);
        throw new Error(`${label}: bad text pattern ${pattern}: ${detail}`);
      }
    });
    const paths = (rule.paths ?? []).flatMap((glob) =>
      expandBraces(glob).map((expanded) => ({ glob, re: globToRegExp(expanded) })),
    );
    if (text.length === 0 && paths.length === 0) {
      throw new Error(`${label}: needs a text pattern or a path glob`);
    }
    return {
      label,
      description: rule.description,
      color: rule.color.toLowerCase(),
      text,
      paths,
    };
  });
}

/* The first reason wins. Text is checked before paths so a conventional
 * commit is what the log names when both would have matched. */
export function explain(compiled: readonly Compiled[], input: LabelInput): Match[] {
  const text = [input.title, input.body, ...(input.commits ?? [])].join("\n");
  const files = input.files ?? [];
  const matches: Match[] = [];
  for (const rule of compiled) {
    if (rule.text.some((re) => re.test(text))) {
      matches.push({ label: rule.label, reason: "text" });
      continue;
    }
    const file = files.find((path) => rule.paths.some((glob) => glob.re.test(path)));
    if (file !== undefined) matches.push({ label: rule.label, reason: `path ${file}` });
  }
  return matches;
}

function expandBraces(pattern: string): string[] {
  const start = pattern.indexOf("{");
  if (start < 0) return [pattern];
  const end = pattern.indexOf("}", start);
  if (end < 0) return [pattern];
  const prefix = pattern.slice(0, start);
  const suffix = pattern.slice(end + 1);
  return pattern
    .slice(start + 1, end)
    .split(",")
    .flatMap((alt) => expandBraces(`${prefix}${alt}${suffix}`));
}

function globToRegExp(glob: string): RegExp {
  let re = "^";
  for (let i = 0; i < glob.length; i++) {
    const c = glob.charAt(i);
    if (c === "*") {
      if (glob.charAt(i + 1) === "*") {
        if (glob.charAt(i + 2) === "/") {
          re += "(?:.*/)?";
          i += 2;
        } else {
          re += ".*";
          i += 1;
        }
      } else {
        re += "[^/]*";
      }
      continue;
    }
    if (c === "?") {
      re += "[^/]";
      continue;
    }
    if (".+^$|()[]{}\\".includes(c)) {
      re += `\\${c}`;
      continue;
    }
    re += c;
  }
  return new RegExp(`${re}$`);
}

type GitHub = {
  pages(path: string): Promise<unknown[]>;
  send(path: string, method: string, body?: unknown): Promise<number>;
};

function github(token: string): GitHub {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${token}`,
    "User-Agent": "losos-labeler",
    "X-GitHub-Api-Version": "2022-11-28",
  };

  async function call(url: string, method: string, body?: unknown): Promise<Response> {
    const response = await fetch(url, {
      method,
      headers: body === undefined ? headers : { ...headers, "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    return response;
  }

  return {
    async pages(path: string): Promise<unknown[]> {
      const out: unknown[] = [];
      let url: string | null = `https://api.github.com${path}${path.includes("?") ? "&" : "?"}per_page=100`;
      let pages = 0;
      while (url !== null && pages < 20) {
        pages += 1;
        const response = await call(url, "GET");
        const json: unknown = await response.json();
        if (!response.ok || !Array.isArray(json)) {
          const detail = typeof json === "string" ? json : JSON.stringify(json);
          throw new Error(`GitHub ${response.status} ${path}: ${detail}`);
        }
        out.push(...json);
        url = nextLink(response.headers.get("link"));
      }
      if (url !== null) {
        console.log(`::warning::stopped after 20 pages of ${path}; later commits or files were not read`);
      }
      return out;
    },
    async send(path: string, method: string, body?: unknown): Promise<number> {
      const response = await call(`https://api.github.com${path}`, method, body);
      if (response.status === 422) return 422;
      if (!response.ok) {
        throw new Error(`GitHub ${response.status} ${path}: ${await response.text()}`);
      }
      return response.status;
    },
  };
}

function nextLink(header: string | null): string | null {
  if (header === null) return null;
  for (const part of header.split(",")) {
    const match = part.match(/<([^>]+)>;\s*rel="next"/);
    const url = match?.[1];
    if (url !== undefined) return url;
  }
  return null;
}

function env(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === "") {
    throw new Error(`${name} is required`);
  }
  return value;
}

function recordString(value: unknown, key: string): string | null {
  if (typeof value !== "object" || value === null || !(key in value)) return null;
  const got = (value as Record<string, unknown>)[key];
  return typeof got === "string" ? got : null;
}

function asItem(event: unknown, key: string): Record<string, unknown> {
  if (typeof event !== "object" || event === null || !(key in event)) {
    throw new Error(`event payload has no ${key}`);
  }
  const item = (event as Record<string, unknown>)[key];
  if (typeof item !== "object" || item === null) throw new Error(`event ${key} is not an object`);
  return item as Record<string, unknown>;
}

async function main(): Promise<void> {
  const token = env("GH_TOKEN");
  const repo = env("REPO");
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo)) {
    throw new Error(`REPO is not owner/name: ${repo}`);
  }
  const eventName = env("GITHUB_EVENT_NAME");
  const event: unknown = JSON.parse(readFileSync(env("GITHUB_EVENT_PATH"), "utf8"));
  const pull = eventName === "pull_request" || eventName === "pull_request_target";
  if (eventName !== "issues" && !pull) throw new Error(`unexpected event ${eventName}`);

  const item = asItem(event, pull ? "pull_request" : "issue");
  if (typeof item.number !== "number" || typeof item.title !== "string") {
    throw new Error("event is missing a number or a title");
  }
  const body = typeof item.body === "string" ? item.body : "";
  const present = new Set(
    (Array.isArray(item.labels) ? item.labels : []).flatMap((label) => {
      const name = recordString(label, "name");
      return name === null ? [] : [name];
    }),
  );

  const api = github(token);
  const compiled = compileRules(rules);
  let commits: string[] = [];
  let files: string[] = [];
  if (pull) {
    const commitPages = await api.pages(`/repos/${repo}/pulls/${item.number}/commits`);
    commits = commitPages.flatMap((entry) => {
      if (typeof entry !== "object" || entry === null || !("commit" in entry)) return [];
      const message = recordString((entry as { commit: unknown }).commit, "message");
      return message === null || message === "" ? [] : [message];
    });
    const filePages = await api.pages(`/repos/${repo}/pulls/${item.number}/files`);
    files = filePages.flatMap((entry) => {
      const name = recordString(entry, "filename");
      return name === null || name === "" ? [] : [name];
    });
  }

  const matches = explain(compiled, { title: item.title, body, commits, files });
  await ensureLabels(api, repo, compiled, new Set(matches.map((match) => match.label)));
  const add = matches.filter((match) => !present.has(match.label));
  const line =
    add.length === 0
      ? matches.length === 0
        ? "no matching label"
        : `already labelled: ${matches.map((match) => match.label).join(", ")}`
      : `adding: ${add.map((match) => `${match.label} (${match.reason})`).join(", ")}`;
  console.log(line);
  const summary = process.env.GITHUB_STEP_SUMMARY;
  if (summary !== undefined && summary !== "") appendFileSync(summary, `${line}\n`);
  if (add.length === 0) return;
  await api.send(`/repos/${repo}/issues/${item.number}/labels`, "POST", {
    labels: add.map((match) => match.label),
  });
}

/* Only the labels this run wants to add are created, and only when the
 * repository does not have them yet. A 422 is the label already existing,
 * which happens when two runs create it together. */
async function ensureLabels(
  api: GitHub,
  repo: string,
  compiled: readonly Compiled[],
  wanted: ReadonlySet<string>,
): Promise<void> {
  const existing = new Set<string>();
  for (const entry of await api.pages(`/repos/${repo}/labels`)) {
    const name = recordString(entry, "name");
    if (name !== null) existing.add(name);
  }
  for (const rule of compiled) {
    if (!wanted.has(rule.label) || existing.has(rule.label)) continue;
    console.log(`creating label ${rule.label}`);
    await api.send(`/repos/${repo}/labels`, "POST", {
      name: rule.label,
      color: rule.color,
      description: rule.description,
    });
  }
}

if (import.meta.main) {
  void main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
