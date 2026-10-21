/* One browser's side of a call: the signalling socket to the box's SFU, the
 * peer connection, the e2ee worker, and the key schedule.
 *
 * Nothing here knows Matrix. Who this browser is comes in as an OpenID
 * token and a device id, and the keys go out through a KeyChannel, which
 * the chat page implements over Olm to-device messages (matrix-keys.ts).
 * The protocol with the SFU is in backend/src/calls/wire.rs.
 */

import { fromBase64, randomKey, SendKey, toBase64 } from './e2ee';
import type { TransformOptions, WorkerIn, WorkerOut, WorkerStats } from './e2ee-protocol';
import { GuestKeys, parseSecret, type RelayBody } from './guests';

export interface OpenIdToken {
  access_token: string;
  matrix_server_name: string;
}

export interface Member {
  pid: number;
  user: string;
  device: string;
  /* A guest's name; absent for the room's own people. */
  guest?: string;
}

export interface Sender {
  user: string;
  device?: string;
}

/* A sender key, as one member hands it to another. */
export interface KeyMessage {
  call: string;
  pid: number;
  index: number;
  key: string;
  /* Invite secrets for the call's guests (guests.ts), member to member
   * over Olm only. */
  guests?: string[];
}

export interface KeyChannel {
  /* Send `msg` to each of these devices, encrypted to that device only. */
  send(to: { user: string; device: string }[], msg: KeyMessage): Promise<void>;
  /* Keys others sent this device. `from` is the sender the transport
   * authenticated (Olm: the user and device whose session it came over),
   * never a claim inside the message. */
  subscribe(handler: (from: Sender, msg: KeyMessage) => void): () => void;
}

export type CallState = 'connecting' | 'connected' | 'ended' | 'failed';

export interface CallEvents {
  state(state: CallState, reason?: string): void;
  /* `live` is the member streaming the call in podcast mode, if one is. */
  members(members: Member[], you: number, live: number | null): void;
  /* A member's camera and microphone, or with `screen` their screen. */
  track(pid: number, stream: MediaStream, screen: boolean): void;
  stats(stats: WorkerStats): void;
}

export interface CallOptions {
  /* `wss://<box>/calls/api/ws`, or a guest's `/calls/guest/ws`. */
  socket: string;
  call: string;
  /* A member's OpenID token and device; a guest has neither. */
  openid?: OpenIdToken;
  device?: string;
  /* A guest's side: the invite from the link, the name it gave and the
   * secret from the link's fragment. */
  guest?: { invite: string; name: string; secret: string };
  local: MediaStream;
  /* Olm, for members; a guest has none. */
  keys?: KeyChannel;
  events: CallEvents;
  /* The e2ee worker; the page's own build by default. */
  worker?: () => Worker;
}

/* A fresh key is announced, then used this much later, so the members
 * have it before the first frame sealed with it arrives. */
const ROTATE_AFTER_MS = 1500;
/* Every sender ratchets this often, so a key taken from a running call
 * opens no more than the last few minutes. */
export const RATCHET_EVERY_MS = 5 * 60_000;

const hasScriptTransform = typeof (globalThis as { RTCRtpScriptTransform?: unknown }).RTCRtpScriptTransform === 'function';
const hasEncodedStreams =
  typeof RTCRtpSender !== 'undefined' && 'createEncodedStreams' in RTCRtpSender.prototype;

/* Whether this browser can encrypt call media. Without it there is no call:
 * media is never sent in the clear. */
export function canEncrypt(): boolean {
  return hasScriptTransform || hasEncodedStreams;
}

/* `p<pid>` is a member's camera and microphone, `p<pid>-screen` their
 * screen (backend/src/calls/wire.rs). */
export const streamPid = (id: string): { pid: number; screen: boolean } | null => {
  const m = /^p(\d+)(-screen)?$/.exec(id);
  return m ? { pid: Number(m[1]), screen: m[2] !== undefined } : null;
};

type Encodable = Omit<RTCRtpSender | RTCRtpReceiver, 'transform'> & {
  transform?: unknown;
  createEncodedStreams?: () => { readable: ReadableStream; writable: WritableStream };
};

export class CallSession {
  private ws: WebSocket | null = null;
  private pc: RTCPeerConnection;
  private worker: Worker;
  private statsPort: MessagePort;
  private send = new SendKey();
  private you = -1;
  private members: Member[] = [];
  private queue: Promise<void> = Promise.resolve();
  private early = new Map<number, { from: Sender; msg: KeyMessage }[]>();
  private timers: ReturnType<typeof setInterval>[] = [];
  private unsubscribe: () => void;
  private videoSender: RTCRtpSender | null = null;
  private screenSender: RTCRtpSender | null = null;
  private screen = new MediaStream();
  private done = false;
  private guests: GuestKeys;
  private keys: KeyChannel;
  /* A rotated key announced but not yet in use. */
  private next: SendKey | null = null;
  private earlyRelay = new Map<number, RelayBody[]>();

