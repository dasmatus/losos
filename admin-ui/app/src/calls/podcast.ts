/* Podcast mode: the people in a call in a grid, under the host's overlay,
 * streamed live to an RTMP address.
 *
 * A live stream is public by nature, and the call's encryption ends where
 * it starts. So the picture is put together here, in the host's browser,
 * from frames this browser already decrypted: the grid, the names, the
 * title and the overlay image are drawn on a canvas, the voices mixed, and
 * the result recorded. Browsers cannot speak RTMP, so the recording goes to
 * the box over the call's own socket path (`/calls/api/stream`), and the
 * box hands it to ffmpeg, which sends it to the address. The box sees the
 * stream that is about to be public anyway, and nothing else of the call:
 * the other members' media stays sealed on its way through the SFU. */

export interface PodcastSource {
  pid: number;
  name: string;
  /* Their camera and microphone, as this browser plays them. */
  stream?: MediaStream;
}

export interface PodcastLook {
  title: string;
  subtitle: string;
  /* Drawn over the whole picture, scaled to it: a PNG with transparency is
   * a frame, a lower third or a logo. */
  overlay: ImageBitmap | null;
  /* A screen someone shares: drawn large, with the people beside it. */
  screen: { name: string; stream: MediaStream } | null;
}

export const WIDTH = 1280;
export const HEIGHT = 720;
const FPS = 30;

/* WebM the box's ffmpeg reads; it encodes H.264 and AAC for RTMP itself. */
const TYPES = ["video/webm;codecs=vp8,opus", "video/webm;codecs=vp9,opus", "video/webm"];

function pickType(): string {
  return TYPES.find((t) => MediaRecorder.isTypeSupported(t)) ?? "video/webm";
}

function columns(n: number): number {
  if (n <= 1) return 1;
  if (n <= 4) return 2;
  return 3;
}

export interface PodcastEvents {
  live(): void;
  ended(error?: string): void;
}

export class Podcast {
  private canvas = document.createElement("canvas");
  private ctx: CanvasRenderingContext2D;
  private videos = new Map<string, HTMLVideoElement>();
  private audio = new AudioContext();
  private mix = this.audio.createMediaStreamDestination();
  private mixed = new Map<string, MediaStreamAudioSourceNode>();
  private recorder: MediaRecorder | null = null;
  private ws: WebSocket | null = null;
  private timer: ReturnType<typeof setInterval>;
  private disposed = false;

  constructor(
    private sources: () => PodcastSource[],
    private look: () => PodcastLook,
    private events: PodcastEvents,
  ) {
    this.canvas.width = WIDTH;
    this.canvas.height = HEIGHT;
    const ctx = this.canvas.getContext("2d");
    if (ctx === null) throw new Error("no 2d canvas");
    this.ctx = ctx;
    this.draw();
    this.timer = setInterval(() => this.draw(), 1000 / FPS);
  }

  /* The picture as it goes out, drawn from the moment the host opens
   * podcast mode, so they see it before they go live. */
  get preview(): HTMLCanvasElement {
    return this.canvas;
  }

  get live(): boolean {
    return this.ws !== null;
  }

  /* Go live: `hello` is what the box needs to know who asks and where to
   * send the stream to. */
  start(socket: string, hello: { call: string; pid: number; rtmp: string }): void {
    if (this.disposed || this.ws !== null) return;
    void this.audio.resume().catch(() => undefined);
    const type = pickType();
    const video = this.canvas.captureStream(FPS).getVideoTracks()[0];
    const tracks = [video, this.mix.stream.getAudioTracks()[0]].filter((t): t is MediaStreamTrack => t !== undefined);
    const ws = new WebSocket(socket);
    ws.binaryType = "arraybuffer";
    this.ws = ws;
    ws.onopen = () => ws.send(JSON.stringify(hello));
    ws.onmessage = (ev) => {
      let m: { type?: string; error?: string | null };
      try {
        m = JSON.parse(String(ev.data));
      } catch {
        return;
      }
      if (m.type === "live" && this.recorder === null) {
        const recorder = new MediaRecorder(new MediaStream(tracks), {
          mimeType: type,
          videoBitsPerSecond: 2_500_000,
          audioBitsPerSecond: 128_000,
        });
        recorder.ondataavailable = (e) => {
          if (e.data.size > 0 && ws.readyState === WebSocket.OPEN) ws.send(e.data);
        };
        recorder.start(500);
        this.recorder = recorder;
        this.events.live();
      }
      if (m.type === "ended") this.finish(m.error ?? undefined);
    };
    ws.onclose = () => this.finish("closed");
  }

  /* End the stream; the preview keeps drawing. */
  stop(): void {
    this.finish();
  }

  /* End the stream and stop drawing. */
  dispose(): void {
    this.finish();
    this.disposed = true;
    clearInterval(this.timer);
    for (const v of this.videos.values()) v.srcObject = null;
    this.videos.clear();
    void this.audio.close().catch(() => undefined);
  }

