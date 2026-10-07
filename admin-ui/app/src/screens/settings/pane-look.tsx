import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  Delete02Icon,
  ImageUpload01Icon,
  PencilEdit02Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import {
  checkPicture,
  chooseBackground,
  getLookState,
  getServerLookState,
  IMAGE_TYPES,
  loadLook,
  lookLimits,
  removeBackground,
  removeHandWidget,
  saveVeil,
  SHIPPED_BACKGROUNDS,
  shippedUrl,
  subscribeLook,
  uploadBackground,
  type HandWidget,
  type LookBackground,
} from "@/lib/look";
import { cn } from "@/lib/utils";
import { addHand, removeHandTiles } from "@/lib/widgets";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";

/* The Look pane: what sits behind the page, and the widgets written by hand.
 *
 * The one pane with no Apply bar. Everything here is lososd's look document
 * (lib/look.ts): a picture, the veil over it, and the hand-written widgets.
 * Each is saved the moment it is picked and takes effect at once, in this
 * browser and every other that opens the box, because none of it is a
 * NixOS option — nothing rebuilds, nothing restarts, and nothing on this
 * pane can reach the one operation on this box that can fail with nobody
 * able to log in and fix it.
 *
 * House style, as on every pane: no emoji, nothing names a runtime, and the
 * cost of a choice is said on the row. The picker is a row of thumbnails
 * rather than a list of names because a picture is the one setting better
 * seen than read.
 */

const HandEditor = React.lazy(() =>
  import("@/widgets/hand-editor").then((m) => ({ default: m.HandEditor })),
);

const SHIPPED_LABEL: Record<string, MessageKey> = {
  tide: "look.background.tide",
  grid: "look.background.grid",
  dusk: "look.background.dusk",
};

