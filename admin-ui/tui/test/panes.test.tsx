/* Every pane, drawn with ink-testing-library over the fixtures, plus the
 * keyboard paths that change something: a switch, Apply and its
 * confirmation, the pane search, an edge secret, and the login screen. */

import assert from "node:assert/strict";
import { afterEach, describe, it } from "node:test";
import { render } from "ink-testing-library";
import type { Transport } from "../src/api";
import { hasKey, LOCALES, setLocale, t, type Locale } from "../src/lang";
import { makeTheme } from "../src/theme";
import { BoxApp, EdgeApp, paneMatches } from "../src/tui/app";
import { BOX_PANES, FILES_APPS } from "../src/tui/box-panes";
import { EDGE_PANES } from "../src/tui/edge-panes";
import { Login, type LoginAnswer } from "../src/tui/login";
import type { HostRecord } from "../src/vault";
import { boxRoute, edgeRoute } from "./fixtures";

const COLS = 120;
const ROWS = 36;
const theme = makeTheme("dark", true);

const BOX: HostRecord = {
  url: "https://mattbox.local",
  kind: "box",
  fingerprint: "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D",
  publicKey: "",
  token: "",
  keyFingerprint: "B".repeat(40),
};
const EDGE: HostRecord = { ...BOX, url: "https://register.losos.dasmat.us", kind: "edge" };

interface Recorded {
  calls: { method: string; path: string; body: unknown }[];
  transport: Transport;
}

function fixtureTransport(route: typeof boxRoute): Recorded {
  const calls: Recorded["calls"] = [];
  return {
    calls,
    transport: {
      request: async (method, path, body) => {
        calls.push({ method, path, body: body ?? null });
        return { ...route(method, path, body ?? null), sealed: true };
      },
    },
  };
}

const strip = (s: string): string => s.replace(/\x1b\[[0-9;]*m/g, "");
const tick = (ms = 30): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

async function waitFor(frame: () => string, test: (text: string) => boolean, what: string, ms = 3000): Promise<string> {
  const end = Date.now() + ms;
  for (;;) {
    const text = strip(frame());
    if (test(text)) return text;
    if (Date.now() > end) assert.fail(`timed out waiting for ${what}; last frame:\n${text}`);
    await tick();
  }
}

function clean(text: string, where: string): void {
  for (const bad of ["undefined", "NaN", "[object", "—"]) {
    assert.ok(!text.includes(bad), `${where} shows ${JSON.stringify(bad)}:\n${text}`);
  }
  const lines = text.split("\n");
  assert.ok(lines.length <= ROWS, `${where} is ${lines.length} rows tall`);
  for (const line of lines) assert.ok([...line].length <= COLS, `${where} has a line wider than ${COLS}: ${line}`);
}

let unmount: (() => void) | null = null;
afterEach(() => {
  unmount?.();
  unmount = null;
  setLocale("en");
});

/** Words each pane shows once its data is in. */
const EXPECT: Record<string, string[]> = {
  overview: ["mattbox", "Files", "jellyfin", "131 GB"],
  storage: ["466 GB", "Use reserve"],
  mesh: ["register.losos.dasmat.us", "23:00", "07:00"],
  apps: ["jellyfin", "vaultwarden", "Search for apps"],
  network: ["mattbox.local", "11000"],
  security: ["Stricter memory handling", "Ignore USB devices"],
  advanced: ["losos.hostName", "losos.tpm.enable", "Set by the installer"],
  history: ["matus/losos-config", "4f2a9c1"],
  backup: ["matus-losos", "Back up now"],
  about: ["mattbox", "Handbook", "23:00-07:00"],
  "edge-overview": ["Holds a LosOS certificate", "Relay routes", "Reconcile"],
  "edge-secrets": ["stripe-secret-key", "claude-api-key", "Not set"],
};

describe("the panes", () => {
  for (const pane of BOX_PANES) {
    it(`draws ${pane.id}`, async () => {
      setLocale("en");
      const { transport } = fixtureTransport(boxRoute);
      const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane={pane.id} />);
      unmount = r.unmount;
      const words = EXPECT[pane.id]!;
      const text = await waitFor(() => r.lastFrame() ?? "", (f) => words.every((w) => f.includes(w)), `${pane.id}: ${words.join(", ")}`);
      assert.ok(text.includes("LosOS"), "the header names LosOS");
      assert.ok(text.includes(pane.label()), "the pane's title");
      assert.ok(text.includes("q  quit"), "the footer shows the keys");
      clean(text, pane.id);
    });
  }

  for (const pane of EDGE_PANES) {
    it(`draws ${pane.id}`, async () => {
      const { transport } = fixtureTransport(edgeRoute);
      const r = render(<EdgeApp host={EDGE} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane={pane.id} />);
      unmount = r.unmount;
      const words = EXPECT[pane.id]!;
      const text = await waitFor(() => r.lastFrame() ?? "", (f) => words.every((w) => f.includes(w)), `${pane.id}: ${words.join(", ")}`);
      clean(text, pane.id);
    });
  }

  it("lists the tenants with their last heartbeat, and the web-only caption on About", async () => {
    const { transport } = fixtureTransport(edgeRoute);
    const r = render(<EdgeApp host={EDGE} transport={transport} theme={theme} locale="en" cols={COLS} rows={90} initialPane="edge-overview" />);
    unmount = r.unmount;
    const text = await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("zuzka-nas"), "the tenants");
    assert.match(text, /mattbox.*21 s|mattbox.*seconds|mattbox.*ago/);
    assert.ok(text.includes("office-01"));
    r.unmount();
    const box = fixtureTransport(boxRoute);
    const about = render(<BoxApp host={BOX} transport={box.transport} theme={theme} locale="en" cols={COLS} rows={90} initialPane="about" />);
    unmount = about.unmount;
    const shown = await waitFor(() => about.lastFrame() ?? "", (f) => f.includes("Handbook"), "about");
    for (const word of [t("tui.about.webOnly")]) assert.ok(shown.includes(word.slice(0, 40)), `about says ${word}`);
  });

  for (const locale of ["sk", "de"] as Locale[]) {
    it(`draws every pane in ${locale}`, async () => {
      setLocale(locale);
      for (const pane of BOX_PANES) {
        const { transport } = fixtureTransport(boxRoute);
        const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale={locale} cols={COLS} rows={ROWS} initialPane={pane.id} />);
        const text = await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(pane.label()) && !f.includes(t("tui.loading")), `${locale} ${pane.id}`);
        r.unmount();
        clean(text, `${locale} ${pane.id}`);
      }
    });
  }

  it("has a word for every key the panes make up at run time", () => {
    const keys = [
      ...FILES_APPS.map((id) => `apps.${id}.name`),
      ...["off", "pending", "ok", "waiting", "refused", "diverged", "unavailable", "error"].map((s) => `history.sync.state.${s}`),
      ...["off", "pending", "ok", "kept", "waiting", "unavailable", "error"].map((s) => `history.update.state.${s}`),
      ...["usbguard", "malloc", "apparmor", "nosmt"].flatMap((n) => [`panes.security.${n}.title`, `panes.security.${n}.detail`]),
      ...BOX_PANES.filter((p) => p.id !== "overview").flatMap((p) => ["label", "summary", "keywords"].map((k) => `settings.panes.${p.id}.${k}`)),
    ];
    const missing = keys.filter((k) => !hasKey(k));
    assert.deepEqual(missing, []);
    for (const locale of LOCALES) {
      setLocale(locale);
      for (const k of keys) if (!k.endsWith(".keywords")) assert.ok(t(k as never).length > 0, `${locale} ${k}`);
    }
  });
});

