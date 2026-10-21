import * as React from "react";
import { useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Clock01Icon, Search01Icon, StarIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/progress";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort, type Entry } from "./api";
import { explain } from "./dialogs";
import { EntryTable, SelectionBar } from "./entries";
import { useRevision } from "./store";

export type ListKind = "recent" | "favorites" | "search";

const SHAPE: Record<ListKind, { title: MessageKey; emptyTitle: MessageKey; emptyBody: MessageKey; icon: typeof StarIcon }> = {
  recent: { title: "cloud.nav.recent", emptyTitle: "cloud.recent.emptyTitle", emptyBody: "cloud.recent.emptyBody", icon: Clock01Icon },
  favorites: {
    title: "cloud.nav.favorites",
    emptyTitle: "cloud.favorites.emptyTitle",
    emptyBody: "cloud.favorites.emptyBody",
    icon: StarIcon,
  },
  search: { title: "cloud.search.title", emptyTitle: "cloud.search.emptyTitle", emptyBody: "cloud.search.emptyBody", icon: Search01Icon },
};

/** A list that cuts across folders: Recent, Favorites, a search. */
export function ListView({ kind }: { kind: ListKind }) {
  const t = useT();
  const [params] = useSearchParams();
  const text = params.get("q") ?? "";
  const revision = useRevision();
  const [entries, setEntries] = React.useState<Entry[] | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const shape = SHAPE[kind];

  React.useEffect(() => {
    const ctl = new AbortController();
    const load =
      kind === "recent" ? cloud.recent(ctl.signal) : kind === "favorites" ? cloud.favorites(ctl.signal) : cloud.search(text, ctl.signal);
    load.then(
      (r) => {
        setEntries(r.entries);
        setError(null);
      },
      (err: unknown) => {
        if (!isAbort(err)) setError(explain(err));
      },
    );
    return () => ctl.abort();
  }, [kind, text, revision]);

  React.useEffect(() => {
    setEntries(null);
    setSelected(new Set());
  }, [kind, text]);

  const picked = (entries ?? []).filter((e) => selected.has(e.path));

  return (
    <div className="flex flex-col gap-4">
      <h1 className="flex min-h-9 items-center text-[19px] leading-tight font-semibold tracking-tight">
        {kind === "search" ? t("cloud.search.for", { text }) : t(shape.title)}
      </h1>
      {picked.length > 0 && <SelectionBar entries={picked} folder="" onClear={() => setSelected(new Set())} />}
      {error !== null ? (
        <Alert variant="crit">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : entries === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : entries.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={shape.icon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t(shape.emptyTitle)}</EmptyTitle>
            <EmptyDescription>{t(shape.emptyBody, { text })}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="rounded-card border border-line bg-surface">
          <EntryTable entries={entries} selected={selected} onSelectedChange={setSelected} showFolder />
        </div>
      )}
    </div>
  );
}
