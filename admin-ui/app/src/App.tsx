import * as React from "react";
import { ReportDialogHost } from "@/components/report-dialog";
import { BrowserRouter, Link, Route, Routes, useLocation } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Compass01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { AppBar, type AppBarPerson } from "@/components/app-bar";
import { AppSidebar } from "@/components/app-sidebar";
import { MeshNotice } from "@/components/mesh-notice";
import { PersonAvatar } from "@/components/person";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Field, FieldDescription, FieldError, FieldLabel } from "@/components/ui/field";
import { HelpLink } from "@/components/ui/help-link";
import type { HandbookEntry } from "@/lib/handbook";
import { Input, MonoInput } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ContextMenuHost, usePageMenu } from "@/components/context-menu";
import { BottomNavHost } from "@/components/ui/bottom-nav";
import { SidebarProvider } from "@/components/ui/sidebar";
import { toast, Toaster } from "@/components/ui/toast";
import {
  getClaimState,
  hasToken,
  isWellFormedToken,
  loadMe,
  makeFirstAccount,
  signIn,
  signInPageReady,
  sentToSignIn,
  sharedApp,
  signInUrl,
  signOut,
  subscribeAuth,
  TOKEN_PATTERN,
  useMe,
} from "@/lib/api";
import { followAuth } from "@/lib/look";
import { followPins } from "@/lib/pins";
import { followPages, getPagesState, subscribePages } from "@/pages/store";
import { readProgress } from "@/screens/wizard/progress";
import { isTourOpen, startTour, subscribeTour } from "@/tour/store";
import { closePalette, isPaletteColon, isPaletteShortcut, openPalette, paletteState, subscribePalette } from "@/palette/store";
import { seededShuffle } from "@/lib/shuffle";
import type { SettingsPaneId } from "@/screens/settings/panes";
import { paneFromPath } from "@/lib/routes";
import { useT } from "@/lib/i18n-react";
import { useWaitGame } from "@/components/play-while-waiting";
import { cn } from "@/lib/utils";
import { chatDocument } from "@/chat/document";

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
const PageEditorDock = React.lazy(() =>
  import("@/pages/editor").then((m) => ({ default: m.PageEditorDock })),
);
/* Documents (losos.office.folder) and the editor, on boxes that edit a
 * folder of documents. */
const Office = React.lazy(() => import("@/screens/Office"));
const AppScreen = React.lazy(() => import("@/apps/screens").then((m) => ({ default: m.AppScreen })));
const AppEditorScreen = React.lazy(() =>
  import("@/apps/screens").then((m) => ({ default: m.AppEditorScreen })),
);
/* The first-run tour (src/tour/): its own chunk, fetched only when it opens,
 * which on most visits is never. */
const Tour = React.lazy(() => import("@/tour/tour"));
/* The command palette (src/palette/): its own chunk, fetched the first time
 * Ctrl/Cmd+Shift+P, a colon or the sidebar's search button opens it. */
const CommandPalette = React.lazy(() => import("@/palette/command-palette"));
/* LosOS Mail (src/mail/): only on the shared side, so most boxes never
 * fetch it. */
const Mail = React.lazy(() => import("@/mail/app"));
/* LosOS Wiki (src/wikis/): the box's directory of wikis and the editor of
 * LosOS Git's spaces. */
const Wikis = React.lazy(() => import("@/wikis/app"));
/* LosOS Contacts (src/contacts/): kept in chat's homeserver, so on the
 * shared side only, like Mail and Chat. */
const Contacts = React.lazy(() => import("@/contacts/app"));
/* LosOS Calendar (src/calendar/): LosOS Cloud's calendars, with
 * subscriptions and invitations. Its reminders are a small chunk of their
 * own the shell holds, so they show on every screen. */
const Calendar = React.lazy(() => import("@/calendar/app"));
const Reminders = React.lazy(() => import("@/calendar/reminders"));
/* LosOS Chat (src/chat/), the same: on the shared side only, and the only
 * screen that brings matrix-js-sdk and its crypto. */
