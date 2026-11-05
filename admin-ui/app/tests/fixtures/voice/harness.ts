/* The page tests/voice-agent.browser.mjs opens in each tab.
 *
 * `speaker` is a person who talks with the same mouth the agent has (a
 * Kokoro voice into their microphone) and who listens with the same ears
 * (Whisper behind the same segmenter), so a spoken question goes in and a
 * spoken answer is read back as text. `host` is the page of the person who
 * adds LosOS: its own session of the call, the agent as the second one, and
 * the keys between the two passed in memory (src/voice/keys.ts), as
 * src/chat/calls.tsx does. Keys between tabs go over a BroadcastChannel in
 * place of Olm. */

import { CallSession, type KeyChannel, type KeyMessage, type Member } from '../../../src/calls/session';
import { VoiceAgent, type Caption, type Persona, type Phase } from '../../../src/voice/agent';
import { Ears } from '../../../src/voice/ears';
import { KeyHub } from '../../../src/voice/keys';
import { openMouth, voiceFor } from '../../../src/voice/mouth';
import { HEIGHT, WIDTH } from '../../../src/voice/avatar';
import { Segmenter, RATE } from '../../../src/voice/vad';
import { VoiceChain, type VoicePreset } from '../../../src/calls/voice-changer';

interface Who {
  user: string;
  device: string;
}

const w = window as unknown as Record<string, unknown>;
const state = {
  members: [] as Member[],
  you: -1,
  states: [] as string[],
  phases: [] as Phase[],
  captions: [] as Caption[],
  heard: [] as string[],
  isolated: self.crossOriginIsolated,
  keysIn: 0,
  /* Per sender: blocks the worklet handed over, and the loudest. */
  tap: {} as Record<string, { blocks: number; peak: number }>,
  context: '',
  voice: '',
};
w.state = state;

/* Keys between tabs, authenticated by the claimed sender the way Olm is. */
function busKeys(me: Who): KeyChannel {
  const bus = new BroadcastChannel('losos-call-keys');
  return {
    async send(to, msg) {
      bus.postMessage({ from: me, to, msg });
    },
    subscribe(handler) {
      const on = (ev: MessageEvent<{ from: Who; to: Who[]; msg: KeyMessage }>) => {
        const { from, to, msg } = ev.data;
        if (!to.some((t) => t.user === me.user && t.device === me.device)) return;
        state.keysIn++;
        handler(from, msg);
      };
      bus.addEventListener('message', on);
      return () => bus.removeEventListener('message', on);
    },
  };
}

function silence(): MediaStream {
  const ctx = new AudioContext();
  const out = ctx.createMediaStreamDestination();
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  gain.gain.value = 0;
  osc.connect(gain).connect(out);
  osc.start();
  return out.stream;
}

const events = (): ConstructorParameters<typeof CallSession>[0]['events'] => ({
  state: (s, reason) => state.states.push(reason ? `${s}:${reason}` : s),
  members: (m, you) => {
    state.members = m;
    state.you = you;
  },
  track: () => undefined,
  stats: () => undefined,
});

let said: ((text: string) => Promise<void>) | null = null;
w.say = (text: string) => said?.(text);

/* A person who speaks with Kokoro and listens with Whisper. */
w.joinSpeaker = async (o: { socket: string; call: string; me: Who }) => {
  const mouth = await openMouth();
  said = (text) => mouth.say(text);
  const ears = await Ears.open();
  const ctx = new AudioContext({ sampleRate: RATE });
  await ctx.audioWorklet.addModule('/voice-worklet/pcm-tap.js');
  const heardFrom = new Set<number>();
  const session = new CallSession({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    local: mouth.stream,
    keys: busKeys(o.me),
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: {
      ...events(),
      track: (pid, stream, screen) => {
        if (screen || heardFrom.has(pid) || stream.getAudioTracks().length === 0) return;
        heardFrom.add(pid);
        const el = document.createElement('audio');
        el.muted = true;
        el.srcObject = stream;
        void el.play().catch(() => undefined);
        const tap = new AudioWorkletNode(ctx, 'pcm-tap');
        const quiet = ctx.createGain();
        quiet.gain.value = 0;
        ctx.createMediaStreamSource(new MediaStream(stream.getAudioTracks())).connect(tap);
        tap.connect(quiet).connect(ctx.destination);
        const segmenter = new Segmenter((u) => void ears.transcribe(u.pcm).then((t) => t && state.heard.push(`p${pid}: ${t}`)));
        tap.port.onmessage = (ev: MessageEvent<Float32Array>) => {
          const t = (state.tap[`p${pid}`] ??= { blocks: 0, peak: 0 });
          t.blocks++;
          for (const x of ev.data) t.peak = Math.max(t.peak, Math.abs(x));
          state.context = ctx.state;
          segmenter.push(ev.data);
        };
      },
    },
  });
  session.start();
  w.session = session;
};

