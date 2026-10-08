/* A byte-stream terminal for real guests where xterm is not shipped.
 *
 * The box's copy carries no xterm.js, but a guest under libvirt is a real
 * serial console: bytes in, keystrokes out. This draws what a busybox shell
 * sends (text, CR, backspace, colours, erasing and cursor left/right), which
 * is all the Lab's guests use, and sends keys as a terminal would. Ported
 * from term.js's ByteTerm; it draws DOM nodes with the simulated terminal's
 * classes (lab.css: .lab-term, .c-31 …), never markup from the guest. */

import type { GuestConsole } from "./types";

interface Cell {
  ch: string;
  sgr: string;
}

const KEYS: Record<string, string> = {
  Enter: "\r",
  Backspace: "\x7f",
  Tab: "\t",
  Escape: "\x1b",
  ArrowUp: "\x1b[A",
  ArrowDown: "\x1b[B",
  ArrowRight: "\x1b[C",
  ArrowLeft: "\x1b[D",
  Home: "\x1b[H",
  End: "\x1b[F",
  Delete: "\x1b[3~",
};

/** SGR parameters ("1;32") → the terminal's classes ("b-1 c-32"). */
function classesOf(sgr: string): string {
  return sgr
    .split(";")
    .filter(Boolean)
    .map((p) => (p === "1" ? "b-1" : /^(3[0-7]|9[0-7])$/.test(p) ? "c-" + p : /^4[0-7]$/.test(p) ? "bg-" + p : ""))
    .filter(Boolean)
    .join(" ");
}

export class ByteTerm implements GuestConsole {
  private lines: Cell[][] = [[]];
  private row = 0;
  private col = 0;
  private sgr = "";
  private esc: string | null = null;
  private dec = new TextDecoder();
  private el: HTMLDivElement | null = null;
  private queued = false;
  /** Keystrokes out to the guest. */
  send: (data: string) => void = () => {};

  constructor(private readonly label: string) {}

  write(bytes: Uint8Array): void {
    for (const ch of this.dec.decode(bytes, { stream: true })) this.put(ch);
    if (this.lines.length > 2000) {
      const cut = this.lines.length - 2000;
      this.lines.splice(0, cut);
      this.row -= cut;
    }
    if (!this.queued) {
      this.queued = true;
      requestAnimationFrame(() => {
        this.queued = false;
        this.render();
      });
    }
  }

  private put(ch: string): void {
    if (this.esc !== null) {
      this.esc += ch;
      if (this.esc === "[" || this.esc === "]") return;
      if (this.esc[0] === "]") {
        if (ch === "\x07" || this.esc.endsWith("\x1b\\")) this.esc = null;
        return;
      }
      if (this.esc[0] !== "[") {
        this.esc = null;
        return;
      }
      if (!/[@-~]/.test(ch)) return;
      const params = this.esc.slice(1, -1);
      const n = parseInt(params, 10) || 1;
      const line = this.lines[this.row]!;
      if (ch === "m") this.sgr = params === "0" || params === "" ? "" : params;
      else if (ch === "K") line.length = Math.min(line.length, this.col);
      else if (ch === "D") this.col = Math.max(0, this.col - n);
      else if (ch === "C") this.col += n;
      else if (ch === "J" && params === "2") {
        this.lines = [[]];
        this.row = 0;
        this.col = 0;
      } else if (ch === "J") {
        line.length = Math.min(line.length, this.col);
        this.lines.length = this.row + 1;
      } else if (ch === "H") this.col = 0;
      this.esc = null;
      return;
    }
    if (ch === "\x1b") {
      this.esc = "";
      return;
    }
    if (ch === "\r") {
      this.col = 0;
      return;
    }
    if (ch === "\n") {
      this.row++;
      if (!this.lines[this.row]) this.lines[this.row] = [];
      this.col = 0;
      return;
    }
    if (ch === "\b") {
      this.col = Math.max(0, this.col - 1);
      return;
    }
    if (ch < " " || ch === "\x7f") return;
    const line = this.lines[this.row]!;
    while (line.length < this.col) line.push({ ch: " ", sgr: "" });
    line[this.col++] = { ch, sgr: this.sgr };
  }

  tail(last = 40): string {
    return this.lines
      .slice(-last)
      .map((l) => l.map((c) => c.ch).join(""))
      .join("\n");
  }

  private render(): void {
    const el = this.el;
    if (!el || !el.isConnected) return;
    const frag = document.createDocumentFragment();
    this.lines.forEach((l, r) => {
      if (r > 0) frag.append("\n");
      let run = "";
      let cls = "";
      const flush = () => {
        if (!run) return;
        if (cls) {
          const span = document.createElement("span");
          span.className = cls;
          span.textContent = run;
          frag.append(span);
        } else frag.append(run);
        run = "";
      };
      const cursor = (ch: string, c: string) => {
        flush();
        const span = document.createElement("span");
        span.className = c ? c + " cur" : "cur";
        span.textContent = ch;
        frag.append(span);
      };
      l.forEach((c, i) => {
        const k = classesOf(c.sgr);
        if (r === this.row && i === this.col) {
          cursor(c.ch, k);
          cls = "";
          return;
        }
        if (k !== cls) {
          flush();
          cls = k;
        }
        run += c.ch;
      });
      flush();
      if (r === this.row && this.col >= l.length) {
        frag.append(" ".repeat(this.col - l.length));
        cursor(" ", "");
      }
    });
    el.replaceChildren(frag);
    el.scrollTop = el.scrollHeight;
  }

  attach(host: HTMLElement): void {
    if (!this.el) {
      const el = document.createElement("div");
      el.className = "lab-term h-full min-h-[300px] overflow-auto p-3";
      el.tabIndex = 0;
      el.setAttribute("role", "textbox");
      el.setAttribute("aria-label", this.label);
      el.dataset["testid"] = "guest-terminal";
      el.addEventListener("keydown", (e) => this.key(e));
      el.addEventListener("paste", (e) => {
        this.send((e.clipboardData?.getData("text") ?? "").replace(/\n/g, "\r"));
        e.preventDefault();
      });
      this.el = el;
    }
    host.appendChild(this.el);
    this.render();
    setTimeout(() => this.el?.focus({ preventScroll: true }), 30);
  }

  private key(e: KeyboardEvent): void {
    let out = KEYS[e.key];
    if (!out && e.ctrlKey && /^[a-z]$/i.test(e.key)) out = String.fromCharCode(e.key.toUpperCase().charCodeAt(0) - 64);
    if (!out && e.key.length === 1 && !e.ctrlKey && !e.metaKey) out = e.key;
    if (!out) return;
    this.send(out);
    e.preventDefault();
  }
}
