/* The page tests/calls.browser.mjs opens in each browser tab: one call
 * member on src/calls/session.ts, with the keys passed between tabs over a
 * BroadcastChannel in place of Olm. A tab can be told to drop the keys sent
 * to it, which is what an SFU (or anyone between) is left with. */

import { CallSession, type KeyMessage, type Member } from '../../../src/calls/session';
import { Podcast } from '../../../src/calls/podcast';
import type { WorkerStats } from '../../../src/calls/e2ee-protocol';

interface Joined {
  user: string;
  device: string;
}

const w = window as unknown as Record<string, unknown>;
const state: {
  members: Member[];
  you: number;
  live: number | null;
  stats: WorkerStats | null;
  states: string[];
  keysIn: number;
  podcast: string[];
} = { members: [], you: -1, live: null, stats: null, states: [], keysIn: 0, podcast: [] };
w.callState = state;

w.startCall = async (opts: {
  socket: string;
  call: string;
  me?: Joined;
  dropKeys?: boolean;
  video?: boolean;
  /* Join as a guest, with no key channel of its own. */
  guest?: { invite: string; name: string; secret: string };
}) => {
  const local = await navigator.mediaDevices.getUserMedia({ audio: true, video: opts.video !== false });
  const bus = new BroadcastChannel('losos-call-keys');
  const me = opts.me ?? { user: '', device: '' };
  const session = new CallSession({
    socket: opts.socket,
    call: opts.call,
    ...(opts.guest
      ? { guest: opts.guest }
      : {
          device: me.device,
          keys: {
            async send(to, msg) {
              bus.postMessage({ from: me.user, to, msg });
            },
            subscribe(handler) {
              const on = (ev: MessageEvent<{ from: string; to: Joined[]; msg: KeyMessage }>) => {
                const { from, to, msg } = ev.data;
                if (!to.some((t) => t.user === me.user && t.device === me.device)) return;
                if (opts.dropKeys) return;
                state.keysIn++;
                handler({ user: from }, msg);
              };
              bus.addEventListener('message', on);
              return () => bus.removeEventListener('message', on);
            },
          },
        }),
    local,
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: {
      state: (s, reason) => state.states.push(reason ? `${s}:${reason}` : s),
      members: (m, you, live) => {
        state.members = m;
        state.you = you;
        state.live = live;
      },
      track: (pid, stream, screen) => {
        const id = screen ? `p${pid}-screen` : `p${pid}`;
        streams.set(id, stream);
        let v = document.getElementById(id) as HTMLVideoElement | null;
        if (!v) {
          v = document.createElement('video');
          v.id = id;
          v.autoplay = true;
          v.muted = true;
          v.playsInline = true;
          document.body.append(v);
        }
        v.srcObject = stream;
      },
      stats: (s) => (state.stats = { ...s }),
    },
  });
  session.start();
  w.session = session;
  w.local = local;
  w.callOpts = opts;
  const mine = document.createElement('video');
  mine.id = 'local';
  mine.autoplay = true;
  mine.muted = true;
  mine.srcObject = local;
  document.body.append(mine);
};

const streams = new Map<string, MediaStream>();

/* A screen to share: a canvas in one flat colour with a white bar that
 * moves, so the encoder keeps sending frames. */
w.share = async (colour: string) => {
  const c = document.createElement('canvas');
  c.width = 640;
  c.height = 360;
  const g = c.getContext('2d');
  if (!g) throw new Error('no canvas');
  let x = 0;
  const paint = (): void => {
    g.fillStyle = colour;
    g.fillRect(0, 0, 640, 360);
    g.fillStyle = '#fff';
    g.fillRect(x, 0, 8, 360);
    x = (x + 8) % 640;
  };
  paint();
  setInterval(paint, 50);
  const track = c.captureStream(20).getVideoTracks()[0];
  const mine = document.createElement('video');
  mine.id = 'local-screen';
  mine.autoplay = true;
  mine.muted = true;
  mine.srcObject = new MediaStream(track ? [track] : []);
  document.body.append(mine);
  await (w.session as CallSession).setScreenTrack(track ?? null);
};

/* Podcast mode from this tab, to `rtmp`, with every member it receives in
 * the grid. */
/* Hand this member an invite secret, as the invite button does. */
w.addGuestSecret = (secret: string) => (w.session as CallSession).addGuestSecret(secret);

w.goLive = (rtmp: string, title: string) => {
  const session = w.session as CallSession;
  const opts = w.callOpts as { socket: string; call: string };
  const podcast = new Podcast(
    () =>
      state.members.map((m) => {
        const stream = m.pid === state.you ? (w.local as MediaStream) : streams.get(`p${m.pid}`);
        return { pid: m.pid, name: m.user, ...(stream ? { stream } : {}) };
      }),
    () => ({ title, subtitle: 'LosOS calls', overlay: null, screen: null }),
    {
      live: () => state.podcast.push('live'),
      ended: (error) => state.podcast.push(error ? `ended:${error}` : 'ended'),
    },
  );
  w.podcast = podcast;
  podcast.start(opts.socket.replace(/\/ws$/, '/stream'), {
    call: opts.call,
    pid: session.pid,
    rtmp,
  });
};

/* A remote (or the local, 'local') picture as 16x12 grey levels, for the
 * test to compare what one tab sends with what another shows. */
w.picture = (id: string): number[] | null => {
  const v = document.getElementById(id) as HTMLVideoElement | null;
  if (!v || v.videoWidth === 0) return null;
  const c = document.createElement('canvas');
  c.width = 16;
  c.height = 12;
  const g = c.getContext('2d', { willReadFrequently: true });
  if (!g) return null;
  g.drawImage(v, 0, 0, 16, 12);
  const d = g.getImageData(0, 0, 16, 12).data;
  const out: number[] = [];
  for (let i = 0; i < d.length; i += 4) out.push(Math.round(((d[i] ?? 0) + (d[i + 1] ?? 0) + (d[i + 2] ?? 0)) / 3));
  return out;
};

/* Every key any tab sends, as an eavesdropper on the key channel would see
 * them; the test uses them to show the SFU's tap is exactly this ciphertext. */
w.overhear = () => {
  const heard: unknown[] = [];
  w.heard = heard;
  new BroadcastChannel('losos-call-keys').addEventListener('message', (ev) => heard.push(ev.data));
};