describe("the keyboard", () => {
  it("turns a switch on, applies it and confirms before the rebuild", async () => {
    const { transport, calls } = fixtureTransport(boxRoute);
    const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane="security" />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("Ignore USB devices"), "security");
    r.stdin.write("\r");
    const dirty = await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("settings.applyBar.discard")), "the apply bar");
    assert.ok(dirty.includes(t("settings.applyBar.pending", { count: 1 })));
    assert.equal(calls.filter((c) => c.path === "/api/apply").length, 0, "nothing applied yet");
    r.stdin.write("a");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("tui.apply.title")), "the confirmation");
    assert.equal(calls.filter((c) => c.path === "/api/apply").length, 0, "the confirmation comes first");
    r.stdin.write("y");
    await waitFor(() => "", () => calls.some((c) => c.path === "/api/apply"), "POST /api/apply");
    const applied = calls.find((c) => c.path === "/api/apply")!;
    assert.equal(applied.method, "POST");
    assert.equal(typeof applied.body, "string");
    assert.match(applied.body as string, /losos\.hardening\.usbguard = true;/);
    assert.match(applied.body as string, /losos\.updates\.extraSubstituters = \[ "https:\/\/cache\.losos\.dasmat\.us" \];/);
  });

  it("discards a draft with d", async () => {
    const { transport } = fixtureTransport(boxRoute);
    const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane="security" />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("Ignore USB devices"), "security");
    r.stdin.write("\r");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("settings.applyBar.discard")), "the apply bar");
    r.stdin.write("d");
    await waitFor(() => r.lastFrame() ?? "", (f) => !f.includes(t("settings.applyBar.discard")), "no apply bar");
  });

  it("filters the panes with / and opens the one picked", async () => {
    const { transport } = fixtureTransport(boxRoute);
    const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("jellyfin"), "the overview");
    r.stdin.write("/");
    await tick();
    for (const ch of "bucket") {
      r.stdin.write(ch);
      await tick(10);
    }
    const filtered = await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("/ bucket"), "the query");
    const sidebar = filtered.split("\n").map((l) => l.slice(0, 26)).join("\n");
    assert.ok(sidebar.includes("Backup"));
    assert.ok(!sidebar.includes("Mesh"));
    r.stdin.write("\r");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("matus-losos"), "the backup pane");
  });

  it("matches a pane by its English words in any language", () => {
    setLocale("sk");
    const backup = BOX_PANES.find((p) => p.id === "backup")!;
    assert.equal(paneMatches(backup, "backup"), true);
    assert.equal(paneMatches(backup, backup.label()), true);
    assert.equal(paneMatches(backup, "zzz-nothing"), false);
  });

  it("moves between panes from the sidebar with j and Enter", async () => {
    const { transport } = fixtureTransport(boxRoute);
    const r = render(<BoxApp host={BOX} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("jellyfin"), "the overview");
    // Typed faster than a render: one chunk, still two keys.
    r.stdin.write("j\r");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("Use reserve"), "the storage pane");
    r.stdin.write("?");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("tui.help.title")), "the help");
  });

  it("sets an edge secret without showing it", async () => {
    const { transport, calls } = fixtureTransport(edgeRoute);
    const r = render(<EdgeApp host={EDGE} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane="edge-secrets" />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("stripe-secret-key"), "secrets");
    r.stdin.write("\r");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("tui.edge.newValue", { name: "stripe-secret-key" })), "the prompt");
    for (const ch of "sk_live_42") {
      r.stdin.write(ch);
      await tick(5);
    }
    const typing = strip(r.lastFrame() ?? "");
    assert.ok(!typing.includes("sk_live_42"), "the value is masked while typed");
    r.stdin.write("\r");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes(t("tui.edge.secretSet", { name: "stripe-secret-key" })), "the notice");
    assert.deepEqual(
      calls.find((c) => c.method === "POST"),
      { method: "POST", path: "/secrets", body: { name: "stripe-secret-key", value: "sk_live_42" } },
    );
  });

  it("asks before reconciling the edge", async () => {
    const { transport, calls } = fixtureTransport(edgeRoute);
    const r = render(<EdgeApp host={EDGE} transport={transport} theme={theme} locale="en" cols={COLS} rows={ROWS} initialPane="edge-overview" />);
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("Reconcile now"), "the overview");
    for (let i = 0; i < 6; i += 1) {
      r.stdin.write("j");
      await tick(10);
    }
    r.stdin.write("\r");
    await tick(60);
    if (!calls.some((c) => c.path === "/reconcile")) {
      r.stdin.write("y");
      await waitFor(() => "", () => calls.some((c) => c.path === "/reconcile"), "POST /reconcile");
    }
    assert.ok(calls.some((c) => c.method === "POST" && c.path === "/reconcile"));
  });
});

