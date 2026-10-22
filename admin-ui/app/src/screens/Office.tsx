/* Documents: the box's Office folder (losos.office.folder) at /office/, in
 * the shell like every other pane, and a document from it full width under
 * the top bar. The fragment says which:
 *
 *   /office/              the folder's documents
 *   /office/#file=<id>    one of them in the editor
 *
 * LosOS Cloud lists its own files and opens the same editor
 * (src/cloud/office/editor.tsx) from them. */

import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert01Icon,
  Doc01Icon,
  File01Icon,
  Pdf01Icon,
  Ppt01Icon,
  Xls01Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { SkeletonLine } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { OfficeEditor } from "@/cloud/office/editor";
import { openOffice, OfficeFailure } from "@/cloud/office/api";
import { formatBytes } from "@/lib/apps";
import { INTL_TAG } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { PaneHeader } from "@/screens/settings/rows";

export const FOLDER_API = "/office/api";

/** The list with nothing after the #, else the editor. */
export default function Office({ hash }: { hash: string }) {
  const fragment = hash.replace(/^#/, "");
  // A new document is a new editor, not the old one with another file.
  if (fragment !== "") return <OfficeDocument key={fragment} fragment={fragment} />;
  return <OfficeFiles />;
}

function OfficeDocument({ fragment }: { fragment: string }) {
  const file = new URLSearchParams(fragment).get("file") ?? "";
  const load = React.useCallback(
    () => (file ? openOffice(file, FOLDER_API) : Promise.reject(new OfficeFailure("notFound"))),
    [file],
  );
  const navigate = useNavigate();
  const close = React.useCallback(() => navigate("/office/"), [navigate]);
  return <OfficeEditor load={load} onClose={close} />;
}

// ── The folder ─────────────────────────────────────────────────────────────

export interface FolderFile {
  id: string;
  name: string;
  /** The folder under the Office folder, `/`-separated; empty at the top. */
  folder: string;
  size: number;
  /** Milliseconds since 1970. */
  modified: number;
  writable: boolean;
}

async function listFolder(): Promise<FolderFile[]> {
  const res = await fetch(`${FOLDER_API}/files`, { credentials: "same-origin" });
  if (res.status === 401) throw new OfficeFailure("signedOut");
  if (res.status === 502 || res.status === 503) throw new OfficeFailure("starting");
  if (!res.ok) throw new OfficeFailure("other");
  return ((await res.json()) as { files: FolderFile[] }).files;
}

const ICONS: Record<string, typeof File01Icon> = {
  doc: Doc01Icon,
  docx: Doc01Icon,
  docm: Doc01Icon,
  dotx: Doc01Icon,
  odt: Doc01Icon,
  rtf: Doc01Icon,
  txt: Doc01Icon,
  xls: Xls01Icon,
  xlsx: Xls01Icon,
  xlsm: Xls01Icon,
  xlsb: Xls01Icon,
  ods: Xls01Icon,
  csv: Xls01Icon,
  ppt: Ppt01Icon,
  pptx: Ppt01Icon,
  pptm: Ppt01Icon,
  ppsx: Ppt01Icon,
  odp: Ppt01Icon,
  pdf: Pdf01Icon,
};

function iconFor(name: string) {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  return ICONS[ext] ?? File01Icon;
}

type State =
  | { kind: "loading" }
  | { kind: "failed"; error: OfficeFailure }
  | { kind: "ready"; files: FolderFile[] };

function OfficeFiles() {
  const t = useT();
  const locale = useLocale();
  const [state, setState] = React.useState<State>({ kind: "loading" });
  const [attempt, setAttempt] = React.useState(0);

  React.useEffect(() => {
    let live = true;
    setState({ kind: "loading" });
    listFolder().then(
      (files) => {
        if (live) setState({ kind: "ready", files });
      },
      (e: unknown) => {
        if (live) {
          setState({ kind: "failed", error: e instanceof OfficeFailure ? e : new OfficeFailure("other") });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [attempt]);

  const when = new Intl.DateTimeFormat(INTL_TAG[locale], { dateStyle: "medium", timeStyle: "short" });

  return (
    <div aria-busy={state.kind === "loading"}>
      <PaneHeader title={t("office.files.title")} summary={t("office.files.summary")} />

      {state.kind === "loading" && (
        <Card className="flex flex-col gap-3 p-5" aria-label={t("office.files.loading")}>
          <SkeletonLine />
          <SkeletonLine />
          <SkeletonLine />
        </Card>
      )}

      {state.kind === "failed" && (
        <Alert variant="crit">
          <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} aria-hidden="true" />
          <AlertDescription className="flex flex-wrap items-center gap-3">
            <span>{t(`office.error.${state.error.reason}`)}</span>
            <Button variant="secondary" size="sm" onClick={() => setAttempt((n) => n + 1)}>
              {t("office.retry")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {state.kind === "ready" && state.files.length === 0 && (
        <Card>
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={Doc01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("office.files.empty.title")}</EmptyTitle>
              <EmptyDescription>{t("office.files.empty.body")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        </Card>
      )}

      {state.kind === "ready" && state.files.length > 0 && (
        <Card className="overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-5">{t("office.files.name")}</TableHead>
                <TableHead className="max-sm:hidden">{t("office.files.modified")}</TableHead>
                <TableHead className="pr-5 text-right max-sm:hidden">{t("office.files.size")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {state.files.map((f) => (
                <TableRow key={f.id} className="relative">
                  <TableCell className="pl-5">
                    <a
                      href={`#file=${f.id}`}
                      className="flex min-w-0 items-center gap-3 outline-none after:absolute after:inset-0 focus-visible:after:ring-2 focus-visible:after:ring-accent/40"
                    >
                      <HugeiconsIcon
                        icon={iconFor(f.name)}
                        size={20}
                        strokeWidth={1.5}
                        className="shrink-0 text-muted"
                        aria-hidden="true"
                      />
                      <span className="min-w-0">
                        <span className="block truncate font-medium">{f.name}</span>
                        {f.folder && <span className="block truncate text-xs text-muted">{f.folder}</span>}
                      </span>
                      {!f.writable && <Badge variant="secondary">{t("office.readOnly")}</Badge>}
                    </a>
                  </TableCell>
                  <TableCell className="whitespace-nowrap text-muted max-sm:hidden">
                    {when.format(new Date(f.modified))}
                  </TableCell>
                  <TableCell className="numeric whitespace-nowrap pr-5 text-right text-muted max-sm:hidden">
                    {formatBytes(f.size)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