export function LookPane({ locked }: { locked: boolean }) {
  const t = useT();
  const state = React.useSyncExternalStore(subscribeLook, getLookState, getServerLookState);
  const look = state.look;
  const limits = lookLimits();
  const disabled = locked || look === null;

  const [uploading, setUploading] = React.useState(false);
  const [editing, setEditing] = React.useState<{ widget?: HandWidget } | null>(null);
  const fileRef = React.useRef<HTMLInputElement>(null);
  const veilId = React.useId();

  const mb = Math.floor(limits.imageBytes / (1024 * 1024));
  const shipped = look?.shipped ?? SHIPPED_BACKGROUNDS;
  const background: LookBackground = look?.background ?? { kind: "none" };

  const pick = async (choice: { kind: "none" } | { kind: "shipped"; name: string }): Promise<void> => {
    try {
      await chooseBackground(choice);
      toast.success(t(choice.kind === "none" ? "look.toast.pictureRemoved" : "look.toast.pictureSet"));
    } catch (error) {
      toast.error(t("look.toast.notSaved"), error instanceof Error ? error.message : "");
    }
  };

  const upload = async (file: File): Promise<void> => {
    const fault = checkPicture(file);
    if (fault === "type") {
      toast.error(t("look.toast.badType"));
      return;
    }
    if (fault === "size") {
      toast.error(t("look.toast.tooBig", { mb }));
      return;
    }
    setUploading(true);
    try {
      await uploadBackground(file);
      toast.success(t("look.toast.pictureSet"));
    } catch (error) {
      toast.error(t("look.toast.notSaved"), error instanceof Error ? error.message : "");
    } finally {
      setUploading(false);
    }
  };

  const remove = async (): Promise<void> => {
    try {
      await removeBackground();
      toast.success(t("look.toast.pictureRemoved"));
    } catch (error) {
      toast.error(t("look.toast.notSaved"), error instanceof Error ? error.message : "");
    }
  };

  const deleteWidget = async (widget: HandWidget): Promise<void> => {
    try {
      await removeHandWidget(widget.id);
      removeHandTiles(widget.id);
      toast.success(t("look.widgets.deleted", { name: widget.name }));
    } catch (error) {
      toast.error(
        t("look.widgets.notDeleted", { name: widget.name }),
        error instanceof Error ? error.message : "",
      );
    }
  };

  return (
    <>
      {state.status === "failed" && (
        <div
          role="alert"
          className={cn(
            "mb-4 flex items-start gap-2.5 rounded-card border border-crit/35 bg-crit/8 p-3",
            "text-[13px] leading-snug text-crit",
          )}
        >
          <HugeiconsIcon
            icon={Alert02Icon}
            size={17}
            strokeWidth={1.5}
            color="currentColor"
            className="mt-px shrink-0"
            aria-hidden="true"
          />
          <p className="min-w-0 flex-1">{t("look.notLoaded", { message: state.error ?? "" })}</p>
          <Button variant="secondary" size="sm" onClick={() => void loadLook()}>
            {t("look.tryAgain")}
          </Button>
        </div>
      )}

      <PaneSection>
        <GroupTitle>{t("look.background.group")}</GroupTitle>
        <Group>
          <StackRow>
            <div
              role="radiogroup"
              aria-label={t("look.background.group")}
              className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 md:grid-cols-5"
            >
              <Swatch
                label={t("look.background.none")}
                selected={background.kind === "none"}
                disabled={disabled}
                onClick={() => void pick({ kind: "none" })}
              >
                <div className="size-full bg-ground" />
              </Swatch>
              {shipped.map((name) => (
                <Swatch
                  key={name}
                  label={SHIPPED_LABEL[name] === undefined ? name : t(SHIPPED_LABEL[name])}
                  selected={background.kind === "shipped" && background.name === name}
                  disabled={disabled}
                  onClick={() => void pick({ kind: "shipped", name })}
                >
                  <img src={shippedUrl(name)} alt="" className="size-full object-cover" />
                </Swatch>
              ))}
              <Swatch
                label={t("look.background.own")}
                selected={background.kind === "upload"}
                disabled={disabled || uploading}
                onClick={() => fileRef.current?.click()}
              >
                {background.kind === "upload" ? (
                  <img src={background.url} alt="" className="size-full object-cover" />
                ) : (
                  <div className="grid size-full place-items-center bg-sunk text-muted">
                    <HugeiconsIcon
                      icon={ImageUpload01Icon}
                      size={22}
                      strokeWidth={1.5}
                      color="currentColor"
                      aria-hidden="true"
                    />
                  </div>
                )}
              </Swatch>
            </div>
            <input
              ref={fileRef}
              type="file"
              accept={IMAGE_TYPES.join(",")}
              className="sr-only"
              tabIndex={-1}
              aria-hidden="true"
              data-testid="background-file"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                if (file !== undefined) void upload(file);
              }}
            />
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                disabled={disabled || uploading}
                onClick={() => fileRef.current?.click()}
              >
                <HugeiconsIcon
                  icon={ImageUpload01Icon}
                  size={15}
                  strokeWidth={1.5}
                  color="currentColor"
                  aria-hidden="true"
                />
                {t(background.kind === "upload" ? "look.background.replace" : "look.background.choose")}
              </Button>
              {background.kind !== "none" && (
                <Button variant="ghost" size="sm" disabled={disabled} onClick={() => void remove()}>
                  {t("look.background.remove")}
                </Button>
              )}
              {uploading && (
                <Spinner label={t("look.background.uploading")} className="text-muted" />
              )}
            </div>
          </StackRow>
          <Row last>
            <RowText
              htmlFor={veilId}
              title={t("look.veil.title")}
              detail={t("look.veil.detail")}
            />
            <div className="flex shrink-0 items-center gap-3">
              <input
                id={veilId}
                type="range"
                min={limits.veil.min}
                max={limits.veil.max}
                step={5}
                value={look?.veil ?? 60}
                disabled={disabled || background.kind === "none"}
                aria-label={t("look.veil.label", { percent: look?.veil ?? 60 })}
                className="w-32 accent-accent"
                onChange={(event) => {
                  const next = Number(event.target.value);
                  saveVeil(next)
                    .then(() => toast.done(t("look.toast.veilSaved")))
                    .catch((error: unknown) =>
                      toast.error(
                        t("look.toast.notSaved"),
                        error instanceof Error ? error.message : "",
                      ),
                    );
                }}
              />
              <span className="numeric w-10 text-right text-[13px] text-muted tabular-nums">
                {look?.veil ?? 60}%
              </span>
            </div>
          </Row>
        </Group>
        <GroupCaption>{t("look.background.caption", { mb })}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>
          {t("look.widgets.group")}
          {look !== null && (
            <span className="numeric ml-2 text-faint">
              {t("look.widgets.count", { count: look.widgets.length, max: limits.widgets })}
            </span>
          )}
        </GroupTitle>
        <Group>
          {(look?.widgets.length ?? 0) === 0 && (
            <Row>
              <RowText title={<span className="text-muted">{t("look.widgets.empty")}</span>} />
            </Row>
          )}
          {look?.widgets.map((widget) => (
            <Row key={widget.id}>
              <RowText
                title={widget.name}
                detail={t(widget.span === "full" ? "look.widgets.full" : "look.widgets.half")}
              />
              <div className="flex shrink-0 items-center gap-0.5">
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  disabled={disabled}
                  aria-label={t("look.widgets.edit", { name: widget.name })}
                  title={t("look.widgets.edit", { name: widget.name })}
                  onClick={() => setEditing({ widget })}
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
                  disabled={disabled}
                  aria-label={t("look.widgets.delete", { name: widget.name })}
                  title={t("look.widgets.delete", { name: widget.name })}
                  onClick={() => void deleteWidget(widget)}
                >
                  <HugeiconsIcon
                    icon={Delete02Icon}
                    size={16}
                    strokeWidth={1.5}
                    color="currentColor"
                    aria-hidden="true"
                  />
                </Button>
              </div>
            </Row>
          ))}
          <Row last>
            <div className="flex-1" />
            <Button
              variant="secondary"
              size="sm"
              disabled={disabled || (look !== null && look.widgets.length >= limits.widgets)}
              onClick={() => setEditing({})}
            >
              <HugeiconsIcon
                icon={PlusSignIcon}
                size={15}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("look.widgets.write")}
            </Button>
          </Row>
        </Group>
        <GroupCaption>{t("look.widgets.caption")}</GroupCaption>
      </PaneSection>

      {editing !== null && (
        <React.Suspense fallback={null}>
          <HandEditor
            open
            onOpenChange={(next) => {
              if (!next) setEditing(null);
            }}
            {...(editing.widget === undefined ? {} : { widget: editing.widget })}
            onSaved={(widget, created) => {
              if (created) addHand(widget.id);
            }}
          />
        </React.Suspense>
      )}
    </>
  );
}

/* One choice in the picker: a thumbnail with its name under it, selected
 * state drawn as an accent ring. A button with role="radio", so the row
 * reads as one choice among several to a screen reader and takes the arrow
 * keys the way a radio group does in the browsers that give it that. */
function Swatch({
  label,
  selected,
  disabled,
  onClick,
  children,
}: {
  label: string;
  selected: boolean;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "group flex flex-col gap-1.5 rounded-card text-left",
        "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
        "disabled:cursor-not-allowed disabled:opacity-50",
      )}
    >
      <span
        className={cn(
          "block aspect-[16/10] w-full overflow-hidden rounded-control border",
          selected ? "border-accent ring-2 ring-accent/40" : "border-line group-hover:border-muted/60",
        )}
      >
        {children}
      </span>
      <span className={cn("text-[12.5px] leading-snug", selected ? "text-ink" : "text-muted")}>
        {label}
      </span>
    </button>
  );
}
