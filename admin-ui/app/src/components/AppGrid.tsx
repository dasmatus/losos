import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Rocket01Icon } from "@hugeicons/core-free-icons";
import { AppTile, AppTileSkeleton, staggerClass } from "@/components/AppTile";
import { buttonVariants } from "@/components/ui/button";
import type { AppGridModel } from "@/lib/apps";
import { SETUP_ROUTE } from "@/lib/apps";
import { cn } from "@/lib/utils";

/* The grid of app tiles.
 *
 * Presentation only: what belongs in it was decided by `deriveApps` in
 * lib/apps.ts, and this file never second-guesses that. In particular there is
 * no disabled tile and no greyed-out tile — an app that is not there is not
 * drawn, because a tile the owner can click and land nowhere is a lie they
 * only discover by trying it.
 *
 * Three shapes, in the order they are reached:
 *   measuring   placeholders where the tiles will be, for one round trip
 *   unfinished  the tiles that do exist, and the way to the rest
 *   settled     the grid
 */

export interface AppGridProps {
  model: AppGridModel;
  /** Where the first-run wizard lives in this SPA. */
  setupRoute?: string;
  className?: string;
}

export function AppGrid({ model, setupRoute = SETUP_ROUTE, className }: AppGridProps) {
  const measuring = model.pendingCount > 0;

  return (
    <div className={cn("flex flex-col gap-4", className)}>
      <ul
        aria-label="Apps on this box"
        aria-busy={measuring}
        className={cn(
          "grid list-none grid-cols-[repeat(auto-fill,minmax(78px,1fr))]",
          "m-0 gap-x-1 gap-y-4 p-0",
        )}
      >
        {/* Placeholders first: the apps still being measured are the ones at
            the front of the catalogue, so the settled grid does not reshuffle
            when they arrive. */}
        {Array.from({ length: model.pendingCount }, (_, index) => (
          <AppTileSkeleton key={`measuring-${String(index)}`} index={index} />
        ))}

        {model.tiles.map((tile, index) => (
          <AppTile key={tile.app.id} tile={tile} index={model.pendingCount + index} />
        ))}
      </ul>

      {measuring && (
        <p className="animate-fade-in text-[12.5px] text-faint" role="status">
          Looking for the apps on this box.
        </p>
      )}

      {model.notice !== null && (
        <p className="animate-fade-in text-[12.5px] text-muted">{model.notice}</p>
      )}

      {model.offerSetup && <SetupPrompt setupRoute={setupRoute} />}
    </div>
  );
}

/* The fresh-box state.
 *
 * Not an error and not an empty grid with an apology: on a box nobody has
 * finished setting up yet, this is the one thing there is to do, so it says
 * so and points at it. No count, no progress bar — this page has no way to
 * measure how far setup got, and inventing a step number would be worse than
 * saying nothing. */
function SetupPrompt({ setupRoute }: { setupRoute: string }) {
  return (
    <div
      className={cn(
        "flex flex-col items-start gap-3 rounded-card border border-dashed border-line",
        "bg-sunk/60 px-4 py-4 sm:flex-row sm:items-center sm:gap-4",
        "animate-rise",
        staggerClass(2),
      )}
    >
      <span className="flex size-10 shrink-0 items-center justify-center rounded-card bg-accent-wash text-accent">
        <HugeiconsIcon
          icon={Rocket01Icon}
          size={21}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
        />
      </span>

      <div className="min-w-0 flex-1">
        <p className="text-[13.5px] font-medium text-ink">This box is not set up yet</p>
        <p className="text-[12.5px] text-muted">
          Your files, photos, calendar and the rest appear here once you have finished. It takes a
          few minutes and you only do it once.
        </p>
      </div>

      {/* A link, not a button: it navigates. The button primitive renders a
          real <button> and does not take asChild, so it lends its classes
          rather than its element. */}
      <Link
        to={setupRoute}
        className={cn(buttonVariants({ variant: "primary", size: "sm" }), "shrink-0 no-underline")}
      >
        Set up this box
      </Link>
    </div>
  );
}
