/* The voice changer: the microphone through an effect in the audio thread
 * (public/voice-worklet/voice-changer.js) before the call seals it. It runs
 * in this browser and nothing about it reaches the box or the others: they
 * hear the changed voice and cannot tell. */

export type VoicePreset = "off" | "deep" | "high" | "giant" | "chipmunk" | "robot";

export const VOICE_PRESETS: readonly VoicePreset[] = ["off", "deep", "high", "giant", "chipmunk", "robot"];

/* What each preset asks of the effect. */
export const SETTINGS: Record<VoicePreset, { semitones: number; ring: number }> = {
  off: { semitones: 0, ring: 0 },
  deep: { semitones: -4, ring: 0 },
  high: { semitones: 4, ring: 0 },
  giant: { semitones: -8, ring: 0 },
  chipmunk: { semitones: 8, ring: 0 },
  robot: { semitones: 0, ring: 55 },
};

const KEY = "losos.calls.voice";

export function preferredVoice(): VoicePreset {
  try {
    const saved = localStorage.getItem(KEY);
    return VOICE_PRESETS.find((p) => p === saved) ?? "off";
  } catch {
    return "off";
  }
}

export function rememberVoice(preset: VoicePreset): void {
  try {
    localStorage.setItem(KEY, preset);
  } catch {
    // A private window keeps nothing; the choice lasts the call.
  }
}

/* One microphone through one effect. */
export class VoiceChain {
  private constructor(
    private context: AudioContext,
    private node: AudioWorkletNode,
    private source: MediaStreamAudioSourceNode,
    readonly track: MediaStreamTrack,
  ) {}

  static async open(mic: MediaStreamTrack): Promise<VoiceChain> {
    const context = new AudioContext({ latencyHint: "interactive" });
    try {
      await context.audioWorklet.addModule(`${import.meta.env.BASE_URL}voice-worklet/voice-changer.js`);
      await context.resume();
      const source = context.createMediaStreamSource(new MediaStream([mic]));
      const node = new AudioWorkletNode(context, "voice-changer", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1] });
      const out = context.createMediaStreamDestination();
      source.connect(node).connect(out);
      const track = out.stream.getAudioTracks()[0];
      if (track === undefined) throw new Error("no processed track");
      return new VoiceChain(context, node, source, track);
    } catch (e) {
      void context.close();
      throw e;
    }
  }

  set(preset: VoicePreset): void {
    this.node.port.postMessage(SETTINGS[preset]);
  }

  close(): void {
    this.source.disconnect();
    this.node.disconnect();
    this.track.stop();
    void this.context.close();
  }
}
