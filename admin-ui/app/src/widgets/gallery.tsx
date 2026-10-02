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
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { addBuiltin, addCustom, hasBuiltin, MAX_WIDGETS, type BuiltinId } from "@/lib/widgets";
import { CATALOGUE } from "./catalogue";
import type { WidgetSpec } from "./spec";

const CustomEditor = React.lazy(() =>
  import("./custom-editor").then((m) => ({ default: m.CustomEditor })),
);

export interface GalleryProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Tiles already on the board — the dialog refuses to overfill it. */
  count: number;
}

export function WidgetGallery({ open, onOpenChange, count }: GalleryProps) {
  const t = useT();
  const [editing, setEditing] = React.useState(false);
  const titleId = React.useId();
  const hintId = React.useId();

  const full = count >= MAX_WIDGETS;

  const add = (id: BuiltinId, name: string): void => {
    if (addBuiltin(id) === null) {
      toast.error(t("widgets.gallery.full"), t("widgets.gallery.fullHint", { max: MAX_WIDGETS }));
      return;
    }
    toast.success(t("widgets.gallery.added", { name }));
  };

  const save = (spec: WidgetSpec): boolean => {
    try {
      if (addCustom(spec) === null) {
        toast.error(t("widgets.gallery.full"), t("widgets.gallery.fullHint", { max: MAX_WIDGETS }));
        return false;
      }
    } catch (error) {
      toast.error(t("widgets.gallery.notAdded"), error instanceof Error ? error.message : "");
      return false;
    }
    toast.success(t("widgets.gallery.added", { name: spec.title }));
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
          <DialogTitle id={titleId}>{t("widgets.gallery.title")}</DialogTitle>
          <DialogDescription id={hintId}>{t("widgets.gallery.description")}</DialogDescription>
        </DialogHeader>

        <DialogBody>
          <ul className="grid gap-3 sm:grid-cols-2">
            {CATALOGUE.map((entry) => {
              const already = hasBuiltin(entry.id);
              const name = t(entry.name);
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
                      <span className="text-[14px] font-semibold">{name}</span>
                    </div>

                    <p className="flex-1 text-[12.5px] leading-snug text-muted">{t(entry.blurb)}</p>

                    <Button
                      variant={already ? "ghost" : "secondary"}
                      size="sm"
                      className="self-start"
                      disabled={full}
                      onClick={() => add(entry.id, name)}
                    >
                      <HugeiconsIcon
                        icon={already ? CheckmarkCircle02Icon : PlusSignIcon}
                        size={15}
                        strokeWidth={1.5}
                        color="currentColor"
                        aria-hidden="true"
                      />
                      {already ? t("widgets.gallery.addAnother") : t("widgets.gallery.add")}
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
                  <span className="text-[14px] font-semibold">{t("widgets.gallery.custom")}</span>
                </div>

                <p className="flex-1 text-[12.5px] leading-snug text-muted">
                  {t("widgets.gallery.customBlurb")}
                </p>

                <Button
                  variant="outline"
                  size="sm"
                  className="self-start"
                  disabled={full}
                  onClick={() => setEditing(true)}
                >
                  {t("widgets.gallery.buildOne")}
                </Button>
              </div>
            </li>
          </ul>
        </DialogBody>

        <DialogFooter>
          {full && (
            <p className="mr-auto text-[12px] text-warn">
              {t("widgets.gallery.holds", { max: MAX_WIDGETS })}
            </p>
          )}
          <Button variant="secondary" onClick={() => onOpenChange(false)}>
            {t("widgets.gallery.done")}
          </Button>
        </DialogFooter>
      </Dialog>

      {editing && (
        <React.Suspense fallback={null}>
          <CustomEditor
            open={open && editing}
            onOpenChange={(next) => {
              setEditing(next);
              if (!next && !open) onOpenChange(false);
            }}
            onSave={save}
          />
        </React.Suspense>
      )}
    </>
  );
}
