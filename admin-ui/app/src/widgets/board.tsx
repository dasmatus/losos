/* The board: nothing, until the owner asks for something.
 *
 * Widgets are OFF by default. A fresh box shows an invitation and a button,
 * not a wall of tiles measuring things nobody asked to have measured — and
 * every tile that is not there is a timer that is not running against a
 * mini-PC that is also serving somebody's photos.
 *
 * The layout is two columns from md up, one below, with heatmap tiles
 * spanning both. Order is the owner's, kept in localStorage, moved with two
 * buttons rather than by dragging: drag needs a mouse and a steady hand, and
 * this page is as likely to be open on a phone as on a laptop.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { DashboardSquare01Icon, PlusSignIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import {
  getBoard,
  getServerBoard,
  MAX_WIDGETS,
  moveWidget,
  removeWidget,
  subscribeBoard,
} from "@/lib/widgets";
import { WidgetGallery } from "./gallery";
import { startUptimeProbe } from "./metrics";
import { WidgetTile } from "./tile";
import "./widgets.css";

export interface WidgetBoardProps {
  className?: string;
  /** Heading above the tiles. Pass null to drop it — the Overview screen has
   *  its own heading and does not want a second one. */
  heading?: string | null;
}

export function WidgetBoard({ className, heading = "Your board" }: WidgetBoardProps) {
  const board = React.useSyncExternalStore(subscribeBoard, getBoard, getServerBoard);
  const [galleryOpen, setGalleryOpen] = React.useState(false);

  /* The uptime heartbeat.
   *
   * Started here rather than in the app shell so that a browser that never
   * opens this page keeps no journal — a record of the box's availability is
   * a thing the owner opted into by adding the widget, not something the
   * admin page does behind them. Idempotent: calling it twice is one timer.
   * The shell may start it too; see widgets/index.ts. */
  React.useEffect(() => startUptimeProbe(), []);

  const widgets = board.widgets;

  return (
    <section className={cn("flex flex-col gap-4", className)}>
      {heading !== null && (
        <div className="flex items-center gap-3">
          <h2 className="text-[15px] font-semibold tracking-tight">{heading}</h2>
          <span className="numeric text-[12px] text-faint">
            {widgets.length} of {MAX_WIDGETS}
          </span>
          <div className="flex-1" />
          <Button variant="secondary" size="sm" onClick={() => setGalleryOpen(true)}>
            <HugeiconsIcon
              icon={PlusSignIcon}
              size={15}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            Add a widget
          </Button>
        </div>
      )}

      {widgets.length === 0 ? (
        <EmptyBoard onAdd={() => setGalleryOpen(true)} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {widgets.map((instance, index) => (
            <WidgetTile
              key={instance.id}
              instance={instance}
              index={index}
              total={widgets.length}
              onRemove={removeWidget}
              onMove={moveWidget}
            />
          ))}
        </div>
      )}

      <WidgetGallery open={galleryOpen} onOpenChange={setGalleryOpen} count={widgets.length} />
    </section>
  );
}

function EmptyBoard({ onAdd }: { onAdd: () => void }) {
  return (
    <Card>
      <CardHeader className="items-start">
        <span
          className={cn(
            "mb-1 flex size-10 items-center justify-center rounded-control",
            "bg-accent-wash text-accent",
          )}
        >
          <HugeiconsIcon
            icon={DashboardSquare01Icon}
            size={22}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
        </span>
        <CardTitle>Nothing on the board yet</CardTitle>
        <CardDescription>
          Add a widget to keep an eye on the things you care about: how much room is left, whether
          the box has been answering, which apps are up. They live in this browser and change
          nothing on the box.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Button onClick={onAdd}>
          <HugeiconsIcon
            icon={PlusSignIcon}
            size={16}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          Add a widget
        </Button>
      </CardContent>
    </Card>
  );
}
