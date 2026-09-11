/* A tiny expression language, parsed and walked — never evaluated.
 *
 * WHY THIS FILE EXISTS
 * --------------------
 * The appliance serves the admin page under `script-src 'self'` with no
 * 'unsafe-eval'. That is not a soft rule: `eval("1+1")` and
 * `new Function("return 1")` both throw a plain EvalError in the page, and
 * every in-browser "run the user's JavaScript" trick bottoms out in one of
 * them — including the ones that look like they do not (`setTimeout("...")`,
 * `import("data:text/javascript,...")`, a Worker built from a blob: URL,
 * which `worker-src` does not permit either).
 *
 * So a widget the owner types into this page CANNOT be JavaScript. Shipping a
 * text box that accepts `async (losos) => ...` and throws EvalError the first
 * time anyone presses Save would be worse than not shipping one: on a box
 * with no shell and no console, the failure is invisible.
 *
 * What is here instead is a small language of its own — arithmetic, string
 * concatenation, comparisons, member access, and calls into two allow-listed
 * namespaces. It is tokenised, parsed into a tree, and walked. No host
 * construct is reachable from it, because none is ever named: the walker only
 * knows the node kinds below, and the only functions it will call are the ones
 * reached through `stat.` and `fmt.`.
 *
 * WHAT IT DELIBERATELY DOES NOT HAVE
 * ----------------------------------
 * No assignment, no variable declaration, no loops, no function literals, no
 * `new`, no object literals, no property writes, no `this`. There is nothing
 * to sandbox because there is nothing to escape *into* — the evaluator never
 * touches a value it was not handed in the environment, and member access
 * refuses `constructor`, `prototype` and `__proto__` outright, which is the
 * one path by which a tree-walker over plain data can be talked into reaching
 * `Function`.
 *
 * Everything is bounded: source length, nesting depth, node count at parse
 * time, and a step budget at evaluation time. A widget cannot hang the tab.
 */

// ── Errors ────────────────────────────────────────────────────────────────

/** A message meant for the person typing the expression, not for a log. */
export class ExprError extends Error {
  /** 0-based offset into the source, when the failure has a place. */
  readonly at: number | null;

  constructor(message: string, at: number | null = null) {
    super(message);
    this.name = "ExprError";
    this.at = at;
  }
}

// ── Limits ────────────────────────────────────────────────────────────────

export const MAX_SOURCE = 2000;
const MAX_DEPTH = 24;
const MAX_NODES = 400;
const MAX_ARGS = 8;
const STEP_BUDGET = 5000;

// ── Tokens ────────────────────────────────────────────────────────────────

type TokenKind = "number" | "string" | "ident" | "punct" | "end";

interface Token {
  kind: TokenKind;
  text: string;
  value?: number | string;
  at: number;
}

/* Longest first: `<=` must be matched before `<`, or `a <= b` tokenises as
 * `a < (= b)` and fails with a message about `=` that names the wrong thing. */
const PUNCT = [
  "===",
  "!==",
  "==",
  "!=",
  "<=",
  ">=",
  "&&",
  "||",
  "??",
  "(",
  ")",
  "[",
  "]",
  ",",
  ".",
  "?",
  ":",
  "+",
  "-",
  "*",
  "/",
  "%",
  "<",
  ">",
  "!",
] as const;

function isIdentStart(ch: string): boolean {
  return /[A-Za-z_$]/.test(ch);
}

function isIdentPart(ch: string): boolean {
  return /[A-Za-z0-9_$]/.test(ch);
}