describe("the login screen", () => {
  it("shows the fingerprint in groups of four and takes a token and a key", async () => {
    let answer: LoginAnswer | null | undefined;
    const r = render(
      <Login
        theme={theme}
        url="https://mattbox.local"
        kind="box"
        fingerprint={BOX.fingerprint}
        keys={[
          { fingerprint: "A".repeat(40), userIds: ["Matus <matus@example.org>"] },
          { fingerprint: "C".repeat(40), userIds: ["LosOS CI <dasmatus/losos>"] },
        ]}
        cols={COLS}
        onDone={(a) => (answer = a)}
      />,
    );
    unmount = r.unmount;
    const first = await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("3F2A 9C1E 0B7D 44A1 C2D3"), "the fingerprint");
    assert.ok(first.includes("E4F5 0617 2839 4A5B 6C7D"));
    clean(first, "login");
    r.stdin.write("y");
    await tick();
    const token = `losos_pat_0a1b2c3d_${"a".repeat(40)}`;
    // Pasted with its newline, in one chunk.
    r.stdin.write(`${token}\r`);
    await tick();
    assert.ok(!strip(r.lastFrame() ?? "").includes(token), "the token is not echoed");
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("LosOS CI <dasmatus/losos>"), "the key picker");
    r.stdin.write("j");
    await tick();
    r.stdin.write("\r");
    await waitFor(() => "", () => answer !== undefined, "the answer");
    assert.deepEqual(answer, { token, keyFingerprint: "C".repeat(40) });
  });

  it("answers null when the fingerprint is refused", async () => {
    let answer: LoginAnswer | null | undefined;
    const r = render(
      <Login theme={theme} url="https://edge.example" kind="edge" fingerprint={BOX.fingerprint} keys={[]} cols={COLS} onDone={(a) => (answer = a)} />,
    );
    unmount = r.unmount;
    await waitFor(() => r.lastFrame() ?? "", (f) => f.includes("3F2A"), "the fingerprint");
    r.stdin.write("n");
    await waitFor(() => "", () => answer !== undefined, "the answer");
    assert.equal(answer, null);
  });
});