const Chat = React.lazy(() => import("@/chat/app"));
/* Another box asks who the person is, for its LosOS Git (src/chat/vouch.tsx). */
const ChatVouch = React.lazy(() => import("@/chat/vouch"));
/* Calls outlive the chat screen, so the shell holds them, on a document
 * loaded at /chat/ only: elsewhere matrix-js-sdk stays out of the page. */
const CallsShell = React.lazy(() => import("@/chat/calls-shell"));
/* Mesh CI (src/ci/): on the shared side too, beside LosOS Git. */
const Ci = React.lazy(() => import("@/ci/app"));
/* LosOS Diagrams (src/lab/): its own chunk with the Rust core's
 * WebAssembly, fetched when /diagrams is first opened. */
const Diagrams = React.lazy(() => import("@/lab/app"));
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
 * Every destination is in one sidebar (components/app-sidebar.tsx, shadcn's
 * Sidebar): Overview, then the settings panes, with storage, mesh and apps at
 * a top-level address because they are what an owner opens this page for,
 * and network, hardware, security, about and reset under /settings/<pane>.
 * lib/routes.ts is the single place that mapping exists, so the sidebar and
 * the URL cannot disagree about where a pane lives.
 *
 * House rules that hold everywhere below this line: no emoji (icons are
 * @hugeicons, Stroke Rounded, 1.5), no inline style attributes (the CSP
 * refuses them), and nothing a user reads may name a container runtime — it
 * is "an app", and it "runs on this box" or "on the mesh". */

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

/* Whether anyone can sign in by name yet; false on a box set up before
 * Authelia, whose owner makes the first account with the spare key. */
let peopleOnBox = true;

