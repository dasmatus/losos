/* The documents in the box's Office folder (losos.office.folder), for
 * /office/ with nothing after the #. Each row is a link to #file=<id>, which
 * the page opens in the editor; Back in the editor comes here again.
 *
 * LosOS cloud lists its own files and opens the same editor from them; this
 * list is only for a box that edits a plain folder. */

import { useEffect, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Doc01Icon,
  File01Icon,
  Pdf01Icon,
  Ppt01Icon,
  Xls01Icon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { SkeletonLine } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { formatBytes } from "@/lib/apps";
import { INTL_TAG } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { OfficeFailure } from "@/cloud/office/api";

export const FOLDER_API = "/office/api";

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
  odt: Doc01Icon,
  rtf: Doc01Icon,
  txt: Doc01Icon,
  xls: Xls01Icon,
  xlsx: Xls01Icon,
  ods: Xls01Icon,
  csv: Xls01Icon,
  ppt: Ppt01Icon,
  pptx: Ppt01Icon,
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

export function OfficeFiles() {
  const t = useT();
  const locale = useLocale();
  const [state, setState] = useState<State>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
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

  useEffect(() => {
    document.title = `${t("office.files.title")} · LosOS`;
  }, [t]);

  const when = new Intl.DateTimeFormat(INTL_TAG[locale], { dateStyle: "medium", timeStyle: "short" });
  const logo = logoFor();

  return (
    <div className="flex min-h-dvh flex-col bg-ground text-ink">
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-line bg-surface px-3">
        <img src={logo.url} alt="" className="size-6" />
        <h1 className="text-sm font-medium">{t("office.files.title")}</h1>
      </header>

      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-6" aria-busy={state.kind === "loading"}>
        {state.kind === "loading" && (
          <div className="flex flex-col gap-3" aria-label={t("office.files.loading")}>
            <SkeletonLine />
            <SkeletonLine />
            <SkeletonLine />
          </div>
        )}

        {state.kind === "failed" && (
          <div className="flex flex-col items-center gap-4 py-16 text-center">
            <p className="max-w-md text-sm text-muted">{t(`office.error.${state.error.reason}`)}</p>
            <Button variant="secondary" onClick={() => setAttempt((n) => n + 1)}>
              {t("office.retry")}
            </Button>
          </div>
        )}

        {state.kind === "ready" && state.files.length === 0 && (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={Doc01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("office.files.empty.title")}</EmptyTitle>
              <EmptyDescription>{t("office.files.empty.body")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}

        {state.kind === "ready" && state.files.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("office.files.name")}</TableHead>
                <TableHead className="max-sm:hidden">{t("office.files.modified")}</TableHead>
                <TableHead className="text-right max-sm:hidden">{t("office.files.size")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {state.files.map((f) => (
                <TableRow key={f.id} className="relative">
                  <TableCell>
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
                  <TableCell className="numeric whitespace-nowrap text-right text-muted max-sm:hidden">
                    {formatBytes(f.size)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </main>
    </div>
  );
}
