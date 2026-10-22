import * as React from "react";
import { useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { InboxIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { when } from "../format";
import { ErrorAlert, failed, Loading, PageTitle } from "../parts";
import { useLoad } from "../store";
import { tracker, type InboxItem } from "./api";
import { Assignee, StateIcon } from "./parts";
import { CreateIssue, type Draft } from "./create";
import { Reader } from "./reader";
import { useTracker } from "./store";

/* What happened to the issues one follows: given to me, a mention, a
 * comment, a change of state, a commit or pull request that names one.
 * Opening an item reads it; the issue opens beside the list. */

const KIND: Record<InboxItem["kind"], MessageKey> = {
  assigned: "tracker.inbox.assigned",
  mention: "tracker.inbox.mention",
  comment: "tracker.inbox.comment",
  state: "tracker.inbox.state",
  link: "tracker.inbox.link",
};

export function InboxScreen() {
  const t = useT();
  const { home, reloadHome } = useTracker();
  const inbox = useLoad((signal) => tracker.inbox(signal), []);
  const [params, setParams] = useSearchParams();
  const open = params.get("open");
  const [items, setItems] = React.useState<InboxItem[] | null>(null);
  const [creating, setCreating] = React.useState<Draft | null>(null);
  React.useEffect(() => {
    if (inbox.data !== null) setItems(inbox.data.items);
  }, [inbox.data]);

  const read = async (ids: number[] | undefined) => {
    setItems((list) => list?.map((i) => (ids === undefined || ids.includes(i.id) ? { ...i, read: true } : i)) ?? null);
    try {
      await tracker.markRead(ids);
      reloadHome();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };
  const openItem = (item: InboxItem) => {
    setParams({ open: item.issue.key });
    const same = (items ?? []).filter((i) => !i.read && i.ticketId === item.ticketId).map((i) => i.id);
    if (same.length > 0) void read(same);
  };
  const unread = (items ?? []).filter((i) => !i.read).length;

  return (
    <div className="flex flex-col gap-3" data-inbox>
      <div className="flex items-center gap-3">
        <PageTitle className="flex-1">{t("tracker.nav.inbox")}</PageTitle>
        <Button variant="secondary" size="sm" onClick={() => void read(undefined)} disabled={unread === 0}>
          {t("tracker.inbox.allRead")}
        </Button>
      </div>
      <Card className="flex h-[min(52rem,calc(100dvh-12rem))] min-h-[28rem] overflow-hidden">
        <section className={cn("min-w-0 flex-col md:flex md:w-96 md:flex-none md:border-r md:border-line", open !== null ? "hidden" : "flex flex-1")} aria-label={t("tracker.nav.inbox")}>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {inbox.error !== null && items === null && (
              <div className="m-3">
                <ErrorAlert error={inbox.error} />
              </div>
            )}
            {items === null && inbox.error === null && (
              <div className="p-3">
                <Loading rows={6} />
              </div>
            )}
            {items !== null && items.length === 0 && (
              <Empty className="m-3 border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={InboxIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>{t("tracker.inbox.empty")}</EmptyTitle>
                  <EmptyDescription>{t("tracker.inbox.emptyBody")}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
            <ul>
              {(items ?? []).map((item) => (
                <li key={item.id}>
                  <button
                    type="button"
                    onClick={() => openItem(item)}
                    aria-current={open === item.issue.key ? "true" : undefined}
                    className={cn("flex w-full items-start gap-3 border-b border-hair px-3 py-2.5 text-left", open === item.issue.key ? "bg-accent-wash" : "hover:bg-sunk")}
                    data-inbox-item={item.kind}
                  >
                    <Assignee login={item.actor} size="sm" className="mt-0.5" />
                    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span className="flex items-center gap-2">
                        <span aria-label={item.read ? undefined : t("tracker.inbox.unread")} className={cn("size-2 shrink-0 rounded-full", item.read ? "bg-transparent" : "bg-accent")} />
                        <span className={cn("min-w-0 flex-1 truncate text-[13.5px]", !item.read && "font-medium")}>{item.issue.title}</span>
                        <span className="shrink-0 text-[12px] text-muted tabular-nums">{when(item.created)}</span>
                      </span>
                      <span className="flex items-center gap-1.5 pl-4 text-[12.5px] text-muted">
                        <StateIcon category={item.issue.state.category} />
                        <span className="font-mono">{item.issue.key}</span>
                        <span className="truncate">
                          {t(KIND[item.kind], { who: home?.people.find((p) => p.login === item.actor)?.fullName || item.actor })}
                        </span>
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>
        <article className={cn("min-w-0 flex-1 flex-col", open !== null ? "flex" : "hidden md:flex")}>
          {open === null ? (
            <Empty className="m-4 border-0">
              <EmptyHeader>
                <EmptyDescription>{t("tracker.inbox.pick")}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : (
            <Reader
              key={open}
              issueKey={open}
              round={0}
              onBack={() => setParams({})}
              onChanged={() => {
                inbox.reload();
                reloadHome();
              }}
              onOpen={(k) => setParams({ open: k })}
              onCreateChild={(parent, project) => setCreating({ parent, project })}
              menu={null}
              onMenu={() => {}}
            />
          )}
        </article>
      </Card>
      {creating !== null && (
        <CreateIssue
          draft={creating}
          onClose={() => setCreating(null)}
          onCreated={(_, more) => {
            if (!more) setCreating(null);
            inbox.reload();
          }}
        />
      )}
    </div>
  );
}