function tokenize(source: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < source.length) {
    const ch = source[i] as string;

    if (/\s/.test(ch)) {
      i += 1;
      continue;
    }

    if (/[0-9]/.test(ch) || (ch === "." && /[0-9]/.test(source[i + 1] ?? ""))) {
      const start = i;
      while (i < source.length && /[0-9]/.test(source[i] ?? "")) i += 1;
      if (source[i] === ".") {
        i += 1;
        while (i < source.length && /[0-9]/.test(source[i] ?? "")) i += 1;
      }
      if (source[i] === "e" || source[i] === "E") {
        const mark = i;
        i += 1;
        if (source[i] === "+" || source[i] === "-") i += 1;
        if (/[0-9]/.test(source[i] ?? "")) {
          while (i < source.length && /[0-9]/.test(source[i] ?? "")) i += 1;
        } else {
          i = mark;
        }
      }
      const text = source.slice(start, i);
      const value = Number(text);
      if (!Number.isFinite(value)) throw new ExprError(`${text} is not a number`, start);
      tokens.push({ kind: "number", text, value, at: start });
      continue;
    }

    if (ch === '"' || ch === "'") {
      const start = i;
      const quote = ch;
      i += 1;
      let out = "";
      while (i < source.length && source[i] !== quote) {
        if (source[i] === "\\") {
          const next = source[i + 1] ?? "";
          // A short, closed escape table. No \u, no \x: there is nothing in
          // this language that needs them and a half-parsed escape is a
          // confusing error message for no gain.
          const mapped =
            next === "n" ? "\n" : next === "t" ? "\t" : next === "" ? "" : next;
          out += mapped;
          i += 2;
          continue;
        }
        out += source[i];
        i += 1;
      }
      if (i >= source.length) throw new ExprError("This text is missing its closing quote", start);
      i += 1;
      tokens.push({ kind: "string", text: out, value: out, at: start });
      continue;
    }

    if (isIdentStart(ch)) {
      const start = i;
      while (i < source.length && isIdentPart(source[i] ?? "")) i += 1;
      tokens.push({ kind: "ident", text: source.slice(start, i), at: start });
      continue;
    }

    const punct = PUNCT.find((candidate) => source.startsWith(candidate, i));
    if (punct === undefined) throw new ExprError(`${ch} does not mean anything here`, i);
    tokens.push({ kind: "punct", text: punct, at: i });
    i += punct.length;
  }

  tokens.push({ kind: "end", text: "", at: source.length });
  return tokens;
}

// ── Nodes ─────────────────────────────────────────────────────────────────

export type Node =
  | { kind: "literal"; value: number | string | boolean | null }
  | { kind: "ident"; name: string; at: number }
  | { kind: "member"; object: Node; name: string; at: number }
  | { kind: "index"; object: Node; index: Node; at: number }
  | { kind: "call"; callee: Node; args: Node[]; at: number }
  | { kind: "unary"; op: "-" | "!"; operand: Node; at: number }
  | { kind: "binary"; op: BinaryOp; left: Node; right: Node; at: number }
  | { kind: "logical"; op: "&&" | "||" | "??"; left: Node; right: Node; at: number }
  | { kind: "conditional"; test: Node; then: Node; other: Node; at: number }
  | { kind: "array"; items: Node[]; at: number };

type BinaryOp = "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=";

const KEYWORDS: Record<string, number | string | boolean | null> = {
  true: true,
  false: false,
  null: null,
};

// ── Parser ────────────────────────────────────────────────────────────────

/* Precedence climbing, lowest binding first. Written out rather than driven
 * by a table so the grammar is readable in one screen: conditional, then ??,
 * ||, &&, equality, comparison, additive, multiplicative, unary, postfix. */
class Parser {
  private readonly tokens: Token[];
  private position = 0;
  private depth = 0;
  private nodes = 0;

  constructor(tokens: Token[]) {
    this.tokens = tokens;
  }

  parse(): Node {
    const node = this.conditional();
    const token = this.peek();
    if (token.kind !== "end") {
      throw new ExprError(`This expression does not end after ${token.text || "here"}`, token.at);
    }
    return node;
  }

  private peek(): Token {
    return this.tokens[this.position] ?? { kind: "end", text: "", at: 0 };
  }

  private next(): Token {
    const token = this.peek();
    this.position += 1;
    return token;
  }

  private eat(text: string): boolean {
    const token = this.peek();
    if (token.kind === "punct" && token.text === text) {
      this.position += 1;
      return true;
    }
    return false;
  }

  private expect(text: string): Token {
    const token = this.peek();
    if (token.kind !== "punct" || token.text !== text) {
      throw new ExprError(`Expected ${text} here`, token.at);
    }
    return this.next();
  }

  private track<T extends Node>(node: T): T {
    this.nodes += 1;
    if (this.nodes > MAX_NODES) {
      throw new ExprError("This expression is too long to be worth reading", null);
    }
    return node;
  }

  private nested<T>(fn: () => T): T {
    this.depth += 1;
    if (this.depth > MAX_DEPTH) {
      throw new ExprError("This expression nests too deeply", this.peek().at);
    }
    try {
      return fn();
    } finally {
      this.depth -= 1;
    }
  }

