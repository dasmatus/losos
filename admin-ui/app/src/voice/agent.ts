/* LosOS in the call.
 *
 * Calls are end-to-end encrypted, so the box cannot hear them: whoever
 * listens has to be a member that holds the call's keys. This agent is that
 * member. It runs in the page of the person who added it, as a second
 * session of the call (same user and device, `agent: true`), hears every
 * other member on a stream of their own, reads what they say with Whisper,
 * and, when the name is said, asks the box's assistant (LosOS Tag) and
 * speaks the answer with Kokoro into its own microphone. Nothing of the
 * call leaves the page except the question, as text, to the person's own
 * box. Close the page and the agent is gone.
 */

import { getTagAsk, postTagAsk } from "@/lib/api";
import { CallSession, type CallState, type KeyChannel, type Member } from "@/calls/session";
import { Dialog, isStop, prompt, type Question } from "./dialog";
import { Ears } from "./ears";
import { openMouth, type Mouth } from "./mouth";
import { SentenceStream } from "./sentences";
import { RATE, Segmenter } from "./vad";

export type Phase = "loading" | "listening" | "thinking" | "speaking";

export interface Caption {
  /* Who said it, or the agent's name. */
  who: string;
  text: string;
  agent: boolean;
}

export interface AgentOptions {
  socket: string;
  call: string;
  /* The device of the page that adds the agent; the agent joins as it. */
  device: string;
  keys: KeyChannel;
  /* A member's name for the captions and the conversation. */
  nameOf(user: string): string;
  /* Whether the asker's own files, mail and rooms may help an answer, which
   * is then read aloud to everyone in the call. Read at each question. */
  grounded(): boolean;
  events: {
    phase(phase: Phase): void;
    /* How much of the engine and the models has come, 0 to 1. */
    load(fraction: number): void;
    caption(caption: Caption): void;
    state(state: CallState, reason?: string): void;
    /* Every clip read, for a page that wants to show or log it. Nothing
     * in the product listens: what is not addressed to LosOS is forgotten. */
    heard?(pid: number, text: string): void;
  };
}

/* How long one answer may take before the agent gives up on it. */
const ANSWER_WITHIN_MS = 120_000;
const POLL_MS = 400;
const SORRY = "Sorry, I could not get an answer just now.";
const GREETING = "LosOS is here. Say my name when you need me.";

interface Listener {
  segmenter: Segmenter;
  node: AudioWorkletNode;
  source: MediaStreamAudioSourceNode;
  element: HTMLAudioElement;
}

export class VoiceAgent {
  private session: CallSession | null = null;
  private listeners = new Map<number, Listener>();
  private members: Member[] = [];
  private dialog = new Dialog();
  private input: AudioContext | null = null;
  private busy = false;
  private later: Question | null = null;
  private generation = 0;
  private closed = false;
  private greeted = false;
  private cancel: AbortController | null = null;

  private constructor(
    private opts: AgentOptions,
    private ears: Ears,
    private mouth: Mouth,
  ) {}

  /* Open the ears and the mouth (the engine and the models are fetched
   * once and kept by the browser), then join the call. */
  static async start(opts: AgentOptions): Promise<VoiceAgent> {
    opts.events.phase("loading");
    const loaded = { ears: { got: 0, of: 1 }, mouth: { got: 0, of: 1 } };
    const report = (): void => {
      const done = loaded.ears.got + loaded.mouth.got;
      const total = loaded.ears.of + loaded.mouth.of;
      opts.events.load(total > 0 ? Math.min(1, done / total) : 0);
    };
    const [ears, mouth] = await Promise.all([
      Ears.open((l, t) => {
        loaded.ears = { got: l, of: Math.max(t, 1) };
        report();
      }),
      openMouth((l, t) => {
        loaded.mouth = { got: l, of: Math.max(t, 1) };
        report();
      }),
    ]);
    const agent = new VoiceAgent(opts, ears, mouth);
    try {
      await agent.join();
    } catch (e) {
      agent.stop();
      throw e;
    }
    return agent;
  }

  private async join(): Promise<void> {
    const context = new AudioContext({ sampleRate: RATE });
    await context.audioWorklet.addModule(`${import.meta.env.BASE_URL}voice-worklet/pcm-tap.js`);
    this.input = context;
    this.session = new CallSession({
      socket: this.opts.socket,
      call: this.opts.call,
      device: this.opts.device,
      agent: true,
      local: this.mouth.stream,
      keys: this.opts.keys,
      events: {
        state: (state, reason) => {
          if (state === "connected") {
            this.opts.events.phase("listening");
            if (!this.greeted) {
              this.greeted = true;
              void this.say(GREETING);
            }
          }
          if (state === "failed" || state === "ended") this.stop();
          this.opts.events.state(state, reason);
        },
        members: (members) => this.onMembers(members),
        track: (pid, stream, screen) => {
          if (!screen) this.listen(pid, stream);
        },
        stats: () => undefined,
      },
    });
    this.session.start();
  }

  /* Who is in the call now. The agent hears people, not other agents. */
  private onMembers(members: Member[]): void {
    this.members = members;
    const here = new Set(members.map((m) => m.pid));
    for (const pid of [...this.listeners.keys()]) if (!here.has(pid)) this.deafen(pid);
    for (const m of members) if (m.agent === true) this.deafen(m.pid);
  }

