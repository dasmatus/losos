import * as React from "react";
import { BrowserRouter, Link, Navigate, Route, Routes, useLocation, useParams } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  Book02Icon,
  Bookmark01Icon,
  CodeIcon,
  Folder01Icon,
  InboxIcon,
  Task01Icon,
  UserCircleIcon,
  GitBranchIcon,
  GitCommitIcon,
  GitPullRequestIcon,
  LaptopIcon,
  Settings02Icon,
} from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { LanguagePicker } from "@/components/ui/language-picker";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuIcon,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from "@/components/ui/sidebar";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { Toaster } from "@/components/ui/toast";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { chatServerOf, chatToken, dropChatToken, git, GitError, matrixSignIn, whenSignedOut, type User } from "./api";
import { ErrorAlert, failed, Loading, NotFound } from "./parts";
import { BranchesView, CommitsView } from "./history";
import { CodeView } from "./code";
import { CompareView } from "./compare";
import { PullView } from "./pull";
import { PullsView } from "./pulls";
import { RepoList } from "./repos";
import { FacesProvider, MeContext, useFaceBook, useFaces, useLoad, useVisitor, VisitorContext } from "./store";
import { SettingsView } from "./settings";
import { tracker } from "./tracker/api";
import { IssuesRoot, TrackerProvider } from "./tracker/screen";
import { useTracker } from "./tracker/store";
import { WorktreesView } from "./worktrees";

/* LosOS Git at /git/: the person's repositories in the browser. The list
 * of repositories, and per repository its code, pull requests with their
 * review, history, branches and the worktrees that push to it, and the
 * issues of every repository and project at /git/issues (tracker/). Its data is
 * LosOS Git's own API at /git/api/v1/, which nginx names the person to
 * when they are signed in to LosOS Login (modules/containers.nix). Someone
 * who is not is a visitor: they read the public repositories and get a
 * Sign in button. With `losos.git.site.requireSignIn` nginx sends them to
 * LosOS Login before the page loads.
 *
 * Someone of another box signs in with their chat address instead: this
 * page sends them to their own box's chat (`/chat/vouch`), which asks
 * them, and comes back here with a token their chat server vouches for
 * (`#chat-token=…&chat-server=…`). LosOS Git checks it with that server and
 * hands back a token of its own (backend/src/forge/matrix.rs). */

export function GitRoot() {
  return (
    <BrowserRouter basename="/git">
      <Shell />
      <Toaster />
    </BrowserRouter>
  );
}

function Shell() {
  const [me, setMe] = React.useState<User | null>(null);
  const [visitor, setVisitor] = React.useState(false);
  const [signedOut, setSignedOut] = React.useState(false);
  const signedIn = React.useRef(false);
  React.useEffect(() => {
    // A 401 after the person was signed in means the session ended; before
    // that, it is a visitor's every call to a page that needs an account.
    whenSignedOut(() => {
      if (signedIn.current) setSignedOut(true);
    });
    const controller = new AbortController();
    const vouched = vouchedHere();
    const who = vouched !== null ? matrixSignIn(vouched.token, vouched.server, vouched.name) : git.me(controller.signal);
    who.then(
      (u) => {
        signedIn.current = true;
        setMe(u);
      },
      (e: unknown) => {
        if (controller.signal.aborted) return;
        if (vouched !== null) failed(translate("git.chatSignIn.failed"), e);
        if (e instanceof GitError && e.status === 401 && chatToken() !== null) {
          // The token is gone (taken back, or the account removed).
          dropChatToken();
          window.location.reload();
          return;
        }
        if (e instanceof GitError && (e.status === 401 || vouched !== null)) setVisitor(true);
      },
    );
    return () => controller.abort();
  }, []);
  const faces = useFaceBook(me);
  // The issues lay a list and an issue side by side, as Mail does: they
  // take the width of the screen.
  const wide = useLocation().pathname.startsWith("/issues");

  return (
    <MeContext.Provider value={me}>
      <VisitorContext.Provider value={visitor}>
        <FacesProvider value={faces}>
          <TrackerProvider>
          <div className="losos-shell min-h-dvh text-ink">
            <TopBar />
            {signedOut ? (
              <SignedOut />
            ) : (
              <SidebarProvider className={cn("mx-auto flex w-full px-4 pt-5 pb-12 sm:px-6", wide ? "max-w-[100rem]" : "max-w-6xl", "max-md:flex-col max-md:gap-3 md:gap-6")}>
                <Places />
                <main className="min-w-0 flex-1">
                  <Routes>
                    <Route path="/" element={<RepoList />} />
                    <Route path="/issues/*" element={<IssuesRoot />} />
                    <Route path="/:owner/:repo/issues" element={<RepoIssues />} />
                    <Route path="/:owner/:repo" element={<CodeView />} />
                    <Route path="/:owner/:repo/tree/*" element={<CodeView />} />
                    <Route path="/:owner/:repo/blob/*" element={<CodeView file />} />
                    <Route path="/:owner/:repo/pulls" element={<PullsView />} />
                    <Route path="/:owner/:repo/pulls/:number" element={<PullView tab="conversation" />} />
                    <Route path="/:owner/:repo/pulls/:number/commits" element={<PullView tab="commits" />} />
                    <Route path="/:owner/:repo/pulls/:number/files" element={<PullView tab="files" />} />
                    <Route path="/:owner/:repo/compare" element={<CompareView />} />
                    <Route path="/:owner/:repo/commits" element={<CommitsView />} />
                    <Route path="/:owner/:repo/branches" element={<BranchesView />} />
                    <Route path="/:owner/:repo/worktrees" element={<WorktreesView />} />
                    <Route path="/:owner/:repo/settings" element={<SettingsView />} />
                    <Route path="*" element={<NotFound />} />
                  </Routes>
                </main>
              </SidebarProvider>
            )}
          </div>
          </TrackerProvider>
        </FacesProvider>
      </VisitorContext.Provider>
    </MeContext.Provider>
  );
}

