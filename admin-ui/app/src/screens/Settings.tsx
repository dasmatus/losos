import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Alert02Icon, SquareLock01Icon } from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { ApplyBar } from "./settings/apply-bar";
import { AboutPane } from "./settings/pane-about";
import { AdvancedPane } from "./settings/pane-advanced";
import { AppsPane } from "./settings/pane-apps";
import { HardwarePane } from "./settings/pane-hardware";
import { HistoryPane } from "./settings/pane-history";
import { LookPane } from "./settings/pane-look";
import { MarketPane } from "./settings/pane-market";
import { MeshPane } from "./settings/pane-mesh";
import { NetworkPane } from "./settings/pane-network";
import { ResetPane } from "./settings/pane-reset";
import { SecurityPane } from "./settings/pane-security";
import { StoragePane } from "./settings/pane-storage";
import { DEFAULT_PANE, paneById, type SettingsPaneId } from "./settings/panes";
import { PaneHeader } from "./settings/rows";
import { useSettingsForm, type SettingsForm } from "./settings/use-settings-form";
import { useStorage, type Storage } from "./settings/use-storage";

/* Settings: one pane of grouped rows. Which pane is the page's single
 * sidebar's business (components/app-sidebar.tsx), which lists every pane
 * among the page's other destinations; this screen shows the one chosen.
 *
 * ONE FORM, SEVEN PANES (and the Look pane, which is the exception below).
 * /api/apply takes the whole of modules/overrides.nix,
 * not a patch, so every pane edits a slice of a single draft and Apply writes
 * all twelve keys at once. A pane that owned its own save would reset the
 * other eleven to their defaults on a box with no shell to fix it from. The
 * draft, the validation and the rebuild watch all live in useSettingsForm;
 * the panes below are views onto it and nothing more. The Look pane is the
 * one that does not: a background picture and the hand-written widgets live
 * in lososd's look document (lib/look.ts), saved the moment they are picked
 * and never rebuilt, so it draws no Apply bar and touches no draft.
 *
 * The pane comes in as a prop; the shell reads it from the address. This
 * screen deliberately does not reach for the router itself: a screen that
 * requires a <BrowserRouter> above it cannot be rendered anywhere else,
 * including a test.
 */

export interface SettingsProps {
  /** The pane to show. The address decides it (lib/routes.ts). */
  pane?: SettingsPaneId;
}

export default function Settings({ pane = DEFAULT_PANE }: SettingsProps) {
  const t = useT();
  // A planned pane is never shown, whatever asks for it: the sidebar draws
  // its entry disabled and lib/routes.ts does not parse its segment, and
  // this is the belt to those braces.
  const current = paneById(pane).planned ? DEFAULT_PANE : pane;

  const form = useSettingsForm();
  const storage = useStorage();
  const meta = paneById(current);

  return (
    <div className="flex gap-4 max-md:flex-col md:gap-5">
      <div className="min-w-0 flex-1">
        <PaneHeader title={t(meta.labelKey)} summary={t(meta.summaryKey)} />

        {form.locked && <LockedNotice />}
        {form.loadError !== null && <LoadErrorNotice message={form.loadError} />}

        {/* Keyed on the pane id so React remounts the subtree and the
            crossfade replays. `animate-fade-in` is disabled wholesale by the
            stylesheet's prefers-reduced-motion block — there is no per-
            component guard to forget here. */}
        <div key={current} className="animate-fade-in">
          <Pane id={current} form={form} storage={storage} />
        </div>

        <ApplyBar form={form} />
      </div>
    </div>
  );
}

function Pane({
  id,
  form,
  storage,
}: {
  id: SettingsPaneId;
  form: SettingsForm;
  storage: Storage;
}) {
  switch (id) {
    case "storage":
      return <StoragePane form={form} storage={storage} />;
    case "mesh":
      return <MeshPane form={form} />;
    case "market":
      // Unreachable while the pane is planned (panes.ts): its row is
      // disabled, `select` refuses it and the router does not parse its
      // segment. Kept so the pane stays compiled and type-checked, and so
      // opening it is one flag away.
      return <MarketPane form={form} />;
    case "apps":
      return <AppsPane form={form} />;
    case "network":
      return <NetworkPane form={form} />;
    case "look":
      // No draft, no Apply bar: the look is saved the moment it is picked.
      return <LookPane locked={form.locked} />;
    case "hardware":
      return <HardwarePane form={form} />;
    case "security":
      return <SecurityPane form={form} />;
    case "advanced":
      return <AdvancedPane form={form} />;
    case "history":
      // No draft, no Apply bar: it reads the repository and syncs on request.
      return <HistoryPane locked={form.locked} />;
    case "about":
      return <AboutPane form={form} />;
    case "reset":
      return <ResetPane form={form} />;
    default: {
      // A new pane id must land here as a build error, not as a blank page.
      const exhaustive: never = id;
      return exhaustive;
    }
  }
}

// ── Notices ───────────────────────────────────────────────────────────────

/* The shell puts its own unlock dialog over the page when the token is gone,
 * so this is what is behind it — and what remains after a 401 mid-session if
 * the dialog is ever made dismissible. Every control in every pane is already
 * disabled on `form.locked`; this says why. */
function LockedNotice() {
  const t = useT();
  return (
    <Notice icon={SquareLock01Icon} tone="muted">
      {t("settings.notice.locked")}
    </Notice>
  );
}

function LoadErrorNotice({ message }: { message: string }) {
  const t = useT();
  return (
    <Notice icon={Alert02Icon} tone="crit">
      {t("settings.notice.loadError", { message })}
    </Notice>
  );
}

function Notice({
  icon,
  tone,
  children,
}: {
  icon: IconSvgElement;
  tone: "muted" | "crit";
  children: React.ReactNode;
}) {
  return (
    <div
      role={tone === "crit" ? "alert" : undefined}
      className={cn(
        "animate-fade-in mb-4 flex items-start gap-2.5 rounded-card border p-3",
        "text-[13px] leading-snug",
        tone === "crit" ? "border-crit/35 bg-crit/8 text-crit" : "border-line bg-sunk text-muted",
      )}
    >
      <HugeiconsIcon
        icon={icon}
        size={17}
        strokeWidth={1.5}
        color="currentColor"
        className="mt-px shrink-0"
        aria-hidden="true"
      />
      <p className="min-w-0 flex-1">{children}</p>
    </div>
  );
}
