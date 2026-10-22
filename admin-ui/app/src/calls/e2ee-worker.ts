/// <reference lib="webworker" />
/* The worker every encoded frame of a call passes through: outgoing frames
 * are sealed with this browser's send key, incoming ones opened with their
 * sender's. Nothing leaves without a key: until the page hands one over,
 * outgoing frames are dropped, never sent in the clear.
 *
 * Frames arrive two ways. Browsers with RTCRtpScriptTransform (Firefox,
 * Safari, current Chromium) raise `rtctransform` here; older Chromium hands
 * the page a readable/writable pair from createEncodedStreams(), which the
 * page transfers here in a `stream` message. Both end in pipe().
 */

import { decryptFrame, ReceiveRing, readTrailer, receivedClearBytes, SendKey, type Kind } from './e2ee';
import type { WorkerIn, WorkerStats, TransformOptions } from './e2ee-protocol';

declare const self: DedicatedWorkerGlobalScope & {
  onrtctransform: ((ev: Event & { transformer: RTCRtpScriptTransformer }) => void) | null;
};

interface RTCRtpScriptTransformer {
  readable: ReadableStream<EncodedFrame>;
  writable: WritableStream<EncodedFrame>;
  options: TransformOptions;
}

interface EncodedFrame {
  data: ArrayBuffer;
  type?: 'key' | 'delta' | 'empty';
}

let send: SendKey | null = null;
/* Stats go back over a port of their own: Chromium stops delivering a
 * worker's own postMessage once an RTCRtpScriptTransform holds it. They are
 * sent from the frame path, at most once a second, rather than from a
 * timer, which Chromium does not reliably run in such a worker either. */
let out: MessagePort | null = null;
let lastStats = 0;
function report(): void {
  const now = Date.now();
  if (now - lastStats < 1000) return;
  lastStats = now;
  out?.postMessage({ type: 'stats', stats });
}
const rings = new Map<string, ReceiveRing>();
const stats: WorkerStats = { sealed: 0, opened: 0, refused: 0, withoutKey: 0 };

function ring(sender: string): ReceiveRing {
  let r = rings.get(sender);
  if (!r) {
    r = new ReceiveRing();
    rings.set(sender, r);
  }
  return r;
}

async function seal(kind: Kind, frame: EncodedFrame): Promise<boolean> {
  if (!send) {
    stats.withoutKey++;
    return false;
  }
  const out = await send.seal(kind, new Uint8Array(frame.data), frame.type ? frame.type === 'key' : undefined);
  frame.data = out.buffer as ArrayBuffer;
  stats.sealed++;
  return true;
}

async function open(kind: Kind, sender: string, frame: EncodedFrame): Promise<boolean> {
  const bytes = new Uint8Array(frame.data);
  // An empty frame (a paused track) carries nothing to decrypt.
  if (bytes.length === 0) return true;
  const t = readTrailer(bytes);
  const ring = rings.get(sender);
  const keys = t ? await ring?.get(t.index) : null;
  if (!keys) {
    stats.withoutKey++;
    return false;
  }
  const plain = await decryptFrame(keys[kind], bytes, receivedClearBytes(kind, bytes));
  if (!plain) {
    // A frame under a rotated key that is still on its way meets this
    // ring's own ratchet; only a key that was handed over and fails
    // means the frame was changed.
    if (t && ring?.guessed(t.index)) stats.withoutKey++;
    else stats.refused++;
    return false;
  }
  frame.data = plain.buffer as ArrayBuffer;
  stats.opened++;
  return true;
}

function pipe(
  readable: ReadableStream<EncodedFrame>,
  writable: WritableStream<EncodedFrame>,
  options: TransformOptions,
): void {
  const transform = new TransformStream<EncodedFrame, EncodedFrame>({
    async transform(frame, controller) {
      try {
        const ok =
          options.op === 'seal'
            ? await seal(options.kind, frame)
            : await open(options.kind, options.sender ?? '', frame);
        // A frame that cannot be sealed is dropped; one that cannot be
        // opened is dropped too, and the decoder asks for a keyframe.
        if (ok) controller.enqueue(frame);
      } catch {
        stats.refused++;
      }
      report();
    },
  });
  readable.pipeThrough(transform).pipeTo(writable).catch(() => undefined);
}

self.onrtctransform = (ev) => {
  const t = ev.transformer;
  pipe(t.readable, t.writable, t.options);
};

self.onmessage = (ev: MessageEvent<WorkerIn>) => {
  const m = ev.data;
  switch (m.type) {
    case 'sendKey':
      send = new SendKey(m.key, m.index);
      break;
    case 'recvKey':
      ring(m.sender).set(m.index, m.key);
      break;
    case 'dropSender':
      rings.get(m.sender)?.clear();
      rings.delete(m.sender);
      break;
    case 'stream':
      pipe(m.readable as ReadableStream<EncodedFrame>, m.writable as WritableStream<EncodedFrame>, m.options);
      break;
    case 'port':
      out = m.port;
      break;
    case 'stop':
      send?.key.fill(0);
      send = null;
      for (const r of rings.values()) r.clear();
      rings.clear();
      break;
  }
};

