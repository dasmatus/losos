/* "This box": the setup drawn from the running box's own API. Only in the
 * copy the appliance serves at /diagrams/. It reads /api/settings and
 * /api/edge with the spare key the admin page keeps for this tab (lib/api.ts,
 * sessionStorage "losos-token") when there is one, else with the LosOS Login
 * cookie, and the core draws the box, its router, the edges it found and a
 * laptop. Nothing here writes to the box. */

const TOKEN_KEY = "losos-token";
const SIGNED_OUT = new Error("signed out");

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
  const headers: Record<string, string> = token ? { Authorization: "Bearer " + token } : {};
  const get = async (path: string): Promise<string> => {
    const r = await fetch(path, { headers, cache: "no-store" });
    if (r.status === 401) throw SIGNED_OUT;
    if (!r.ok) throw new Error(`${path} answered ${r.status}`);
    const text = await r.text();
    JSON.parse(text);
    return text;
  };
  try {
    const [settings, edge] = await Promise.all([get("/api/settings"), get("/api/edge").catch(() => "null")]);
    return { settings, edge };
  } catch (e) {
    if (e === SIGNED_OUT) return { error: "signedOut" };
    return { error: "noAnswer", detail: e instanceof Error ? e.message : String(e) };
  }
}
