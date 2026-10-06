import * as React from "react";
import { BrowserRouter, NavLink, Route, Routes, useNavigate, useParams } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  HardDriveIcon,
  Home01Icon,
  LayoutGridIcon,
  Settings01Icon,
  Share08Icon,
  ViewIcon,
  ViewOffSlashIcon,
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
import { FieldError, Input, MonoInput } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { Spinner } from "@/components/ui/progress";
import { LanguagePicker } from "@/components/ui/language-picker";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { toast, Toaster } from "@/components/ui/toast";
import {
  dropToken,
  getClaimState,
  hasToken,
  isNotReady,
  isWellFormedToken,
  signIn,
  signInWithPassword,
  subscribeAuth,
  TOKEN_PATTERN,
} from "@/lib/api";
import { isSettingsPaneId, type SettingsPaneId } from "@/screens/settings/panes";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* Screens load on demand.
 *
 * The shell — gate, chrome, sign-in — is all a returning owner needs for the
 * first paint, and the unlock dialog is what most visits start on. The wizard
 * runs once in the life of a box and settings is five panes of forms; neither
 * should be parsed and compiled on the main thread before the page can show
 * anything. Each is its own chunk, fetched when its route is first reached. */
const Wizard = React.lazy(() => import("@/screens/Wizard"));
const Home = React.lazy(() => import("@/screens/Home"));
const Settings = React.lazy(() => import("@/screens/Settings"));
const WidgetBoard = React.lazy(() =>
  import("@/widgets/board").then((m) => ({ default: m.WidgetBoard })),
);

function ScreenFallback() {
  return (
    <div className="grid min-h-48 place-items-center text-muted">
      <Spinner />
    </div>
  );
}

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
  label: MessageKey;
  icon: IconSvgElement;
  end?: boolean;
}