function useGate(): Gate {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const [claimed, setClaimed] = React.useState<boolean | null>(null);
  // An Authelia session is a cookie the page cannot see; GET /api/me is how
  // it finds out. Until that answers, the page does not know who is here.
  const [asked, setAsked] = React.useState(false);

  React.useEffect(() => {
    const controller = new AbortController();
    let live = true;
    void (async () => {
      try {
        await loadMe({ signal: controller.signal });
      } catch {
        /* no answer: the claim state below decides what to show */
      }
      if (live) setAsked(true);
    })();
    void (async () => {
      try {
        const state = await getClaimState({ signal: controller.signal });
        if (live) {
          peopleOnBox = state.people !== false;
          setClaimed(state.claimed);
        }
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
  if (claimed === null || (claimed && !asked)) return "unknown";
  return claimed ? "signin" : "setup";
}

function CallsHolder({ children }: { children: React.ReactNode }) {
  return chatDocument() ? <CallsShell>{children}</CallsShell> : <>{children}</>;
}

function Shell() {
  const gate = useGate();
  const t = useT();
  const location = useLocation();

  /* The look — the wallpaper and the hand-written widgets — follows the
   * token: loaded on sign-in, dropped on sign-out. Once, for the page. */
  React.useEffect(() => {
    followAuth();
    // The sidebar's pins follow the token the same way (lib/pins.ts), and
    // so do the pages an administrator changed (src/pages/store.ts).
    followPins();
    followPages();
  }, []);

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
  // A reload mid-setup holds the token already; the wizard's own note says
  // it has not finished (screens/wizard/progress.ts).
  const [setupStarted, setSetupStarted] = React.useState(
    () => hasToken() && readProgress() !== null,
  );
  const [setupDone, setSetupDone] = React.useState(false);
  React.useEffect(() => {
    if (gate === "setup") setSetupStarted(true);
  }, [gate]);

  /* The wizard's last step shows the owner their desktop before they are
   * let into it: the real one, signed in, drawn behind the card small and
   * out of reach (`preview`). "Let's go" grows it to the window
   * (`leaving`), and only once that has played out does setup end. The
   * desktop is the same element all the way through, so nothing on it
   * loads twice. */
  const [preview, setPreview] = React.useState(false);
  const [leaving, setLeaving] = React.useState(false);
  const stageRef = React.useRef<HTMLDivElement>(null);
  // The same element as state, for the phone's bottom bar to be drawn
  // inside while it is a preview (components/ui/bottom-nav.tsx).
  const [stageElement, setStageElement] = React.useState<HTMLDivElement | null>(null);
  const holdStage = React.useCallback((element: HTMLDivElement | null) => {
    stageRef.current = element;
    setStageElement(element);
  }, []);
  const leaveSetup = React.useCallback(() => setLeaving(true), []);
  React.useEffect(() => {
    if (!leaving) return;
    let live = true;
    const done = (): void => {
      if (!live) return;
      setSetupDone(true);
      setLeaving(false);
      setPreview(false);
      // Then the tour, once, over the desktop (src/tour/).
      startTour();
    };
    /* The growth is the stage's own transitions (index.css), and setup ends
     * when they have: no timer of its own to drift from the stylesheet, and
     * nothing to wait for under reduced motion, where there are none. A
     * transition cut short settles its promise too. */
    const growing = stageRef.current?.getAnimations() ?? [];
    if (growing.length === 0) done();
    else void Promise.allSettled(growing.map((a) => a.finished)).then(done);
    return () => {
      live = false;
    };
  }, [leaving]);

  const inSetup = setupStarted && !setupDone;
  const signedIn = gate === "open";
  /* A rule of LosOS Login holds this person to their own account
   * (backend/src/login.rs): every address shows it, and nothing else opens
   * until they comply. lososd refuses the rest anyway; this is so the page
   * says why instead of failing pane by pane. */
  const me = useMe();
  const held = signedIn && (me?.locked?.length ?? 0) > 0;
  /* LosOS Diagrams draws on a canvas beside an inspector, LosOS Calendar
   * a week of seven columns, and LosOS Wiki a page beside its directory
   * and page tree, which want more room than a pane of rows: on
   * /diagrams, /calendar and /wikis the bar and the page are wider. */
  const path = useLocation().pathname;
  const wide =
    path.startsWith("/diagrams") ||
    path.startsWith("/calendar") ||
    path.startsWith("/wikis");
  const tourOpen = React.useSyncExternalStore(subscribeTour, isTourOpen, () => false);
  // An administrator editing a page (src/pages/editor.tsx): the page keeps
  // drawing beside the panel, so it gives the panel its room.
  const pageEditor = React.useSyncExternalStore(subscribePages, () => getPagesState().editor, () => null);
  const editingPage = signedIn && !held && me?.admin === true && pageEditor !== null;
  const paletteOpen = React.useSyncExternalStore(subscribePalette, () => paletteState().open, () => false);
  const searchable = signedIn && !held;
  const pageMenu = usePageMenu(() => [
    {
      id: "palette",
      label: t("menu.palette"),
      icon: Search01Icon,
      keys: ["Mod", "Shift", "P"],
      hidden: !searchable,
      onSelect: () => openPalette(),
    },
  ]);
  // Once fetched, the palette stays mounted so it can close the way it opened.
  const [paletteLoaded, setPaletteLoaded] = React.useState(false);
  React.useEffect(() => {
    if (paletteOpen) setPaletteLoaded(true);
  }, [paletteOpen]);

  /* Ctrl/Cmd+Shift+P from anywhere on the page, signed in, or a colon typed
   * outside a text field. A key some field has already handled (a code
   * editor's own binding) is left to it. */
  React.useEffect(() => {
    if (!signedIn || inSetup || held) {
      closePalette();
      return;
    }
    const onKey = (event: KeyboardEvent): void => {
      if (event.defaultPrevented) return;
      if (isPaletteShortcut(event)) {
        event.preventDefault();
        if (paletteState().open) closePalette();
        else openPalette();
      } else if (isPaletteColon(event) && !paletteState().open) {
        event.preventDefault();
        openPalette();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [signedIn, inSetup, held]);

  // A box being set up gets the wizard and nothing else: no chrome, no
  // navigation, no dialog over the top, until its last step shows the
  // desktop behind it. It is a sequence the owner should not be able to
  // wander out of halfway.
  const wizard = inSetup ? (
    <div className={cn(leaving && "wizard-leave")} inert={leaving}>
      <React.Suspense
        fallback={
          <div className="grid min-h-dvh place-items-center bg-ground text-ink">
            <Spinner label="Loading setup" />
          </div>
        }
      >
        <Wizard onDone={leaveSetup} onPreview={setPreview} />
      </React.Suspense>
    </div>
  ) : null;

  if (inSetup && !preview) {
    // The same three places as below, so the wizard stays mounted when the
    // desktop arrives in the first.
    return (
      <>
        {null}
        {wizard}
        <Toaster />
      </>
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

  /* An open document takes the whole width under the top bar: the editor
   * has toolbars and a sidebar of its own, and its own <main>. */
  const editing =
    signedIn && !held && location.pathname.startsWith("/office") && location.hash.length > 1;

  const stage = inSetup ? (leaving ? "live" : "preview") : undefined;

  return (
    <>
    <BottomNavHost.Provider value={stage === undefined ? null : stageElement}>
    <div
      ref={holdStage}
      data-testid="desktop"
      data-stage={stage}
      data-page-editor={editingPage ? "" : undefined}
      inert={stage === "preview"}
      className={cn(
        "losos-shell min-h-dvh text-ink",
        editing && "flex h-dvh flex-col",
        stage !== undefined && "desktop-stage h-dvh overflow-hidden",
      )}
    >
      <TopBar signedIn={signedIn} wide={wide} />
      <ContextMenuHost page={pageMenu} search={searchable ? (text) => openPalette(text) : undefined} />

      {editing ? (
        <div className="min-h-0 flex-1">
          <React.Suspense fallback={<ScreenFallback />}>
            <Office hash={location.hash} />
          </React.Suspense>
        </div>
      ) : (
      <SidebarProvider
        className={cn(
          "mx-auto flex w-full px-4 pt-5 pb-12 sm:px-6",
          wide ? "max-w-[96rem]" : "max-w-6xl",
          "max-md:flex-col max-md:gap-3 md:gap-6",
        )}
      >
        <AppSidebar />
        <main className="min-w-0 flex-1">
          {signedIn && !held && <MeshNotice />}
          <React.Suspense fallback={<ScreenFallback />}>
          <CallsHolder>
          {held ? (
            <Settings pane="account" />
          ) : (
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
            <Route path="/machines" element={<SettingsRoute pane="machines" />} />
            <Route path="/apps" element={<SettingsRoute pane="apps" />} />
            <Route path="/office" element={<Office hash={location.hash} />} />
            <Route path="/apps/new" element={<AppEditorScreen />} />
            <Route path="/apps/:id" element={<AppScreen />} />
            <Route path="/apps/:id/edit" element={<AppEditorScreen />} />
            {/* nginx answers /mail with 404 off the shared side
                (modules/containers.nix); the screen says so too if the box
                leaves it while the page is open. */}
            <Route path="/mail" element={<Mail />} />
            {/* nginx answers /contacts with 404 off the shared side too. */}
            <Route path="/contacts" element={<Contacts />} />
            {/* nginx answers /calendar with 404 without LosOS Cloud too. */}
            <Route path="/calendar" element={<Calendar />} />
            {/* nginx serves /chat/ only on the shared side, under chat's
                own policy (modules/chat.nix); off it, /chat/ goes home. */}
            <Route path="/chat/vouch" element={<ChatVouch />} />
            <Route path="/chat/*" element={<Chat />} />
            {/* The same for Mesh CI. LosOS Git's checks link a run's job as
                /ci/runs/<id>#job-<job>. */}
            <Route path="/ci" element={<Ci />} />
            <Route path="/ci/runs/:id" element={<Ci />} />
            <Route path="/diagrams/*" element={<Diagrams />} />
            <Route path="/wikis/*" element={<Wikis />} />
            <Route path="/settings" element={<SettingsRoute />} />
            <Route path="/settings/:pane" element={<SettingsRoute />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
          )}
          </CallsHolder>
          </React.Suspense>
        </main>
      </SidebarProvider>
      )}

      <SignInDialog open={!signedIn} />
      {signedIn && paletteLoaded && (
        <React.Suspense fallback={null}>
          <CommandPalette />
        </React.Suspense>
      )}
      {signedIn && !held && sharedApp("calendar") && (
        <React.Suspense fallback={null}>
          <Reminders />
        </React.Suspense>
      )}
      {signedIn && !held && tourOpen && (
        <React.Suspense fallback={null}>
          <Tour />
        </React.Suspense>
      )}
      {signedIn && <ReportDialogHost />}
      {editingPage && (
        <React.Suspense fallback={null}>
          <PageEditorDock target={pageEditor} />
        </React.Suspense>
      )}
    </div>
    </BottomNavHost.Provider>
    {wizard}
    <Toaster />
    </>
  );
}

// ── Chrome ────────────────────────────────────────────────────────────────

function TopBar({ signedIn, wide }: { signedIn: boolean; wide: boolean }) {
  const t = useT();
  const me = useMe();
  let person: AppBarPerson | null = null;
  if (signedIn && me !== null) {
    const name = me.spareKey ? t("shell.me.spareKey") : (me.displayName ?? me.name ?? "");
    const tip = me.spareKey
      ? t("shell.me.spareKeyTip")
      : `${t("shell.me.signedIn", { name: me.name ?? name })} ${t(me.admin ? "shell.me.admin" : "shell.me.limited")}`;
    person = { label: name, tip };
    if (!me.spareKey && me.name) {
      person.avatar = (
        <PersonAvatar
          size="xs"
          data-testid="signed-in-avatar"
          person={{ name: me.name, displayName: me.displayName ?? me.name, picture: me.picture, accent: me.accent }}
        />
      );
      person.accountLink = <Link to="/settings/account" />;
    }
  }
  return (
    <AppBar
      homeLink={<Link to="/" />}
      badge={<Badge variant="outline">{t("shell.thisBox")}</Badge>}
      person={person}
      onSignOut={signedIn ? signOut : undefined}
      width={wide ? "wide" : "page"}
    />
  );
}

// ── Placeholders ──────────────────────────────────────────────────────────

/* Delete these as the real screens land. Kept deliberately plain: they exist
 * to prove the routing and the palette, not to suggest a layout. */
function SettingsRoute({ pane }: { pane?: SettingsPaneId }) {
  const { pathname } = useLocation();
  // The route's own pane, else the one the path names (lib/routes.ts), so
  // /settings/hardware opens hardware and /settings opens Network, the first
  // entry under Settings in the sidebar.
  const selected = pane ?? paneFromPath(pathname) ?? undefined;
  return <Settings pane={selected} />;
}

function NotFound() {
  const t = useT();
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={Compass01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>{t("shell.notFound.title")}</EmptyTitle>
        <EmptyDescription>{t("shell.notFound.body")}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

// ── Sign-in ───────────────────────────────────────────────────────────────

/* The gate.
 *
 * People sign in at /auth/, Authelia's page on this box, with their own name
 * and password (modules/identity.nix); the dialog sends them there and
 * Authelia sends them back to the page they were on. Its cookie is Secure, so
 * the sign-in happens over https even when this page was opened over http.
 *
 * The spare admin key is the other way in, behind a link: it opens the box
 * when Authelia cannot, and lososd checks it itself. On a box set up before
 * people had names, the key is also how the owner makes the first account.
 *
 * Non-dismissible: there is nothing to look at behind it. */
/* Shown under the spinner while the sign-in page starts, in an order
 * shuffled with the seed "salmon": random to read, the same on every load. */
const FACTS = seededShuffle(
  [
    "shell.facts.1",
    "shell.facts.2",
    "shell.facts.3",
    "shell.facts.4",
    "shell.facts.5",
    "shell.facts.6",
    "shell.facts.7",
    "shell.facts.8",
    "shell.facts.9",
    "shell.facts.10",
    "shell.facts.11",
    "shell.facts.12",
    "shell.facts.13",
    "shell.facts.14",
    "shell.facts.15",
    "shell.facts.16",
    "shell.facts.17",
  ] as const,
  "salmon",
);

function SignInDialog({ open }: { open: boolean }) {
  const t = useT();
  const [mode, setMode] = React.useState<"signin" | "key" | "first">(
    peopleOnBox ? "signin" : "first",
  );
  const [value, setValue] = React.useState("");
  const [name, setName] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  /* The handbook page the problem leads to, shown as "What to do" under
   * the sentence: a refused key is the forgotten-password page, no answer
   * at all is the reachability page. */
  const [help, setHelp] = React.useState<HandbookEntry>("forgot-password");
  const [busy, setBusy] = React.useState(false);
  const errorId = React.useId();
  const titleId = React.useId();
  const hintId = React.useId();

  /* Authelia starts after the box does. Until its page answers, the
   * dialog waits with a spinner and a fact about LosOS instead of sending
   * anyone to an error page, and goes on by itself once it is up. */
  const [portal, setPortal] = React.useState<"checking" | "starting" | "ready">("checking");
  const game = useWaitGame(open && mode === "signin" && portal === "starting");
  const [wanted, setWanted] = React.useState(sentToSignIn);
  const [fact, setFact] = React.useState(0);
  React.useEffect(() => {
    if (!open || mode !== "signin" || portal === "ready") return;
    const controller = new AbortController();
    let timer: number | undefined;
    const ask = async (): Promise<void> => {
      if (await signInPageReady(controller.signal)) {
        setPortal("ready");
        return;
      }
      if (controller.signal.aborted) return;
      setPortal("starting");
      timer = window.setTimeout(() => void ask(), 2000);
    };
    void ask();
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [open, mode, portal]);
  React.useEffect(() => {
    if (portal !== "starting") return;
    const timer = window.setInterval(() => setFact((n) => (n + 1) % FACTS.length), 7000);
    return () => window.clearInterval(timer);
  }, [portal]);
  React.useEffect(() => {
    if (wanted && portal === "ready") window.location.assign(signInUrl());
  }, [wanted, portal]);

  const switchTo = (next: "signin" | "key" | "first"): void => {
    setMode(next);
    setValue("");
    setProblem(null);
  };

  const submit = async (event: React.FormEvent): Promise<void> => {
    event.preventDefault();
    if (mode === "signin") {
      if (portal === "ready") window.location.assign(signInUrl());
      else setWanted(true);
      return;
    }
    const candidate = value.trim();
    if (!isWellFormedToken(candidate)) {
      setHelp("forgot-password");
      setProblem(t("shell.signIn.badShape"));
      return;
    }
    setBusy(true);
    setProblem(null);
    try {
      if (mode === "first") {
        // The key proves the box is theirs; the account is made with it,
        // then the tab carries on with the key.
        const made = await makeFirstAccount(name.trim(), password, { token: candidate });
        await signIn(candidate);
        setValue("");
        setPassword("");
        toast.success(t("shell.signIn.made"), t("shell.signIn.madeBody", { name: made.name }));
        return;
      }
      if (await signIn(candidate)) {
        setValue("");
        toast.success(t("shell.signIn.unlocked"), t("shell.signIn.unlockedBody"));
      } else {
        setHelp("forgot-password");
        setProblem(t("shell.signIn.rejected"));
      }
    } catch (error) {
      if (mode === "first" && error instanceof Error && !(error instanceof TypeError)) {
        setHelp("sign-in-and-spare-key");
        setProblem(`${t("shell.signIn.notMade")} ${error.message}`);
      } else {
        setHelp("cannot-reach-the-box");
        setProblem(t("shell.signIn.noAnswer"));
      }
    } finally {
      setBusy(false);
    }
  };

  const keyField = (
    <>
      <FieldLabel htmlFor="admin-key">{t("shell.signIn.label")}</FieldLabel>
      <MonoInput
        id="admin-key"
        name="admin-key"
        autoComplete="off"
        autoFocus={mode === "key"}
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
  );

  const ready =
    mode === "signin" ||
    (mode === "key" && value.trim().length > 0) ||
    (mode === "first" && value.trim().length > 0 && name.trim().length > 0 && password.length > 0);

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
          <DialogTitle id={titleId}>
            {t(mode === "first" ? "shell.signIn.firstTitle" : "shell.signIn.title")}
          </DialogTitle>
          <DialogDescription id={hintId}>
            {t(
              mode === "key"
                ? "shell.signIn.hint"
                : mode === "first"
                  ? "shell.signIn.firstHint"
                  : "shell.signIn.authHint",
            )}
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-2">
          {mode === "first" && (
            <>
              <Field disabled={busy}>
                <FieldLabel htmlFor="first-name">{t("shell.signIn.nameLabel")}</FieldLabel>
                <Input
                  id="first-name"
                  name="username"
                  autoComplete="username"
                  autoCapitalize="none"
                  spellCheck={false}
                  value={name}
                  disabled={busy}
                  onChange={(event) => setName(event.target.value.toLowerCase())}
                />
              </Field>
              <Field disabled={busy}>
                <FieldLabel htmlFor="first-password">{t("shell.signIn.newPassword")}</FieldLabel>
                <Input
                  id="first-password"
                  name="new-password"
                  type="password"
                  autoComplete="new-password"
                  value={password}
                  disabled={busy}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </Field>
            </>
          )}
          {mode === "signin" && portal === "starting" && (
            <div className="flex flex-col gap-3 rounded-card border border-line bg-sunk p-3">
              <div className="flex items-center gap-2 text-[13px] text-muted">
                <Spinner />
                <p aria-live="polite" className="flex-1">{t("shell.signIn.starting")}</p>
                {game.offer}
              </div>
              {game.panel ?? (
                <p className="text-[13px] text-ink">
                  <span className="font-medium">{t("shell.facts.title")}. </span>
                  {t(FACTS[fact] ?? "shell.facts.1")}
                </p>
              )}
            </div>
          )}
          {mode === "signin" && portal !== "starting" && game.panel}
          {mode !== "signin" && (
            <Field invalid={problem !== null} disabled={busy}>
              {keyField}
              <FieldError id={errorId}>{problem}</FieldError>
              {problem !== null && <HelpLink entry={help} />}
              {mode === "key" && <FieldDescription>{t("shell.signIn.remembered")}</FieldDescription>}
            </Field>
          )}
          {mode !== "first" && (
            <button
              type="button"
              className="mt-1 self-start text-[13px] text-accent underline-offset-2 hover:underline"
              onClick={() => switchTo(mode === "key" ? "signin" : "key")}
            >
              {t(mode === "key" ? "shell.signIn.useSignIn" : "shell.signIn.useKey")}
            </button>
          )}
        </DialogBody>
        <DialogFooter>
          {busy && <Spinner label={t("shell.signIn.checking")} className="mr-auto text-muted" />}
          <Button
            type="submit"
            disabled={busy || !ready || (mode === "signin" && wanted)}
            autoFocus={mode === "signin"}
          >
            {t(
              mode === "signin"
                ? "shell.signIn.go"
                : mode === "first"
                  ? "shell.signIn.make"
                  : "shell.signIn.unlock",
            )}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
