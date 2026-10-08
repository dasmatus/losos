/* What the Lab's controls do: each one a core call, then a refresh and the
 * UI state that follows. Components call these and never the core directly,
 * so the order of "change the world, re-read it, move the selection" lives
 * in one place. */

import { t } from "@/lib/i18n";
import { isErr, type HttpResult } from "./core";
import type { Engine } from "./engine";
import { download, MAX_FILE, readLast, writeLast } from "./files";
import { type LabStore, type Tab, type View } from "./store";

export class Actions {
  constructor(
    readonly s: LabStore,
    readonly engine: Engine,
  ) {}

  // ── setups ─────────────────────────────────────────────────────────────
  pick(key: string): void {
    const s = this.s;
    this.engine.stopAll();
    if (key === "last") {
      const text = readLast();
      if (text === null) return;
      const r = s.core.openLast(text);
      if (isErr(r)) {
        s.toast(r.error);
        s.extras = s.extras.filter((x) => x.key !== "last");
        s.startedWorld(null);
        return;
      }
      this.setExtra("last", r.name);
      s.startedWorld(r.focus ?? null);
      if (r.skipped) s.toast(t("lab.toast.openedSkipped", { name: r.name, count: r.skipped }));
      return;
    }
    if (key === "file") {
      if (s.openedText) this.importText(s.openedText.text, s.openedText.fileName, false);
      return;
    }
    const r = s.ok(s.core.loadScenario(key));
    if (r) s.startedWorld(r.focus);
  }

  loadThisBox(settings: string, edge: string): boolean {
    const s = this.s;
    const r = s.ok(s.core.loadThisBox(settings, edge));
    if (!r) return false;
    s.scenarios = s.core.catalog().scenarios;
    s.startedWorld(r.focus);
    return true;
  }

  private setExtra(key: "file" | "last", name: string): void {
    const s = this.s;
    s.extras = [...s.extras.filter((x) => x.key !== key), { key, name }];
    s.emit("model");
  }

  async save(): Promise<void> {
    const s = this.s;
    const doc = s.core.exportSetup();
    const r = await download(doc.filename, doc.text);
    if (r === "saved") s.toast(t("lab.toast.saved", { name: doc.name, file: doc.filename }));
    else if (r === "unsupported") s.toast(t("lab.toast.cannotSave"));
  }

  async open(file: File): Promise<void> {
    const s = this.s;
    if (file.size > MAX_FILE) {
      s.toast(t("lab.toast.tooBig"));
      return;
    }
    const text = await file.text();
    this.importText(text, file.name, true);
  }

  private importText(text: string, fileName: string, fresh: boolean): void {
    const s = this.s;
    this.engine.stopAll();
    const r = s.core.importSetup(text, fileName);
    if (isErr(r)) {
      s.toast(r.error);
      s.emit("model");
      return;
    }
    if (fresh) s.openedText = { text, fileName };
    this.setExtra("file", r.name);
    s.startedWorld(r.focus ?? null);
    s.toast(r.skipped ? t("lab.toast.openedSkipped", { name: r.name, count: r.skipped }) : t("lab.toast.opened", { name: r.name }));
  }

  /** keepLast(): every 2 s and on pagehide. */
  keepLast(): void {
    const text = this.s.core.lastSetup();
    if (typeof text === "string" && text.length > 0) writeLast(text);
  }

  // ── view and tools ─────────────────────────────────────────────────────
  setView(view: View): void {
    const s = this.s;
    s.setUi({ view });
    if (view === "physical") s.placePhysical();
    s.fit();
  }

  setMode(mode: "realtime" | "simulation"): void {
    const s = this.s;
    if (!s.ok(s.core.setMode(mode))) return;
    s.refresh();
    s.toast(mode === "simulation" ? t("lab.toast.simulation") : t("lab.toast.realtime"));
  }

  cancelTool(): void {
    this.s.setUi({ tool: "select", placeType: null, linkKind: null, connectFrom: null, connectPort: "" });
  }

