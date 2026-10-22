/* What a call asks of the microphone and the camera, and how hard its
 * encoders may work. The page, the guest page and the tests share it.
 *
 * Sound comes first. The microphone is asked for 48 kHz with the browser's
 * echo cancellation, noise suppression and level control on: most people
 * call from a laptop's speakers, and without those the others hear
 * themselves back. Opus then gets 128 kb/s, a cap the browser also takes
 * as its target (it otherwise stays near 32 kb/s), so voices and music
 * come through at full band. Its in-band error correction is on (the SFU
 * answers with useinbandfec=1) and DTX is off, so a quiet start of a word
 * is not cut.
 *
 * The camera is asked for 720p at 30 frames a second and sends at most
 * 1.5 Mb/s. When the network is short the browser drops resolution and
 * keeps the frame rate, which is what makes a face look smooth; a screen
 * does the opposite and keeps its text sharp. */

export const MIC: MediaTrackConstraints = {
  echoCancellation: true,
  noiseSuppression: true,
  autoGainControl: true,
  channelCount: { ideal: 1 },
  sampleRate: { ideal: 48_000 },
};

export const CAMERA: MediaTrackConstraints = {
  width: { ideal: 1280 },
  height: { ideal: 720 },
  frameRate: { ideal: 30, max: 30 },
};

export const AUDIO_SEND: RTCRtpEncodingParameters = { maxBitrate: 128_000, priority: "high", networkPriority: "high" };
export const CAMERA_SEND: RTCRtpEncodingParameters = { maxBitrate: 1_500_000, maxFramerate: 30 };
export const SCREEN_SEND: RTCRtpEncodingParameters = { maxBitrate: 2_500_000, maxFramerate: 30 };

export type Degradation = "maintain-framerate" | "maintain-resolution" | "balanced";

/* Tell a sender what to give up first when the network is short. */
export async function degrade(sender: RTCRtpSender, preference: Degradation): Promise<void> {
  // TypeScript's DOM types do not carry degradationPreference yet.
  const params = sender.getParameters() as RTCRtpSendParameters & { degradationPreference?: Degradation };
  if (params.degradationPreference === preference) return;
  params.degradationPreference = preference;
  await sender.setParameters(params).catch(() => undefined);
}
