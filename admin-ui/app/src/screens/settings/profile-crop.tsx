import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { FieldError } from "@/components/ui/field";
import type { ProfileImage } from "@/lib/api";
import { useT } from "@/lib/i18n-react";

/* Pick the part of a photo that becomes a profile picture or a banner.
 *
 * The photo is decoded with createImageBitmap, which turns it upright by its
 * own orientation tag and needs no URL: the admin page's `img-src 'self'
 * data:` has no `blob:`, and a canvas needs none. Dragging moves the photo,
 * the slider or the + and - keys zoom, the arrow keys nudge. What leaves is
 * a PNG of the chosen part at the stored size (smaller when the photo is),
 * drawn fresh on a canvas, so no camera tag goes with it; lososd crops,
 * scales and encodes again anyway (backend/src/profiles.rs). */

/** The stored size of each kind, as lososd keeps it. */
export const IMAGE_SIZE: Record<ProfileImage, { width: number; height: number }> = {
  picture: { width: 512, height: 512 },
  banner: { width: 1500, height: 500 },
};

/** What the page offers to the file picker; lososd reads the type off the
 *  bytes either way. */
export const PROFILE_IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

const MAX_ZOOM = 4;

interface View {
  zoom: number;
  /** The photo's middle, in photo pixels, at the frame's middle. */
  cx: number;
  cy: number;
}

/** The scale at which the photo just covers a frame of `w` by `h`. */
function coverScale(bitmap: ImageBitmap, w: number, h: number): number {
  return Math.max(w / bitmap.width, h / bitmap.height);
}

/** Keep the frame inside the photo. */
function clampView(bitmap: ImageBitmap, w: number, h: number, view: View): View {
  const scale = coverScale(bitmap, w, h) * view.zoom;
  const halfW = w / scale / 2;
  const halfH = h / scale / 2;
  return {
    zoom: view.zoom,
    cx: Math.min(Math.max(view.cx, halfW), bitmap.width - halfW),
    cy: Math.min(Math.max(view.cy, halfH), bitmap.height - halfH),
  };
}

/** The part of the photo the frame shows, in photo pixels. */
function sourceRect(bitmap: ImageBitmap, w: number, h: number, view: View) {
  const scale = coverScale(bitmap, w, h) * view.zoom;
  const sw = w / scale;
  const sh = h / scale;
  return { sx: view.cx - sw / 2, sy: view.cy - sh / 2, sw, sh };
}

