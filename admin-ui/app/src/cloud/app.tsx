import * as React from "react";
import { BrowserRouter, Link, Navigate, Route, Routes, useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Clock01Icon,
  Delete02Icon,
  Folder01Icon,
  Image01Icon,
  LaptopPhoneSyncIcon,
  NoteIcon,
  Search01Icon,
  StarIcon,
  UserSharingIcon,
} from "@hugeicons/core-free-icons";
import { ContextMenuHost, usePageMenu } from "@/components/context-menu";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { LanguagePicker } from "@/components/ui/language-picker";
import { Progress } from "@/components/ui/progress";
import {
  Sidebar,
  SidebarContent,
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
import { EditPage } from "./collab/page";
import { ConnectPage } from "./connect";
import { DevicesView } from "./devices";
import { FilesView } from "./files";
import { formatBytes } from "./format";
import { ListView } from "./lists";
import { SharedFolderView, SharedView } from "./mesh/shared";
import { NotesOpen, NotesPage } from "./notes/page";
import { OfficePage } from "./office/page";
import { PhotosView } from "./photos";
import { useMe } from "./store";
import { TrashView } from "./trash";
import { UploadsPanel } from "./uploads";

/* LosOS Cloud at /cloud/: the person's files in the browser, beside the
 * desktop and phone apps that sync them. One sidebar of places (Files,
 * Notes, Recent, Favorites, Photos, Trash, Devices), a search in the top
 * bar, and
 * the uploads in the corner. /cloud/connect/<token> is the page the apps
 * open to sign in, and draws without the sidebar. */

export function CloudRoot() {
  return (
    <BrowserRouter basename="/cloud">
      <Shell />
      <Toaster />
    </BrowserRouter>
  );
}

interface Place {
  to: string;
  label: MessageKey;
  icon: IconSvgElement;
}

const PLACES: readonly Place[] = [
  { to: "/files", label: "cloud.nav.files", icon: Folder01Icon },
  { to: "/notes", label: "notes.title", icon: NoteIcon },
  { to: "/recent", label: "cloud.nav.recent", icon: Clock01Icon },
  { to: "/favorites", label: "cloud.nav.favorites", icon: StarIcon },
  { to: "/shared", label: "mesh.shared.title", icon: UserSharingIcon },
  { to: "/photos", label: "cloud.nav.photos", icon: Image01Icon },
  { to: "/trash", label: "cloud.nav.trash", icon: Delete02Icon },
  { to: "/devices", label: "cloud.nav.devices", icon: LaptopPhoneSyncIcon },
];

function Shell() {
  const { pathname } = useLocation();
  const connecting = pathname.startsWith("/connect/");
  const editing = pathname.startsWith("/edit/");
  const office = pathname.startsWith("/office/");
  const noting = pathname === "/notes" || pathname.startsWith("/notes/");
  const navigate = useNavigate();
  const pageMenu = usePageMenu();
  return (
    <div className={cn("losos-shell min-h-dvh text-ink", office && "flex h-dvh flex-col")}>
      <TopBar search={!connecting && !editing && !noting && !office} wide={noting || office} />
      <ContextMenuHost
        page={pageMenu}
        search={connecting ? undefined : (text) => navigate(`/search?q=${encodeURIComponent(text)}`)}
      />
      {office ? (
        // The document under the top bar, edge to edge, its bars its own.
        <main className="min-h-0 flex-1">
          <Routes>
            <Route path="/office/:file" element={<OfficePage />} />
          </Routes>
        </main>
      ) : connecting ? (
        <Routes>
          <Route path="/connect/:token" element={<ConnectPage />} />
        </Routes>
      ) : noting ? (
        // A vault has panes of its own and wants the width.
        <div className="px-4 pt-5 pb-12 sm:px-6">
          <Routes>
            <Route path="/notes" element={<NotesPage />} />
            <Route path="/notes/open" element={<NotesOpen />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </div>
      ) : editing ? (
        // A document gets the width; the places are one link away.
        <main className="px-4 pt-5 pb-12 sm:px-6">
          <Routes>
            <Route path="/edit/:file" element={<EditPage />} />
          </Routes>
        </main>
      ) : (
        <SidebarProvider className={cn("mx-auto flex w-full max-w-6xl px-4 pt-5 pb-12 sm:px-6", "max-md:flex-col max-md:gap-3 md:gap-6")}>
          <Places />
          <main className="min-w-0 flex-1">
            <Routes>
              <Route path="/" element={<Navigate to="/files" replace />} />
              <Route path="/files/*" element={<FilesView />} />
              <Route path="/recent" element={<ListView kind="recent" />} />
              <Route path="/favorites" element={<ListView kind="favorites" />} />
              <Route path="/search" element={<ListView kind="search" />} />
              <Route path="/shared" element={<SharedView />} />
              <Route path="/shared/:share" element={<SharedFolderView />} />
              <Route path="/photos" element={<PhotosView />} />
              <Route path="/trash" element={<TrashView />} />
              <Route path="/devices" element={<DevicesView />} />
              <Route path="*" element={<NotFound />} />
            </Routes>
          </main>
          <UploadsPanel />
        </SidebarProvider>
      )}
    </div>
  );
}

function TopBar({ search, wide = false }: { search: boolean; wide?: boolean }) {
  const t = useT();
  const logo = logoFor();
  const me = useMe();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { pathname } = useLocation();
  const [text, setText] = React.useState(pathname === "/search" ? (params.get("q") ?? "") : "");

  React.useEffect(() => {
    if (pathname !== "/search") setText("");
  }, [pathname]);

  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className={cn("mx-auto flex w-full items-center gap-3 px-4 py-3 sm:px-6", wide ? "max-w-[96rem]" : "max-w-6xl")}>
        <Link to="/files" className="flex shrink-0 items-center gap-2.5 rounded-control focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
          <img src={logo.url} alt="" width={32} height={32} className="size-8" />
          <span className="text-[15px] font-semibold tracking-tight max-sm:sr-only">{me?.product ?? "LosOS Cloud"}</span>
        </Link>
        {search ? (
          <form
            role="search"
            className="relative mx-auto w-full max-w-md"
            onSubmit={(event) => {
              event.preventDefault();
              const q = text.trim();
              if (q !== "") navigate(`/search?q=${encodeURIComponent(q)}`);
            }}
          >
            <HugeiconsIcon
              icon={Search01Icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-faint"
              aria-hidden="true"
            />
            <Input
              type="search"
              value={text}
              onChange={(event) => setText(event.target.value)}
              placeholder={t("cloud.search.placeholder")}
              aria-label={t("cloud.search.label")}
              className="pl-9"
            />
          </form>
        ) : (
          <div className="flex-1" />
        )}
        <div className="flex shrink-0 items-center gap-2 max-md:hidden">
          <LanguagePicker />
          <ThemeToggle />
        </div>
        {me !== null && (
          <span className="max-w-40 truncate text-[13px] text-muted max-lg:hidden" title={me.email || me.login}>
            {me.name || me.login}
          </span>
        )}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            const home = `${window.location.origin}/cloud/`;
            window.location.assign(`${window.location.origin}/auth/logout?rd=${encodeURIComponent(home)}`);
          }}
        >
          {t("cloud.signOut")}
        </Button>
      </div>
    </header>
  );
}