  choosePlace(type: string): void {
    this.s.setUi({ tool: "place", placeType: type, linkKind: null, connectFrom: null, connectPort: "" });
  }

  chooseLink(kind: string): void {
    this.s.setUi({ tool: "connect", linkKind: kind, placeType: null, connectFrom: null, connectPort: "" });
  }

  toggleDelete(): void {
    const s = this.s;
    s.setUi({ tool: s.ui.tool === "delete" ? "select" : "delete", placeType: null, linkKind: null, connectFrom: null });
  }

  select(sel: LabStore["ui"]["sel"]): void {
    const s = this.s;
    let tab: Tab = s.ui.tab;
    if (sel?.kind === "dev" && (s.ui.sel?.kind !== "dev" || s.ui.sel.id !== sel.id)) {
      const d = s.dev(sel.id);
      if (d && (tab === "desktop" ? d.type !== "laptop" : tab === "console" ? !s.consoleable(d) : false)) tab = "status";
    }
    s.setUi({ sel, tab });
  }

  // ── editing ────────────────────────────────────────────────────────────
  place(x: number, y: number, opts: { site?: string; px?: number; py?: number }, keepTool: boolean): void {
    const s = this.s;
    const type = s.ui.placeType;
    if (!type) return;
    const r = s.ok(s.core.addDevice(type, Math.round(x), Math.round(y), opts));
    if (!r) return;
    s.refresh();
    s.placePhysical();
    s.setUi({
      sel: { kind: "dev", id: r.device.id },
      tab: r.device.type === "laptop" ? "desktop" : "config",
      ...(keepTool ? {} : { tool: "select", placeType: null }),
    });
  }

  /** A device clicked with the connect tool. `port`: the port the canvas
   *  saw clicked (the physical view); otherwise the picker's, or the core's. */
  connectTo(id: string, keepTool: boolean, port?: string): void {
    const s = this.s;
    const from = s.ui.connectFrom;
    if (!from) {
      s.setUi({ connectFrom: id, connectPort: port && s.scene.freePorts(id).includes(port) ? port : "" });
      return;
    }
    const kind = s.ui.linkKind ?? "auto";
    const r = s.core.connect(from, id, kind, s.ui.connectPort || undefined, port && s.scene.freePorts(id).includes(port) ? port : undefined);
    if (isErr(r)) s.toast(r.error);
    else {
      s.linkBorn.set(r.link.id, performance.now());
      window.setTimeout(() => s.emit("model"), 1500);
      s.toast(t("lab.toast.connected", { a: s.name(r.link.a.dev), ap: r.link.a.port, b: s.name(r.link.b.dev), bp: r.link.b.port }));
    }
    s.refresh();
    s.setUi({ connectFrom: null, connectPort: "", ...(keepTool ? {} : { tool: "select", linkKind: null }) });
  }

  removeDevice(id: string): void {
    const s = this.s;
    this.engine.stop(id);
    if (!s.ok(s.core.removeDevice(id))) return;
    s.refresh();
  }

  removeLink(id: string): void {
    const s = this.s;
    if (!s.ok(s.core.removeLink(id))) return;
    s.refresh();
  }

  removeSelection(): void {
    const sel = this.s.ui.sel;
    if (!sel) return;
    if (sel.kind === "dev") this.removeDevice(sel.id);
    else this.removeLink(sel.id);
    this.s.setUi({ sel: null });
  }

  togglePower(id: string): void {
    const s = this.s;
    const d = s.dev(id);
    if (!d) return;
    const on = !d.power;
    const r = s.ok(s.core.setPower(id, on));
    if (!r) return;
    if (this.engine.available && s.catalog.types[d.type]?.emulate) {
      if (on) void this.engine.start(id);
      else this.engine.stop(id);
    }
    s.consolePower.get(id)?.(on, r.banner);
    s.refresh();
    s.toast(on ? t("lab.toast.poweredOn", { name: d.name }) : t("lab.toast.poweredOff", { name: d.name }));
  }

