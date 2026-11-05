/* An answer that is still being written, cut into sentences to speak.
 *
 * The assistant's answer grows while the turn runs. Speaking waits for a
 * whole sentence, which is what makes the voice sound like speech and not
 * like a ticker, and what lets the first sentence start while the rest is
 * still being written. */

/* Markdown has no sound. */
export function plain(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, " ")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*(?:[-*+]|\d+[.)])\s+/gm, "")
    .replace(/[*_~]{1,3}([^*_~\n]+)[*_~]{1,3}/g, "$1")
    .replace(/[\u{1F000}-\u{1FFFF}☀-➿]/gu, "")
    .replace(/\s+/g, " ")
    .trim();
}

/* A sentence that gets this long without an end is cut at a comma. */
const LONGEST = 220;
const ABBREVIATIONS = /\b(?:mr|mrs|ms|dr|prof|sr|jr|vs|etc|e\.g|i\.e|st|no)\.$/i;

export class SentenceStream {
  /* How much of the raw answer has been handed out. */
  private done = 0;

  /* The sentences completed by `answer`, which is the whole text so far.
   * With `final`, whatever is left is a sentence too. */
  feed(answer: string, final = false): string[] {
    const out: string[] = [];
    let from = this.done;
    for (let i = from; i < answer.length; i++) {
      const c = answer[i];
      // Code is not read aloud, and its lines are not sentences.
      if (c === "`" && answer.startsWith("```", i)) {
        const close = answer.indexOf("```", i + 3);
        if (close < 0) {
          if (!final) break;
          this.done = answer.length;
          return out;
        }
        i = close + 2;
        continue;
      }
      const end = c === "." || c === "!" || c === "?" || c === "\n";
      if (end) {
        const next = answer[i + 1];
        // Wait for what follows: "3." may be "3.5".
        if (next === undefined && !final) break;
        if (next !== undefined && !/\s/.test(next) && c !== "\n") continue;
        const piece = answer.slice(from, i + 1);
        if (c === "." && ABBREVIATIONS.test(piece.trimEnd())) continue;
        this.take(piece, out);
        from = i + 1;
        this.done = from;
      } else if (i - from > LONGEST && (c === "," || c === ";" || c === " ")) {
        this.take(answer.slice(from, i + 1), out);
        from = i + 1;
        this.done = from;
      }
    }
    if (final && from < answer.length) {
      this.take(answer.slice(from), out);
      this.done = answer.length;
    }
    return out;
  }

  private take(piece: string, out: string[]): void {
    const text = plain(piece);
    if (/[\p{L}\p{N}]/u.test(text)) out.push(text);
  }
}
