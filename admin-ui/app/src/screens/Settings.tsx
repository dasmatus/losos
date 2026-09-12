import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Alert02Icon, SquareLock01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";
import { ApplyBar } from "./settings/apply-bar";
import { AboutPane } from "./settings/pane-about";
import { AppsPane } from "./settings/pane-apps";
import { HardwarePane } from "./settings/pane-hardware";
import { MeshPane } from "./settings/pane-mesh";
import { NetworkPane } from "./settings/pane-network";
import { ResetPane } from "./settings/pane-reset";
import { SecurityPane } from "./settings/pane-security";
import { StoragePane } from "./settings/pane-storage";
import { DEFAULT_PANE, paneById, type SettingsPaneId } from "./settings/panes";
import { PaneHeader } from "./settings/rows";
import { Sidebar } from "./settings/sidebar";
import { useSettingsForm, type SettingsForm } from "./settings/use-settings-form";
import { useStorage, type Storage } from "./settings/use-storage";

/* Settings, in the macOS System Settings idiom: a recessed sidebar of panes
 * on the left, one pane of grouped rows on the right.
 *
 * ONE FORM, SEVEN PANES. /api/apply takes the whole of modules/overrides.nix,
 * not a patch, so every pane edits a slice of a single draft and Apply writes
 * all twelve keys at once. A pane that owned its own save would reset the
 * other eleven to their defaults on a box with no shell to fix it from. The
 * draft, the validation and the rebuild watch all live in useSettingsForm;
 * the panes below are views onto it and nothing more.
 *
 * The pane selection is uncontrolled by default and controllable by props, so
 * this screen works dropped straight into a route and also works under
 * `/settings/:pane` if the shell would rather own the URL. It deliberately
 * does not reach for the router itself — a screen that requires a
 * <BrowserRouter> above it cannot be rendered anywhere else, including a
 * test.
 */

export interface SettingsProps {
  /** Controlled selection. Omit to let this screen keep its own. */
  pane?: SettingsPaneId;
  /** Fires on every selection, controlled or not. Wire it to the URL. */
  onPaneChange?: (pane: SettingsPaneId) => void;
}

export default function Settings({ pane, onPaneChange }: SettingsProps) {
  const [ownPane, setOwnPane] = React.useState<SettingsPaneId>(DEFAULT_PANE);
  const current = pane ?? ownPane;

  const select = React.useCallback(
    (next: SettingsPaneId) => {
      setOwnPane(next);
      onPaneChange?.(next);
    },
    [onPaneChange],
  );

  const form = useSettingsForm();
  const storage = useStorage();
  const meta = paneById(current);

  return (
    <div className="flex gap-4 max-md:flex-col md:gap-5">
      <Sidebar current={current} onSelect={select} />

      <div className="min-w-0 flex-1">
        <PaneHeader title={meta.label} summary={meta.summary} />

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
    case "apps":
      return <AppsPane form={form} />;
    case "network":
      return <NetworkPane form={form} />;
    case "hardware":
      return <HardwarePane form={form} />;
    case "security":
      return <SecurityPane form={form} />;
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
  return (
    <Notice icon={SquareLock01Icon} tone="muted">
      Paste the admin key to change anything here. Nothing on this screen can be read or written
      without it.
    </Notice>
  );
}

function LoadErrorNotice({ message }: { message: string }) {
  return (
    <Notice icon={Alert02Icon} tone="crit">
      This box&apos;s settings could not be read: {message}
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
