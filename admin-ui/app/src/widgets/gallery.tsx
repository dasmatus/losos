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
import {
  CheckmarkCircle02Icon,
  Delete02Icon,
  PencilEdit02Icon,
  PlusSignIcon,
  PuzzleIcon,
  SourceCodeIcon,
} from "@hugeicons/core-free-icons";
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
import {
  Item,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import {
  getLookState,
  getServerLookState,
  removeHandWidget,
  subscribeLook,
  type HandWidget,
} from "@/lib/look";
import {
  addBuiltin,
  addCustom,
  addHand,
  hasBuiltin,
  hasHand,
  MAX_WIDGETS,
  removeHandTiles,
  type BuiltinId,
} from "@/lib/widgets";
import { CATALOGUE } from "./catalogue";
import type { WidgetSpec } from "./spec";

const CustomEditor = React.lazy(() =>
  import("./custom-editor").then((m) => ({ default: m.CustomEditor })),
);
const HandEditor = React.lazy(() =>
  import("./hand-editor").then((m) => ({ default: m.HandEditor })),
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
  /* The hand-written editor: null closed, {} writing a new one, or the
   * widget being edited. A separate dialog from the spec editor because
   * they are different things that happen to end on the same board. */
  const [writing, setWriting] = React.useState<{ widget?: HandWidget } | null>(null);
  const look = React.useSyncExternalStore(subscribeLook, getLookState, getServerLookState);
  const titleId = React.useId();
  const hintId = React.useId();

  const full = count >= MAX_WIDGETS;
  const hand = look.look?.widgets ?? [];

  const addHandWidget = (widget: HandWidget): void => {
    if (addHand(widget.id) === null) {
      toast.error(t("widgets.gallery.full"), t("widgets.gallery.fullHint", { max: MAX_WIDGETS }), { help: "look-and-widgets" });
      return;
    }
    toast.success(t("widgets.gallery.added", { name: widget.name }));
  };

  const deleteHandWidget = async (widget: HandWidget): Promise<void> => {
    try {
      await removeHandWidget(widget.id);
      removeHandTiles(widget.id);
      toast.success(t("look.widgets.deleted", { name: widget.name }));
    } catch (error) {
      toast.error(
        t("look.widgets.notDeleted", { name: widget.name }),
        error instanceof Error ? error.message : "",
        { help: "look-and-widgets" },
      );
    }
  };

  const add = (id: BuiltinId, name: string): void => {
    if (addBuiltin(id) === null) {
      toast.error(t("widgets.gallery.full"), t("widgets.gallery.fullHint", { max: MAX_WIDGETS }), { help: "look-and-widgets" });
      return;
    }
    toast.success(t("widgets.gallery.added", { name }));
  };

  const save = (spec: WidgetSpec): boolean => {
    try {
      if (addCustom(spec) === null) {
        toast.error(t("widgets.gallery.full"), t("widgets.gallery.fullHint", { max: MAX_WIDGETS }), { help: "look-and-widgets" });
        return false;
      }
    } catch (error) {
      toast.error(t("widgets.gallery.notAdded"), error instanceof Error ? error.message : "", { help: "look-and-widgets" });
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
        open={open && !editing && writing === null}
        // Handing over to an editor closes this dialog, and a native <dialog>
        // reports every close the same way; that one is not the owner leaving.
        onOpenChange={(next) => {
          if (!next && (editing || writing !== null)) return;
          onOpenChange(next);
        }}
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
                <li key={entry.id} className="flex">
                  {/* A shadcn Item: media, title, a sentence, one action. */}
                  <Item variant="outline" className="h-full content-start">
                    <ItemMedia variant="icon">
                      <HugeiconsIcon
                        icon={entry.icon}
                        strokeWidth={1.5}
                        color="currentColor"
                        aria-hidden="true"
                      />
                    </ItemMedia>
                    <ItemContent>
                      <ItemTitle>{name}</ItemTitle>
                      <ItemDescription>{t(entry.blurb)}</ItemDescription>
                    </ItemContent>
                    <ItemFooter className="justify-start">
                      <Button
                        variant={already ? "ghost" : "secondary"}
                        size="sm"
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
                    </ItemFooter>
                  </Item>
                </li>
              );
            })}

            <li className="flex">
              <Item variant="dashed" className="h-full content-start">
                <ItemMedia variant="hatched">
                  <HugeiconsIcon
                    icon={PuzzleIcon}
                    strokeWidth={1.5}
                    color="currentColor"
                    aria-hidden="true"
                  />
                </ItemMedia>
                <ItemContent>
                  <ItemTitle>{t("widgets.gallery.custom")}</ItemTitle>
                  <ItemDescription>{t("widgets.gallery.customBlurb")}</ItemDescription>
                </ItemContent>
                <ItemFooter className="justify-start">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={full}
                    onClick={() => setEditing(true)}
                  >
                    {t("widgets.gallery.buildOne")}
                  </Button>
                </ItemFooter>
              </Item>
            </li>

            <li className="flex">
              <Item variant="dashed" className="h-full content-start">
                <ItemMedia variant="hatched">
                  <HugeiconsIcon
                    icon={SourceCodeIcon}
                    strokeWidth={1.5}
                    color="currentColor"
                    aria-hidden="true"
                  />
                </ItemMedia>
                <ItemContent>
                  <ItemTitle>{t("look.gallery.hand")}</ItemTitle>
                  <ItemDescription>{t("look.gallery.handBlurb")}</ItemDescription>
                </ItemContent>
                <ItemFooter className="justify-start">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={full || look.look === null}
                    onClick={() => setWriting({})}
                  >
                    {t("look.gallery.writeOne")}
                  </Button>
                </ItemFooter>
              </Item>
            </li>
          </ul>

          {/* The widgets the box keeps, written by hand on it: each one can
              go on the board, be edited, or be deleted from the box. */}
          {hand.length > 0 && (
            <section className="mt-5" aria-labelledby={`${titleId}-hand`}>
              <h3 id={`${titleId}-hand`} className="mb-2 text-[12.5px] font-medium text-muted">
                {t("look.gallery.section")}
              </h3>
              <ul className="grid gap-3 sm:grid-cols-2">
                {hand.map((widget) => {
                  const already = hasHand(widget.id);
                  return (
                    <li key={widget.id} className="flex">
                      <Item variant="outline" className="h-full content-start">
                        <ItemMedia variant="icon">
                          <HugeiconsIcon
                            icon={SourceCodeIcon}
                            strokeWidth={1.5}
                            color="currentColor"
                            aria-hidden="true"
                          />
                        </ItemMedia>
                        <ItemContent>
                          <ItemTitle>{widget.name}</ItemTitle>
                          <ItemDescription>
                            {t(widget.span === "full" ? "look.widgets.full" : "look.widgets.half")}
                          </ItemDescription>
                        </ItemContent>
                        <ItemFooter className="justify-start gap-1">
                          <Button
                            variant={already ? "ghost" : "secondary"}
                            size="sm"
                            disabled={full}
                            onClick={() => addHandWidget(widget)}
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
                          <Button
                            variant="ghost"
                            size="icon"
                            className="size-8"
                            aria-label={t("look.widgets.edit", { name: widget.name })}
                            title={t("look.widgets.edit", { name: widget.name })}
                            onClick={() => setWriting({ widget })}
                          >
                            <HugeiconsIcon
                              icon={PencilEdit02Icon}
                              size={16}
                              strokeWidth={1.5}
                              color="currentColor"
                              aria-hidden="true"
                            />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            className="size-8"
                            aria-label={t("look.widgets.delete", { name: widget.name })}
                            title={t("look.widgets.delete", { name: widget.name })}
                            onClick={() => void deleteHandWidget(widget)}
                          >
                            <HugeiconsIcon
                              icon={Delete02Icon}
                              size={16}
                              strokeWidth={1.5}
                              color="currentColor"
                              aria-hidden="true"
                            />
                          </Button>
                        </ItemFooter>
                      </Item>
                    </li>
                  );
                })}
              </ul>
            </section>
          )}
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

      {writing !== null && (
        <React.Suspense fallback={null}>
          <HandEditor
            open={open}
            onOpenChange={(next) => {
              if (!next) setWriting(null);
            }}
            {...(writing.widget === undefined ? {} : { widget: writing.widget })}
            onSaved={(widget, created) => {
              if (created) {
                if (addHand(widget.id) === null) {
                  toast.error(
                    t("widgets.gallery.full"),
                    t("widgets.gallery.fullHint", { max: MAX_WIDGETS }),
                    { help: "look-and-widgets" },
                  );
                  return;
                }
                onOpenChange(false);
              }
            }}
          />
        </React.Suspense>
      )}

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
