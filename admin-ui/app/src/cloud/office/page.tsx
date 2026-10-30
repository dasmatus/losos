/* A cloud document in LosOS Office, at /cloud/office/:file. The editor
 * takes the width and the height under the cloud's top bar, like a text
 * document at /cloud/edit/:file. A `.paper` or `.slides` file is LosOS
 * Office's own (paper/), a Markdown file opens in the Markdown editor
 * (markdown/), and everything else is Collabora's. */

import * as React from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort, type Entry } from "../api";
import { OfficeEditor } from "./editor";
import { openOffice } from "./api";
import { MarkdownOffice } from "./markdown/editor";
import { isMarkdown } from "./markdown/new";
import { isPaper } from "./paper/new";

const PaperOffice = React.lazy(() => import("./paper/editor").then((m) => ({ default: m.PaperOffice })));

export function OfficePage() {
  const t = useT();
  const { file = "" } = useParams();
  const navigate = useNavigate();
  const load = React.useCallback(() => openOffice(file), [file]);
  // Back where the person came from; a document opened in a tab of its own
  // goes to the files.
  const close = React.useCallback(() => {
    if (window.history.state?.idx > 0) navigate(-1);
    else navigate("/files");
  }, [navigate]);

  // Which editor is the file's name's business.
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
  // Anything else, and a file the list cannot name, is Collabora's: its
  // editor says what is wrong in its own words.
  return <OfficeEditor key={file} load={load} onClose={close} />;
}
