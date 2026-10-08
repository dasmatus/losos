/* The inspector: what the selected device or cable is, how it is set, its
 * hardware, its console and (on a laptop) its desktop. With nothing selected
 * it explains the setup. Ported from side.js. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import { Badge, StatusDot } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { MESSAGES } from "@/i18n";
import type { MessageKey } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import type { BoxState, Device, Link } from "./core";
import { Desktop } from "./desktop";
import { useEngine } from "./engine-hook";
import { DeviceIcon, hwDrawing } from "./icons";
import { type LabStore, type Tab, useLab } from "./store";
import { SimTerminal } from "./terminal";

/* `code` spans in a sentence. */
function Codey({ text }: { text: string }) {
  const parts = text.split(/`([^`]+)`/);
  return (
    <>
      {parts.map((p, i) =>
        i % 2 ? (
          <code key={i} className="font-mono text-[0.95em]">
            {p}
          </code>
        ) : (
          p
        ),
      )}
    </>
  );
}

function Kv({ children }: { children: React.ReactNode }) {
  return <dl className="mb-3 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[13px]">{children}</dl>;
}
function Row({ k, children, mono }: { k: React.ReactNode; children: React.ReactNode; mono?: boolean }) {
  return (
    <>
      <dt className="text-muted">{k}</dt>
      <dd className={cn("m-0 min-w-0 [overflow-wrap:anywhere]", mono && "font-mono")}>{children}</dd>
    </>
  );
}
function Sect({ children }: { children: React.ReactNode }) {
  return <div className="mt-3.5 mb-1.5 text-[11px] font-semibold tracking-[0.07em] text-muted uppercase">{children}</div>;
}
function Tag({ tone, children }: { tone?: "ok" | "warn" | "crit" | "acc"; children: React.ReactNode }) {
  const variant = tone === "acc" ? "local" : (tone ?? "neutral");
  return (
    <Badge variant={variant} className="px-2 text-[11.5px] leading-[18px] font-normal">
      {children}
    </Badge>
  );
}
function StatusCard({ children }: { children: React.ReactNode }) {
  return <div className="my-2 rounded-card border border-line px-3 py-2 text-[13px]">{children}</div>;
}

function tabsFor(s: LabStore, d: Device): [Tab, MessageKey][] {
  const list: [Tab, MessageKey][] = [
    ["status", "lab.tab.status"],
    ["config", "lab.tab.config"],
    ["physical", "lab.tab.physical"],
  ];
  if (s.consoleable(d)) list.splice(1, 0, ["console", "lab.tab.console"]);
  if (d.type === "laptop") list.unshift(["desktop", "lab.tab.desktop"]);
  return list;
}

export function Inspector() {
  const s = useLab();
  const sel = s.ui.sel;
  if (sel?.kind === "link") {
    const l = s.link(sel.id);
    if (l) return <LinkPane s={s} l={l} />;
  }
  const d = sel?.kind === "dev" ? s.dev(sel.id) : undefined;
  if (!d) return <EmptyPane s={s} />;
  return <DevicePane s={s} d={d} />;
}

function EmptyPane({ s }: { s: LabStore }) {
  const t = useT();
  const engine = useEngine();
  const key = s.snap.scenario;
  const sc = s.scenarios.find((x) => x.key === key);
  const name = sc?.name ?? s.snap.fileName ?? t("lab.top.mySetup");
  const blurb = sc?.blurb ?? t("lab.empty.fileBlurb");
  return (
    <div className="overflow-auto px-3.5 py-4.5 text-[13px] text-muted" data-testid="inspector-empty">
      <h3 className="mt-0 mb-1.5 text-[15px] font-semibold text-ink">{name || t("lab.top.mySetup")}</h3>
      <p className="mb-2">{blurb}</p>
      <ol className="my-2 list-decimal space-y-1 pl-[18px]">
        <li>{t("lab.empty.step1")}</li>
        <li>
          <Rich k="lab.empty.step2" vars={{ desktop: <b>{t("lab.tab.desktop")}</b>, local: <code>.local</code> }} />
        </li>
        <li>
          <Rich k="lab.empty.step3" vars={{ simulation: <b>{t("lab.mode.simulation")}</b> }} />
        </li>
        <li>{t("lab.empty.step4")}</li>
      </ol>
      <p>
        <b>{t("lab.empty.realTitle")}</b> {engine.available ? t("lab.empty.realEmulated", { engine: engine.probe.label }) : t("lab.empty.realSimulated")}{" "}
        {t("lab.empty.realModel")}
      </p>
    </div>
  );
}

function LinkPane({ s, l }: { s: LabStore; l: Link }) {
  const t = useT();
  const A = s.dev(l.a.dev);
  const B = s.dev(l.b.dev);
  const st = s.linkState(l);
  const label = s.catalog.linkKinds[l.kind]?.label ?? (l.kind === "wan" ? "WAN line" : l.kind);
  return (
    <div className="overflow-auto px-3.5 py-3" data-testid="inspector-link">
      <h2 className="m-0 text-[16px] font-semibold">{t("lab.link.title", { kind: label })}</h2>
      <div className="mb-2.5 text-[12px] text-muted">
        {A?.name} {l.a.port} ↔ {B?.name} {l.b.port}
      </div>
      <Kv>
        <Row k={t("lab.link.state")}>
          {st === "up" ? (
            <Tag tone="ok">up</Tag>
          ) : (
            <>
              <Tag tone="crit">down</Tag> {t("lab.link.oneOff")}
            </>
          )}
        </Row>
        <Row k={t("lab.link.medium")}>
          {l.kind === "wifi"
            ? `802.11, SSID ${String(A?.cfg["ssid"] ?? "losos-lab")}`
            : l.kind === "fiber"
              ? "1000BASE-LX"
              : l.kind === "wan"
                ? t("lab.link.ispLine")
                : "1000BASE-T, Cat6"}
        </Row>
      </Kv>
      <Button variant="secondary" size="sm" onClick={() => actions().removeLink(l.id)}>
        {l.kind === "wifi" ? t("lab.link.removeWifi") : t("lab.link.removeCable")}
      </Button>
    </div>
  );
}

function DevicePane({ s, d }: { s: LabStore; d: Device }) {
  const t = useT();
  const a = actions();
  const T = s.catalog.types[d.type];
  const list = tabsFor(s, d);
  const tab: Tab = list.some(([k]) => k === s.ui.tab) ? s.ui.tab : list[0]![0];
  const site = s.catalog.sites[d.site]?.name ?? d.site;

  // A serial console wants 72 columns: the inspector widens while it shows one.
  React.useEffect(() => {
    const want = tab === "console" ? "console" : s.ui.wide === "desk" && tab === "desktop" ? "desk" : "none";
    if (s.ui.wide !== want) {
      s.setUi({ wide: want });
      requestAnimationFrame(() => s.fit());
    }
  }, [tab, s]);

  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid="inspector-device">
      <div className="flex items-center gap-2.5 px-3.5 pt-3">
        <DeviceIcon type={d.type} size={44} className="flex-none" />
        <div className="min-w-0">
          <h2 className="m-0 text-[16px] font-semibold [text-wrap:balance]">{d.name}</h2>
          <div className="text-[12px] text-muted [text-wrap:balance]">
            {[T?.label ?? d.type, site, d.power ? t("lab.dev.on") : t("lab.dev.off")].map((part, i, all) => (
              <React.Fragment key={i}>
                <span className="whitespace-nowrap">{i < all.length - 1 ? `${part} ·` : part}</span>
                {i < all.length - 1 && " "}
              </React.Fragment>
            ))}
          </div>
        </div>
        <span className="flex-1" />
        <Button variant="secondary" size="xs" className="flex-none" onClick={() => a.togglePower(d.id)}>
          {d.power ? t("lab.dev.powerOff") : t("lab.dev.powerOn")}
        </Button>
      </div>
      <Tabs value={tab} onValueChange={(v) => s.setUi({ tab: v as Tab })} className="min-h-0 flex-1 gap-0">
        <TabsList className="mx-3.5 mt-2.5 mb-0 w-fit max-w-[calc(100%-28px)] overflow-x-auto">
          {list.map(([k, label]) => (
            <TabsTrigger key={k} value={k}>
              {t(label)}
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value="status" className="min-h-0 flex-1 overflow-auto px-3.5 pt-3 pb-4">
          <StatusPane s={s} d={d} />
        </TabsContent>
        <TabsContent value="config" className="min-h-0 flex-1 overflow-auto px-3.5 pt-3 pb-4">
          <ConfigPane s={s} d={d} />
        </TabsContent>
        <TabsContent value="physical" className="min-h-0 flex-1 overflow-auto px-3.5 pt-3 pb-4">
          <PhysicalPane s={s} d={d} />
        </TabsContent>
        <TabsContent value="console" className="mt-3 flex min-h-0 flex-1 flex-col">
          <ConsolePane s={s} d={d} />
        </TabsContent>
        <TabsContent value="desktop" className="mt-3 flex min-h-0 flex-1 flex-col">
          <Desktop d={d} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function tunnelTag(st: BoxState) {
  if (st.tunnel === "enrolled" || st.tunnel === "registered") return <Tag tone="ok">{st.tunnel}</Tag>;
  if (st.tunnel === "refused")
    return (
      <>
        <Tag tone="crit">refused</Tag> {st.reason}
      </>
    );
  if (st.tunnel === "stopped")
    return (
      <>
        <Tag tone="warn">stopped</Tag> {st.reason}
      </>
    );
  return "off";
}

function StatusPane({ s, d }: { s: LabStore; d: Device }) {
  const t = useT();
  const engine = useEngine();
  const net = s.snap.net;
  const T = s.catalog.types[d.type];
  const nm = (id: string) => s.name(id);
  const a = net.addr[d.id] ?? (d.type === "router" ? net.addr[d.id + "#lan"] : undefined);
  const f = net.iface[d.id];
  return (
    <div data-testid="pane-status">
      <Kv>
        {d.type === "router" ? (
          <>
            <Row k="LAN" mono>
              {net.subnet[d.id]}.0/24 · {t("lab.status.gatewayDot1")}
            </Row>
            <Row k="DHCP">{d.power ? t("lab.status.dhcpServing") : t("lab.dev.off")}</Row>
            <Row k="WAN" mono>
              {net.addr[d.id] ? `${net.addr[d.id]!.ip} (${net.addr[d.id]!.src})` : t("lab.status.notConnected")}
            </Row>
            <Row k="NAT">{t("lab.status.nat")}</Row>
          </>
        ) : T?.endpoint ? (
          <>
            <Row k={t("lab.status.interface")} mono>
              {f ? f.port : "—"}
            </Row>
            <Row k={t("lab.status.address")} mono>
              {a ? (
                <>
                  {a.ip}/{a.mask} <Tag>{a.src}</Tag>
                </>
              ) : (
                t("lab.status.none")
              )}
            </Row>
            {a?.gw && (
              <Row k={t("lab.status.gateway")} mono>
                {a.gw}
              </Row>
            )}
            <Row k={t("lab.status.internet")}>{net.internet[d.id] ? <Tag tone="ok">{t("lab.status.yes")}</Tag> : <Tag>{t("lab.status.no")}</Tag>}</Row>
            {a && a.src !== "public" && (
              <Row k={t("lab.status.mdns")} mono>
                {d.name}.local
              </Row>
            )}
            {engine.available && T?.emulate && (
              <Row k={t("lab.status.guest")}>
                {engine.isRunning(d.id) ? <Tag tone="ok">{t("lab.status.guestRunning", { backend: engine.backendLabel(d.id) })}</Tag> : t("lab.status.notRunning")}
              </Row>
            )}
          </>
        ) : (
          <>
            <Row k={t("lab.status.ports")}>{T?.ports.length ?? 0}</Row>
            <Row k={t("lab.status.inUse")}>{s.linksOf(d.id).length}</Row>
          </>
        )}
      </Kv>
      {d.type === "box" && net.losos.box[d.id] && <BoxStatus s={s} d={d} st={net.losos.box[d.id]!} />}
      {d.type === "edge-local" && net.losos.spoke[d.id] && (
        <>
          <Sect>{t("lab.status.gatewaySect")}</Sect>
          <Kv>
            <Row k={t("lab.status.lanAdvert")}>
              {d.cfg["advertise"] ? `_losos-edge._tcp, enrol=${d.cfg["openEnrolment"] ? "open" : "closed"}` : "off"}
            </Row>
            <Row k={t("lab.status.uplink")}>
              {net.losos.spoke[d.id]!.uplink === "up" ? (
                <>
                  <Tag tone="ok">up</Tag> → {nm(net.losos.spoke[d.id]!.hub ?? "")}
                </>
              ) : (
                net.losos.spoke[d.id]!.uplink
              )}
            </Row>
            <Row k={t("lab.status.zone")} mono>
              {String(d.cfg["zone"] ?? "")}
            </Row>
            <Row k={t("lab.status.enrolled")}>{net.losos.spoke[d.id]!.enrolled.map(nm).join(", ") || "—"}</Row>
            <Row k={t("lab.status.relayed")} mono>
              {net.losos.spoke[d.id]!.relayed.map((b) => net.losos.box[b]?.publicName ?? b).join("\n") || "—"}
            </Row>
            <Row k="Mesh">
              {d.cfg["cluster"] ? `rke2 server · ${net.losos.spoke[d.id]!.cluster.map(nm).join(", ") || "no nodes"}` : "off"}
            </Row>
          </Kv>
        </>
      )}
      {d.type === "edge-official" && net.losos.hub[d.id] && (
        <>
          <Sect>{t("lab.status.officialSect")}</Sect>
          <Kv>
            <Row k={t("lab.status.certificate")}>
              {d.cfg["certified"] ? <Tag tone="ok">{t("lab.status.signed")}</Tag> : <Tag tone="warn">{t("lab.status.notSigned")}</Tag>}
            </Row>
            <Row k={t("lab.status.registrar")} mono>
              register.{String(d.cfg["domain"] ?? "")}
            </Row>
            <Row k={t("lab.status.tenants")}>{net.losos.hub[d.id]!.tenants.map(nm).join(", ") || "—"}</Row>
            <Row k={t("lab.status.relayed")} mono>
              {net.losos.hub[d.id]!.relays.map((b) => net.losos.box[b]?.publicName ?? b).join("\n") || "—"}
            </Row>
            <Row k="Mesh">{net.losos.hub[d.id]!.cluster.map(nm).join(", ") || "—"}</Row>
          </Kv>
        </>
      )}
      {d.type === "laptop" && (
        <p className="mt-0 mb-2 text-[12px] text-muted">
          <Rich k="lab.status.laptopHint" vars={{ desktop: <b>{t("lab.tab.desktop")}</b> }} />
        </p>
      )}
    </div>
  );
}

function BoxStatus({ s, d, st }: { s: LabStore; d: Device; st: BoxState }) {
  const t = useT();
  const nm = (id: string) => s.name(id);
  return (
    <>
      <Sect>{t("lab.status.edgesSect")}</Sect>
      {st.edges.length === 0 && (
        <StatusCard>
          <div className="flex items-center gap-2 font-semibold">
            <StatusDot state="warn" />
            {t("lab.status.noEdge")}
          </div>
          <p className="mt-1 mb-0 text-[12.5px] text-muted">
            <Codey text={d.cfg["proxy"] ? t("lab.status.noEdgeBodyProxy") : t("lab.status.noEdgeBody")} />
          </p>
        </StatusCard>
      )}
      {st.edges.map((e) => (
        <StatusCard key={e.id + e.source}>
          <div className="flex flex-wrap items-center gap-2 font-semibold">
            <HugeiconsIcon
              icon={e.official ? CheckmarkCircle02Icon : Alert02Icon}
              size={15}
              strokeWidth={1.8}
              color="currentColor"
              className={e.official ? "text-ok" : "text-warn"}
              aria-hidden="true"
            />
            {nm(e.id)}
            <Tag>{e.source === "lan" ? t("lab.status.foundLan") : t("lab.status.configured")}</Tag>
            {st.path?.id === e.id && <Tag tone="acc">{t("lab.status.path")}</Tag>}
          </div>
          <p className="mt-1 mb-0 font-mono text-[12.5px] text-muted">{e.url}</p>
          {!e.official && <p className="mt-1 mb-0 text-[12.5px] text-muted">{t("lab.status.notOfficial")}</p>}
        </StatusCard>
      ))}
      <Sect>{t("lab.status.tunnelSect")}</Sect>
      <Kv>
        <Row k={t("lab.status.path")}>
          {st.path ? (
            <>
              {nm(st.path.id)} <Tag>{st.path.source === "lan" ? t("lab.status.localFirst") : t("lab.status.officialSecond")}</Tag>
            </>
          ) : (
            t("lab.status.none")
          )}
        </Row>
        <Row k={t("lab.status.tunnel")}>{tunnelTag(st)}</Row>
        <Row k={t("lab.status.publicName")} mono>
          {st.publicName ? "https://" + st.publicName : "—"}
        </Row>
        <Row k="Mesh">{st.mesh}</Row>
        <Row k="Market">{st.market === "available" ? <Tag tone="ok">available</Tag> : st.market}</Row>
      </Kv>
      <Button variant="secondary" size="sm" onClick={() => actions().scan(d.id)}>
        {t("lab.status.scan")}
      </Button>
    </>
  );
}

/* The settings a type has, grouped as the panes and the options name them.
 * Labels and help are chrome (i18n); a key the table does not know is shown
 * by its own name, so a new cfg key in the core never disappears. */
const CFG_LAYOUT: Record<string, [string, string[]][]> = {
  box: [
    ["losos.proxy", ["proxy", "tenantOnHub"]],
    ["lab.cfg.meshPane", ["joinMesh", "shareCompute"]],
  ],
  "edge-local": [["losos.edge", ["advertise", "openEnrolment", "cluster", "uplink", "zone"]]],
  "edge-official": [["losos.edge", ["certified", "acceptsRelay", "cluster", "domain"]]],
  router: [["", ["subnet"]]],
  ap: [["", ["ssid"]]],
};

function cfgText(t: ReturnType<typeof useT>, type: string, key: string): { label: string; help: string } {
  const lk = `lab.cfg.${type}.${key}` as MessageKey;
  const hk = `lab.cfg.${type}.${key}.help` as MessageKey;
  const has = (k: string) => k in MESSAGES;
  return { label: has(lk) ? t(lk) : key, help: has(hk) ? t(hk) : "" };
}

function ConfigPane({ s, d }: { s: LabStore; d: Device }) {
  const t = useT();
  const a = actions();
  const [name, setName] = React.useState(d.name);
  React.useEffect(() => setName(d.name), [d.name, d.id]);
  const layout = CFG_LAYOUT[d.type] ?? [];
  const known = new Set(layout.flatMap(([, keys]) => keys));
  const extra = Object.keys(d.cfg).filter((k) => !known.has(k));
  const groups: [string, string[]][] = [...layout, ...(extra.length ? ([["", extra]] as [string, string[]][]) : [])];

  const editor = (key: string) => {
    if (!(key in d.cfg)) return null;
    const v = d.cfg[key];
    const { label, help } = cfgText(t, d.type, key);
    const id = `cfg-${d.id}-${key}`;
    if (typeof v === "boolean") {
      return (
        <div key={key} className="my-2">
          <div className="flex items-center gap-2 text-[13px]">
            <label htmlFor={id} className="min-w-0 flex-1">
              <Codey text={label} />
            </label>
            <Switch id={id} size="sm" isSelected={v} onChange={(on) => a.setCfg(d.id, key, on)} aria-label={label} />
          </div>
          {help && (
            <p className="mt-0.5 mb-2 text-[12px] text-muted">
              <Codey text={help} />
            </p>
          )}
        </div>
      );
    }
    return (
      <div key={key + d.id + String(v)} className="my-2">
        <div className="flex items-center gap-2 text-[13px]">
          <label htmlFor={id} className="min-w-0 flex-1">
            <Codey text={label} />
          </label>
          <Input
            id={id}
            defaultValue={String(v)}
            spellCheck={false}
            className="h-8 min-w-0 flex-[1.3] font-mono text-[12.5px]"
            onBlur={(e) => {
              if (e.target.value.trim() !== v) a.setCfg(d.id, key, e.target.value.trim());
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            }}
          />
        </div>
        {help && (
          <p className="mt-0.5 mb-2 text-[12px] text-muted">
            <Codey text={help} />
          </p>
        )}
      </div>
    );
  };

  return (
    <div data-testid="pane-config">
      <div className="my-2 flex items-center gap-2 text-[13px]">
        <label htmlFor={`cfg-name-${d.id}`} className="min-w-0 flex-1">
          {t("lab.cfg.hostName")}
        </label>
        <Input
          id={`cfg-name-${d.id}`}
          value={name}
          spellCheck={false}
          className="h-8 min-w-0 flex-[1.3] font-mono text-[12.5px]"
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name !== d.name && !a.setName(d.id, name)) setName(d.name);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          }}
        />
      </div>
      <div className="my-2 flex items-center gap-2 text-[13px]">
        <label htmlFor={`cfg-site-${d.id}`} className="min-w-0 flex-1">
          {t("lab.cfg.location")}
        </label>
        <NativeSelect id={`cfg-site-${d.id}`} value={d.site} className="h-8" onChange={(e) => a.setSite(d.id, e.target.value)}>
          {Object.entries(s.catalog.sites).map(([k, v]) => (
            <NativeSelectOption key={k} value={k}>
              {v.name}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
      {groups.map(([title, keys], i) => (
        <React.Fragment key={i}>
          {title && <Sect>{title.startsWith("lab.") ? t(title as MessageKey) : title}</Sect>}
          {keys.map(editor)}
        </React.Fragment>
      ))}
      <Sect>{t("lab.cfg.device")}</Sect>
      <Button variant="secondary" size="sm" onClick={() => a.removeDevice(d.id)}>
        {t("lab.cfg.remove", { name: d.name })}
      </Button>
    </div>
  );
}

function PhysicalPane({ s, d }: { s: LabStore; d: Device }) {
  const t = useT();
  const hw = hwDrawing(d, (p) => s.scene.portState(d.id, p));
  const pad = 14;
  const T = s.catalog.types[d.type];
  const site = s.catalog.sites[d.site];
  return (
    <div data-testid="pane-physical">
      <svg
        className="lab-front block h-auto w-full rounded-card border border-line bg-sunk"
        viewBox={`${-pad} ${-pad} ${hw.w + pad * 2} ${hw.h + pad * 2}`}
        role="img"
        aria-label={t("lab.phys.front", { name: d.name })}
        onClick={(e) => {
          if ((e.target as Element).closest("[data-power]")) actions().togglePower(d.id);
        }}
      >
        {hw.node}
      </svg>
      <p className="mt-1.5 mb-2 text-[12px] text-muted">{t("lab.phys.help")}</p>
      <Sect>{t("lab.status.ports")}</Sect>
      <Kv>
        {(T?.ports ?? []).map(([port, kind]) => {
          const ls = s.snap.world.links.filter((l) => (l.a.dev === d.id && l.a.port === port) || (l.b.dev === d.id && l.b.port === port));
          return (
            <Row key={port} k={<span className="font-mono">{port}</span>}>
              {ls.length ? (
                ls.map((l) => {
                  const o = l.a.dev === d.id ? l.b : l.a;
                  const st = s.linkState(l);
                  return (
                    <div key={l.id}>
                      {s.name(o.dev)} {o.port} <Tag tone={st === "up" ? "ok" : "crit"}>{st}</Tag>
                    </div>
                  );
                })
              ) : (
                <span className="text-muted">{kind === "wifi-ap" ? t("lab.phys.noClients") : t("lab.phys.empty")}</span>
              )}
            </Row>
          );
        })}
      </Kv>
      <Kv>
        <Row k="MAC" mono>
          {d.mac}
        </Row>
        <Row k={t("lab.cfg.location")}>
          {site?.name}, {site?.sub}
        </Row>
      </Kv>
    </div>
  );
}

function ConsolePane({ s, d }: { s: LabStore; d: Device }) {
  const t = useT();
  const engine = useEngine();
  const host = React.useRef<HTMLDivElement>(null);
  const running = engine.isRunning(d.id);
  React.useEffect(() => {
    if (running && host.current) engine.attach(d.id, host.current);
  }, [running, d.id, engine, engine.version]);

  // A real guest when some backend can run this device; the console header
  // names which one (libvirt's label, or "QEMU in this tab").
  const backend = engine.available && engine.canBoot(d.id) ? engine.backendLabel(d.id) : "";
  if (backend) {
    return (
      <>
        <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-1.5 text-[12px] text-muted" data-testid="console-head">
          <StatusDot state={running ? "ok" : "idle"} />
          <b className="text-ink">{backend}</b>
          <span>
            {t("lab.console.serial")}
            {running ? "" : " · " + t("lab.console.notRunning")}
          </span>
        </div>
        {/* A host per device and per run: the guest's terminal is moved in
            by attach(), outside React, so a host kept across devices
            stacked one guest's console above the next one's. */}
        <div key={d.id + (running ? ":on" : ":off")} ref={host} className="lab-xterm min-h-[300px] flex-1 p-1">
          {!running && (
            <div className="p-3.5 font-mono text-[12.5px] leading-normal text-ink">
              {d.power ? t("lab.console.notYet") : t("lab.console.off")}
              <br />
              <br />
              {d.power && engine.full(d.id) ? (
                t("lab.console.full", { max: engine.max })
              ) : (
                <Button variant="secondary" size="sm" onClick={() => actions().bootGuest(d.id)}>
                  {t("lab.console.bootOn", { backend })}
                </Button>
              )}
            </div>
          )}
        </div>
      </>
    );
  }
  return (
    <>
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-1.5 text-[12px] text-muted">
        <StatusDot state="warn" />
        <b className="text-ink">{t("lab.console.simulated")}</b>
        <span>{t("lab.console.fromModel")}</span>
      </div>
      <SimTerminal key={d.id + ":" + s.consoleEpoch} devId={d.id} />
    </>
  );
}

