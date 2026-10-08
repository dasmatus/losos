/* "This box": the setup drawn from the running box's own API. Only in the
 * copy the appliance serves at /lab/. It reads /api/settings and /api/edge
 * with the key the admin page keeps for this tab (lib/api.ts, sessionStorage
 * "losos-token"), and the core draws the box, its router, the edges it found
 * and a laptop. Nothing here writes to the box. */

const TOKEN_KEY = "losos-token";

export type ThisBox =
  | { settings: string; edge: string }
  | { error: "signedOut" }
  | { error: "noAnswer"; detail: string };

export async function readThisBox(): Promise<ThisBox> {
  let token: string | null = null;
  try {
    token = window.sessionStorage.getItem(TOKEN_KEY);
  } catch {
    /* storage blocked */
  }
  if (!token) return { error: "signedOut" };
  const get = async (path: string): Promise<string> => {
    const r = await fetch(path, { headers: { Authorization: "Bearer " + token }, cache: "no-store" });
    if (!r.ok) throw new Error(`${path} answered ${r.status}`);
    const text = await r.text();
    JSON.parse(text);
    return text;
  };
  try {
    const [settings, edge] = await Promise.all([get("/api/settings"), get("/api/edge").catch(() => "null")]);
    return { settings, edge };
  } catch (e) {
    return { error: "noAnswer", detail: e instanceof Error ? e.message : String(e) };
  }
}
