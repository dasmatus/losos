/* A Word or OpenDocument file opened as a LosOS document, at
 * /cloud/office/:file?as=paper: the box converts it (convert.ts), the
 * document is saved beside it, and the editor opens that one in its place. */

import * as React from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { isAbort, type Entry } from "../../api";
import { openAsDocument } from "./convert";

export function ImportPage({
  entry,
  onClose,
}: {
  entry: Entry;
  onClose: () => void;
}) {
  const t = useT();
  const navigate = useNavigate();
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    const ac = new AbortController();
    openAsDocument(entry, ac.signal).then(
      (made) => navigate(`/office/${made.id}`, { replace: true }),
      (err: unknown) => {
        if (!isAbort(err))
          setError(err instanceof Error ? err.message : String(err));
      },
    );
    return () => ac.abort();
  }, [entry, navigate]);

  return (
    <div
      className="flex h-full flex-col items-center justify-center gap-4 p-6 text-center"
      data-testid="paper-import"
      data-state={error === null ? "working" : "failed"}
    >
      {error === null ? (
        <>
          <Spinner
            size={28}
            label={t("paper.import.working", { name: entry.name })}
          />
          <p className="text-sm">
            {t("paper.import.working", { name: entry.name })}
          </p>
          <p className="max-w-md text-[13px] text-muted">
            {t("paper.import.kept")}
          </p>
        </>
      ) : (
        <>
          <p className="text-sm font-medium">
            {t("paper.import.failed", { name: entry.name })}
          </p>
          <p role="alert" className="max-w-md text-[13px] text-muted">
            {error}
          </p>
          <Button variant="secondary" onClick={onClose}>
            {t("office.back")}
          </Button>
        </>
      )}
    </div>
  );
}
