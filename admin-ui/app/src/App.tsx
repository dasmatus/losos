import * as React from "react";
import { BrowserRouter, NavLink, Route, Routes, useNavigate, useParams } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  HardDriveIcon,
  Home01Icon,
  LayoutGridIcon,
  Settings01Icon,
  Share08Icon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FieldError, MonoInput } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { Spinner } from "@/components/ui/progress";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { Toaster } from "@/components/ui/toast";
import {
  dropToken,
  getClaimState,
  hasToken,
  isWellFormedToken,
  signIn,
  subscribeAuth,
  TOKEN_PATTERN,
} from "@/lib/api";
import Wizard from "@/screens/Wizard";
import Home from "@/screens/Home";
import Settings from "@/screens/Settings";
import { isSettingsPaneId, type SettingsPaneId } from "@/screens/settings/panes";
import { WidgetBoard } from "@/widgets";
import { cn } from "@/lib/utils";

/* The shell: sign-in gate, navigation, routes.
 *
 * Routes are real paths, not hashes, which needs `try_files $uri /index.html`
 * on the admin location in modules/containers.nix so a deep link survives a
 * reload.
 *
 * Three of the five nav entries are shortcuts into a settings pane rather than
 * screens of their own: storage, mesh and apps are what an owner actually opens
 * this page for, so they get a top-level address, while network, hardware,
 * about and reset live under /settings/<pane>. `paneHref` is the single place
 * that mapping exists, so the sidebar and the URL cannot disagree about where a
 * pane lives.
 *
 * House rules that hold everywhere below this line: no emoji (icons are
 * @hugeicons, Stroke Rounded, 1.5), no inline style attributes (the CSP
 * refuses them), and nothing a user reads may name a container runtime — it
 * is "an app", and it "runs on this box" or "on the mesh". */

interface NavItem {
  to: string;
  label: string;
  icon: IconSvgElement;
  end?: boolean;
}

const NAV: readonly NavItem[] = [
  { to: "/", label: "Overview", icon: Home01Icon, end: true },
  { to: "/apps", label: "Apps", icon: LayoutGridIcon },
  { to: "/storage", label: "Storage", icon: HardDriveIcon },
  { to: "/mesh", label: "Mesh", icon: Share08Icon },
  { to: "/settings", label: "Settings", icon: Settings01Icon },
];

export default function App() {
  return (
    <BrowserRouter>
      <Shell />
    </BrowserRouter>
  );
}

/* Which of the three things this page can be.
 *
 * "unknown" is a real state and not a loading spinner's excuse: until lososd
 * has answered, showing either the wizard or the key prompt would be a guess,
 * and both guesses are bad. A returning owner flashed the setup wizard thinks
 * the box has been wiped. */
type Gate = "unknown" | "setup" | "signin" | "open";

function useGate(): Gate {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const [claimed, setClaimed] = React.useState<boolean | null>(null);

  React.useEffect(() => {
    const controller = new AbortController();
    let live = true;
    void (async () => {
      try {
        const state = await getClaimState({ signal: controller.signal });
        if (live) setClaimed(state.claimed);
      } catch {
        // An unreachable or older daemon is not an unclaimed box. Fall back to
        // the key prompt, which is wrong for a fresh appliance but harmless on
        // one that is merely offline — whereas opening setup on a box that is
        // actually owned is an invitation.
        if (live) setClaimed(true);
      }
    })();
    return () => {
      live = false;
      controller.abort();
    };
  }, []);

  if (signedIn) return "open";
  if (claimed === null) return "unknown";
  return claimed ? "signin" : "setup";
}