  private listen(pid: number, stream: MediaStream): void {
    const context = this.input;
    const member = this.members.find((m) => m.pid === pid);
    if (context === null || this.closed || this.listeners.has(pid)) return;
    if (member?.agent === true || stream.getAudioTracks().length === 0) return;
    // A remote stream gives a Web Audio graph nothing until a media element
    // has taken it too, in Chromium; a muted one does.
    const element = document.createElement("audio");
    element.muted = true;
    element.srcObject = stream;
    void element.play().catch(() => undefined);
    const source = context.createMediaStreamSource(new MediaStream(stream.getAudioTracks()));
    const node = new AudioWorkletNode(context, "pcm-tap", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1] });
    const silent = context.createGain();
    silent.gain.value = 0;
    source.connect(node);
    node.connect(silent);
    silent.connect(context.destination);
    const segmenter = new Segmenter((u) => void this.heard(pid, u.pcm));
    node.port.onmessage = (ev: MessageEvent<Float32Array>) => segmenter.push(ev.data);
    this.listeners.set(pid, { segmenter, node, source, element });
  }

  private deafen(pid: number): void {
    const l = this.listeners.get(pid);
    if (l === undefined) return;
    this.listeners.delete(pid);
    l.node.port.onmessage = null;
    l.source.disconnect();
    l.node.disconnect();
    l.element.srcObject = null;
  }

  /* One clip of one person's speech. */
  private async heard(pid: number, pcm: Float32Array): Promise<void> {
    if (this.closed) return;
    const member = this.members.find((m) => m.pid === pid);
    if (member === undefined) return;
    let text: string | null;
    try {
      text = await this.ears.transcribe(pcm);
    } catch {
      return;
    }
    if (text === null || this.closed) return;
    this.opts.events.heard?.(pid, text);
    // Stop is for anyone, with or without the name.
    if (this.mouth.speaking && isStop(text)) {
      this.hush();
      return;
    }
    const name = this.opts.nameOf(member.user);
    const question = this.dialog.hear({ user: member.user, name, text, at: Date.now() });
    if (question === null) return;
    if (this.busy) {
      // One at a time; the newest waits its turn.
      this.later = question;
      return;
    }
    void this.answer(question);
  }

  /* Stop talking now, and drop what was about to be said. */
  hush(): void {
    this.generation++;
    this.later = null;
    this.cancel?.abort();
    this.mouth.stop();
    if (!this.closed) this.opts.events.phase("listening");
  }

  private async say(text: string): Promise<void> {
    if (this.closed) return;
    this.opts.events.phase("speaking");
    try {
      await this.mouth.say(text);
    } catch {
      // A sentence the voice could not make is skipped.
    }
    if (!this.busy && !this.closed) this.opts.events.phase("listening");
  }

  private async answer(q: Question): Promise<void> {
    const generation = ++this.generation;
    const abort = new AbortController();
    this.cancel = abort;
    this.busy = true;
    this.opts.events.phase("thinking");
    this.opts.events.caption({ who: q.name, text: q.text === "" ? "…" : q.text, agent: false });
    const spoken: string[] = [];
    const speeches: Promise<void>[] = [];
    const speak = (sentence: string): void => {
      spoken.push(sentence);
      this.opts.events.phase("speaking");
      speeches.push(this.mouth.say(sentence).catch(() => undefined));
    };
    try {
      const ask = await postTagAsk(prompt(q), { voice: true, grounded: this.opts.grounded() });
      const stream = new SentenceStream();
      const until = Date.now() + ANSWER_WITHIN_MS;
      for (;;) {
        if (abort.signal.aborted || generation !== this.generation) return;
        const state = await getTagAsk(ask.id, { signal: abort.signal });
        const final = state.state === "done";
        for (const s of stream.feed(state.answer, final)) speak(s);
        if (final) break;
        if (state.state === "failed" || state.state === "stopped") throw new Error(state.error ?? state.state);
        if (Date.now() > until) throw new Error("timeout");
        await new Promise((r) => setTimeout(r, POLL_MS));
      }
      if (spoken.length === 0) speak("I have nothing to add.");
    } catch {
      if (abort.signal.aborted || generation !== this.generation) return;
      if (spoken.length === 0) speak(SORRY);
    } finally {
      const current = generation === this.generation;
      if (current) await Promise.all(speeches);
      if (generation === this.generation) {
        const said = spoken.join(" ");
        if (said !== "") {
          this.opts.events.caption({ who: "LosOS", text: said, agent: true });
          this.dialog.answered(q.user, said, Date.now());
        }
      }
      // A newer answer, or a hush, has taken over this one's place.
      if (this.cancel === abort) {
        this.busy = false;
        this.cancel = null;
      }
      if (current && generation === this.generation && !this.closed) {
        this.opts.events.phase("listening");
        const next = this.later;
        this.later = null;
        if (next !== null) void this.answer(next);
      }
    }
  }

  /* Leave the call and give back the microphone, the models' memory and the
   * audio graph. */
  stop(): void {
    if (this.closed) return;
    this.closed = true;
    this.generation++;
    this.cancel?.abort();
    for (const pid of [...this.listeners.keys()]) this.deafen(pid);
    void this.session?.leave();
    this.session = null;
    this.mouth.close();
    this.ears.close();
    void this.input?.close();
    this.input = null;
  }
}