const NAV: readonly NavItem[] = [
  { to: "/", label: "shell.nav.overview", icon: Home01Icon, end: true },
  { to: "/apps", label: "shell.nav.apps", icon: LayoutGridIcon },
  { to: "/storage", label: "shell.nav.storage", icon: HardDriveIcon },
  { to: "/mesh", label: "shell.nav.mesh", icon: Share08Icon },
  { to: "/settings", label: "shell.nav.settings", icon: Settings01Icon },
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
  const t = useT();

  /* Setup runs to the end once it has started, and this latch is what makes
   * that true.
   *
   * Without it the wizard tears itself down halfway. Step 2 claims the box,
   * which flips `claimed` AND stores the admin token, so the gate would
   * immediately read "open" and swap the wizard for the app — before the owner
   * has copied the spare admin key step 2 shows exactly once, and skipping the
   * sign-in check. The owner would never see the key, and would not know they
   * had not seen it.
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
  if (inSetup) {
    return (
      <React.Suspense
        fallback={
          <div className="grid min-h-dvh place-items-center bg-ground text-ink">
            <Spinner label="Loading setup" />
          </div>
        }
      >
        <Wizard onDone={() => setSetupDone(true)} />
        {/* The wizard has its own shell, so it gets its own stack: step 2's
            "ready" and "password set" and step 1's "copied" land here. */}
        <Toaster />
      </React.Suspense>
    );
  }

  if (gate === "unknown") {
    return (
      <div className="grid min-h-dvh place-items-center bg-ground text-ink">
        <Spinner />
        <span className="sr-only">{t("shell.asking")}</span>
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
          <React.Suspense fallback={<ScreenFallback />}>
          <Routes>
            <Route
              path="/"
              element={
                <Home
                  widgets={
                    <React.Suspense fallback={null}>
                      <WidgetBoard />
                    </React.Suspense>
                  }
                  showThemeSwitch={false}
                />
              }
            />
            <Route path="/storage" element={<SettingsRoute pane="storage" />} />
            <Route path="/mesh" element={<SettingsRoute pane="mesh" />} />
            <Route path="/apps" element={<SettingsRoute pane="apps" />} />
            <Route path="/settings" element={<SettingsRoute />} />
            <Route path="/settings/:pane" element={<SettingsRoute />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
          </React.Suspense>
        </main>
      </div>

      <SignInDialog open={!signedIn} />
      <Toaster />
    </div>
  );
}

// ── Chrome ────────────────────────────────────────────────────────────────

function TopBar({ signedIn }: { signedIn: boolean }) {
  const t = useT();
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-6xl items-center gap-3 px-4 py-3 sm:px-6">
        <span className="text-[15px] font-semibold tracking-tight">LosOS</span>
        <Badge variant="outline" className="hidden sm:inline-flex">
          {t("shell.thisBox")}
        </Badge>
        <div className="flex-1" />
        <LanguagePicker />
        <ThemeToggle />
        {signedIn && (
          <Button variant="ghost" size="sm" onClick={dropToken}>
            {t("shell.signOut")}
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
  const t = useT();
  return (
    <nav
      aria-label={t("shell.nav.label")}
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
              {t(item.label)}
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
  const t = useT();
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("shell.notFound.title")}</CardTitle>
        <CardDescription>{t("shell.notFound.body")}</CardDescription>
      </CardHeader>
    </Card>
  );
}

// ── Sign-in ───────────────────────────────────────────────────────────────

/* The gate.
 *
 * It asks for the owner's password — the one the wizard set, the one that
 * signs in to LosOS cloud — and lososd checks it by asking LosOS cloud
 * (`POST /api/sign-in`, backend/src/signin.rs). What comes back is the admin
 * token, held in sessionStorage: per-tab on purpose, and closing the tab
 * signs out. Before this the dialog asked for that token itself, a
 * 64-character key the box shows once; an owner who had not copied it was
 * locked out of these pages with a password that worked everywhere else.
 *
 * The key is still a way in, behind a link, because the password route has
 * one gap: it is LosOS cloud that checks the password, so while LosOS cloud is
 * not running (still starting, or broken) the password cannot be checked at
 * all. lososd says so with a 503, and the dialog then points at the link and
 * the printed sheet rather than at the password field.
 *
 * Non-dismissible — there is nothing to look at behind it. */
function SignInDialog({ open }: { open: boolean }) {
  const t = useT();
  const [mode, setMode] = React.useState<"password" | "key">("password");
  const [value, setValue] = React.useState("");
  const [visible, setVisible] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const errorId = React.useId();
  const titleId = React.useId();
  const hintId = React.useId();

  const switchTo = (next: "password" | "key"): void => {
    setMode(next);
    setValue("");
    setProblem(null);
  };

  const submit = async (event: React.FormEvent): Promise<void> => {
    event.preventDefault();
    const candidate = mode === "key" ? value.trim() : value;
    if (mode === "key" && !isWellFormedToken(candidate)) {
      setProblem(t("shell.signIn.badShape"));
      return;
    }
    setBusy(true);
    setProblem(null);
    try {
      const accepted =
        mode === "key" ? await signIn(candidate) : await signInWithPassword(candidate);
      if (accepted) {
        setValue("");
        toast.success(t("shell.signIn.unlocked"), t("shell.signIn.unlockedBody"));
      } else setProblem(t(mode === "key" ? "shell.signIn.rejected" : "shell.signIn.wrongPassword"));
    } catch (error) {
      // LosOS cloud is what checks the password, and it could not be asked.
      // Say so, and point at the spare key rather than at a retry.
      if (mode === "password" && isNotReady(error)) setProblem(t("shell.signIn.cloudDown"));
      else setProblem(t("shell.signIn.noAnswer"));
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
      {/* noValidate: `pattern` on the key field still documents the shape and
          drives :invalid, but the browser's own bubble would say "match the
          requested format" where this form can say what a key actually looks
          like. One message, ours. */}
      <form onSubmit={submit} noValidate>
        <DialogHeader>
          <DialogTitle id={titleId}>{t("shell.signIn.title")}</DialogTitle>
          <DialogDescription id={hintId}>
            {t(mode === "key" ? "shell.signIn.hint" : "shell.signIn.passwordHint")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-2">
          {mode === "password" ? (
            <>
              <Label htmlFor="owner-password">{t("shell.signIn.passwordLabel")}</Label>
              <div className="flex items-center gap-2">
                <Input
                  id="owner-password"
                  name="owner-password"
                  type={visible ? "text" : "password"}
                  autoComplete="current-password"
                  autoFocus
                  value={value}
                  disabled={busy}
                  aria-invalid={problem !== null}
                  aria-describedby={problem !== null ? errorId : undefined}
                  onChange={(event) => {
                    setValue(event.target.value);
                    setProblem(null);
                  }}
                />
                <Button
                  variant="secondary"
                  size="icon"
                  aria-pressed={visible}
                  aria-label={visible ? t("shell.signIn.hide") : t("shell.signIn.show")}
                  onClick={() => setVisible((shown) => !shown)}
                >
                  <HugeiconsIcon
                    icon={visible ? ViewOffSlashIcon : ViewIcon}
                    size={18}
                    strokeWidth={1.5}
                    color="currentColor"
                    aria-hidden="true"
                  />
                </Button>
              </div>
            </>
          ) : (
            <>
              <Label htmlFor="admin-key">{t("shell.signIn.label")}</Label>
              <MonoInput
                id="admin-key"
                name="admin-key"
                autoComplete="off"
                autoFocus
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
            </>
          )}
          <FieldError id={errorId}>{problem}</FieldError>
          <LabelHint>{t("shell.signIn.remembered")}</LabelHint>
          <button
            type="button"
            className="mt-1 self-start text-[13px] text-accent underline-offset-2 hover:underline"
            onClick={() => switchTo(mode === "key" ? "password" : "key")}
          >
            {t(mode === "key" ? "shell.signIn.usePassword" : "shell.signIn.useKey")}
          </button>
        </DialogBody>
        <DialogFooter>
          {busy && <Spinner label={t("shell.signIn.checking")} className="mr-auto text-muted" />}
          <Button type="submit" disabled={busy || value.trim().length === 0}>
            {t("shell.signIn.unlock")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