function Shell() {
  const gate = useGate();

  /* Setup runs to the end once it has started, and this latch is what makes
   * that true.
   *
   * Without it the wizard tears itself down halfway. Step 2 claims the box,
   * which flips `claimed` AND stores the admin token, so the gate would
   * immediately read "open" and swap the wizard for the app — skipping step 3,
   * the recovery code, which is the one thing in this whole product that has to
   * leave the box. The owner would never see it, and would not know they had
   * not seen it.
   *
   * So the gate decides whether setup STARTS; only the wizard decides when it
   * is over. There is no route that reaches around this: every path renders the
   * wizard while it is up, so a deep link to /settings on an unclaimed box gets
   * setup too, not a half-configured settings page. */
  const [setupStarted, setSetupStarted] = React.useState(false);
  const [setupDone, setSetupDone] = React.useState(false);
  React.useEffect(() => {
    if (gate === "setup") setSetupStarted(true);
  }, [gate]);

  const inSetup = setupStarted && !setupDone;
  const signedIn = gate === "open";

  // A box being set up gets the wizard and nothing else: no chrome, no
  // navigation, no dialog over the top. There is nothing behind it worth
  // showing yet, and it is a sequence the owner should not be able to wander
  // out of halfway.
  if (inSetup) return <Wizard onDone={() => setSetupDone(true)} />;

  if (gate === "unknown") {
    return (
      <div className="grid min-h-dvh place-items-center bg-ground text-ink">
        <Spinner />
        <span className="sr-only">Asking this box whether it has been set up.</span>
      </div>
    );
  }

  return (
    <div className="min-h-dvh bg-ground text-ink">
      <TopBar signedIn={signedIn} />

      <div
        className={cn(
          "mx-auto flex w-full max-w-6xl px-4 pt-5 pb-12 sm:px-6",
          "max-md:flex-col max-md:gap-3 md:gap-6",
        )}
      >
        <SectionNav />
        <main className="min-w-0 flex-1">
          <Routes>
            <Route path="/" element={<Home widgets={<WidgetBoard />} showThemeSwitch={false} />} />
            <Route path="/storage" element={<SettingsRoute pane="storage" />} />
            <Route path="/mesh" element={<SettingsRoute pane="mesh" />} />
            <Route path="/apps" element={<SettingsRoute pane="apps" />} />
            <Route path="/settings" element={<SettingsRoute />} />
            <Route path="/settings/:pane" element={<SettingsRoute />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </main>
      </div>

      <SignInDialog open={!signedIn} />
      <Toaster />
    </div>
  );
}

// ── Chrome ────────────────────────────────────────────────────────────────

function TopBar({ signedIn }: { signedIn: boolean }) {
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-6xl items-center gap-3 px-4 py-3 sm:px-6">
        <span className="text-[15px] font-semibold tracking-tight">LosOS</span>
        <Badge variant="outline" className="hidden sm:inline-flex">
          this box
        </Badge>
        <div className="flex-1" />
        <ThemeToggle />
        {signedIn && (
          <Button variant="ghost" size="sm" onClick={dropToken}>
            Sign out
          </Button>
        )}
      </div>
    </header>
  );
}

/* One nav, two shapes: a vertical rail beside the content from md up, and a
 * horizontally scrolling row above it on a phone. Not two lists and not a
 * hamburger — five destinations fit on a narrow screen, and the appliance is
 * as likely to be configured from a phone on the same network as from a
 * laptop. */
