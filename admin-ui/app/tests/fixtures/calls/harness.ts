/* The page tests/calls.browser.mjs opens in each browser tab: one call
 * member on src/calls/session.ts, with the keys passed between tabs over a
 * BroadcastChannel in place of Olm. A tab can be told to drop the keys sent
 * to it, which is what an SFU (or anyone between) is left with. */

import { CallSession, type KeyMessage, type Member } from '../../../src/calls/session';
import type { WorkerStats } from '../../../src/calls/e2ee-protocol';

interface Joined {
  user: string;
  device: string;
}

const w = window as unknown as Record<string, unknown>;
const state: {
  members: Member[];
  you: number;
  stats: WorkerStats | null;
  states: string[];
  keysIn: number;
} = { members: [], you: -1, stats: null, states: [], keysIn: 0 };
w.callState = state;

w.startCall = async (opts: {
  socket: string;
  call: string;
  token: string;
  server: string;
  me: Joined;
  dropKeys?: boolean;
  video?: boolean;
}) => {
  const local = await navigator.mediaDevices.getUserMedia({ audio: true, video: opts.video !== false });
  const bus = new BroadcastChannel('losos-call-keys');
  const session = new CallSession({
    socket: opts.socket,
    call: opts.call,
    openid: { access_token: opts.token, matrix_server_name: opts.server },
    device: opts.me.device,
    local,
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    keys: {
      async send(to, msg) {
        bus.postMessage({ from: opts.me.user, to, msg });
      },
      subscribe(handler) {
        const on = (ev: MessageEvent<{ from: string; to: Joined[]; msg: KeyMessage }>) => {
          const { from, to, msg } = ev.data;
          if (!to.some((t) => t.user === opts.me.user && t.device === opts.me.device)) return;
          if (opts.dropKeys) return;
          state.keysIn++;
          handler({ user: from }, msg);
        };
        bus.addEventListener('message', on);
        return () => bus.removeEventListener('message', on);
      },
    },
    events: {
      state: (s, reason) => state.states.push(reason ? `${s}:${reason}` : s),
      members: (m, you) => {
        state.members = m;
        state.you = you;
      },
      track: (pid, stream) => {
        let v = document.getElementById(`p${pid}`) as HTMLVideoElement | null;
        if (!v) {
          v = document.createElement('video');
          v.id = `p${pid}`;
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
  const mine = document.createElement('video');
  mine.id = 'local';
  mine.autoplay = true;
  mine.muted = true;
  mine.srcObject = local;
  document.body.append(mine);
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