  constructor(private opts: CallOptions) {
    if (!canEncrypt()) throw new Error('noEncryption');
    this.worker =
      opts.worker?.() ?? new Worker(new URL('./e2ee-worker.ts', import.meta.url), { type: 'module' });
    // Held on the session: a port nothing references can be collected,
    // and its messages with it.
    const back = new MessageChannel();
    this.statsPort = back.port1;
    this.statsPort.onmessage = (ev: MessageEvent<WorkerOut>) => {
      if (ev.data.type === 'stats') this.opts.events.stats(ev.data.stats);
    };
    this.post({ type: 'port', port: back.port2 }, [back.port2]);
    this.post({ type: 'sendKey', key: this.send.key, index: this.send.index });
    const config: RTCConfiguration & { encodedInsertableStreams?: boolean } = {
      bundlePolicy: 'max-bundle',
      rtcpMuxPolicy: 'require',
      iceServers: [],
    };
    if (!hasScriptTransform) config.encodedInsertableStreams = true;
    this.pc = new RTCPeerConnection(config);
    this.pc.ontrack = (ev) => this.onTrack(ev);
    this.pc.onconnectionstatechange = () => {
      const s = this.pc.connectionState;
      if (s === 'connected') this.opts.events.state('connected');
      if (s === 'failed') this.fail('media');
    };
    this.keys = opts.keys ?? { send: async () => undefined, subscribe: () => () => undefined };
    this.guests = new GuestKeys(opts.call, (to, body) => this.signal({ type: 'relay', to, body }));
    if (opts.guest) this.guests.addSecret(opts.guest.secret);
    this.unsubscribe = this.keys.subscribe((from, msg) => this.onKey(from, msg));
    this.timers.push(setInterval(() => this.serial(() => this.ratchet()), RATCHET_EVERY_MS));
  }

  private post(m: WorkerIn, transfer: Transferable[] = []): void {
    this.worker.postMessage(m, transfer);
  }

  private attach(t: Encodable, options: TransformOptions): void {
    const Script = (globalThis as { RTCRtpScriptTransform?: new (w: Worker, o: unknown) => unknown })
      .RTCRtpScriptTransform;
    if (Script) {
      t.transform = new Script(this.worker, options);
    } else if (t.createEncodedStreams) {
      const { readable, writable } = t.createEncodedStreams();
      this.post({ type: 'stream', readable, writable, options }, [readable, writable]);
    }
  }

  start(): void {
    this.opts.events.state('connecting');
    const ws = new WebSocket(this.opts.socket);
    this.ws = ws;
    const g = this.opts.guest;
    ws.onopen = () =>
      this.signal(
        g
          ? { type: 'guest', call: this.opts.call, invite: g.invite, name: g.name }
          : { type: 'join', call: this.opts.call, openid: this.opts.openid, device: this.opts.device },
      );
    ws.onmessage = (ev) => {
      let m: { type: string; [k: string]: unknown };
      try {
        m = JSON.parse(String(ev.data));
      } catch {
        return;
      }
      void this.onSignal(m);
    };
    ws.onclose = () => {
      if (!this.done) this.fail('signalling');
    };
  }

