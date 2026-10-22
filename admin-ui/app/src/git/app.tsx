import * as React from "react";
import { BrowserRouter, Link, Route, Routes, useLocation } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Book02Icon,
  CodeIcon,
  GitBranchIcon,
  GitCommitIcon,
  GitPullRequestIcon,
  LaptopIcon,
  Settings02Icon,
} from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
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
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { git, GitError, whenSignedOut, type User } from "./api";
import { NotFound } from "./parts";
import { BranchesView, CommitsView } from "./history";
import { CodeView } from "./code";
import { CompareView } from "./compare";
import { PullView } from "./pull";
import { PullsView } from "./pulls";
import { RepoList } from "./repos";
import { FacesProvider, MeContext, useFaceBook, useFaces, useLoad, useVisitor, VisitorContext } from "./store";
import { SettingsView } from "./settings";
import { WorktreesView } from "./worktrees";

/* LosOS Git at /git/: the person's repositories in the browser. The list
 * of repositories, and per repository its code, pull requests with their
 * review, history, branches and the worktrees that push to it. Its data is
 * LosOS Git's own API at /git/api/v1/, which nginx names the person to
 * when they are signed in to LosOS Login (modules/containers.nix). Someone
 * who is not is a visitor: they read the public repositories and get a
 * Sign in button. With `losos.git.site.requireSignIn` nginx sends them to
 * LosOS Login before the page loads. */

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
    git.me(controller.signal).then(
      (u) => {
        signedIn.current = true;
        setMe(u);
      },
      (e: unknown) => {
        if (e instanceof GitError && e.status === 401) setVisitor(true);
      },
    );
    return () => controller.abort();
  }, []);
  const faces = useFaceBook(me);

  return (
    <MeContext.Provider value={me}>
      <VisitorContext.Provider value={visitor}>
        <FacesProvider value={faces}>
          <div className="losos-shell min-h-dvh text-ink">
            <TopBar />
            {signedOut ? (
              <SignedOut />
            ) : (
              <SidebarProvider className={cn("mx-auto flex w-full max-w-6xl px-4 pt-5 pb-12 sm:px-6", "max-md:flex-col max-md:gap-3 md:gap-6")}>
                <Places />
                <main className="min-w-0 flex-1">
                  <Routes>
                    <Route path="/" element={<RepoList />} />
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
        </FacesProvider>
      </VisitorContext.Provider>
    </MeContext.Provider>
  );
}

/** LosOS Login, and back to this page once signed in. */
function signIn() {
  window.location.assign(`${window.location.origin}/auth/?rd=${encodeURIComponent(window.location.href)}`);
}

function TopBar() {
  const t = useT();
  const logo = logoFor();
  const me = React.useContext(MeContext);
  const visitor = useVisitor();
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
          <Button size="sm" onClick={signIn} data-sign-in>
            {t("git.visitor.signIn")}
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              const home = `${window.location.origin}/git/`;
              window.location.assign(`${window.location.origin}/auth/logout?rd=${encodeURIComponent(home)}`);
            }}
          >
            {t("git.signOut")}
          </Button>
        )}
      </div>
    </header>
  );
}

function MeChip({ me }: { me: User }) {
  const faces = useFaces();
  return (
    <span className="flex max-w-48 items-center gap-2 text-[13px] text-muted max-lg:hidden" title={me.email || me.login}>
      <PersonAvatar person={faces(me.login)} size="xs" />
      <span className="truncate">{me.full_name || me.login}</span>
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
  const { pathname } = useLocation();
  const { state, isMobile, setOpenMobile } = useSidebar();
  const rail = !isMobile && state === "collapsed";
  const parts = pathname.split("/").filter((p) => p !== "");
  const inRepo = parts.length >= 2;
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

  const item = (to: string, label: string, icon: IconSvgElement, active: boolean) => (
    <SidebarMenuItem key={to}>
      <SidebarMenuButton isActive={active} tooltip={label} render={<Link to={to} aria-current={active ? "page" : undefined} />}>
        <SidebarMenuIcon active={active}>
          <HugeiconsIcon icon={icon} size={13} strokeWidth={1.5} color="currentColor" />
        </SidebarMenuIcon>
        <span className={cn(rail && "sr-only")}>{label}</span>
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
          <SidebarMenu>{item("/", t("git.nav.repos"), Book02Icon, pathname === "/")}</SidebarMenu>
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
