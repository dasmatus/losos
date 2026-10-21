/* The black background: what a camera sends in a call while it is on,
 * unless its owner turns it off.
 *
 * A person on a call shows the room behind them, and a room says a lot
 * about where someone lives. So the camera's picture goes through a person
 * segmenter (MediaPipe's selfie segmenter, in WebAssembly, served by the
 * box beside the chat page) on this device, and everything that is not the
 * person is painted black before the frame reaches the encoder, and so
 * before it is encrypted. The room never leaves the device.
 *
 * The output starts black and stays black on any frame the segmenter
 * cannot read: a failure shows nothing rather than the room. */

import type { ImageSegmenter } from "@mediapipe/tasks-vision";

/* Where the vite build puts the segmenter's files (vite.config.ts). */
const BASE = "/chat/calls-ml";
const FPS = 30;

let loading: Promise<ImageSegmenter> | null = null;

function segmenter(): Promise<ImageSegmenter> {
  loading ??= (async () => {
    const { FilesetResolver, ImageSegmenter } = await import("@mediapipe/tasks-vision");
    const files = await FilesetResolver.forVisionTasks(BASE);
    return ImageSegmenter.createFromOptions(files, {
      baseOptions: { modelAssetPath: `${BASE}/selfie_segmenter.tflite`, delegate: "CPU" },
      runningMode: "VIDEO",
      outputCategoryMask: false,
      outputConfidenceMasks: true,
    });
  })();
  loading.catch(() => {
    loading = null;
  });
  return loading;
}

export class BlackBackground {
  readonly track: MediaStreamTrack;
  private running = true;
  private video: HTMLVideoElement;

  private constructor(
    private camera: MediaStreamTrack,
    private seg: ImageSegmenter,
  ) {
    const { width = 640, height = 480 } = camera.getSettings();
    const out = document.createElement("canvas");
    out.width = width;
    out.height = height;
    const ctx = out.getContext("2d");
    if (ctx === null) throw new Error("no 2d canvas");
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, width, height);
    this.track = out.captureStream(FPS).getVideoTracks()[0] as MediaStreamTrack;

    const mask = document.createElement("canvas");
    const maskCtx = mask.getContext("2d");
    if (maskCtx === null) throw new Error("no 2d canvas");

    this.video = document.createElement("video");
    this.video.muted = true;
    this.video.playsInline = true;
    this.video.srcObject = new MediaStream([camera]);
    void this.video.play().catch(() => undefined);

    let image: ImageData | null = null;
    const draw = (): void => {
      if (this.video.readyState < 2) return;
      ctx.globalCompositeOperation = "copy";
      ctx.fillStyle = "#000";
      try {
        const result = this.seg.segmentForVideo(this.video, performance.now());
        const confidence = result.confidenceMasks?.[0];
        if (confidence === undefined) throw new Error("no mask");
        const w = confidence.width;
        const h = confidence.height;
        const values = confidence.getAsFloat32Array();
        if (image === null || image.width !== w || image.height !== h) {
          mask.width = w;
          mask.height = h;
          image = maskCtx.createImageData(w, h);
        }
        const px = image.data;
        for (let i = 0; i < values.length; i++) px[i * 4 + 3] = Math.round((values[i] ?? 0) * 255);
        result.close();
        maskCtx.putImageData(image, 0, 0);
        // The mask, scaled smooth; the person through it; black under it.
        ctx.drawImage(mask, 0, 0, width, height);
        ctx.globalCompositeOperation = "source-in";
        ctx.drawImage(this.video, 0, 0, width, height);
        ctx.globalCompositeOperation = "destination-over";
        ctx.fillRect(0, 0, width, height);
      } catch {
        ctx.fillRect(0, 0, width, height);
      }
    };

    const loop = (): void => {
      if (!this.running) return;
      draw();
      // A timer, not the video's frame callback: the video is never on
      // the page, and a detached video need not report frames.
      setTimeout(loop, 1000 / FPS);
    };
    loop();
  }

  /* Start blacking out `camera`'s background; fails when the segmenter
   * cannot load, and then the caller sends no picture at all. */
  static async start(camera: MediaStreamTrack): Promise<BlackBackground> {
    return new BlackBackground(camera, await segmenter());
  }

  /* Stop drawing. The camera track is the caller's and keeps running. */
  stop(): void {
    this.running = false;
    this.track.stop();
    this.video.srcObject = null;
  }

  get source(): MediaStreamTrack {
    return this.camera;
  }
}

const PREFERENCE = "losos.calls.background";

/* Black unless this browser's owner chose otherwise. */
export function preferredBackground(): "black" | "real" {
  try {
    return localStorage.getItem(PREFERENCE) === "real" ? "real" : "black";
  } catch {
    return "black";
  }
}

export function rememberBackground(value: "black" | "real"): void {
  try {
    localStorage.setItem(PREFERENCE, value);
  } catch {
    // A private window keeps no preference; black stays the default.
  }
}