function SectionNav() {
  return (
    <nav
      aria-label="Sections"
      className={cn(
        "shrink-0",
        "max-md:-mx-4 max-md:overflow-x-auto max-md:px-4 max-md:pb-2 sm:max-md:-mx-6 sm:max-md:px-6",
        "md:w-44",
      )}
    >
      <ul
        className={cn(
          "flex gap-1",
          "max-md:w-max",
          "md:sticky md:top-20 md:flex-col md:gap-0.5",
        )}
      >
        {NAV.map((item) => (
          <li key={item.to}>
            <NavLink
              to={item.to}
              end={item.end === true}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2.5 rounded-control px-3 py-2 text-[13.5px] whitespace-nowrap",
                  "transition-colors duration-150",
                  isActive
                    ? "bg-accent-wash font-medium text-accent"
                    : "text-muted hover:bg-sunk hover:text-ink",
                )
              }
            >
              <HugeiconsIcon
                icon={item.icon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {item.label}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}

// ── Placeholders ──────────────────────────────────────────────────────────

/* Delete these as the real screens land. Kept deliberately plain: they exist
 * to prove the routing and the palette, not to suggest a layout. */
/* The one place a settings pane's address is decided.
 *
 * Storage, mesh and apps are what an owner opens this page for, so they get a
 * top-level address; network, hardware, about and reset sit under /settings.
 * Both the sidebar and the router read this, so they cannot disagree about
 * where a pane lives, and a deep link to any of them survives a reload because
 * nginx serves index.html for unknown paths under the admin location.
 */
const TOP_LEVEL_PANES: readonly SettingsPaneId[] = ["storage", "mesh", "apps"];

function paneHref(pane: SettingsPaneId): string {
  return TOP_LEVEL_PANES.includes(pane) ? `/${pane}` : `/settings/${pane}`;
}

function SettingsRoute({ pane }: { pane?: SettingsPaneId }) {
  const navigate = useNavigate();
  const params = useParams();
  // A pane named in the URL wins over the route's own default, so
  // /settings/hardware opens hardware rather than the first pane.
  const fromUrl = params["pane"];
  const selected =
    pane ?? (typeof fromUrl === "string" && isSettingsPaneId(fromUrl) ? fromUrl : undefined);

  return <Settings pane={selected} onPaneChange={(next) => navigate(paneHref(next))} />;
}

function NotFound() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Nothing here</CardTitle>
        <CardDescription>
          That address does not match any section of this box&apos;s admin page.
        </CardDescription>
      </CardHeader>
    </Card>
  );
}

// ── Sign-in ───────────────────────────────────────────────────────────────

/* The admin token gate.
 *
 * lososd mints a 64-hex-character token on first start and keeps it at
 * /var/secrets/losos-admin-token, mode 0600. There is no SSH and no shell
 * login, so the owner reads it off the box itself. It is held in
 * sessionStorage: per-tab on purpose, and closing the tab signs out.
 *
 * Non-dismissible — there is nothing to look at behind it. */
function SignInDialog({ open }: { open: boolean }) {
  const [value, setValue] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const errorId = React.useId();
  const titleId = React.useId();
  const hintId = React.useId();

  const submit = async (event: React.FormEvent): Promise<void> => {
    event.preventDefault();
    const candidate = value.trim();
    if (!isWellFormedToken(candidate)) {
      setProblem("An admin key is 64 characters, digits and the letters a to f.");
      return;
    }
    setBusy(true);
    setProblem(null);
    try {
      if (await signIn(candidate)) setValue("");
      else setProblem("That key was not accepted.");
    } catch {
      setProblem("This box did not answer. Check that it is switched on.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={() => undefined}
      dismissible={false}
      labelledBy={titleId}
      describedBy={hintId}
    >
      {/* noValidate: `pattern` below still documents the shape and drives
          :invalid, but the browser's own bubble would say "match the requested
          format" where this form can say what an admin key actually looks
          like. One message, ours. */}
      <form onSubmit={submit} noValidate>
        <DialogHeader>
          <DialogTitle id={titleId}>Unlock this box</DialogTitle>
          <DialogDescription id={hintId}>
            Paste the admin key. It is printed on the box and never leaves it.
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-2">
          <Label htmlFor="admin-key">Admin key</Label>
          <MonoInput
            id="admin-key"
            name="admin-key"
            autoComplete="off"
            pattern={TOKEN_PATTERN.source}
            value={value}
            disabled={busy}
            aria-invalid={problem !== null}
            aria-describedby={problem !== null ? errorId : undefined}
            onChange={(event) => {
              setValue(event.target.value);
              setProblem(null);
            }}
          />
          <FieldError id={errorId}>{problem}</FieldError>
          <LabelHint>The key is remembered until this tab is closed.</LabelHint>
        </DialogBody>
        <DialogFooter>
          {busy && <Spinner label="Checking the key" className="mr-auto text-muted" />}
          <Button type="submit" disabled={busy || value.trim().length === 0}>
            Unlock
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