  private signal(m: object): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(m));
  }

  private async onSignal(m: { type: string; [k: string]: unknown }): Promise<void> {
    switch (m.type) {
      case 'joined': {
        this.you = m.you as number;
        await this.offer();
        this.onMembers(m.members as Member[], null);
        break;
      }
      case 'members':
        this.onMembers(m.members as Member[], typeof m.live === 'number' ? m.live : null);
        break;
      case 'answer':
        await this.pc.setRemoteDescription({ type: 'answer', sdp: m.sdp as string });
        break;
      case 'offer': {
        await this.pc.setRemoteDescription({ type: 'offer', sdp: m.sdp as string });
        const answer = await this.pc.createAnswer();
        await this.pc.setLocalDescription(answer);
        this.signal({ type: 'answer', sdp: answer.sdp });
        break;
      }
      case 'relay':
        if (typeof m.from === 'number' && typeof m.body === 'object' && m.body !== null) {
          await this.onRelay(m.from, m.body as RelayBody);
        }
        break;
      case 'error':
        this.fail(String(m.code));
        break;
    }
  }

  /* Whether keys to and from `m` go through a guest pair, not Olm. */
  private pairwise(m: Member): boolean {
    return this.opts.guest !== undefined || m.guest !== undefined;
  }

  private async onRelay(from: number, body: RelayBody): Promise<void> {
    const member = this.members.find((m) => m.pid === from);
    if (!member) {
      // A hello can overtake the member list, like a key.
      const list = this.earlyRelay.get(from) ?? [];
      if (list.length < 4) list.push(body);
      this.earlyRelay.set(from, list);
      return;
    }
    if (!this.pairwise(member)) return;
    if (body.t === 'hello') {
      if (await this.guests.onHello(this.you, from, body)) this.serial(() => this.sendPairKey(from));
    } else if (body.t === 'key') {
      const msg = (await this.guests.open(this.you, from, body)) as KeyMessage | null;
      if (msg === null || typeof msg !== 'object' || msg.pid !== from) return;
      this.onKey({ user: member.user, device: member.device }, { ...msg, guests: undefined });
    }
  }

  /* The key in use, and one announced, to a peer whose pair key is new. */
  private async sendPairKey(pid: number): Promise<void> {
    for (const k of this.next ? [this.send, this.next] : [this.send]) {
      const sealed = await this.guests.seal(this.you, pid, {
        call: this.opts.call,
        pid: this.you,
        index: k.index,
        key: toBase64(k.key),
      });
      if (sealed) this.signal({ type: 'relay', to: pid, body: sealed });
    }
  }

  private greetAll(): void {
    for (const m of this.others()) if (this.pairwise(m)) void this.guests.greet(this.you, m.pid);
  }

  /* An invite secret this page made or was handed: kept, sent to the other
   * members over Olm, and used to greet the guests already here. */
  async addGuestSecret(secret: string): Promise<void> {
    if (this.opts.guest || !this.guests.addSecret(secret)) return;
    this.greetAll();
    await new Promise<void>((done) =>
      this.serial(async () => {
        await this.sendKey(this.others().filter((m) => !this.pairwise(m)));
        done();
      }),
    );
  }

  /* The secret new invite links reuse, so a call has one. */
  get guestSecret(): string | null {
    return this.guests.knownSecrets()[0] ?? null;
  }

  /* The one offer this page makes: send-only audio, camera and screen, in
   * that order (the SFU takes the second video m-line for the screen).
   * Muting toggles the track and sharing replaces the screen's; the
   * m-lines never change. The screen is sealed with the same video keys
   * and the same frame counter as the camera, in the one worker. */
  private async offer(): Promise<void> {
    const [audio] = this.opts.local.getAudioTracks();
    const [video] = this.opts.local.getVideoTracks();
    const a = this.pc.addTransceiver(audio ?? 'audio', { direction: 'sendonly', streams: [this.opts.local] });
    const v = this.pc.addTransceiver(video ?? 'video', { direction: 'sendonly', streams: [this.opts.local] });
    this.attach(a.sender as Encodable, { op: 'seal', kind: 'audio' });
    const s = this.pc.addTransceiver('video', { direction: 'sendonly', streams: [this.screen] });
    this.attach(v.sender as Encodable, { op: 'seal', kind: 'video' });
    this.attach(s.sender as Encodable, { op: 'seal', kind: 'video' });
    this.videoSender = v.sender;
    this.screenSender = s.sender;
    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);
    this.signal({ type: 'offer', sdp: offer.sdp });
  }

  private onTrack(ev: RTCTrackEvent): void {
    const stream = ev.streams[0];
    const from = stream ? streamPid(stream.id) : null;
    if (from === null || !stream) return;
    this.attach(ev.receiver as Encodable, {
      op: 'open',
      kind: ev.track.kind === 'video' ? 'video' : 'audio',
      sender: `p${from.pid}`,
    });
    this.opts.events.track(from.pid, stream, from.screen);
  }

  /* Key changes run one after another: a join's ratchet and a leave's
   * rotation must not interleave. */
  private serial(f: () => Promise<void>): void {
    this.queue = this.queue.then(f).catch(() => undefined);
  }

  private onMembers(next: Member[], live: number | null): void {
    const before = new Map(this.members.map((m) => [m.pid, m]));
    const after = new Map(next.map((m) => [m.pid, m]));
    this.members = next;
    this.opts.events.members(next, this.you, live);
    const joined = next.filter((m) => !before.has(m.pid) && m.pid !== this.you);
    const left = [...before.values()].filter((m) => !after.has(m.pid) && m.pid !== this.you);
    for (const m of left) {
      this.post({ type: 'dropSender', sender: `p${m.pid}` });
      this.guests.forget(m.pid);
    }
    for (const m of joined) {
      for (const k of this.early.get(m.pid) ?? []) this.onKey(k.from, k.msg);
      this.early.delete(m.pid);
      for (const b of this.earlyRelay.get(m.pid) ?? []) void this.onRelay(m.pid, b);
      this.earlyRelay.delete(m.pid);
    }
    this.greetAll();
    const firstList = before.size === 0;
    this.serial(async () => {
      if (left.length > 0) await this.rotate();
      // Someone new: ratchet first, so they get a key that opens nothing
      // sent before they came. When this page itself just joined, the
      // members already there get its first key as it is.
      if (joined.length > 0 && !firstList) await this.ratchet();
      if (joined.length > 0) await this.sendKey(joined);
    });
  }

  private others(): Member[] {
    return this.members.filter((m) => m.pid !== this.you);
  }

  /* `k` to these members: over Olm to the room's own people, with the
   * guests' invite secrets, and over its pair key to each guest that has
   * one. A guest without a pair key yet gets the key in use once it has. */
  private async sendKey(to: Member[], k: SendKey = this.send): Promise<void> {
    const olm = to.filter((m) => !this.pairwise(m));
    if (olm.length > 0) {
      const guests = this.guests.knownSecrets();
      await this.keys.send(
        olm.map((m) => ({ user: m.user, device: m.device })),
        {
          call: this.opts.call,
          pid: this.you,
          index: k.index,
          key: toBase64(k.key),
          ...(guests.length > 0 ? { guests } : {}),
        },
      );
    }
    for (const m of to.filter((m) => this.pairwise(m))) {
      const sealed = await this.guests.seal(this.you, m.pid, {
        call: this.opts.call,
        pid: this.you,
        index: k.index,
        key: toBase64(k.key),
      });
      if (sealed) this.signal({ type: 'relay', to: m.pid, body: sealed });
    }
  }

  private async ratchet(): Promise<void> {
    await this.send.ratchet();
    this.post({ type: 'sendKey', key: this.send.key, index: this.send.index });
  }

  /* A fresh key for the members who stayed, used once they have it. */
  private async rotate(): Promise<void> {
    const next = new SendKey(randomKey(), this.send.index + 1);
    this.next = next;
    await this.sendKey(this.others(), next);
    await new Promise((r) => setTimeout(r, ROTATE_AFTER_MS));
    this.send.key.fill(0);
    this.send = next;
    this.next = null;
    this.post({ type: 'sendKey', key: next.key, index: next.index });
  }

  private onKey(from: Sender, msg: KeyMessage): void {
    if (msg.call !== this.opts.call || typeof msg.pid !== 'number' || msg.pid === this.you) return;
    const member = this.members.find((m) => m.pid === msg.pid);
    if (!member) {
      // The key can overtake the SFU's member list; keep a few.
      const list = this.early.get(msg.pid) ?? [];
      if (list.length < 4) list.push({ from, msg });
      this.early.set(msg.pid, list);
      return;
    }
    // The SFU says who is behind a pid; Olm says who sent the key. A key
    // for someone else's pid, or from another of their devices, is ignored.
    if (member.user !== from.user) return;
    if (from.device !== undefined && from.device !== member.device) return;
    let key: Uint8Array;
    try {
      key = fromBase64(msg.key);
    } catch {
      return;
    }
    if (key.length !== 32 || !Number.isInteger(msg.index)) return;
    this.post({ type: 'recvKey', sender: `p${msg.pid}`, key, index: msg.index });
    // Secrets come only from the room's own people, over Olm.
    if (!this.pairwise(member) && Array.isArray(msg.guests) && !this.opts.guest) {
      let fresh = false;
      for (const s of msg.guests.slice(0, 8)) if (parseSecret(s) && this.guests.addSecret(s)) fresh = true;
      if (fresh) {
        this.greetAll();
        void this.guests.retry(this.you).then((opened) => {
          for (const pid of opened) this.serial(() => this.sendPairKey(pid));
        });
      }
    }
  }

  setMuted(kind: 'audio' | 'video', muted: boolean): void {
    for (const t of kind === 'audio' ? this.opts.local.getAudioTracks() : this.opts.local.getVideoTracks()) {
      t.enabled = !muted;
    }
  }

  /* Turn the camera on or off. Off sends nothing at all, so the others'
   * receiving track goes quiet and their tile shows the avatar. The m-line
   * stays, so no renegotiation is needed. */
  async setVideoTrack(track: MediaStreamTrack | null): Promise<void> {
    await this.videoSender?.replaceTrack(track);
  }

  /* This page's participant id, once the SFU said it. */
  get pid(): number {
    return this.you;
  }

  /* Share a screen, or stop sharing with null. */
  async setScreenTrack(track: MediaStreamTrack | null): Promise<void> {
    await this.screenSender?.replaceTrack(track);
  }

  private fail(reason: string): void {
    if (this.done) return;
    this.close();
    this.opts.events.state('failed', reason);
  }

  leave(): void {
    if (this.done) return;
    this.signal({ type: 'leave' });
    this.close();
    this.opts.events.state('ended');
  }

  private close(): void {
    this.done = true;
    for (const t of this.timers) clearInterval(t);
    this.unsubscribe();
    this.ws?.close();
    this.pc.close();
    this.post({ type: 'stop' });
    this.worker.terminate();
    this.statsPort.close();
    this.send.key.fill(0);
  }
}

