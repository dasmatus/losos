import { cn } from "@/lib/utils";

/* How every floating part on Base UI (menus, tooltips) comes and goes, so a
 * tooltip, the person's menu and the pin menu move alike. It grows out of
 * the control that opened it (Base UI puts that point in
 * --transform-origin), drifts in a couple of pixels from that side, and
 * leaves faster than it came. Opacity, scale and translate only: those run
 * on the compositor, so a box with two slow cores draws them as smoothly as
 * a laptop. index.css turns all of it off under prefers-reduced-motion. */
export const popupMotion = cn(
  "origin-(--transform-origin) transition-[opacity,scale,translate] duration-150 ease-snap",
  "data-starting-style:scale-95 data-starting-style:opacity-0",
  "data-starting-style:data-[side=bottom]:-translate-y-1 data-starting-style:data-[side=top]:translate-y-1",
  "data-starting-style:data-[side=left]:translate-x-1 data-starting-style:data-[side=right]:-translate-x-1",
  "data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-ending-style:duration-100 data-ending-style:ease-exit",
);