  private conditional(): Node {
    return this.nested(() => {
      const test = this.coalesce();
      if (!this.eat("?")) return test;
      const then = this.conditional();
      this.expect(":");
      const other = this.conditional();
      return this.track<Node>({ kind: "conditional", test, then, other, at: 0 });
    });
  }

  private coalesce(): Node {
    let left = this.or();
    while (this.peek().kind === "punct" && this.peek().text === "??") {
      const at = this.next().at;
      const right = this.or();
      left = this.track<Node>({ kind: "logical", op: "??", left, right, at });
    }
    return left;
  }

  private or(): Node {
    let left = this.and();
    while (this.peek().kind === "punct" && this.peek().text === "||") {
      const at = this.next().at;
      const right = this.and();
      left = this.track<Node>({ kind: "logical", op: "||", left, right, at });
    }
    return left;
  }

  private and(): Node {
    let left = this.equality();
    while (this.peek().kind === "punct" && this.peek().text === "&&") {
      const at = this.next().at;
      const right = this.equality();
      left = this.track<Node>({ kind: "logical", op: "&&", left, right, at });
    }
    return left;
  }

  private equality(): Node {
    let left = this.comparison();
    for (;;) {
      const token = this.peek();
      if (token.kind !== "punct") break;
      // `==` and `===` both mean the same thing here; see evaluate().
      const op = token.text === "==" || token.text === "===" ? "==" : token.text === "!=" || token.text === "!==" ? "!=" : null;
      if (op === null) break;
      this.next();
      const right = this.comparison();
      left = this.track<Node>({ kind: "binary", op, left, right, at: token.at });
    }
    return left;
  }

  private comparison(): Node {
    let left = this.additive();
    for (;;) {
      const token = this.peek();
      if (token.kind !== "punct") break;
      if (token.text !== "<" && token.text !== "<=" && token.text !== ">" && token.text !== ">=") {
        break;
      }
      this.next();
      const right = this.additive();
      left = this.track<Node>({ kind: "binary", op: token.text, left, right, at: token.at });
    }
    return left;
  }

  private additive(): Node {
    let left = this.multiplicative();
    for (;;) {
      const token = this.peek();
      if (token.kind !== "punct") break;
      if (token.text !== "+" && token.text !== "-") break;
      this.next();
      const right = this.multiplicative();
      left = this.track<Node>({ kind: "binary", op: token.text, left, right, at: token.at });
    }
    return left;
  }

  private multiplicative(): Node {
    let left = this.unary();
    for (;;) {
      const token = this.peek();
      if (token.kind !== "punct") break;
      if (token.text !== "*" && token.text !== "/" && token.text !== "%") break;
      this.next();
      const right = this.unary();
      left = this.track<Node>({ kind: "binary", op: token.text, left, right, at: token.at });
    }
    return left;
  }

  private unary(): Node {
    const token = this.peek();
    if (token.kind === "punct" && (token.text === "-" || token.text === "!")) {
      this.next();
      const operand = this.nested(() => this.unary());
      return this.track<Node>({ kind: "unary", op: token.text, operand, at: token.at });
    }
    return this.postfix();
  }

  private postfix(): Node {
    let node = this.primary();
    for (;;) {
      const token = this.peek();
      if (token.kind !== "punct") break;

      if (token.text === ".") {
        this.next();
        const name = this.next();
        if (name.kind !== "ident") throw new ExprError("Expected a name after the dot", name.at);
        node = this.track<Node>({ kind: "member", object: node, name: name.text, at: name.at });
        continue;
      }

      if (token.text === "[") {
        this.next();
        const index = this.nested(() => this.conditional());
        this.expect("]");
        node = this.track<Node>({ kind: "index", object: node, index, at: token.at });
        continue;
      }

      if (token.text === "(") {
        this.next();
        const args: Node[] = [];
        if (!this.eat(")")) {
          for (;;) {
            args.push(this.nested(() => this.conditional()));
            if (args.length > MAX_ARGS) {
              throw new ExprError("Too many values passed here", token.at);
            }
            if (this.eat(",")) continue;
            this.expect(")");
            break;
          }
        }
        node = this.track<Node>({ kind: "call", callee: node, args, at: token.at });
        continue;
      }

      break;
    }
    return node;
  }