  private finish(error?: string): void {
    if (this.recorder?.state === "recording") this.recorder.stop();
    this.recorder = null;
    if (this.ws === null) return;
    const ws = this.ws;
    this.ws = null;
    ws.onclose = null;
    ws.onmessage = null;
    // A text frame tells the box to stop before the socket closes.
    if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "stop" }));
    ws.close();
    this.events.ended(error);
  }

  private videoFor(stream: MediaStream): HTMLVideoElement {
    let v = this.videos.get(stream.id);
    if (v === undefined) {
      v = document.createElement("video");
      v.muted = true;
      v.playsInline = true;
      v.srcObject = stream;
      void v.play().catch(() => undefined);
      this.videos.set(stream.id, v);
    }
    return v;
  }

  /* Each voice joins the mix once, the first time it is drawn. */
  private hear(stream: MediaStream): void {
    if (this.mixed.has(stream.id) || stream.getAudioTracks().length === 0) return;
    const node = this.audio.createMediaStreamSource(stream);
    node.connect(this.mix);
    this.mixed.set(stream.id, node);
  }

  /* `video` into the box, covering it (cropped) or fitting it (bars). */
  private place(video: HTMLVideoElement, x: number, y: number, w: number, h: number, fit: "cover" | "contain"): void {
    const pick = fit === "cover" ? Math.max : Math.min;
    const scale = pick(w / video.videoWidth, h / video.videoHeight);
    const vw = video.videoWidth * scale;
    const vh = video.videoHeight * scale;
    this.ctx.drawImage(video, x + (w - vw) / 2, y + (h - vh) / 2, vw, vh);
  }

  private label(text: string, x: number, y: number): void {
    const ctx = this.ctx;
    ctx.font = "500 22px system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.textBaseline = "alphabetic";
    const lw = ctx.measureText(text).width + 20;
    ctx.fillStyle = "rgba(11, 13, 16, 0.75)";
    ctx.beginPath();
    ctx.roundRect(x, y - 34, lw, 34, 8);
    ctx.fill();
    ctx.fillStyle = "#ffffff";
    ctx.fillText(text, x + 10, y - 10);
  }

  private person(p: PodcastSource, x: number, y: number, w: number, h: number): void {
    const ctx = this.ctx;
    ctx.save();
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, 16);
    ctx.clip();
    ctx.fillStyle = "#1a1e24";
    ctx.fillRect(x, y, w, h);
    const track = p.stream?.getVideoTracks()[0];
    if (p.stream !== undefined) this.hear(p.stream);
    const video = p.stream !== undefined && track !== undefined && !track.muted ? this.videoFor(p.stream) : null;
    if (video !== null && video.videoWidth > 0) {
      this.place(video, x, y, w, h, "cover");
    } else {
      ctx.fillStyle = "#e8eaed";
      ctx.font = `600 ${Math.round(Math.min(56, h / 3))}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(initials(p.name), x + w / 2, y + h / 2);
    }
    ctx.restore();
    this.label(p.name, x + 12, y + h - 12);
  }

  private draw(): void {
    const ctx = this.ctx;
    const look = this.look();
    const people = this.sources();
    ctx.fillStyle = "#0b0d10";
    ctx.fillRect(0, 0, WIDTH, HEIGHT);

    const pad = 24;
    const bottom = look.title || look.subtitle ? 120 : pad;
    const screen = look.screen === null ? null : this.videoFor(look.screen.stream);
    if (look.screen !== null && screen !== null && screen.videoWidth > 0) {
      // The screen in the middle, the people in a column beside it.
      const side = 280;
      const sw = WIDTH - pad * 3 - side;
      const sh = HEIGHT - pad - bottom;
      ctx.fillStyle = "#000";
      ctx.fillRect(pad, pad, sw, sh);
      this.place(screen, pad, pad, sw, sh, "contain");
      this.label(look.screen.name, pad + 12, pad + sh - 12);
      const n = Math.max(1, people.length);
      const ph = Math.min((sh - pad * (n - 1)) / n, (side * 9) / 16);
      people.forEach((p, i) => this.person(p, pad * 2 + sw, pad + i * (ph + pad), side, ph));
    } else {
      const cols = columns(people.length);
      const rows = Math.max(1, Math.ceil(people.length / cols));
      const cellW = (WIDTH - pad * (cols + 1)) / cols;
      const cellH = (HEIGHT - pad - bottom - pad * (rows - 1)) / rows;
      people.forEach((p, i) => {
        this.person(p, pad + (i % cols) * (cellW + pad), pad + Math.floor(i / cols) * (cellH + pad), cellW, cellH);
      });
    }

    if (look.title || look.subtitle) {
      ctx.fillStyle = "#ffffff";
      ctx.textAlign = "left";
      ctx.textBaseline = "alphabetic";
      ctx.font = "700 40px system-ui, sans-serif";
      ctx.fillText(look.title, pad, HEIGHT - 66);
      ctx.fillStyle = "#b8bec7";
      ctx.font = "400 26px system-ui, sans-serif";
      ctx.fillText(look.subtitle, pad, HEIGHT - 28);
    }
    if (look.overlay !== null) ctx.drawImage(look.overlay, 0, 0, WIDTH, HEIGHT);
  }
}

function initials(name: string): string {
  return (
    name
      .replace(/^@/, "")
      .split(/[\s:._-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase())
      .join("") || "?"
  );
}

/* An RTMP address the box will send to. */
export function validRtmp(url: string): boolean {
  return /^rtmps?:\/\/[^\s/]+\/\S+$/.test(url.trim()) && url.length <= 1024;
}
