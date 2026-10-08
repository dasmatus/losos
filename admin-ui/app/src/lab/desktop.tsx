/* LosOS Desktop on the laptop: the systemd-homed sign-in screen, then
 * derisk's overview (workspaces along the top, window tiles, and the clock,
 * suggested apps, services, calendar and notes beside them). Danube is the
 * browser and shows the core's pages; the Terminal runs the simulated shell.
 * Ported from side.js's desktopPane. This is a picture of the laptop's
 * screen, so its own words stay as LosOS Desktop says them. */

import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { ArrowExpand01Icon, ArrowLeft01Icon, DashboardSquare01Icon, RefreshIcon, ArrowShrink02Icon, ComputerTerminal01Icon, Folder01Icon, Globe02Icon, ShoppingBag01Icon } from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import type { Device } from "./core";
import { PageView } from "./page-view";
import { useLab } from "./store";
import { SimTerminal } from "./terminal";

/* The apps' glyphs are the admin UI's own icon set, in the house accent. */
const glyph = (icon: IconSvgElement) => <HugeiconsIcon icon={icon} size={16} strokeWidth={1.8} color="currentColor" className="text-accent" />;
const APPS = {
  danube: { name: "Danube", icon: glyph(Globe02Icon) },
  term: { name: "Terminal", icon: glyph(ComputerTerminal01Icon) },
  files: { name: "Files", icon: glyph(Folder01Icon) },
  bazaar: { name: "Bazaar", icon: glyph(ShoppingBag01Icon) },
} as const;
type AppKey = keyof typeof APPS;

function deskClock(now: Date) {
  return {
    hm: now.toTimeString().slice(0, 5),
    day: now.toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" }).replace(",", ""),
  };
}

function Calendar({ now }: { now: Date }) {
  const y = now.getFullYear();
  const m = now.getMonth();
  const first = (new Date(y, m, 1).getDay() + 6) % 7;
  const days = new Date(y, m + 1, 0).getDate();
  return (
    <div className="cal">
      {["M", "T", "W", "T", "F", "S", "S"].map((x, i) => (
        <i key={i}>{x}</i>
      ))}
      {Array.from({ length: first }, (_, i) => (
        <span key={"e" + i} />
      ))}
      {Array.from({ length: days }, (_, i) => (
        <span key={i} className={i + 1 === now.getDate() ? "today" : undefined}>
          {i + 1}
        </span>
      ))}
    </div>
  );
}