  private primary(): Node {
    const token = this.next();

    if (token.kind === "number" || token.kind === "string") {
      return this.track<Node>({ kind: "literal", value: token.value ?? 0 });
    }

    if (token.kind === "ident") {
      if (Object.hasOwn(KEYWORDS, token.text)) {
        return this.track<Node>({ kind: "literal", value: KEYWORDS[token.text] ?? null });
      }
      return this.track<Node>({ kind: "ident", name: token.text, at: token.at });
    }

    if (token.kind === "punct" && token.text === "(") {
      const node = this.nested(() => this.conditional());
      this.expect(")");
      return node;
    }

    if (token.kind === "punct" && token.text === "[") {
      const items: Node[] = [];
      if (!this.eat("]")) {
        for (;;) {
          items.push(this.nested(() => this.conditional()));
          if (items.length > MAX_ARGS) throw new ExprError("This list is too long", token.at);
          if (this.eat(",")) continue;
          this.expect("]");
          break;
        }
      }
      return this.track<Node>({ kind: "array", items, at: token.at });
    }

    throw new ExprError(
      token.kind === "end" ? "This expression stops early" : `${token.text} does not belong here`,
      token.at,
    );
  }
}

/** Parse once, evaluate many times. Throws {@link ExprError} on bad input. */
export function parseExpr(source: string): Node {
  if (typeof source !== "string") throw new ExprError("An expression has to be text");
  if (source.trim().length === 0) throw new ExprError("This expression is empty");
  if (source.length > MAX_SOURCE) {
    throw new ExprError(`An expression may be at most ${MAX_SOURCE} characters`);
  }
  return new Parser(tokenize(source)).parse();
}

// ── Evaluation ────────────────────────────────────────────────────────────

/** Names bound for an expression. Values are plain data plus the two callable
 *  namespaces; nothing else is reachable, because nothing else is named. */
export type Env = Readonly<Record<string, unknown>>;

/* Names that must never resolve through member access.
 *
 * This is the one real attack on a tree-walking interpreter over plain data:
 * `x.constructor.constructor("...")()` reaches Function even though the word
 * "Function" never appears in the source. Refusing these three closes it, and
 * the own-property check below closes the rest — every prototype method,
 * including `toString` and `valueOf`, is simply not found. */
const FORBIDDEN_KEYS = new Set(["__proto__", "constructor", "prototype"]);

interface Machine {
  env: Env;
  /** Objects whose members may be CALLED. Identity, not name. */
  callable: ReadonlySet<unknown>;
  steps: number;
}

function member(value: unknown, key: string, at: number): unknown {
  if (FORBIDDEN_KEYS.has(key)) throw new ExprError(`${key} is not readable`, at);
  if (value === null || value === undefined) return undefined;
  if (typeof value === "string") {
    return key === "length" ? value.length : undefined;
  }
  if (typeof value !== "object" && typeof value !== "function") return undefined;
  const object = value as Record<string, unknown>;
  // Own properties only. Inherited ones are the prototype chain, and the
  // prototype chain is the part of the host this language does not have.
  if (!Object.hasOwn(object, key)) return undefined;
  return object[key];
}

function truthy(value: unknown): boolean {
  if (typeof value === "number") return value !== 0 && !Number.isNaN(value);
  if (typeof value === "string") return value.length > 0;
  if (Array.isArray(value)) return value.length > 0;
  return value !== null && value !== undefined && value !== false;
}

