import * as React from "react";
import { BrowserRouter, Link, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { AppSidebar } from "@/components/app-sidebar";
import { ShellBar, ShellBody, ShellDocument } from "@/components/app-shell";
import { ContextMenuHost, usePageMenu } from "@/components/context-menu";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Progress } from "@/components/ui/progress";
import { Toaster } from "@/components/ui/toast";
import { loadMe } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { followAuth } from "@/lib/look";
import { followPins } from "@/lib/pins";
import { SiteBaseProvider } from "@/lib/site";
import { cn } from "@/lib/utils";
import { PaletteHost } from "@/palette/host";
import { openPalette } from "@/palette/store";
import { EditPage } from "./collab/page";
import { ConnectPage } from "./connect";
import { DevicesView } from "./devices";
import { FilesView } from "./files";
import { formatBytes } from "./format";
import { ListView } from "./lists";
import { SharedFolderView, SharedView } from "./mesh/shared";
import { NotesOpen, NotesPage } from "./notes/page";
import { OfficePage } from "./office/page";
import { OpenFile } from "./open";
import { PhotosView } from "./photos";
import { useFollowChanges, useMe } from "./store";
import { TrashView } from "./trash";
import { UploadsPanel } from "./uploads";

/* LosOS Cloud at /cloud/: the person's files in the browser, beside the
 * desktop and phone apps that sync them. It is a document of its own (its
 * own nginx location and API, src/cloud/api.ts) in the box's one frame
 * (components/app-shell.tsx): the same top bar and sidebar as the admin
 * pages, with LosOS Cloud's places (Files, Notes, Recent, Favorites,
 * Shared with you, Photos, Trash, Devices) under its Cloud entry, the same
 * command palette on the same keys, and the uploads in the corner. A
 * document opens in `main`, beside the sidebar. /cloud/connect/<token> is
 * the page the apps open to sign in, and draws without the sidebar. */

export function CloudRoot() {
  return (
    <BrowserRouter basename="/cloud">
      <SiteBaseProvider value="/cloud">
        <Shell />
      </SiteBaseProvider>
      <Toaster />
    </BrowserRouter>
  );
}

function Shell() {
  const { pathname } = useLocation();
  const connecting = pathname.startsWith("/connect/");
  const editing = pathname.startsWith("/edit/") || pathname.startsWith("/office/");
  const noting = pathname === "/notes" || pathname.startsWith("/notes/");
  const pageMenu = usePageMenu();
  // Files added by other apps show up without a reload.
  useFollowChanges();

  /* Who is here, as the admin page asks it (GET /api/me): the sidebar's
   * entries and the person's menu are the same as there. The pins and the
   * look follow it the same way. */
  React.useEffect(() => {
    void loadMe().catch(() => undefined);
    followAuth();
    followPins();
  }, []);

  if (connecting) {
    return (
      <div className="losos-shell min-h-dvh text-ink">
        <ShellBar signedIn />
        <Routes>
          <Route path="/connect/:token" element={<ConnectPage />} />
        </Routes>
      </div>
    );
  }

  const sidebar = <AppSidebar footer={<Usage />} />;
  return (
    <div className={cn("losos-shell min-h-dvh text-ink", editing && "flex h-dvh flex-col")}>
      <ShellBar signedIn />
      <ContextMenuHost page={pageMenu} search={(text) => openPalette(text)} />
      {editing ? (
        // A document beside the sidebar, the rest of the window's height.
        <ShellBody sidebar={sidebar} fill>
          <ShellDocument>
            <Routes>
              <Route path="/office/:file" element={<OfficePage />} />
              <Route path="/edit/:file" element={<div className="h-full overflow-auto p-4 sm:p-6"><EditPage /></div>} />
            </Routes>
          </ShellDocument>
        </ShellBody>
      ) : (
        // A vault has panes of its own and wants the width.
        <ShellBody sidebar={sidebar} wide={noting}>
          <Routes>
            <Route path="/" element={<Navigate to="/files" replace />} />
            <Route path="/files/*" element={<FilesView />} />
            <Route path="/notes" element={<NotesPage />} />
            <Route path="/notes/open" element={<NotesOpen />} />
            <Route path="/open/:file" element={<OpenFile />} />
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
        </ShellBody>
      )}
      <UploadsPanel />
      <PaletteHost enabled />
    </div>
  );
}

/* The person's used and free space, at the foot of the sidebar. */
function Usage() {
  const t = useT();
  const me = useMe();
  const total = me === null ? 0 : me.used + me.free;
  if (me === null || total <= 0) return null;
  return (
    <div className="flex flex-col gap-1.5" data-usage>
      <Progress value={(me.used / total) * 100} label={t("cloud.usage.label")} />
      <p className="text-[12px] text-muted">{t("cloud.usage.text", { used: formatBytes(me.used), free: formatBytes(me.free) })}</p>
    </div>
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
