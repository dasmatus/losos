/* A cloud document in LosOS Office, at /cloud/office/:file. Native
 * `.paper`/`.slides` and Markdown open in LosOS editors; legacy formats
 * are imported into native LosOS Office (Collabora has been removed). */

import * as React from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort, type Entry } from "../api";
import { openOffice } from "./api";
import { MarkdownOffice } from "./markdown/editor";
import { isMarkdown } from "./markdown/new";
import { isPaper } from "./paper/new";
import { LegacyImportView } from "./import";

const PaperOffice = React.lazy(() => import("./paper/editor").then((m) => ({ default: m.PaperOffice })));

export function OfficePage() {
  const t = useT();
  const { file = "" } = useParams();
  const navigate = useNavigate();
  const load = React.useCallback(() => openOffice(file), [file]);
  const close = React.useCallback(() => {
    if (window.history.state?.idx > 0) navigate(-1);
    else navigate("/files");
  }, [navigate]);
  const onImported = React.useCallback((id: string) => navigate(`/cloud/office/${id}`), [navigate]);

  const [entry, setEntry] = React.useState<{ id: string; entry: Entry | null } | null>(null);
  React.useEffect(() => {
    const ac = new AbortController();
    const id = Number(file);
    if (!Number.isSafeInteger(id)) {
      setEntry({ id: file, entry: null });
      return;
    }
    cloud.byId(id, ac.signal).then(
      (e) => setEntry({ id: file, entry: e }),
      (error: unknown) => {
        if (!isAbort(error)) setEntry({ id: file, entry: null });
      },
    );
    return () => ac.abort();
  }, [file]);

  if (entry === null || entry.id !== file) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner size={28} label={t("office.openingPlain")} />
      </div>
    );
  }
  if (entry.entry !== null && isPaper(entry.entry.name ?? "")) {
    return (
      <React.Suspense
        fallback={
          <div className="flex h-full items-center justify-center">
            <Spinner size={28} label={t("office.openingPlain")} />
          </div>
        }
      >
        <PaperOffice key={file} entry={entry.entry} onClose={close} />
      </React.Suspense>
    );
  }
  if (entry.entry !== null && isMarkdown(entry.entry.name ?? "")) {
    return <MarkdownOffice key={file} entry={entry.entry} onClose={close} />;
  }
  return <LegacyImportView key={file} load={load} onClose={close} onImported={onImported} />;
}
