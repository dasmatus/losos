/* The gallery: pick a widget, put it on the board.
 *
 * Windows 7's gadget gallery, which is the reference the owner of a mini-PC
 * in a hallway is most likely to have in their head — a grid of cards, each
 * with a picture, a name and a sentence, and a button that adds one. What it
 * is NOT is that gallery's chrome: this is a modal dialog on the platform's
 * own <dialog>, with a focus trap and an Escape key that both come from the
 * browser rather than from a library that would have injected a stylesheet
 * the appliance CSP then ignores.
 *
 * Adding a widget writes localStorage. It does not touch the box. Nothing in
 * here can start a rebuild, which is the reason the board is a browser
 * preference in the first place — see lib/widgets.ts.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { CheckmarkCircle02Icon, PlusSignIcon, PuzzleIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { toast } from "@/components/ui/toast";
import { cn } from "@/lib/utils";
import { addBuiltin, addCustom, hasBuiltin, MAX_WIDGETS, type BuiltinId } from "@/lib/widgets";
import { CATALOGUE } from "./catalogue";
import { CustomEditor } from "./custom-editor";
import type { WidgetSpec } from "./spec";

export interface GalleryProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Tiles already on the board — the dialog refuses to overfill it. */
  count: number;
}

export function WidgetGallery({ open, onOpenChange, count }: GalleryProps) {
  const [editing, setEditing] = React.useState(false);
  const titleId = React.useId();
  const hintId = React.useId();

  const full = count >= MAX_WIDGETS;

  const add = (id: BuiltinId, name: string): void => {
    if (addBuiltin(id) === null) {
      toast.error("The board is full.", `Remove a widget first. ${MAX_WIDGETS} is the most.`);
      return;
    }
    toast.success(`${name} added.`);
  };

  const save = (spec: WidgetSpec): boolean => {
    try {
      if (addCustom(spec) === null) {
        toast.error("The board is full.", `Remove a widget first. ${MAX_WIDGETS} is the most.`);
        return false;
      }
    } catch (error) {
      toast.error("That widget could not be added.", error instanceof Error ? error.message : "");
      return false;
    }
    toast.success(`${spec.title} added.`);
    setEditing(false);
    onOpenChange(false);
    return true;
  };

  return (
    <>
      <Dialog
        open={open && !editing}
        onOpenChange={onOpenChange}
        labelledBy={titleId}
        describedBy={hintId}
        dialogClassName="w-[min(44rem,calc(100vw-2rem))] max-w-[44rem]"
      >
        <DialogHeader>
          <DialogTitle id={titleId}>Add a widget</DialogTitle>
          <DialogDescription id={hintId}>
            Widgets live in this browser. Adding one changes nothing on the box and starts nothing
            running.
          </DialogDescription>
        </DialogHeader>

        <DialogBody>
          <ul className="grid gap-3 sm:grid-cols-2">
            {CATALOGUE.map((entry) => {
              const already = hasBuiltin(entry.id);
              return (
                <li key={entry.id}>
                  <div
                    className={cn(
                      "flex h-full flex-col gap-2 rounded-card border border-line bg-surface p-4",
                      "transition-colors duration-150 hover:border-accent",
                    )}
                  >
                    <div className="flex items-center gap-2.5">
                      <span
                        className={cn(
                          "flex size-9 shrink-0 items-center justify-center rounded-control",
                          "bg-accent-wash text-accent",
                        )}
                      >
                        <HugeiconsIcon
                          icon={entry.icon}
                          size={21}
                          strokeWidth={1.5}
                          color="currentColor"
                          aria-hidden="true"
                        />
                      </span>
                      <span className="text-[14px] font-semibold">{entry.name}</span>
                    </div>

                    <p className="flex-1 text-[12.5px] leading-snug text-muted">{entry.blurb}</p>

                    <Button
                      variant={already ? "ghost" : "secondary"}
                      size="sm"
                      className="self-start"
                      disabled={full}
                      onClick={() => add(entry.id, entry.name)}
                    >
                      <HugeiconsIcon
                        icon={already ? CheckmarkCircle02Icon : PlusSignIcon}
                        size={15}
                        strokeWidth={1.5}
                        color="currentColor"
                        aria-hidden="true"
                      />
                      {already ? "Add another" : "Add"}
                    </Button>
                  </div>
                </li>
              );
            })}

            <li>
              <div
                className={cn(
                  "flex h-full flex-col gap-2 rounded-card border border-dashed border-line p-4",
                  "transition-colors duration-150 hover:border-accent",
                )}
              >
                <div className="flex items-center gap-2.5">
                  <span
                    className={cn(
                      "flex size-9 shrink-0 items-center justify-center rounded-control",
                      "hatched text-accent",
                    )}
                  >
                    <HugeiconsIcon
                      icon={PuzzleIcon}
                      size={21}
                      strokeWidth={1.5}
                      color="currentColor"
                      aria-hidden="true"
                    />
                  </span>
                  <span className="text-[14px] font-semibold">Custom</span>
                </div>

                <p className="flex-1 text-[12.5px] leading-snug text-muted">
                  Build your own from one of the readings this box publishes. Pick a shape, write
                  what to show, and see it before you keep it.
                </p>

                <Button
                  variant="outline"
                  size="sm"
                  className="self-start"
                  disabled={full}
                  onClick={() => setEditing(true)}
                >
                  Build one
                </Button>
              </div>
            </li>
          </ul>
        </DialogBody>

        <DialogFooter>
          {full && (
            <p className="mr-auto text-[12px] text-warn">
              The board holds {MAX_WIDGETS}. Remove one to add another.
            </p>
          )}
          <Button variant="secondary" onClick={() => onOpenChange(false)}>
            Done
          </Button>
        </DialogFooter>
      </Dialog>

      <CustomEditor
        open={open && editing}
        onOpenChange={(next) => {
          setEditing(next);
          if (!next && !open) onOpenChange(false);
        }}
        onSave={save}
      />
    </>
  );
}