  setCfg(id: string, key: string, value: boolean | string): void {
    const s = this.s;
    const d = s.dev(id);
    if (!s.ok(s.core.setCfg(id, key, value))) {
      s.emit("model");
      return;
    }
    s.refresh();
    if (d && typeof value === "boolean") s.toast(`${d.name}: ${key} ${value ? "on" : "off"}.`);
  }

  setName(id: string, name: string): boolean {
    const s = this.s;
    const r = s.ok(s.core.setName(id, name));
    s.refresh();
    return r !== null;
  }

  setSite(id: string, site: string): void {
    const s = this.s;
    if (!s.ok(s.core.setSite(id, site))) return;
    s.refresh();
    s.placePhysical();
  }

  scan(id: string): void {
    const s = this.s;
    s.ok(s.core.scan(id) as object);
    s.toast(t("lab.toast.scanning"));
  }

  // ── traffic ────────────────────────────────────────────────────────────
  play(on: boolean): void {
    this.s.core.setPlaying(on);
    this.s.refresh();
  }

  step(): void {
    const s = this.s;
    const r = s.core.step();
    s.absorb(r, this.httpDone);
  }

  clear(): void {
    this.s.core.clear();
    this.s.events = [];
    this.s.emit("events");
    this.s.refresh();
  }

  setClockSpeed(speed: number): void {
    const s = this.s;
    if (!s.ok(s.core.setClockSpeed(speed))) return;
    s.refresh();
  }

  skip(): void {
    const s = this.s;
    s.ok(s.core.skipToNextTimer());
    s.refresh();
  }

  setFilter(proto: string, on: boolean): void {
    this.s.core.setFilter(proto, on);
    this.s.refresh();
    this.s.emit("events");
  }

  setSimSpeed(speed: number): void {
    this.s.core.setSimSpeed(speed);
    this.s.refresh();
  }

  // ── the laptop's browser ───────────────────────────────────────────────
  navigate(id: string, raw: string, reload = false): void {
    const s = this.s;
    let url = raw.trim();
    if (!url) return;
    if (!/^\w+:\/\//.test(url)) url = "http://" + url;
    const st = s.desk(id);
    st.url = url;
    st.loading = true;
    st.job = null;
    if (!reload) st.history.push(url);
    if (!s.snap.net.iface[id]) {
      st.loading = false;
      st.html = s.core.errorPage("offline", "");
      s.emit("model");
      return;
    }
    const r = s.core.http(id, url);
    if (isErr(r)) {
      st.loading = false;
      st.html = s.core.errorPage("badurl", "");
    } else if (!r.busy && r.result) this.showPage(id, r.result);
    else st.job = r.job;
    s.emit("model");
  }

  httpDone = (dev: string, job: number, _url: string, result: HttpResult): void => {
    const st = this.s.desks.get(dev);
    if (!st || st.job !== job) return;
    this.showPage(dev, result);
    this.s.emit("model");
  };

  private showPage(id: string, result: HttpResult): void {
    const st = this.s.desk(id);
    st.loading = false;
    st.job = null;
    st.html = "html" in result ? result.html : this.s.core.errorPage(result.error, result.host ?? "");
  }

  back(id: string): void {
    const st = this.s.desk(id);
    st.history.pop();
    const prev = st.history.pop();
    if (prev) this.navigate(id, prev);
  }

  // ── the engine ─────────────────────────────────────────────────────────
  bootGuest(id: string): void {
    const d = this.s.dev(id);
    if (!d) return;
    if (!d.power) this.togglePower(id);
    else void this.engine.start(id);
  }
}

let current: Actions | null = null;
export function setActions(a: Actions): void {
  current = a;
}
export function actions(): Actions {
  if (current === null) throw new Error("the Lab is not loaded yet");
  return current;
}
