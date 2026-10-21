import * as React from "react";
import type { MatrixClient } from "matrix-js-sdk";
import { cn } from "@/lib/utils";
import { mediaUrl } from "./media";
import { hue, initial } from "./model";

/* Eight tints for avatars without a picture, from the palette's own
 * colours so they hold up in both themes. Picked by the ID's hash, so a
 * person is the same colour everywhere on the page. */
const TINTS = [
  "bg-accent-wash text-accent",
  "bg-sunk text-ok",
  "bg-sunk text-warn",
  "bg-sunk text-crit",
  "bg-accent text-surface",
  "bg-ok text-surface",
  "bg-warn text-surface",
  "bg-ink text-surface",
] as const;

export interface AvatarProps {
  client: MatrixClient;
  id: string;
  name: string;
  mxc?: string | null | undefined;
  size?: number;
  className?: string;
}

/** A person's or room's picture, or their initial on a tint. */
export function Avatar({ client, id, name, mxc, size = 32, className }: AvatarProps) {
  const [url, setUrl] = React.useState<string | null>(null);
  React.useEffect(() => {
    setUrl(null);
    if (mxc === null || mxc === undefined || mxc === "") return;
    let live = true;
    mediaUrl(client, { url: mxc }, "image/png", Math.max(64, size * 2))
      .then((found) => {
        if (live) setUrl(found);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [client, mxc, size]);

  const box = cn(
    "inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full font-semibold select-none",
    size <= 24 ? "size-6 text-[11px]" : size <= 32 ? "size-8 text-[13px]" : size <= 40 ? "size-10 text-[15px]" : "size-16 text-[22px]",
    className,
  );
  if (url !== null) return <img src={url} alt="" className={cn(box, "object-cover")} />;
  return (
    <span aria-hidden className={cn(box, TINTS[hue(id)])}>
      {initial(name)}
    </span>
  );
}
