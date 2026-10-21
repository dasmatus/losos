import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { GameController03Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";

/* "Play while you wait": a small game offered during the box's long waits.
 *
 * A rebuild after Apply, LosOS Cloud starting while the wizard waits for
 * the first sign-in, LosOS Login starting, an app installing:
 * each of those screens calls this hook with whether its wait is on, and
 * places the two things it returns. `offer` is a ghost button for the
 * wait's own status row, there only while the wait is; `panel` is the game,
 * there from the press until the owner closes it, for under the status.
 * The game never covers the status and never starts by itself.
 *
 * The games are a chunk of their own (src/games/), fetched from the box's
 * /assets/ on the first press and never before, so the pages that wait
 * carry one button's worth of code. They ask the box for nothing, which
 * matters for the waits that come before anyone has signed in.
 *
 * When the wait ends the panel turns into a line saying so, which stays
 * until it is closed: a game that vanished mid-jump would read as a crash.
 * A screen whose wait surface leaves with the wait (the apply bar, the
 * wizard's waiting panel) keeps `panel` outside it, or keeps the surface
 * while `open`.
 *
 * A rebuild can swap the admin UI's files while the page is open, and the
 * chunk this page knows by name is then gone from the box. Pointing at the
 * button starts the fetch early for that reason, and a fetch that fails
 * anyway is a line with a Close button, never a broken page. */

const Arcade = React.lazy(() => import("@/games/arcade"));

/** Start fetching the chunk before the press lands. A fetch that fails
 *  here is said by the panel, if the owner presses Play at all. */
function preload(): void {
  import("@/games/arcade").catch(() => undefined);
}

export interface WaitGame {
  /** The Play button, or null. */
  offer: React.ReactNode;
  /** The game, or the line that says the wait is over, or null. */
  panel: React.ReactNode;
  /** The panel is on screen. */
  open: boolean;
}

export function useWaitGame(waiting: boolean): WaitGame {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  const label = t("games.offer");

  const offer =
    waiting && !open ? (
      <Button
        size="sm"
        variant="ghost"
        title={label}
        aria-label={label}
        data-testid="play-while-waiting"
        className="max-sm:size-8 max-sm:p-0"
        onPointerEnter={preload}
        onFocus={preload}
        onClick={() => setOpen(true)}
      >
        <HugeiconsIcon icon={GameController03Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <span className="max-sm:hidden">{label}</span>
      </Button>
    ) : null;

  const panel = open ? (
    <React.Suspense
      fallback={
        <div className="flex items-center gap-2 rounded-control border border-line bg-sunk p-3 text-[13px] text-muted">
          <Spinner size={16} label={t("games.loading")} />
          {t("games.loading")}
        </div>
      }
    >
      <Fallen message={t("games.failed")} close={t("games.close")} onClose={() => setOpen(false)}>
        <Arcade waiting={waiting} onClose={() => setOpen(false)} />
      </Fallen>
    </React.Suspense>
  ) : null;

  return { offer, panel, open };
}

/* What the panel says when the games could not be loaded or could not
 * start (no 2D canvas). */
class Fallen extends React.Component<
  { message: string; close: string; onClose: () => void; children: React.ReactNode },
  { failed: boolean }
> {
  override state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  override render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div
        role="status"
        className="flex items-center gap-3 rounded-control border border-line bg-surface px-3 py-2"
      >
        <p className="min-w-0 flex-1 text-[13px] text-muted">{this.props.message}</p>
        <Button size="sm" variant="secondary" onClick={this.props.onClose}>
          {this.props.close}
        </Button>
      </div>
    );
  }
}
