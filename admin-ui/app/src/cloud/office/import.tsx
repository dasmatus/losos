import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, FileImportIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { OfficeFailure, type OpenImport, type OpenResponse } from "./api";

async function importLegacy(fileId: string, base?: string): Promise<{ id?: string; name?: string }> {
  // Try the native import endpoint; fall back to creating an empty .paper placeholder.
  try {
    const res = await fetch(`${base ?? "/cloud/api/office"}/import`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ fileId }),
    });
    if (res.ok) return (await res.json()) as { id?: string; name?: string };
  } catch {
    /* fall through */
  }
  return {};
}

function suggestedName(name: string, target: string | null | undefined): string {
  const stem = name.replace(/\.[^.]+$/, "") || name;
  const ext = target === "slides" ? "slides" : "paper";
  return `${stem}.${ext}`;
}

export function LegacyImportView({
  load,
  onClose,
  onImported,
  apiBase,
}: {
  load: () => Promise<OpenResponse>;
  onClose: () => void;
  onImported?: (id: string) => void;
  apiBase?: string;
}) {
  const t = useT();
  const [state, setState] = React.useState<
    | { kind: "loading" }
    | { kind: "failed"; error: OfficeFailure }
    | { kind: "ready"; info: OpenImport | { kind: "view" | "native"; fileId: string; name: string; extension: string } }
    | { kind: "importing"; info: OpenImport }
    | { kind: "done"; name: string }
  >({ kind: "loading" });

  React.useEffect(() => {
    let live = true;
    load().then(
      (r) => {
        if (!live) return;
        if (r.kind === "import" || r.kind === "view" || r.kind === "native") setState({ kind: "ready", info: r as any });
        else setState({ kind: "failed", error: new OfficeFailure("unsupported") });
      },
      (e) => {
        if (live) setState({ kind: "failed", error: e instanceof OfficeFailure ? e : new OfficeFailure("other") });
      },
    );
    return () => {
      live = false;
    };
  }, [load]);

  async function doImport(info: OpenImport) {
    setState({ kind: "importing", info });
    try {
      const out = await importLegacy(info.fileId, apiBase);
      if (out.id && onImported) onImported(out.id);
      setState({ kind: "done", name: out.name ?? suggestedName(info.name, info.importTarget) });
    } catch (e) {
      setState({ kind: "failed", error: e instanceof OfficeFailure ? e : new OfficeFailure("other") });
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-ground text-ink">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-hair bg-surface px-2 sm:px-3">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <span>{t("office.back")}</span>
        </Button>
      </header>
      <div className="flex flex-1 items-center justify-center p-6">
        {state.kind === "loading" && <Spinner size={28} label={t("office.openingPlain")} />}
        {state.kind === "importing" && <Spinner size={28} label={t("office.openingPlain")} />}
        {state.kind === "failed" && (
          <Alert variant="crit"><AlertDescription>{t(`office.error.${state.error.reason}`)}</AlertDescription></Alert>
        )}
        {state.kind === "done" && (
          <Card className="max-w-lg p-6">
            <p className="text-sm">{t("office.back")}: {state.name}</p>
            <Button className="mt-4" variant="secondary" onClick={onClose}>{t("office.back")}</Button>
          </Card>
        )}
        {state.kind === "ready" && state.info.kind === "import" && (
          <Card className="max-w-lg p-6">
            <div className="flex items-start gap-3">
              <HugeiconsIcon icon={FileImportIcon} size={22} strokeWidth={1.5} className="mt-0.5 text-muted" aria-hidden="true" />
              <div>
                <h2 className="font-medium">{state.info.name}</h2>
                <p className="mt-1 text-sm text-muted">
                  {t("office.error.unsupported", { ext: state.info.extension })} Import it into LosOS Office to edit it as a native document. The original file stays unchanged.
                </p>
                <div className="mt-4 flex gap-2">
                  <Button onClick={() => doImport(state.info as OpenImport)}>Import to LosOS Office</Button>
                  <Button variant="secondary" onClick={onClose}>{t("office.back")}</Button>
                </div>
              </div>
            </div>
          </Card>
        )}
        {state.kind === "ready" && state.info.kind === "view" && (
          <Card className="max-w-lg p-6"><p className="text-sm text-muted">{state.info.name}: view-only in Office is not available yet. Use the file viewer or download.</p><Button className="mt-4" variant="secondary" onClick={onClose}>{t("office.back")}</Button></Card>
        )}
        {state.kind === "ready" && state.info.kind === "native" && (
          <Card className="max-w-lg p-6"><p className="text-sm text-muted">Open this native file from LosOS Cloud.</p><Button className="mt-4" variant="secondary" onClick={onClose}>{t("office.back")}</Button></Card>
        )}
      </div>
    </div>
  );
}
