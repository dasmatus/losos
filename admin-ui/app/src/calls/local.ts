/* What this browser sends into a call: its microphone, its picture and its
 * screen, and the switches between them.
 *
 * The picture is the camera through the black background
 * (background.ts) unless its owner turned that off. When the background
 * cannot be blacked out, the camera sends nothing rather than the room:
 * `picture` is null and `backgroundFailed` says why. */

import { BlackBackground, preferredBackground, rememberBackground } from "./background";

export type Background = "black" | "real";

export class LocalMedia {
  /* Microphone and picture, as the call sends them and the own tile shows
   * them. */
  readonly stream = new MediaStream();
  background: Background = preferredBackground();
  backgroundFailed = false;
  private camera: MediaStreamTrack | null = null;
  private black: BlackBackground | null = null;
  private screenTrack: MediaStreamTrack | null = null;

  private constructor(opened: MediaStream) {
    for (const t of opened.getAudioTracks()) this.stream.addTrack(t);
    this.camera = opened.getVideoTracks()[0] ?? null;
  }

  /* Ask for the microphone, and for the camera with `video`. */
  static async open(video: boolean): Promise<LocalMedia> {
    const opened = await navigator.mediaDevices.getUserMedia({ audio: true, video });
    const media = new LocalMedia(opened);
    await media.dress();
    return media;
  }

  get cameraOn(): boolean {
    return this.camera !== null;
  }

  get picture(): MediaStreamTrack | null {
    return this.stream.getVideoTracks()[0] ?? null;
  }

  get screen(): MediaStreamTrack | null {
    return this.screenTrack;
  }

  /* Put the camera's picture, through the chosen background, into the
   * sent stream. */
  private async dress(): Promise<void> {
    for (const t of this.stream.getVideoTracks()) this.stream.removeTrack(t);
    this.black?.stop();
    this.black = null;
    this.backgroundFailed = false;
    if (this.camera === null) return;
    if (this.background === "real") {
      this.stream.addTrack(this.camera);
      return;
    }
    try {
      this.black = await BlackBackground.start(this.camera);
      this.stream.addTrack(this.black.track);
    } catch {
      this.backgroundFailed = true;
    }
  }

  /* Turn the camera on or off; off stops it, so its light goes out. */
  async setCamera(on: boolean): Promise<void> {
    if (on === this.cameraOn) return;
    if (!on) {
      this.camera?.stop();
      this.camera = null;
      await this.dress();
      return;
    }
    const opened = await navigator.mediaDevices.getUserMedia({ video: true });
    this.camera = opened.getVideoTracks()[0] ?? null;
    await this.dress();
  }

  async setBackground(background: Background): Promise<void> {
    this.background = background;
    rememberBackground(background);
    await this.dress();
  }

  /* Ask which screen or window to share. The browser's own "Stop sharing"
   * ends the track, and `onEnded` is told. */
  async share(onEnded: () => void): Promise<MediaStreamTrack> {
    const display = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: false });
    const track = display.getVideoTracks()[0];
    if (track === undefined) throw new Error("no screen");
    track.contentHint = "detail";
    track.addEventListener("ended", onEnded);
    this.screenTrack = track;
    return track;
  }

  stopSharing(): void {
    this.screenTrack?.stop();
    this.screenTrack = null;
  }

  setMuted(muted: boolean): void {
    for (const t of this.stream.getAudioTracks()) t.enabled = !muted;
  }

  stop(): void {
    this.black?.stop();
    this.camera?.stop();
    this.stopSharing();
    for (const t of this.stream.getTracks()) t.stop();
  }
}