function numeric(value: unknown): number {
  if (typeof value === "number") return Number.isFinite(value) ? value : 0;
  if (typeof value === "boolean") return value ? 1 : 0;
  if (typeof value === "string") {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function step(machine: Machine): void {
  machine.steps += 1;
  if (machine.steps > STEP_BUDGET) {
    throw new ExprError("This expression does too much work", null);
  }
}

function walk(node: Node, machine: Machine): unknown {
  step(machine);

  switch (node.kind) {
    case "literal":
      return node.value;

    case "ident": {
      if (FORBIDDEN_KEYS.has(node.name)) throw new ExprError(`${node.name} is not readable`, node.at);
      if (!Object.hasOwn(machine.env, node.name)) {
        throw new ExprError(`There is nothing called ${node.name} here`, node.at);
      }
      return machine.env[node.name];
    }

    case "member":
      return member(walk(node.object, machine), node.name, node.at);

    case "index": {
      const object = walk(node.object, machine);
      const index = walk(node.index, machine);
      if (Array.isArray(object)) {
        const i = Math.trunc(numeric(index));
        return i >= 0 && i < object.length ? object[i] : undefined;
      }
      return member(object, String(index), node.at);
    }

    case "call": {
      /* A call is legal only when the callee is `<namespace>.<name>` and the
       * namespace object is one the caller explicitly marked callable. A bare
       * `f()`, a call on a value plucked out of a metric, or a call on a
       * function that arrived through member access from anywhere else is a
       * refusal — the allow-list is over objects, not over names, so nothing
       * a widget can construct can spoof its way in. */
      if (node.callee.kind !== "member") {
        throw new ExprError("Only stat.* and fmt.* can be called", node.at);
      }
      const owner = walk(node.callee.object, machine);
      if (!machine.callable.has(owner)) {
        throw new ExprError("Only stat.* and fmt.* can be called", node.at);
      }
      const fn = member(owner, node.callee.name, node.callee.at);
      if (typeof fn !== "function") {
        throw new ExprError(`There is no such function as ${node.callee.name}`, node.callee.at);
      }
      const args = node.args.map((arg) => walk(arg, machine));
      return (fn as (...rest: unknown[]) => unknown)(...args);
    }

    case "unary": {
      const operand = walk(node.operand, machine);
      return node.op === "-" ? -numeric(operand) : !truthy(operand);
    }

    case "logical": {
      const left = walk(node.left, machine);
      if (node.op === "&&") return truthy(left) ? walk(node.right, machine) : left;
      if (node.op === "||") return truthy(left) ? left : walk(node.right, machine);
      return left === null || left === undefined ? walk(node.right, machine) : left;
    }

    case "conditional":
      return truthy(walk(node.test, machine))
        ? walk(node.then, machine)
        : walk(node.other, machine);

    case "array":
      return node.items.map((item) => walk(item, machine));

    case "binary": {
      const left = walk(node.left, machine);
      const right = walk(node.right, machine);
      switch (node.op) {
        case "+":
          // The one place either type is accepted: a caption is usually
          // `"on " + m.hostName`, and forcing fmt.* for that is noise.
          if (typeof left === "string" || typeof right === "string") {
            return `${asText(left)}${asText(right)}`;
          }
          return numeric(left) + numeric(right);
        case "-":
          return numeric(left) - numeric(right);
        case "*":
          return numeric(left) * numeric(right);
        case "/": {
          // 0, not Infinity. A tile reading "Infinity GB" is a bug report;
          // a tile reading "0 B" next to a caption is a missing measurement.
          const divisor = numeric(right);
          return divisor === 0 ? 0 : numeric(left) / divisor;
        }
        case "%": {
          const divisor = numeric(right);
          return divisor === 0 ? 0 : numeric(left) % divisor;
        }
        case "==":
          return left === right;
        case "!=":
          return left !== right;
        case "<":
          return numeric(left) < numeric(right);
        case "<=":
          return numeric(left) <= numeric(right);
        case ">":
          return numeric(left) > numeric(right);
        case ">=":
          return numeric(left) >= numeric(right);
      }
    }
  }
}

function asText(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return value.map(asText).join(", ");
  return "";
}

export interface EvaluateOptions {
  /** Objects whose members may be called. Pass the `stat` and `fmt` objects
   *  themselves; identity is what the check uses. */
  callable?: readonly unknown[];
}

/** Walk a parsed expression against an environment. Throws {@link ExprError}. */
export function evaluate(node: Node, env: Env, options: EvaluateOptions = {}): unknown {
  return walk(node, {
    env,
    callable: new Set(options.callable ?? []),
    steps: 0,
  });
}

/** Parse and walk in one go. For a preview; cache the tree for anything hot. */
export function run(source: string, env: Env, options: EvaluateOptions = {}): unknown {
  return evaluate(parseExpr(source), env, options);
}

/** Coerce a result to the number a widget field wants. `null` stays null —
 *  "not measured" is a state the renderers draw, and 0 is a different claim. */
export function toNumberOrNull(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "boolean") return value ? 1 : 0;
  if (typeof value === "string") {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/** Coerce a result to display text. */
export function toText(value: unknown): string {
  return asText(value);
}
