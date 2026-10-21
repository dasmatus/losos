/* What the Lab's controls do: each one a core call, then a refresh and the
 * UI state that follows. Components call these and never the core directly,
 * so the order of "change the world, re-read it, move the selection" lives
 * in one place. */

import { ACCENTS, accentFor, nameOf, type Accent } from "@/cloud/collab/people";
import { t } from "@/lib/i18n";
import { isErr, type HttpResult } from "./core";
import type { Engine } from "./engine";
import { CloudFail, readFile, roomInfo, writeFile } from "./canvas/cloud";
import { LiveDiagram, live, type Room, setLive } from "./canvas/shared";
import type { PresenceUser } from "@/cloud/collab/cursors";
import { exportPng, exportSvg } from "./canvas/export";
import { CanvasOps } from "./canvas/ops";
import { download, MAX_FILE, readLast, writeLast } from "./files";
import { type LabStore, type Tab, type Workspace } from "./store";

export class Actions {
  /** The diagram's own operations (canvas/ops.ts). */
  readonly canvas: CanvasOps;

  constructor(
    readonly s: LabStore,
    readonly engine: Engine,
  ) {
    this.canvas = new CanvasOps(s, engine);
  }

  // ── setups ─────────────────────────────────────────────────────────────
  pick(key: string): void {
    const s = this.s;
    this.leaveRoom();
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
    this.leaveRoom();
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
    if (this.importText(text, file.name, true)) this.s.cloudFile = null;
  }

  private importText(text: string, fileName: string, fresh: boolean, inRoom = false): boolean {
    const s = this.s;
    if (!inRoom) this.leaveRoom();
    this.engine.stopAll();
    const r = s.core.importSetup(text, fileName);
    if (isErr(r)) {
      s.toast(r.error);
      s.emit("model");
      return false;
    }
    if (fresh) s.openedText = { text, fileName };
    this.setExtra("file", r.name);
    s.startedWorld(r.focus ?? null);
    s.toast(r.skipped ? t("lab.toast.openedSkipped", { name: r.name, count: r.skipped }) : t("lab.toast.opened", { name: r.name }));
    return true;
  }

  // ── LosOS Cloud ────────────────────────────────────────────────────────
  private cloudProblem(e: unknown): string {
    if (e instanceof CloudFail) {
      if (e.code === "signIn") return t("dg.cloud.signIn");
      if (e.code === "notFound") return t("dg.cloud.notFound");
      if (e.code === "full") return t("dg.cloud.full");
    }
    return t("dg.cloud.failed");
  }

  async openCloud(path: string): Promise<boolean> {
    const s = this.s;
    try {
      const f = await readFile(path);
      if (f.text.length > MAX_FILE) {
        s.toast(t("lab.toast.tooBig"));
        return false;
      }
      const name = path.slice(path.lastIndexOf("/") + 1);
      if (!this.importText(f.text, name, true)) return false;
      s.cloudFile = { path, etag: f.etag, ...(f.id === undefined ? {} : { id: f.id }) };
      s.setUi({ view: "canvas" });
      s.fit();
      if (f.id !== undefined) void this.joinRoom(f.id);
      return true;
    } catch (e) {
      s.toast(this.cloudProblem(e));
      return false;
    }
  }

  /** Saves the diagram at `path` in LosOS Cloud. `etag`: overwrite that
   *  version (a replace the person agreed to); by default the version this
   *  tab read, or a new file. "changed": someone else saved it meanwhile. */
  async saveCloud(path: string, etag?: string): Promise<"saved" | "changed" | "failed"> {
    const s = this.s;
    const doc = s.core.exportSetup();
    const guard = etag ?? (s.cloudFile?.path === path ? s.cloudFile.etag || "*" : null);
    try {
      const e = await writeFile(path, doc.text, guard);
      s.cloudFile = { path, etag: e.etag, id: e.id };
      s.emit("model");
      s.toast(t("dg.cloud.saved", { name: doc.name, path }));
      if (live()?.file !== e.id) void this.joinRoom(e.id);
      return "saved";
    } catch (e) {
      if (e instanceof CloudFail && (e.status === 412 || e.code === "exists")) return "changed";
      s.toast(this.cloudProblem(e));
      return "failed";
    }
  }