/* The page of the person who adds LosOS to the call. */
w.joinHost = async (o: { socket: string; call: string; me: Who }) => {
  const hub = new KeyHub(busKeys(o.me), o.me);
  const session = new CallSession({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    local: silence(),
    keys: hub.channel(),
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: events(),
  });
  session.start();
  w.session = session;
  w.agent = await VoiceAgent.start({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    keys: hub.channel(),
    nameOf: (user) => user.replace(/^@|:.*$/g, ''),
    grounded: () => false,
    events: {
      phase: (p) => state.phases.push(p),
      load: () => undefined,
      heard: (pid, text) => state.heard.push(`p${pid}: ${text}`),
      caption: (c) => state.captions.push(c),
      state: (s, reason) => state.states.push(`agent:${reason ? `${s}:${reason}` : s}`),
    },
  });
};

const audioStats = async (pc: RTCPeerConnection) => {
  const out: Record<string, unknown>[] = [];
  for (const r of (await pc.getStats()).values()) {
    if (r.kind !== 'audio') continue;
    if (r.type === 'inbound-rtp') out.push({ type: r.type, packets: r.packetsReceived, energy: r.totalAudioEnergy, samples: r.totalSamplesReceived, lost: r.packetsLost });
    if (r.type === 'outbound-rtp') out.push({ type: r.type, packets: r.packetsSent, bytes: r.bytesSent });
    if (r.type === 'media-source') out.push({ type: r.type, energy: r.totalAudioEnergy, level: r.audioLevel });
  }
  return out;
};
w.stats = async () => audioStats((w.session as unknown as { pc: RTCPeerConnection }).pc);
w.agentStats = async () => audioStats(((w.agent as unknown as { session: { pc: RTCPeerConnection } }).session).pc);

/* A person whose microphone is a steady tone, through the voice changer. */
let chain: VoiceChain | null = null;
w.joinTone = async (o: { socket: string; call: string; me: Who; hz: number }) => {
  const ctx = new AudioContext();
  await ctx.resume();
  const osc = ctx.createOscillator();
  osc.frequency.value = o.hz;
  const level = ctx.createGain();
  level.gain.value = 0.4;
  const out = ctx.createMediaStreamDestination();
  osc.connect(level).connect(out);
  osc.start();
  const mic = out.stream.getAudioTracks()[0] as MediaStreamTrack;
  w.mic = mic;
  const session = new CallSession({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    local: out.stream,
    keys: busKeys(o.me),
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: events(),
  });
  session.start();
  w.session = session;
};
w.changeVoice = async (preset: VoicePreset) => {
  const session = w.session as CallSession;
  if (preset === 'off') {
    await session.setAudioTrack(w.mic as MediaStreamTrack);
    return;
  }
  chain ??= await VoiceChain.open(w.mic as MediaStreamTrack);
  chain.set(preset);
  await session.setAudioTrack(chain.track);
};

/* The pitch of what arrives from the other member, by rising zero crossings
 * over `ms` of the decoded stream. */
w.listenPitch = async (o: { socket: string; call: string; me: Who }) => {
  const local = new AudioContext().createMediaStreamDestination().stream;
  const session = new CallSession({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    local,
    keys: busKeys(o.me),
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: {
      ...events(),
      track: (_pid, stream, screen) => {
        if (screen || stream.getAudioTracks().length === 0) return;
        const el = document.createElement('audio');
        el.muted = true;
        el.srcObject = stream;
        void el.play().catch(() => undefined);
        const ctx = new AudioContext();
        const analyser = ctx.createAnalyser();
        analyser.fftSize = 2048;
        ctx.createMediaStreamSource(stream).connect(analyser);
        w.analyser = { ctx, analyser };
      },
    },
  });
  session.start();
  w.session = session;
};
w.pitch = async (ms: number) => {
  const { ctx, analyser } = w.analyser as { ctx: AudioContext; analyser: AnalyserNode };
  const buf = new Float32Array(analyser.fftSize);
  let crossings = 0;
  let seen = 0;
  let peak = 0;
  const end = performance.now() + ms;
  let last = 0;
  while (performance.now() < end) {
    analyser.getFloatTimeDomainData(buf);
    for (let i = 1; i < buf.length; i++) {
      peak = Math.max(peak, Math.abs(buf[i]!));
      if (buf[i - 1]! < 0 && buf[i]! >= 0) crossings++;
    }
    seen += buf.length;
    await new Promise((r) => setTimeout(r, (buf.length / ctx.sampleRate) * 1000));
    last = performance.now();
  }
  void last;
  return { hz: crossings / (seen / ctx.sampleRate), peak };
};


