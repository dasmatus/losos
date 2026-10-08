/* A small terminal for the simulated consoles: the core interprets each line
 * (exec), this draws the scrollback with its ANSI colours, keeps the input
 * line, the history and Tab completion. Each device keeps its session while
 * the inspector shows something else; a new setup starts them over. */

import * as React from "react";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { type ConsoleInfo, isErr } from "./core";
import { type LabStore, store } from "./store";

class Session {
  lines: string[] = [];
  input = "";
  hist: string[] = [];
  hi = 0;
  busy = false;
  job: number | null = null;
  info: ConsoleInfo = { prompt: "$ ", boot: [], completions: [] };
  version = 0;
  private listeners = new Set<() => void>();

  constructor(
    private readonly s: LabStore,
    readonly devId: string,
  ) {
    this.reload();
    this.lines = [...this.info.boot];
    s.consoleSinks.set(devId, (lines, done) => {
      this.push(lines);
      if (done) {
        this.busy = false;
        this.job = null;
      }
      this.changed();
    });
    s.consolePower.set(devId, (on, banner) => {
      this.push([on ? "\x1b[90m[sim] power on\x1b[0m" : "\x1b[90m[sim] power off\x1b[0m"]);
      if (on) this.push(banner);
      this.reload();
      this.changed();
    });
  }

  /** The prompt and completions follow the device (a rename, a reboot). */
  reload(): void {
    const r = this.s.core.consoleInfo(this.devId);
    if (!isErr(r)) this.info = r;
  }

  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
  changed(): void {
    this.version += 1;
    for (const fn of this.listeners) fn();
  }

  push(lines: string[]): void {
    for (const l of lines) this.lines.push(...String(l).split("\n"));
    if (this.lines.length > 800) this.lines.splice(0, this.lines.length - 800);
  }

  run(line: string): void {
    this.push([this.info.prompt + line]);
    if (line.trim()) {
      this.hist.push(line);
      this.hi = this.hist.length;
    }
    const r = this.s.core.exec(this.devId, line.trim());
    if (isErr(r)) this.push([r.error]);
    else {
      if (r.clear) this.lines = [];
      this.push(r.lines);
      this.busy = r.busy;
      this.job = r.busy ? r.job : null;
    }
    this.reload();
    this.changed();
  }

  key(e: React.KeyboardEvent): void {
    if (e.ctrlKey && e.key === "l") {
      this.lines = [];
    } else if (e.ctrlKey && e.key === "c") {
      this.push([this.info.prompt + this.input + "^C"]);
      this.input = "";
      this.busy = false;
      this.job = null;
    } else if (this.busy) {
      return;
    } else if (e.key === "Enter") {
      const line = this.input;
      this.input = "";
      this.run(line);
      e.preventDefault();
      return;
    } else if (e.key === "Backspace") this.input = this.input.slice(0, -1);
    else if (e.key === "ArrowUp") {
      if (this.hi > 0) this.input = this.hist[--this.hi] ?? "";
    } else if (e.key === "ArrowDown") {
      if (this.hi < this.hist.length) this.input = this.hist[++this.hi] ?? "";
    } else if (e.key === "Tab") {
      const c = this.info.completions.find((x) => x.startsWith(this.input) && x !== this.input);
      if (c) this.input = c;
    } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) this.input += e.key;
    else return;
    e.preventDefault();
    this.changed();
  }
}

const sessions = new Map<string, Session>();
let epoch = -1;

function session(devId: string): Session {
  const s = store();
  if (epoch !== s.consoleEpoch) {
    sessions.clear();
    s.consoleSinks.clear();
    s.consolePower.clear();
    epoch = s.consoleEpoch;
  }
  let x = sessions.get(devId);
  if (!x) {
    x = new Session(s, devId);
    sessions.set(devId, x);
  }
  return x;
}

/** One line with its ANSI SGR codes as spans. */
export function Ansi({ text }: { text: string }) {
  const parts = text.split(/\x1b\[([\d;]*)m/);
  const out: React.ReactNode[] = [];
  let cls: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i] ?? "";
    if (i % 2 === 0) {
      if (part) out.push(cls.length ? <span key={i} className={cls.join(" ")}>{part}</span> : part);
      continue;
    }
    const codes = part.split(";").filter(Boolean);
    if (codes.length === 0 || codes.includes("0")) cls = [];
    for (const c of codes) {
      if (c === "0") continue;
      cls.push(c === "1" ? "b-1" : c === "44" ? "bg-44" : c === "97" ? "c-37" : "c-" + c);
    }
  }
  return <>{out}</>;
}

export function SimTerminal({ devId, className, autoFocus = true }: { devId: string; className?: string; autoFocus?: boolean }) {
  const t = useT();
  const sess = session(devId);
  React.useSyncExternalStore(
    (fn) => sess.subscribe(fn),
    () => sess.version,
  );
  const el = React.useRef<HTMLDivElement>(null);
  React.useEffect(() => {
    const node = el.current;
    if (node) node.scrollTop = node.scrollHeight;
  });
  React.useEffect(() => {
    if (!autoFocus) return;
    const h = window.setTimeout(() => el.current?.focus({ preventScroll: true }), 30);
    return () => window.clearTimeout(h);
  }, [devId, autoFocus]);
  return (
    <div
      ref={el}
      className={cn("lab-term min-h-[260px] flex-1 overflow-auto px-2.5 py-2", className)}
      tabIndex={0}
      role="textbox"
      aria-label={t("lab.console.label", { name: store().name(devId) })}
      aria-multiline="true"
      data-testid="sim-terminal"
      onKeyDown={(e) => sess.key(e)}
      onPaste={(e) => {
        sess.input += (e.clipboardData.getData("text") || "").replace(/\n/g, " ");
        sess.changed();
        e.preventDefault();
      }}
    >
      {sess.lines.map((l, i) => (
        <div key={i}>
          <Ansi text={l} />
          {l === "" ? "​" : null}
        </div>
      ))}
      {!sess.busy && (
        <div>
          {sess.info.prompt}
          {sess.input}
          <span className="cur"> </span>
        </div>
      )}
    </div>
  );
}
