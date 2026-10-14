/* Reading from the terminal outside the full-screen view: a value typed
 * without echo, or everything piped on stdin. */

export class Cancelled extends Error {
  constructor() {
    super("cancelled");
    this.name = "Cancelled";
  }
}

/** One line typed with no echo. Ctrl-C and Ctrl-D cancel. */
export function readHidden(prompt: string, stdin: NodeJS.ReadStream = process.stdin, out: NodeJS.WriteStream = process.stderr): Promise<string> {
  return new Promise((resolve, reject) => {
    out.write(prompt);
    const wasRaw = stdin.isRaw;
    stdin.setRawMode(true);
    stdin.resume();
    stdin.setEncoding("utf8");
    let value = "";
    const done = (error: Error | null): void => {
      stdin.removeListener("data", onData);
      stdin.setRawMode(wasRaw);
      stdin.pause();
      out.write("\n");
      if (error === null) resolve(value);
      else reject(error);
    };
    const onData = (chunk: string): void => {
      for (const ch of chunk) {
        if (ch === "\r" || ch === "\n") return done(null);
        if (ch === "\u0003" || ch === "\u0004") return done(new Cancelled());
        if (ch === "\u007f" || ch === "\b") value = [...value].slice(0, -1).join("");
        else if (ch === "\u0015") value = "";
        else if (ch >= " ") value += ch;
      }
    };
    stdin.on("data", onData);
  });
}

/** Everything on stdin, for a value piped in by a script. */
export async function readAllStdin(stdin: NodeJS.ReadableStream = process.stdin): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of stdin) chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : (chunk as Buffer));
  return Buffer.concat(chunks).toString("utf8");
}

/** A value from the terminal without echo, or from stdin when it is piped. A
 *  piped value loses one trailing newline, the one `echo` adds. */
export async function readSecret(prompt: string): Promise<string> {
  if (process.stdin.isTTY) return readHidden(prompt);
  return (await readAllStdin()).replace(/\r?\n$/, "");
}

/** y or n, on a terminal. */
export function askYesNo(prompt: string, stdin: NodeJS.ReadStream = process.stdin, out: NodeJS.WriteStream = process.stderr): Promise<boolean> {
  return new Promise((resolve) => {
    out.write(prompt);
    const wasRaw = stdin.isRaw;
    stdin.setRawMode(true);
    stdin.resume();
    stdin.setEncoding("utf8");
    const onData = (chunk: string): void => {
      const ch = chunk[0] ?? "";
      if (!/[yYnN\u0003\r]/.test(ch)) return;
      stdin.removeListener("data", onData);
      stdin.setRawMode(wasRaw);
      stdin.pause();
      out.write(`${ch === "y" || ch === "Y" ? "y" : "n"}\n`);
      resolve(ch === "y" || ch === "Y");
    };
    stdin.on("data", onData);
  });
}