/* A call of one person with one of their assistants, as src/chat/calls.tsx
 * makes it in a chat with an assistant: the person's own session speaks with
 * a Kokoro voice of its own, the assistant joins from the same page as a
 * second session with a face, and what comes back from it (its picture and
 * its voice, sealed and opened again) is read from the person's session. */
const faces = { frames: 0, width: 0, height: 0 };
w.faces = faces;
w.joinPerson = async (o: { socket: string; call: string; me: Who; assistant: string; name: string; video: boolean; voice?: string }) => {
  const mouth = await openMouth(undefined, o.voice ?? 'am_adam');
  said = (text) => mouth.say(text);
  const hub = new KeyHub(busKeys(o.me), o.me);
  const session = new CallSession({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    local: mouth.stream,
    keys: hub.channel(),
    worker: () => new Worker('/e2ee-worker.js', { type: 'module' }),
    events: {
      ...events(),
      track: (_pid, stream, screen) => {
        if (screen) return;
        // The only other member with a picture is the assistant.
        const video = stream.getVideoTracks()[0];
        if (video === undefined) return;
        // The assistant's face, decoded from the call into a <video> the
        // test can read pixels from.
        const el = document.createElement('video');
        el.muted = true;
        el.playsInline = true;
        el.srcObject = new MediaStream([video]);
        el.addEventListener('resize', () => {
          faces.width = el.videoWidth;
          faces.height = el.videoHeight;
        });
        (el as HTMLVideoElement & { requestVideoFrameCallback(cb: () => void): void }).requestVideoFrameCallback(function count() {
          faces.frames++;
          (el as HTMLVideoElement & { requestVideoFrameCallback(cb: () => void): void }).requestVideoFrameCallback(count);
        });
        document.body.append(el);
        void el.play().catch(() => undefined);
        w.face = el;
      },
    },
  });
  session.start();
  w.session = session;
  const persona: Persona = { slug: o.assistant, name: o.name, voice: voiceFor(o.assistant), direct: true, video: o.video };
  state.voice = persona.voice;
  w.agent = await VoiceAgent.start({
    socket: o.socket,
    call: o.call,
    device: o.me.device,
    keys: hub.channel(),
    persona,
    nameOf: (user) => user.replace(/^@|:.*$/g, ''),
    grounded: () => true,
    events: {
      phase: (p) => state.phases.push(p),
      load: () => undefined,
      heard: (pid, text) => state.heard.push(`p${pid}: ${text}`),
      caption: (c) => state.captions.push(c),
      state: (s, reason) => state.states.push(`agent:${reason ? `${s}:${reason}` : s}`),
    },
  });
};

/* The share of the face's pixels that are ink (the eyes, the mouth) inside
 * the box that holds the mouth, and of skin over the whole picture, from the
 * last decoded frame. */
w.faceLook = () => {
  const el = w.face as HTMLVideoElement | undefined;
  if (el === undefined || el.videoWidth === 0) return null;
  const c = document.createElement('canvas');
  c.width = el.videoWidth;
  c.height = el.videoHeight;
  const g = c.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D;
  g.drawImage(el, 0, 0);
  const u = Math.min(c.width, c.height) / 360;
  const area = (x: number, y: number, w2: number, h2: number) => {
    const d = g.getImageData(Math.round(x), Math.round(y), Math.round(w2), Math.round(h2)).data;
    let ink = 0;
    let skin = 0;
    for (let i = 0; i < d.length; i += 4) {
      const r = d[i]!;
      const gr = d[i + 1]!;
      const b = d[i + 2]!;
      if (r < 90 && gr < 70 && b < 70) ink++;
      if (r > 190 && gr > 100 && gr < 170 && b > 70 && b < 150) skin++;
    }
    return { ink: ink / (d.length / 4), skin: skin / (d.length / 4) };
  };
  const cx = c.width / 2;
  const hy = c.height * 0.46;
  return {
    mouth: area(cx - 34 * u, hy + 18 * u, 68 * u, 62 * u),
    all: area(0, 0, c.width, c.height),
    size: [c.width, c.height, WIDTH, HEIGHT],
  };
};