  /** Ctrl+S: back to LosOS Cloud when the diagram came from there, else a download. */
  async saveQuick(): Promise<void> {
    const s = this.s;
    if (live() && s.live?.state !== "offline") {
      // The room writes every change to the file as it goes.
      const where = s.live?.from ? t("dg.live.on", { box: s.live.from.box }) : (s.cloudFile?.path ?? "");
      s.toast(s.live?.readOnly ? t("dg.live.viewOnly") : t("dg.live.saved", { path: where }));
      return;
    }
    if (!s.cloudFile) {
      await this.save();
      return;
    }
    const r = await this.saveCloud(s.cloudFile.path);
    if (r === "changed") s.toast(t("dg.cloud.changedElsewhere"));
  }

  /** Opens the file's live-editing room, where this box has one: from
   *  then on everyone with the file open edits the same diagram. */
  private async joinRoom(id: number): Promise<void> {
    const s = this.s;
    this.leaveRoom();
    const info = await roomInfo(id);
    if (!info || info.perm === "none" || s.cloudFile?.id !== id) return;
    const me = await this.presence(info);
    if (s.cloudFile?.id !== id || live()) return;
    this.startRoom({ file: id, readOnly: info.perm === "read", from: null }, me);
  }

  /** A diagram someone on another box shared with the person here. */
  async openShared(share: string, id: number): Promise<boolean> {
    const s = this.s;
    const info = await roomInfo(id, share);
    if (!info || info.perm === "none") {
      s.toast(t("dg.cloud.notFound"));
      return false;
    }
    const me = await this.presence(info);
    this.leaveRoom();
    const name = info.name;
    this.startRoom(
      {
        file: id,
        share,
        readOnly: info.perm === "read",
        from: { owner: info.owner, box: info.box },
        opening: (text) => {
          if (!this.importText(text, name, true, true)) return false;
          s.cloudFile = null;
          s.setUi({ view: "canvas" });
          s.fit();
          return true;
        },
      },
      me,
    );
    return true;
  }

  private async presence(info: { me: { name: string; address: string } }): Promise<PresenceUser> {
    const login = nameOf(info.me.address);
    let accent = accentFor(login);
    let picture: string | null = null;
    try {
      // The colour and picture of their profile; the colour of their name until then.
      const r = await fetch(`/api/profiles/${encodeURIComponent(login)}`, { credentials: "same-origin", cache: "no-store" });
      if (r.ok) {
        const p = (await r.json()) as { accent?: Accent; picture?: string | null };
        if (p.accent && (ACCENTS as readonly string[]).includes(p.accent)) accent = p.accent;
        picture = typeof p.picture === "string" ? p.picture : null;
      }
    } catch {
      /* the colour of their name */
    }
    return { name: info.me.name, address: info.me.address, accent, picture };
  }

  private startRoom(room: Room, me: PresenceUser): void {
    const s = this.s;
    setLive(
      new LiveDiagram(s, room, me, () => {
        this.engine.stopAll();
        s.desks.clear();
        s.consoleEpoch += 1;
        s.linkBorn.clear();
        s.events = [];
        s.ui.sel = null;
        s.emit("events");
      }),
    );
    s.emit("model");
  }

  leaveRoom(): void {
    setLive(null);
  }

  // ── pictures ───────────────────────────────────────────────────────────
  async exportPicture(kind: "svg" | "png"): Promise<void> {
    const s = this.s;
    const base = s.core.exportSetup().filename.replace(/\.canvas$/, "");
    if (s.canvas.nodes.length === 0) {
      s.toast(t("dg.toast.nothingToExport"));
      return;
    }
    const svg = exportSvg(s);
    const blob = kind === "svg" ? new Blob([svg], { type: "image/svg+xml" }) : await exportPng(svg);
    if (!blob) {
      s.toast(t("dg.toast.pngFailed"));
      return;
    }
    const r = await download(`${base}.${kind}`, blob);
    if (r === "unsupported") s.toast(t("lab.toast.cannotSave"));
  }

  /** keepLast(): every 2 s and on pagehide. */
  keepLast(): void {
    const text = this.s.core.lastSetup();
    if (typeof text === "string" && text.length > 0) writeLast(text);
  }

  // ── view and tools ─────────────────────────────────────────────────────
  setView(view: Workspace): void {
    const s = this.s;
    s.setUi({ view, cardTool: null, editing: null });
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