/** LosOS Login, and back to this page once signed in. */
function signIn() {
  window.location.assign(`${window.location.origin}/auth/?rd=${encodeURIComponent(window.location.href)}`);
}

/** What the person's own box sent back, once: the hash is cleared. */
function vouchedHere(): { token: string; server: string; name: string } | null {
  const hash = new URLSearchParams(window.location.hash.slice(1));
  const token = hash.get("chat-token");
  const server = hash.get("chat-server");
  if (token === null || server === null) return null;
  window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
  return { token, server, name: hash.get("chat-name") ?? "" };
}

/** Ask the person's own box who they are; it sends them back here. */
function ChatSignIn({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const t = useT();
  const id = React.useId();
  const [address, setAddress] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  React.useEffect(() => {
    if (open) {
      setAddress("");
      setError(null);
    }
  }, [open]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} dialogClassName="w-[min(30rem,calc(100vw-2rem))]">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const server = chatServerOf(address);
          if (server === null) {
            setError(t("git.chatSignIn.bad"));
            return;
          }
          if (server === window.location.host) {
            signIn();
            return;
          }
          const back = `${window.location.origin}${window.location.pathname}`;
          window.location.assign(
            `https://${server}/chat/vouch?for=${encodeURIComponent(window.location.host)}&back=${encodeURIComponent(back)}`,
          );
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{t("git.chatSignIn.title")}</DialogTitle>
          <DialogDescription>{t("git.chatSignIn.body")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field invalid={error !== null}>
            <FieldLabel htmlFor={`${id}-address`}>{t("git.chatSignIn.address")}</FieldLabel>
            <Input
              id={`${id}-address`}
              value={address}
              onChange={(e) => {
                setAddress(e.target.value);
                setError(null);
              }}
              placeholder="@you:yourbox.local"
              autoComplete="username"
              autoCapitalize="none"
              spellCheck={false}
              autoFocus
              data-chat-address
            />
            {error !== null ? <FieldError>{error}</FieldError> : <FieldDescription>{t("git.chatSignIn.hint")}</FieldDescription>}
          </Field>
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {t("git.cancel")}
          </Button>
          <Button type="submit" disabled={address.trim() === ""} data-chat-continue>
            {t("git.chatSignIn.go")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function TopBar() {
  const t = useT();
  const logo = logoFor();
  const me = React.useContext(MeContext);
  const visitor = useVisitor();
  const [asking, setAsking] = React.useState(false);
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-6xl items-center gap-3 px-4 py-3 sm:px-6">
        <Link to="/" className="flex shrink-0 items-center gap-2.5 rounded-control focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
          <img src={logo.url} alt="" width={32} height={32} className="size-8" />
          <span className="text-[15px] font-semibold tracking-tight max-sm:sr-only">LosOS Git</span>
        </Link>
        <div className="flex-1" />
        <div className="flex shrink-0 items-center gap-2 max-md:hidden">
          <LanguagePicker />
          <ThemeToggle />
        </div>
        {me !== null && <MeChip me={me} />}
        {visitor ? (
          <>
            <Button variant="ghost" size="sm" onClick={() => setAsking(true)} data-chat-sign-in>
              {t("git.chatSignIn.open")}
            </Button>
            <Button size="sm" onClick={signIn} data-sign-in>
              {t("git.visitor.signIn")}
            </Button>
          </>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              const home = `${window.location.origin}/git/`;
              if (me?.matrix_id) {
                // Someone of another box: forget this browser's token.
                dropChatToken();
                window.location.assign(home);
                return;
              }
              window.location.assign(`${window.location.origin}/auth/logout?rd=${encodeURIComponent(home)}`);
            }}
          >
            {t("git.signOut")}
          </Button>
        )}
      </div>
      <ChatSignIn open={asking} onOpenChange={setAsking} />
    </header>
  );
}

function MeChip({ me }: { me: User }) {
  const faces = useFaces();
  return (
    <span className="flex max-w-56 items-center gap-2 text-[13px] text-muted max-lg:hidden" title={me.matrix_id ?? (me.email || me.login)}>
      <PersonAvatar person={faces(me.login)} size="xs" />
      <span className="truncate" data-me>{me.full_name || me.matrix_id || me.login}</span>
    </span>
  );
}

interface Place {
  to: string;
  label: MessageKey;
  icon: IconSvgElement;
  /** Further paths that light this place up. */
  also?: string[];
}

function repoPlaces(base: string, admin: boolean): Place[] {
  const places: Place[] = [
    { to: base, label: "git.nav.code", icon: CodeIcon, also: [`${base}/tree`, `${base}/blob`] },
    { to: `${base}/issues`, label: "git.nav.issues", icon: Task01Icon },
    { to: `${base}/pulls`, label: "git.nav.pulls", icon: GitPullRequestIcon, also: [`${base}/compare`] },
    { to: `${base}/commits`, label: "git.nav.commits", icon: GitCommitIcon },
    { to: `${base}/branches`, label: "git.nav.branches", icon: GitBranchIcon },
    { to: `${base}/worktrees`, label: "git.nav.worktrees", icon: LaptopIcon },
  ];
  if (admin) places.push({ to: `${base}/settings`, label: "git.nav.settings", icon: Settings02Icon });
  return places;
}

function Places() {
  const t = useT();
  const { pathname, search } = useLocation();
  const visitor = useVisitor();
  const { state, isMobile, setOpenMobile } = useSidebar();
  const rail = !isMobile && state === "collapsed";
  const parts = pathname.split("/").filter((p) => p !== "");
  const inIssues = parts[0] === "issues";
  const inRepo = parts.length >= 2 && !inIssues;
  const base = inRepo ? `/${parts[0]}/${parts[1]}` : null;
  const repo = useLoad(
    (signal) => (inRepo ? git.repo(parts[0] ?? "", parts[1] ?? "", signal).catch(() => null) : Promise.resolve(null)),
    [parts[0], parts[1], inRepo],
  );
  const places = base === null ? [] : repoPlaces(base, repo.data?.permissions.admin === true);
  const lit = (p: Place) =>
    p.to === base
      ? pathname === base || (p.also ?? []).some((a) => pathname.startsWith(a))
      : pathname === p.to || pathname.startsWith(`${p.to}/`) || (p.also ?? []).some((a) => pathname.startsWith(a));
  const current = places.find(lit);

  React.useEffect(() => setOpenMobile(false), [pathname, setOpenMobile]);

  const item: PlaceItem = (to, label, icon, active, count) => (
    <SidebarMenuItem key={to}>
      <SidebarMenuButton isActive={active} tooltip={label} render={<Link to={to} aria-current={active ? "page" : undefined} />}>
        <SidebarMenuIcon active={active}>
          <HugeiconsIcon icon={icon} size={13} strokeWidth={1.5} color="currentColor" />
        </SidebarMenuIcon>
        <span className={cn("min-w-0 flex-1 truncate", rail && "sr-only")}>{label}</span>
        {count !== undefined && count > 0 && !rail && (
          <span className="rounded-full bg-accent px-1.5 text-[11px] leading-4 font-medium text-surface tabular-nums" data-count>
            {count}
          </span>
        )}
      </SidebarMenuButton>
    </SidebarMenuItem>
  );

  return (
    <>
      {isMobile && (
        <div className="flex w-full items-center gap-1.5 rounded-card border border-line bg-sunk p-2">
          <SidebarTrigger label={t("git.nav.expand")} />
          <button type="button" onClick={() => setOpenMobile(true)} className="min-w-0 flex-1 truncate text-left text-[13.5px] font-medium text-ink">
            {current !== undefined ? t(current.label) : t("git.nav.repos")}
          </button>
        </div>
      )}
      <Sidebar aria-label={t("git.nav.label")} mobileTitle={t("git.nav.label")} closeLabel={t("git.close")}>
        <SidebarHeader className={cn(isMobile && "pr-10")}>
          {!isMobile && <SidebarTrigger label={t(rail ? "git.nav.expand" : "git.nav.collapse")} />}
        </SidebarHeader>
        <SidebarContent>
          <SidebarMenu>
            {item("/", t("git.nav.repos"), Book02Icon, pathname === "/")}
            {visitor ? null : item("/issues", t("git.nav.issues"), Task01Icon, false)}
          </SidebarMenu>
          {inIssues && <IssuePlaces pathname={pathname} search={search} rail={rail} item={item} />}
          {base !== null && (
            <SidebarGroup>
              {!rail && <SidebarGroupLabel className="truncate">{parts[1]}</SidebarGroupLabel>}
              <SidebarMenu>{places.map((p) => item(p.to, t(p.label), p.icon, current === p))}</SidebarMenu>
            </SidebarGroup>
          )}
        </SidebarContent>
      </Sidebar>
    </>
  );
}

type PlaceItem = (to: string, label: string, icon: IconSvgElement, active: boolean, count?: number) => React.ReactNode;

/** Under Issues: the inbox, mine, all, each project and each saved view. */
function IssuePlaces({ pathname, search, rail, item }: { pathname: string; search: string; rail: boolean; item: PlaceItem }) {
  const t = useT();
  const { home } = useTracker();
  const open = new URLSearchParams(search).get("open");
  const inProject = (key: string) => pathname.startsWith(`/issues/p/${key}`) || (open?.startsWith(`${key}-`) ?? false);
  return (
    <>
      <SidebarGroup>
        {!rail && <SidebarGroupLabel>{t("git.nav.issues")}</SidebarGroupLabel>}
        <SidebarMenu>
          {item("/issues/inbox", t("tracker.nav.inbox"), InboxIcon, pathname === "/issues/inbox", home?.unread)}
          {item("/issues", t("tracker.nav.mine"), UserCircleIcon, pathname === "/issues" && open === null, home?.assigned)}
          {item("/issues/all", t("tracker.nav.all"), Task01Icon, pathname === "/issues/all")}
        </SidebarMenu>
      </SidebarGroup>
      <SidebarGroup>
        {!rail && <SidebarGroupLabel>{t("tracker.nav.projects")}</SidebarGroupLabel>}
        <SidebarMenu>
          {(home?.projects ?? [])
            .filter((p) => !p.archived)
            .map((p) => item(`/issues/p/${p.key}`, p.name, Folder01Icon, inProject(p.key), p.counts?.open))}
          {item("/issues/new-project", t("tracker.project.new"), Add01Icon, pathname === "/issues/new-project")}
        </SidebarMenu>
      </SidebarGroup>
      {(home?.views.length ?? 0) > 0 && (
        <SidebarGroup>
          {!rail && <SidebarGroupLabel>{t("tracker.nav.views")}</SidebarGroupLabel>}
          <SidebarMenu>{(home?.views ?? []).map((v) => item(`/issues/views/${v.id}`, v.name, Bookmark01Icon, pathname === `/issues/views/${v.id}`))}</SidebarMenu>
        </SidebarGroup>
      )}
    </>
  );
}

/** A repository's issues are its project's: found, or made on first ask
 *  by someone who may write the repository. */
function RepoIssues() {
  const t = useT();
  const { owner = "", repo = "" } = useParams();
  const { reloadHome } = useTracker();
  const project = useLoad((signal) => tracker.repoProject(owner, repo, signal), [owner, repo]);
  React.useEffect(() => {
    if (project.data !== null) reloadHome();
  }, [project.data, reloadHome]);
  if (project.data !== null) return <Navigate to={`/issues/p/${project.data.key}`} replace />;
  if (project.error !== null) {
    return project.error instanceof GitError && project.error.status === 404 ? (
      <Empty className="mt-10">
        <EmptyHeader>
          <EmptyTitle>{t("tracker.repo.none")}</EmptyTitle>
          <EmptyDescription>{t("tracker.repo.noneBody")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    ) : (
      <ErrorAlert error={project.error} />
    );
  }
  return <Loading rows={4} />;
}

function SignedOut() {
  const t = useT();
  return (
    <Empty className="mx-auto mt-16 min-h-72 max-w-lg">
      <EmptyHeader>
        <EmptyTitle>{t("git.signedOut.title")}</EmptyTitle>
        <EmptyDescription>{t("git.signedOut.body")}</EmptyDescription>
      </EmptyHeader>
      <Button onClick={() => window.location.reload()}>{t("git.signedOut.again")}</Button>
    </Empty>
  );
}