export function Desktop({ d }: { d: Device }) {
  const s = useLab();
  const t = useT();
  const a = actions();
  const st = s.desk(d.id);
  const wide = s.ui.wide === "desk";
  const now = new Date(s.snap.clock.t);
  const ck = deskClock(now);
  const [pw, setPw] = React.useState("");
  const pwRef = React.useRef<HTMLInputElement>(null);
  const [url, setUrl] = React.useState(st.url);
  React.useEffect(() => setUrl(st.url), [st.url]);
  React.useEffect(() => {
    if (!st.signedIn && d.power) {
      const h = window.setTimeout(() => pwRef.current?.focus({ preventScroll: true }), 30);
      return () => window.clearTimeout(h);
    }
  }, [st.signedIn, d.power, d.id]);

  const toggleWide = () => {
    s.setUi({ wide: wide ? "none" : "desk" });
    requestAnimationFrame(() => s.fit());
  };
  const expand = (
    <button
      type="button"
      className="dk-expand"
      aria-label={wide ? t("lab.desk.shrink") : t("lab.desk.expand")}
      title={wide ? t("lab.desk.shrink") : t("lab.desk.expand")}
      onClick={toggleWide}
    >
      <HugeiconsIcon icon={wide ? ArrowShrink02Icon : ArrowExpand01Icon} size={14} strokeWidth={1.8} color="currentColor" />
    </button>
  );

  if (!d.power) {
    return (
      <div className="desk">
        <div className="desk-off">{t("lab.desk.off")}</div>
      </div>
    );
  }

  if (!st.signedIn) {
    return (
      <div className={cn("desk login", wide && "wide")} data-testid="desk-login">
        {expand}
        <div className="dk-login">
          <div className="big">{ck.hm}</div>
          <div className="date">{ck.day}</div>
          <div className="user">matus</div>
          <input
            ref={pwRef}
            type="password"
            aria-label="Password"
            placeholder={st.err ? "Sorry, try again" : "Password"}
            autoComplete="off"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Enter") return;
              if (!pw) st.err = true;
              else {
                st.err = false;
                st.signedIn = true;
              }
              s.emit("model");
            }}
          />
          <div className={st.err ? "bad" : "hint"}>
            {st.err ? "Password incorrect or not sufficient for authentication of user matus." : "Enter your password to log in"}
          </div>
          <div className="homed">systemd-homed · LUKS home for matus</div>
        </div>
      </div>
    );
  }

  const net = s.snap.net;
  const f = net.iface[d.id];
  const addr = net.addr[d.id];
  const wifiLink = s.snap.world.links.find((l) => l.kind === "wifi" && (l.a.dev === d.id || l.b.dev === d.id));
  const ap = s.dev(wifiLink?.a.dev);
  const netText = f
    ? (f.port === "wlan0" ? `Wi-Fi ${String(ap?.cfg["ssid"] ?? "")}` : "Wired") + (addr ? " · " + addr.ip : "")
    : "Offline";
  const app = APPS[st.app];
  const openApp = (k: AppKey) => {
    if (k === "files" || k === "bazaar") {
      s.toast(t("lab.desk.notHere", { app: APPS[k].name }));
      return;
    }
    st.app = k;
    if (!st.open.includes(k)) st.open.push(k);
    s.emit("model");
  };
  const suggestions = s.core.suggestUrls(d.id);

  return (
    <div className={cn("desk", wide && "wide")} data-testid="desk">
      <div className="desk-bar">
        <span className="sq" aria-hidden="true" />
        <span className="search">Search or ask…</span>
        <span className="cur">
          {app.icon}
          {app.name}
        </span>
        <span className="win">Window</span>
        <span className="net">
          {f && (
            <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
              <path d="M1.5 6a9.5 9.5 0 0 1 13 0M4 8.6a6 6 0 0 1 8 0M6.4 11.1a2.4 2.4 0 0 1 3.2 0" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
              <circle cx="8" cy="13.4" r="1.1" fill="currentColor" />
            </svg>
          )}
          {netText}
        </span>
        <span className="wsn" title="Workspaces">
          <HugeiconsIcon icon={DashboardSquare01Icon} size={13} strokeWidth={1.8} color="currentColor" className="inline align-[-2px]" /> {st.open.length}
        </span>
        {expand}
      </div>
      <div className="desk-body">
        <div className="desk-tiles">
          <div className="ws">
            <div className="on">
              {st.open.map((k) => (
                <button key={k} type="button" className={cn("mini", k === st.app && "on")} title={APPS[k].name} onClick={() => openApp(k)}>
                  {APPS[k].icon}
                </button>
              ))}
            </div>
            <div className="plus" aria-hidden="true">
              +
            </div>
          </div>
          <div className="dwin">
            {st.app === "term" ? (
              <>
                <div className="dwin-head">
                  <b>Terminal</b>
                  <span className="muted">matus@{d.name}</span>
                </div>
                <div className="dk-term">
                  <SimTerminal devId={d.id} />
                </div>
              </>
            ) : (
              <>
                <div className="dwin-head">
                  <button type="button" aria-label="Back" onClick={() => a.back(d.id)}>
                    <HugeiconsIcon icon={ArrowLeft01Icon} size={13} strokeWidth={1.8} color="currentColor" />
                  </button>
                  <button type="button" aria-label="Reload" onClick={() => st.url && a.navigate(d.id, st.url, true)}>
                    <HugeiconsIcon icon={RefreshIcon} size={13} strokeWidth={1.8} color="currentColor" />
                  </button>
                  <input
                    aria-label="Address"
                    list={`lab-sugg-${d.id}`}
                    value={url}
                    placeholder="Type a box name, like mattbox.local"
                    spellCheck={false}
                    data-testid="danube-url"
                    onChange={(e) => setUrl(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") a.navigate(d.id, url);
                    }}
                  />
                  <datalist id={`lab-sugg-${d.id}`}>
                    {suggestions.map((u) => (
                      <option key={u} value={u} />
                    ))}
                  </datalist>
                </div>
                <div className="dwin-body">
                  {st.loading ? (
                    <div className="page err">
                      <p>Loading {st.url}…</p>
                      <p className="muted">{s.snap.sim.mode === "simulation" ? t("lab.desk.loadingSim") : ""}</p>
                    </div>
                  ) : st.html ? (
                    <PageView html={st.html} />
                  ) : (
                    <div className="page err">
                      <h1>Danube</h1>
                      <p>Try one of these from this laptop:</p>
                      {suggestions.map((u) => (
                        <p key={u}>
                          <a
                            href={u}
                            onClick={(e) => {
                              e.preventDefault();
                              a.navigate(d.id, u);
                            }}
                          >
                            <code>{u}</code>
                          </a>
                        </p>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}
          </div>
        </div>
        <div className="side-wid">
          <div className="w-clock">
            <div className="clock">{ck.hm}</div>
            <div className="muted">{ck.day}</div>
          </div>
          <div>
            <div className="wt">Suggested</div>
            <div className="apps">
              {(Object.keys(APPS) as AppKey[]).map((k) => (
                <button key={k} type="button" onClick={() => openApp(k)}>
                  {APPS[k].icon}
                  <span>{APPS[k].name}</span>
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="wt">Services</div>
            <div className="muted">All user services running ✓</div>
          </div>
          <div className="w-cal">
            <div className="wt">{ck.day}</div>
            <Calendar now={now} />
          </div>
          <div className="w-notes">
            <div className="wt">Notes</div>
            <textarea
              aria-label="Notes"
              placeholder="Write something down"
              defaultValue={st.notes}
              onChange={(e) => {
                st.notes = e.target.value;
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