export function ProfileCropDialog({
  kind,
  file,
  busy,
  onCancel,
  onDone,
}: {
  kind: ProfileImage;
  /** The photo to crop; null keeps the dialog closed. */
  file: File | null;
  busy: boolean;
  onCancel: () => void;
  onDone: (picture: Blob) => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const zoomId = React.useId();
  const canvasRef = React.useRef<HTMLCanvasElement>(null);
  const [bitmap, setBitmap] = React.useState<ImageBitmap | null>(null);
  const [failed, setFailed] = React.useState(false);
  const [view, setView] = React.useState<View>({ zoom: 1, cx: 0, cy: 0 });
  const drag = React.useRef<{ x: number; y: number; view: View } | null>(null);
  const round = kind === "picture";
  // The frame on screen, in CSS pixels.
  const frame = round ? { w: 288, h: 288 } : { w: 480, h: 160 };

  React.useEffect(() => {
    setBitmap(null);
    setFailed(false);
    if (file === null) return;
    let live = true;
    createImageBitmap(file)
      .then((decoded) => {
        if (!live) {
          decoded.close();
          return;
        }
        setBitmap(decoded);
        setView({ zoom: 1, cx: decoded.width / 2, cy: decoded.height / 2 });
      })
      .catch(() => {
        if (live) setFailed(true);
      });
    return () => {
      live = false;
    };
  }, [file]);

  React.useEffect(() => () => bitmap?.close(), [bitmap]);

  // Paint the frame: the chosen part, and for a picture the circle it will
  // be shown in, with what falls outside dimmed.
  React.useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null || bitmap === null) return;
    const ratio = window.devicePixelRatio || 1;
    const width = Math.round(canvas.clientWidth * ratio);
    const height = Math.round(canvas.clientHeight * ratio);
    if (width === 0 || height === 0) return;
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (ctx === null) return;
    const { sx, sy, sw, sh } = sourceRect(bitmap, frame.w, frame.h, view);
    ctx.imageSmoothingQuality = "high";
    ctx.clearRect(0, 0, width, height);
    ctx.drawImage(bitmap, sx, sy, sw, sh, 0, 0, width, height);
    if (round) {
      ctx.fillStyle = "rgb(0 0 0 / 0.45)";
      ctx.beginPath();
      ctx.rect(0, 0, width, height);
      ctx.arc(width / 2, height / 2, Math.min(width, height) / 2, 0, Math.PI * 2);
      ctx.fill("evenodd");
    }
  }, [bitmap, view, round, frame.w, frame.h]);

  const update = (next: View) => {
    if (bitmap !== null) setView(clampView(bitmap, frame.w, frame.h, next));
  };

  const finish = () => {
    if (bitmap === null) return;
    const { sx, sy, sw, sh } = sourceRect(bitmap, frame.w, frame.h, view);
    const size = IMAGE_SIZE[kind];
    // Never larger than the part of the photo it comes from.
    const width = Math.max(1, Math.round(Math.min(size.width, sw)));
    const height = Math.max(1, Math.round((width * size.height) / size.width));
    const out = document.createElement("canvas");
    out.width = width;
    out.height = height;
    const ctx = out.getContext("2d");
    if (ctx === null) return;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(bitmap, sx, sy, sw, sh, 0, 0, width, height);
    out.toBlob((blob) => {
      if (blob !== null) onDone(blob);
    }, "image/png");
  };

  return (
    <Dialog
      open={file !== null}
      onOpenChange={(open) => {
        if (!open && !busy) onCancel();
      }}
      dismissible={!busy}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t(`profile.crop.title.${kind}`)}</DialogTitle>
        <DialogDescription id={bodyId}>{t("profile.crop.body")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col items-center gap-4">
        {failed ? (
          <FieldError className="self-stretch" data-testid="profile-crop-failed">
            {t("profile.crop.unreadable")}
          </FieldError>
        ) : (
          <canvas
            ref={canvasRef}
            data-testid="profile-crop-canvas"
            tabIndex={0}
            role="img"
            aria-label={t("profile.crop.canvas")}
            className={
              round
                ? "aspect-square w-72 max-w-full cursor-grab touch-none rounded-card bg-sunk active:cursor-grabbing"
                : "aspect-[3/1] w-[30rem] max-w-full cursor-grab touch-none rounded-card bg-sunk active:cursor-grabbing"
            }
            onPointerDown={(event) => {
              event.currentTarget.setPointerCapture(event.pointerId);
              drag.current = { x: event.clientX, y: event.clientY, view };
            }}
            onPointerMove={(event) => {
              const start = drag.current;
              if (start === null || bitmap === null) return;
              const shown = event.currentTarget.clientWidth / frame.w;
              const scale = coverScale(bitmap, frame.w, frame.h) * start.view.zoom * shown;
              update({
                zoom: start.view.zoom,
                cx: start.view.cx - (event.clientX - start.x) / scale,
                cy: start.view.cy - (event.clientY - start.y) / scale,
              });
            }}
            onPointerUp={() => {
              drag.current = null;
            }}
            onPointerCancel={() => {
              drag.current = null;
            }}
            onWheel={(event) => {
              update({ ...view, zoom: Math.min(MAX_ZOOM, Math.max(1, view.zoom * (event.deltaY < 0 ? 1.1 : 1 / 1.1))) });
            }}
            onKeyDown={(event) => {
              if (bitmap === null) return;
              const step = 24 / (coverScale(bitmap, frame.w, frame.h) * view.zoom);
              const moves: Record<string, [number, number]> = {
                ArrowLeft: [-step, 0],
                ArrowRight: [step, 0],
                ArrowUp: [0, -step],
                ArrowDown: [0, step],
              };
              const move = moves[event.key];
              if (move !== undefined) {
                event.preventDefault();
                update({ ...view, cx: view.cx + move[0], cy: view.cy + move[1] });
              } else if (event.key === "+" || event.key === "=") {
                update({ ...view, zoom: Math.min(MAX_ZOOM, view.zoom + 0.25) });
              } else if (event.key === "-") {
                update({ ...view, zoom: Math.max(1, view.zoom - 0.25) });
              }
            }}
          />
        )}
        <div className="flex w-full items-center gap-3">
          <label htmlFor={zoomId} className="text-[13px] text-muted">
            {t("profile.crop.zoom")}
          </label>
          <input
            id={zoomId}
            type="range"
            min={1}
            max={MAX_ZOOM}
            step={0.05}
            value={view.zoom}
            disabled={bitmap === null || busy}
            data-testid="profile-crop-zoom"
            className="min-w-0 flex-1 accent-accent"
            onChange={(event) => update({ ...view, zoom: Number(event.target.value) })}
          />
        </div>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
          {t("profile.crop.cancel")}
        </Button>
        <Button
          size="sm"
          data-testid="profile-crop-save"
          disabled={bitmap === null || busy}
          onClick={finish}
        >
          {t(busy ? "profile.crop.saving" : "profile.crop.save")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
