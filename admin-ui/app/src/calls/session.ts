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

export interface OpenIdToken {
  access_token: string;
  matrix_server_name: string;
}

export interface Member {
  pid: number;
  user: string;
  device: string;
}

/* A sender key, as one member hands it to another. */
export interface KeyMessage {
  call: string;
  pid: number;
  index: number;
  key: string;
}

export interface KeyChannel {
  /* Send `msg` to each of these devices, encrypted to that device only. */
  send(to: { user: string; device: string }[], msg: KeyMessage): Promise<void>;
  /* Keys others sent this device. `from` is the Matrix id the transport
   * authenticated, never a claim inside the message. */
  subscribe(handler: (from: string, msg: KeyMessage) => void): () => void;
}

export type CallState = 'connecting' | 'connected' | 'ended' | 'failed';

export interface CallEvents {
  state(state: CallState, reason?: string): void;
  members(members: Member[], you: number): void;
  track(pid: number, stream: MediaStream): void;
  stats(stats: WorkerStats): void;
}

export interface CallOptions {
  /* `wss://<box>/calls/api/ws`. */
  socket: string;
  call: string;
  openid: OpenIdToken;
  device: string;
  local: MediaStream;
  keys: KeyChannel;
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

export const streamPid = (id: string): number | null => {
  const m = /^p(\d+)$/.exec(id);
  return m ? Number(m[1]) : null;
};

type Encodable = (RTCRtpSender | RTCRtpReceiver) & {
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
  private early = new Map<number, { from: string; msg: KeyMessage }[]>();
  private timers: ReturnType<typeof setInterval>[] = [];
  private unsubscribe: () => void;
  private done = false;

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
    this.unsubscribe = this.opts.keys.subscribe((from, msg) => this.onKey(from, msg));
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
    ws.onopen = () =>
      this.signal({ type: 'join', call: this.opts.call, openid: this.opts.openid, device: this.opts.device });
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
        this.onMembers(m.members as Member[]);
        break;
      }
      case 'members':
        this.onMembers(m.members as Member[]);
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
      case 'error':
        this.fail(String(m.code));
        break;
    }
  }

  /* The one offer this page makes: send-only audio and video. Muting
   * toggles the track; the m-lines never change. */
  private async offer(): Promise<void> {
    const [audio] = this.opts.local.getAudioTracks();
    const [video] = this.opts.local.getVideoTracks();
    const a = this.pc.addTransceiver(audio ?? 'audio', { direction: 'sendonly', streams: [this.opts.local] });
    const v = this.pc.addTransceiver(video ?? 'video', { direction: 'sendonly', streams: [this.opts.local] });
    this.attach(a.sender as Encodable, { op: 'seal', kind: 'audio' });
    this.attach(v.sender as Encodable, { op: 'seal', kind: 'video' });
    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);
    this.signal({ type: 'offer', sdp: offer.sdp });
  }

  private onTrack(ev: RTCTrackEvent): void {
    const stream = ev.streams[0];
    const pid = stream ? streamPid(stream.id) : null;
    if (pid === null || !stream) return;
    this.attach(ev.receiver as Encodable, {
      op: 'open',
      kind: ev.track.kind === 'video' ? 'video' : 'audio',
      sender: `p${pid}`,
    });
    this.opts.events.track(pid, stream);
  }

  /* Key changes run one after another: a join's ratchet and a leave's
   * rotation must not interleave. */
  private serial(f: () => Promise<void>): void {
    this.queue = this.queue.then(f).catch(() => undefined);
  }

  private onMembers(next: Member[]): void {
    const before = new Map(this.members.map((m) => [m.pid, m]));
    const after = new Map(next.map((m) => [m.pid, m]));
    this.members = next;
    this.opts.events.members(next, this.you);
    const joined = next.filter((m) => !before.has(m.pid) && m.pid !== this.you);
    const left = [...before.values()].filter((m) => !after.has(m.pid) && m.pid !== this.you);
    for (const m of left) this.post({ type: 'dropSender', sender: `p${m.pid}` });
    for (const m of joined) {
      for (const k of this.early.get(m.pid) ?? []) this.onKey(k.from, k.msg);
      this.early.delete(m.pid);
    }
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

  private async sendKey(to: Member[]): Promise<void> {
    if (to.length === 0) return;
    await this.opts.keys.send(
      to.map((m) => ({ user: m.user, device: m.device })),
      { call: this.opts.call, pid: this.you, index: this.send.index, key: toBase64(this.send.key) },
    );
  }

  private async ratchet(): Promise<void> {
    await this.send.ratchet();
    this.post({ type: 'sendKey', key: this.send.key, index: this.send.index });
  }

  /* A fresh key for the members who stayed, used once they have it. */
  private async rotate(): Promise<void> {
    const next = new SendKey(randomKey(), this.send.index + 1);
    await this.opts.keys.send(
      this.others().map((m) => ({ user: m.user, device: m.device })),
      { call: this.opts.call, pid: this.you, index: next.index, key: toBase64(next.key) },
    );
    await new Promise((r) => setTimeout(r, ROTATE_AFTER_MS));
    this.send.key.fill(0);
    this.send = next;
    this.post({ type: 'sendKey', key: next.key, index: next.index });
  }

  private onKey(from: string, msg: KeyMessage): void {
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
    // for someone else's pid is ignored.
    if (member.user !== from) return;
    let key: Uint8Array;
    try {
      key = fromBase64(msg.key);
    } catch {
      return;
    }
    if (key.length !== 32 || !Number.isInteger(msg.index)) return;
    this.post({ type: 'recvKey', sender: `p${msg.pid}`, key, index: msg.index });
  }

  setMuted(kind: 'audio' | 'video', muted: boolean): void {
    for (const t of kind === 'audio' ? this.opts.local.getAudioTracks() : this.opts.local.getVideoTracks()) {
      t.enabled = !muted;
    }
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