function Places() {
  const t = useT();
  const { pathname } = useLocation();
  const { state, isMobile, setOpenMobile } = useSidebar();
  const rail = !isMobile && state === "collapsed";
  const me = useMe();
  const current = PLACES.find((p) => pathname === p.to || pathname.startsWith(`${p.to}/`));

  React.useEffect(() => setOpenMobile(false), [pathname, setOpenMobile]);

  const total = me === null ? 0 : me.used + me.free;
  return (
    <>
      {isMobile && (
        <div className="flex w-full items-center gap-1.5 rounded-card border border-line bg-sunk p-2">
          <SidebarTrigger label={t("cloud.nav.expand")} />
          <button type="button" onClick={() => setOpenMobile(true)} className="min-w-0 flex-1 truncate text-left text-[13.5px] font-medium text-ink">
            {current !== undefined ? t(current.label) : t("cloud.nav.label")}
          </button>
        </div>
      )}
      <Sidebar aria-label={t("cloud.nav.label")} mobileTitle={t("cloud.nav.label")} closeLabel={t("cloud.close")}>
        <SidebarHeader className={cn(isMobile && "pr-10")}>
          {!isMobile && <SidebarTrigger label={t(rail ? "cloud.nav.expand" : "cloud.nav.collapse")} />}
        </SidebarHeader>
        <SidebarContent>
          <SidebarMenu>
            {PLACES.map((place) => {
              const active = current === place;
              const label = t(place.label);
              return (
                <SidebarMenuItem key={place.to}>
                  <SidebarMenuButton
                    isActive={active}
                    tooltip={label}
                    render={<Link to={place.to} aria-current={active ? "page" : undefined} />}
                  >
                    <SidebarMenuIcon active={active}>
                      <HugeiconsIcon icon={place.icon} size={13} strokeWidth={1.5} color="currentColor" />
                    </SidebarMenuIcon>
                    <span className={cn(rail && "sr-only")}>{label}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
          {me !== null && total > 0 && !rail && (
            <div className="mt-auto flex flex-col gap-1.5 px-2 pt-6 pb-1" data-usage>
              <Progress value={(me.used / total) * 100} label={t("cloud.usage.label")} />
              <p className="text-[12px] text-muted">{t("cloud.usage.text", { used: formatBytes(me.used), free: formatBytes(me.free) })}</p>
            </div>
          )}
        </SidebarContent>
      </Sidebar>
    </>
  );
}

function NotFound() {
  const t = useT();
  return (
    <Empty className="min-h-72">
      <EmptyHeader>
        <EmptyTitle>{t("cloud.notFound.title")}</EmptyTitle>
        <EmptyDescription>
          <Link to="/files" className="underline">
            {t("cloud.folder.home")}
          </Link>
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
