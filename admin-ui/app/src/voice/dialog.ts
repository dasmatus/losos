/* Who the agent is spoken to by, and what it is asked.
 *
 * The agent hears everyone in the call and answers only when it is
 * addressed: by name ("LosOS, what is on the list for Friday?"), or, in the
 * seconds after it has answered someone, by that person going on. What it
 * hears is kept in the page, not sent anywhere, until it is addressed; then
 * the last minutes of the conversation go with the question, so "what do
 * you think?" has something to be about.
 *
 * Plain text in, plain text out, no browser needed. */

/* The words Whisper makes of the name. With the name in its prompt the
 * usual spelling is "LosOS" or "Losos"; the rest are the ways a model
 * hears it without that. */
const NAME = String.raw`(?:los[\s.\-]?o\.?s\.?|loss[\s.\-]?o\.?s\.?|laws[\s.\-]?o\.?s\.?|lows[\s.\-]?o\.?s\.?|loess[\s.\-]?o\.?s\.?|lo[wz]sos|lossos|lawsos|losos|lo\s?sauce)`;
const GREETING = String.raw`(?:hey|hi|hello|ok|okay|yo|dear)`;
/* "LosOS" as a clause of its own: at the start of the sentence or after a
 * greeting, or at the end after a comma ("what time is it, LosOS?"). A name
 * in the middle of a sentence is a conversation about the thing ("LosOS
 * Cloud is slow", "I love LosOS"), not a request to it. */
const LEADING = new RegExp(String.raw`^\s*(?:${GREETING}[\s,.!-]+)?${NAME}\b[\s,:.!?-]*`, "i");
const TRAILING = new RegExp(String.raw`,[\s]*${NAME}[\s.!?]*$`, "i");
/* Words that make the name a product, not a person addressed. */
const PRODUCT = new RegExp(String.raw`^\s*(?:${GREETING}[\s,.!-]+)?${NAME}\s+(?:cloud|git|chat|mail|office|diagrams|login|is|has|was|will|does|did|runs?|needs?)\b`, "i");

/* How long after an answer the same person may go on without the name. */
export const FOLLOW_UP_MS = 20_000;
/* How much of the conversation goes with a question. */
const CONTEXT_LINES = 24;
const CONTEXT_MS = 5 * 60_000;
const MAX_LINE = 400;

export interface Heard {
  /* Who spoke, as the call lists them. */
  user: string;
  name: string;
  text: string;
  at: number;
}

export interface Question {
  user: string;
  name: string;
  /* What was asked, with the name taken off. */
  text: string;
  /* The conversation before it, oldest first. */
  context: Heard[];
}

/* The question inside `text`, or null when nobody asked the agent. */
export function addressed(text: string): string | null {
  const clean = text.trim();
  if (PRODUCT.test(clean)) return null;
  const lead = LEADING.exec(clean);
  if (lead) return clean.slice(lead[0].length).trim() || "";
  const tail = TRAILING.exec(clean);
  if (tail) return clean.slice(0, tail.index).trim();
  return null;
}

/* What Whisper writes for silence and noise; never a request. */
const NOISE = /^[\s.,!?…\-–—*[\]()♪]*(?:\[[^\]]*\]|\([^)]*\)|\*[^*]*\*|music|applause|silence|you|thanks for watching\.?|thank you\.?|bye\.?)?[\s.,!?…\-–—*[\]()♪]*$/i;

export function isNoise(text: string): boolean {
  return NOISE.test(text.trim()) || !/[\p{L}\p{N}]/u.test(text);
}

const words = (text: string): string[] => text.toLowerCase().match(/[\p{L}\p{N}']+/gu) ?? [];

/* Whether `text` is mostly the agent's own words coming back through a
 * microphone: a laptop speaker is heard by the laptop's microphone even
 * through echo cancellation, now and then. */
export function isEcho(text: string, spoken: string): boolean {
  const heard = words(text);
  if (heard.length < 3) return false;
  const said = new Set(words(spoken));
  const hits = heard.filter((w) => said.has(w)).length;
  return hits / heard.length >= 0.7;
}

/* What stops the agent mid-sentence: said alone, by anyone. */
const STOP = /^\W*(?:(?:hey|ok|okay)\W+)?(?:stop|quiet|enough|be quiet|shut up|never mind|cancel|that's enough)(?:\W+(?:please|thanks|now))?\W*$/i;

export function isStop(text: string): boolean {
  const clean = text.trim();
  if (STOP.test(clean)) return true;
  const asked = addressed(clean);
  return asked !== null && STOP.test(asked);
}

export class Dialog {
  private lines: Heard[] = [];
  private followUp = new Map<string, number>();
  /* What the agent said last, for the echo check. */
  private spoken = "";
  private spokeAt = 0;

  /* Everything said is kept for context, what it is worth. */
  hear(line: Heard): Question | null {
    const text = line.text.trim().slice(0, MAX_LINE);
    if (isNoise(text)) return null;
    // Its own voice, heard again through a speaker, is not a request.
    if (line.at - this.spokeAt < 15_000 && isEcho(text, this.spoken)) return null;
    const asked = addressed(text);
    const following = asked === null && (this.followUp.get(line.user) ?? 0) > line.at;
    const context = this.context(line.at);
    this.lines.push({ ...line, text });
    if (this.lines.length > 200) this.lines.splice(0, this.lines.length - 200);
    if (asked === null && !following) return null;
    const question = (asked ?? text).trim();
    // "LosOS." on its own is a person getting its attention.
    return { user: line.user, name: line.name, text: question, context };
  }

  private context(now: number): Heard[] {
    return this.lines.filter((l) => now - l.at <= CONTEXT_MS).slice(-CONTEXT_LINES);
  }

  /* The agent answered `user`: their next sentence is for it too. */
  answered(user: string, text: string, now: number): void {
    this.followUp.set(user, now + FOLLOW_UP_MS);
    this.spoken = text;
    this.spokeAt = now;
    this.lines.push({ user: "agent", name: "LosOS", text, at: now });
  }

  /* The agent is done with `user`: the next sentence needs the name again. */
  closed(user: string): void {
    this.followUp.delete(user);
  }
}

/* The ask the box's assistant answers: the conversation, then the
 * question, and the register of a voice. */
export function prompt(q: Question): string {
  const lines = q.context.map((l) => `${l.name}: ${l.text}`).join("\n");
  return [
    "You are LosOS, taking part in a voice call. Your answer is read aloud to everyone in the call.",
    "Answer in plain spoken English, in at most three short sentences. No lists, no markdown, no links, no emoji.",
    "If you need to look something up on the box, do it, then say only the result.",
    lines === "" ? "" : `\nThe conversation so far:\n${lines}`,
    `\n${q.name} asks you: ${q.text === "" ? "(they said your name and nothing else; ask what they need)" : q.text}`,
  ]
    .filter((l) => l !== "")
    .join("\n");
}
