/* Spatial focus for a remote.
 *
 * A television has no pointer. The arrows have to land on a neighbour the
 * eye already groups with the current tile: the one to its right, not the
 * one that happens to start a few pixels higher. `nextFocus` is that
 * choice and nothing else, so it can be checked without a browser.
 *
 * A candidate counts only when its centre lies strictly past the current
 * centre in the arrow's direction. Among those, the score is the distance
 * along the arrow plus twice the distance off the arrow, so a tile in
 * line beats a nearer tile in the corner. No candidate means the focus
 * stays: the ring does not wrap around the screen.
 */

export type TvDirection = "left" | "right" | "up" | "down";

export interface TvRect {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export function directionOf(key: string): TvDirection | null {
  if (key === "ArrowLeft") return "left";
  if (key === "ArrowRight") return "right";
  if (key === "ArrowUp") return "up";
  if (key === "ArrowDown") return "down";
  return null;
}

function center(rect: TvRect): { x: number; y: number } {
  return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
}

/** The id to focus next, or null to leave the ring where it is. */
export function nextFocus(
  items: readonly TvRect[],
  currentId: string,
  direction: TvDirection,
): string | null {
  const current = items.find((item) => item.id === currentId);
  if (current === undefined) return items[0]?.id ?? null;
  const from = center(current);
  let best: { id: string; score: number } | null = null;
  for (const item of items) {
    if (item.id === currentId) continue;
    const to = center(item);
    const dx = to.x - from.x;
    const dy = to.y - from.y;
    const primary =
      direction === "left" ? -dx : direction === "right" ? dx : direction === "up" ? -dy : dy;
    if (primary <= 0) continue;
    const secondary = direction === "left" || direction === "right" ? Math.abs(dy) : Math.abs(dx);
    const score = primary + secondary * 2;
    if (best === null || score < best.score) best = { id: item.id, score };
  }
  return best?.id ?? null;
}
