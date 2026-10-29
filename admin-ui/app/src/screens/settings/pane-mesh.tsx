import * as React from "react";
import { ButtonGroup, ButtonGroupText } from "@/components/ui/button-group";
import { FieldError, Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { HelpLink } from "@/components/ui/help-link";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, Cancel01Icon, CheckmarkBadge01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";
import { Rich, useT } from "@/lib/i18n-react";
import { postEdgeCluster, postEdgeManage, useMe, type EdgePath, type EdgeProxy, type EdgeTier, type PublicTls } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { HourStrip } from "./hour-strip";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow, SwitchRow } from "./rows";
import { describeWindow } from "./window";
import { fmt } from "@/widgets/format";
import { useEdge, type EdgeView } from "./use-edge";
import type { SettingsForm } from "./use-settings-form";

/* The name every box ships with (losos.hostName's default). The edge turns
 * away a second mesh node by the same name. */
const STOCK_NAME = "mattbox";

/* Joining the mesh, and the hours this box lends while nobody is using it.
 *
 * The one thing this pane must not get wrong is whose clock the hours are on.
 * They are the OWNER's. The taint that closes the window can only be written
 * by the edge — NodeRestriction lets nobody else — so the comparison happens
 * on a machine that is not this one, and for a while it happened against the
 * edge's own clock: a 23:00→07:00 window entered in Berlin was enforced
 * 00:00→08:00 in winter and 01:00→09:00 in summer, sliding an hour at each
 * DST change, which handed strangers the first hours of the owner's working
 * day — the exact thing the feature exists to prevent. The zone now travels
 * with the two bounds and the edge evaluates each box in its own.
 *
 * So the copy says "your local time", the strip is drawn in local hours with
 * no conversion anywhere, and the caption says the hours travel with the
 * setting. That sentence is load-bearing, not reassurance.
 *
 * Above all of it sits the edge proxy. The mesh is other boxes behind an
 * edge, and lososd looks for one every few seconds (on the LAN by DNS-SD and
 * at the configured registrar) and refuses to turn joining on when none
 * answers. The first group shows that reading, and the Join switch is greyed
 * with the same reason while nothing is in reach — a switch the box is going
 * to refuse should not look like it will take. A box already joined keeps
 * its switch live, so the owner can still leave while the edge is away.
 */

/* The first group: what the box found when it last looked for an edge.
 *
 * One row per edge, because a box may have several in reach at once (its
 * own network's and the public one, say): sharing is allowed while ANY of
 * them answers, and trading only through one that passed the gateway checks
 * (its TPM showed a published LosOS image, its owner's signed passport and
 * LosOS's fee addresses). That sits next to each edge's name as a sign with
 * a tooltip: a check for a trusted gateway, a warning for any other, a stop
 * sign for one this box caught tampered and stopped using. The sign is a
 * button so the same words are its accessible name — Base UI shows no
 * tooltip on touch, and a settings row must not hide half its meaning in a
 * hover. */

type SignKind = "trusted" | "warning" | "disabled";

function signOf(edge: EdgeProxy): SignKind {
  if (edge.gateway?.state === "disabled") return "disabled";
  return edge.official ? "trusted" : "warning";
}

function EdgeSign({ edge }: { edge: EdgeProxy }) {
  const t = useT();
  const kind = signOf(edge);
  const g = edge.gateway;
  const label =
    g?.state === "trusted"
      ? t("panes.mesh.edge.officialTip", { owner: g.owner, image: g.image })
      : g?.state === "disabled"
        ? t("panes.mesh.edge.disabledTip", { reason: g.reason })
        : g?.state === "refused"
          ? t("panes.mesh.edge.refusedTip", { reason: g.reason })
          : t("panes.mesh.edge.companyTip");
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-label={label}
            data-edge-sign={kind === "trusted" ? "official" : kind}
            className={cn(
              "inline-flex size-5 shrink-0 items-center justify-center rounded-full align-middle",
              "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
              kind === "trusted" ? "text-ok" : kind === "disabled" ? "text-danger" : "text-warn",
            )}
          />
        }
      >
        <HugeiconsIcon
          icon={kind === "trusted" ? CheckmarkBadge01Icon : kind === "disabled" ? Cancel01Icon : Alert02Icon}
          size={16}
          strokeWidth={2}
        />
      </TooltipTrigger>
      <TooltipContent side="top" align="start" className="max-w-[22rem] whitespace-pre-line">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

/* Every link is one of three tiers (handbook in-depth/edge-federation,
 * "Three tiers"). Edges join edges in the main cluster: primary, which a box
 * never holds itself. A box joins a local gateway of its secondary cluster:
 * secondary. Only when no gateway of its own cluster answers does it go
 * straight to an official edge: tertiary. The first row says which one the
 * box is on now, each edge row which one that edge would be. */
function tierWord(t: ReturnType<typeof useT>, tier: EdgeTier | undefined): string {
  return t(tier === "tertiary" ? "panes.mesh.tier.tertiary" : tier === "primary" ? "panes.mesh.tier.primary" : "panes.mesh.tier.secondary");
}

function LinkRow({
  path,
  cluster,
  held,
  leftBanned,
}: {
  path: EdgePath;
  cluster: string | null;
  held: string | null;
  leftBanned: string | null;
}) {
  const t = useT();
  const tier = path.tier ?? "secondary";
  const title =
    tier === "secondary"
      ? t("panes.mesh.link.secondary", { gateway: path.member ?? path.name })
      : t("panes.mesh.link.tertiary", { name: path.name });
  const detail =
    tier === "secondary"
      ? path.cluster !== undefined
        ? t("panes.mesh.link.secondaryDetail", { cluster: path.cluster })
        : t("panes.mesh.link.secondaryLone")
      : cluster !== null
        ? t("panes.mesh.link.tertiaryFallback", { cluster })
        : t("panes.mesh.link.tertiaryDetail");
  /* A live custom domain holds the box to this edge, and a ban is the one
   * thing that moves it anyway (backend/src/edge.rs, choose_path). */
  const notes = [
    detail,
    held !== null ? t("panes.mesh.link.held") : null,
    leftBanned !== null ? t("panes.mesh.link.leftBanned", { url: leftBanned }) : null,
  ].filter((n): n is string => n !== null);
  return (
    <Row data-link-tier={tier} data-link-held={held !== null ? "yes" : "no"}>
      <RowText title={title} detail={notes.join(" ")} />
      <RowValue className={tier === "secondary" ? "text-ok" : undefined}>{tierWord(t, tier)}</RowValue>
    </Row>
  );
}

/* The box turns sharing off by itself after a long stretch with no edge and
 * back on a minute after one answers (backend/src/edge.rs, `sharing_step`).
 * While it is paused this row says so above the edges, in both states. */
function PausedRow({ at, reachable }: { at: number; reachable: boolean }) {
  const t = useT();
  return (
    <Row data-sharing="paused">
      <RowText
        title={t("panes.mesh.edge.paused", { date: fmt.date(at * 1000), time: fmt.time(at * 1000) })}
        detail={t("panes.mesh.edge.pausedDetail")}
      />
      <RowValue className={reachable ? "text-ok" : "text-warn"}>
        {t(reachable ? "panes.mesh.edge.resuming" : "panes.mesh.edge.waiting")}
      </RowValue>
    </Row>
  );
}

/* Whether the edge can read what passes through it: the box ends TLS for its
 * public names itself once it holds a certificate for them, and looks at
 * each from outside every hour (modules/public-tls.nix). */
function TlsRow({ tls }: { tls: PublicTls }) {
  const t = useT();
  const seen = Object.entries(tls.seen);
  const foreign = seen.find(([, v]) => v === "foreign")?.[0];
  const unreachable = seen.filter(([, v]) => v === "unreachable").map(([n]) => n);
  let title: string;
  let detail: string;
  let value: string;
  let tone: string;
  if (tls.own === null) {
    title = t("panes.mesh.tls.edgeReads");
    detail = t("panes.mesh.tls.edgeReadsDetail");
    value = t("panes.mesh.tls.readable");
    tone = "text-warn";
  } else if (foreign !== undefined) {
    title = t("panes.mesh.tls.foreign", { name: foreign });
    detail = t("panes.mesh.tls.foreignDetail", { name: foreign });
    value = t("panes.mesh.tls.intercepted");
    tone = "text-danger";
  } else if (tls.own.length === 0) {
    title = t("panes.mesh.tls.lapsed");
    detail = t("panes.mesh.tls.lapsedDetail");
    value = t("panes.mesh.tls.renewing");
    tone = "text-warn";
  } else {
    title = t("panes.mesh.tls.sealed");
    const notes = [t("panes.mesh.tls.sealedDetail", { names: tls.own.join(", ") })];
    if (tls.checkedAt !== null) {
      const at = Date.parse(tls.checkedAt);
      if (!Number.isNaN(at)) notes.push(t("panes.mesh.tls.checked", { time: fmt.time(at) }));
    }
    if (unreachable.length > 0) notes.push(t("panes.mesh.tls.unreachable", { names: unreachable.join(", ") }));
    detail = notes.join(" ");
    value = t("panes.mesh.tls.encrypted");
    tone = "text-ok";
  }
  return (
    <Row data-public-tls={tls.own === null ? "edge" : foreign !== undefined ? "foreign" : "box"}>
      <RowText title={title} detail={detail} />
      <RowValue className={tone}>{value}</RowValue>
    </Row>
  );
}

function edgeDetail(t: ReturnType<typeof useT>, edge: EdgeProxy): string {
  const via = t(edge.source === "configured" ? "panes.mesh.edge.viaInternet" : "panes.mesh.edge.viaLan");
  const cluster = edge.cluster ?? edge.officialCluster;
  return cluster !== undefined
    ? t("panes.mesh.edge.inCluster", { via, cluster, url: edge.url })
    : t("panes.mesh.edge.where", { via, url: edge.url });
}

export function EdgeGroup({ view }: { view: EdgeView }) {
  const t = useT();
  const { state } = view;
  const pausedAt = state.kind === "known" ? (state.edge.sharingPausedAt ?? null) : null;
  if (state.kind === "known" && state.edge.reachable) {
    const edges = state.edge.edges;
    const path = state.edge.path ?? null;
    return (
      <PaneSection>
        <GroupTitle>{t("panes.mesh.edge.title")}</GroupTitle>
        <TooltipProvider>
          <Group>
            {pausedAt !== null && <PausedRow at={pausedAt} reachable />}
            {path !== null ? <LinkRow
                path={path}
                cluster={state.edge.cluster ?? null}
                held={state.edge.held ?? null}
                leftBanned={state.edge.leftBanned ?? null}
              /> : null}
            {path !== null && state.edge.publicTls !== undefined ? (
              <TlsRow tls={state.edge.publicTls} />
            ) : null}
            {edges.map((edge, i) => {
              const inUse = path !== null && path.url === edge.url;
              const detail = edgeDetail(t, edge);
              return (
                <Row
                  key={edge.url}
                  last={i === edges.length - 1}
                  data-edge="found"
                  data-edge-official={edge.official ? "yes" : "no"}
                  data-edge-gateway={edge.gateway?.state ?? "none"}
                  data-edge-tier={edge.tier ?? "secondary"}
                  data-edge-in-use={inUse ? "yes" : "no"}
                >
                  <RowText
                    title={
                      <span className="inline-flex items-center gap-1.5">
                        <span>{t("panes.mesh.edge.found", { name: edge.member ?? edge.name })}</span>
                        <EdgeSign edge={edge} />
                      </span>
                    }
                    detail={
                      edge.gateway?.state === "trusted"
                        ? t("panes.mesh.edge.ownedBy", { url: detail, owner: edge.gateway.owner })
                        : detail
                    }
                  />
                  {edge.gateway?.state === "disabled" ? (
                    <RowValue className="text-danger">{t("panes.mesh.edge.disabled")}</RowValue>
                  ) : (
                    <RowValue className={inUse ? "text-ok" : undefined}>
                      {inUse
                        ? t("panes.mesh.edge.inUse", { tier: tierWord(t, edge.tier) })
                        : tierWord(t, edge.tier)}
                    </RowValue>
                  )}
                </Row>
              );
            })}
          </Group>
        </TooltipProvider>
        <GroupCaption>
          {t(state.edge.official ? "panes.mesh.edge.caption" : "panes.mesh.edge.captionCompany")}
        </GroupCaption>
      </PaneSection>
    );
  }
  let title: string;
  let detail: React.ReactNode;
  let value: React.ReactNode = null;
  let tone: string | undefined;
  if (state.kind === "loading") {
    title = t("panes.mesh.edge.looking");
    value = <Spinner size={16} />;
  } else if (state.kind === "failed") {
    title = t("panes.mesh.edge.unknown");
    detail = state.message;
  } else {
    title = t("panes.mesh.edge.none");
    const tried = state.edge.configuredUrl;
    const why = state.edge.lanSearched
      ? tried === null
        ? t("panes.mesh.edge.noneDetail")
        : t("panes.mesh.edge.noneTried", { url: tried })
      : t("panes.mesh.edge.lanUnsearched");
    /* The refusal the switches below are about to give has a handbook page;
     * it is linked here, where the owner reads why, rather than only from
     * the 409 toast an apply would raise. */
    detail = (
      <>
        {why} <HelpLink entry="edge-not-found" />
      </>
    );
    value = t("panes.mesh.edge.sharingOff");
    tone = "text-danger";
  }
  return (
    <PaneSection>
      <GroupTitle>{t("panes.mesh.edge.title")}</GroupTitle>
      <Group>
        {pausedAt !== null && <PausedRow at={pausedAt} reachable={false} />}
        <Row last data-edge={state.kind === "known" ? "none" : state.kind}>
          <RowText title={title} detail={detail} />
          <RowValue className={tone}>{value}</RowValue>
        </Row>
      </Group>
      <GroupCaption>{t("panes.mesh.edge.caption")}</GroupCaption>
    </PaneSection>
  );
}

/* The secondary clusters in reach. A box belongs to one: the one its owner
 * put it in here, or the one it settled in first (the one with the most
 * gateways). It stays there while none of that cluster's gateways answers,
 * going tertiary meanwhile, because a box never moves its own membership:
 * only this button does. Taking effect at once, like the manage switch. */
export function ClusterGroup({ view, locked }: { view: EdgeView; locked: boolean }) {
  const t = useT();
  const me = useMe();
  const [busy, setBusy] = React.useState<string | null>(null);
  const { state } = view;
  if (state.kind !== "known" || state.edge.clusters === undefined) return null;
  const mine = state.edge.cluster ?? null;
  const rows = [...state.edge.clusters];
  if (mine !== null && !rows.some((c) => c.name === mine)) rows.push({ name: mine, gateways: [] });
  if (rows.length === 0) return null;
  const admin = me === null || me.admin;
  const held = (state.edge.held ?? null) !== null;
  const move = async (name: string): Promise<void> => {
    setBusy(name);
    try {
      await postEdgeCluster(name);
      toast.done(t("panes.mesh.cluster.moved", { cluster: name }));
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      toast.error(t("panes.mesh.cluster.failed"), message, { help: "edge-tiers" });
    } finally {
      setBusy(null);
      view.refresh();
    }
  };
  return (
    <PaneSection>
      <GroupTitle>{t("panes.mesh.cluster.title")}</GroupTitle>
      <Group>
        {rows.map((c, i) => {
          const isMine = c.name === mine;
          return (
            <Row key={c.name} last={i === rows.length - 1} data-cluster={c.name} data-cluster-mine={isMine ? "yes" : "no"}>
              <RowText
                title={c.name}
                detail={
                  c.gateways.length === 0
                    ? t("panes.mesh.cluster.noneAnswers")
                    : t("panes.mesh.cluster.gateways", { gateways: c.gateways.join(", ") })
                }
              />
              {isMine ? (
                <RowValue className={c.gateways.length === 0 ? "text-warn" : "text-ok"}>
                  {t("panes.mesh.cluster.mine")}
                </RowValue>
              ) : admin ? (
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={locked || held || busy !== null}
                  onClick={() => void move(c.name)}
                >
                  {busy === c.name ? <Spinner size={14} /> : null}
                  {t("panes.mesh.cluster.move")}
                </Button>
              ) : null}
            </Row>
          );
        })}
      </Group>
      <GroupCaption>
        {t(held ? "panes.mesh.cluster.captionHeld" : "panes.mesh.cluster.caption")} <HelpLink entry="edge-tiers" />
      </GroupCaption>
    </PaneSection>
  );
}

/* Sharing as kindashared: the third data account, for the local gateways of
 * the network this box sits on and nothing further (losos.sharingLocally).
 * The daemon turns it on only while such a gateway answers, an official edge
 * not being one, so the switch is greyed with that reason when none does,
 * unless it is already on and must stay possible to turn off. Like the
 * mesh's switches it goes out with Apply. The last row says which account
 * the shared side's apps (chat, mail, calls, Mesh CI) run as, from the saved
 * settings: shared in the mesh, else kindashared, else none
 * (lososInternal.sideUser). */
export function LocalShareGroup({ view, form }: { view: EdgeView; form: SettingsForm }) {
  const t = useT();
  const id = React.useId();
  const edge = view.edge;
  const on = form.draft?.sharingLocally ?? false;
  const saved = form.saved;
  const refused = edge !== null && edge.localGateway === false && !(saved?.sharingLocally ?? false);
  const gateways = (edge?.edges ?? [])
    .filter((e) => e.tier === "secondary")
    .map((e) => e.member ?? e.name);
  const paused = edge?.localSharingPausedAt ?? null;
  const side =
    saved === null
      ? null
      : saved.sharingMyStorage && saved.clusterEnable
        ? "shared"
        : saved.sharingLocally
          ? "kindashared"
          : null;
  return (
    <PaneSection data-section="local-share">
      <GroupTitle>{t("panes.mesh.local.title")}</GroupTitle>
      <Group>
        {paused !== null && (
          <Row data-local-sharing="paused">
            <RowText
              title={t("panes.mesh.local.paused", { date: fmt.date(paused * 1000), time: fmt.time(paused * 1000) })}
              detail={t("panes.mesh.local.pausedDetail")}
            />
            <RowValue className={gateways.length > 0 ? "text-ok" : "text-warn"}>
              {t(gateways.length > 0 ? "panes.mesh.edge.resuming" : "panes.mesh.local.waiting")}
            </RowValue>
          </Row>
        )}
        <SwitchRow
          id={id}
          title={t("panes.mesh.local.switch")}
          description={refused && !form.locked ? t("panes.mesh.local.needed") : undefined}
          checked={on}
          disabled={form.locked || !form.ready || refused}
          onCheckedChange={(next) => form.set("sharingLocally", next)}
        />
        <Row data-local-gateways={gateways.length}>
          <RowText title={t("panes.mesh.local.gateways")} />
          <RowValue className={gateways.length > 0 ? "text-ok" : undefined}>
            {gateways.length > 0 ? gateways.join(", ") : t("panes.mesh.local.none")}
          </RowValue>
        </Row>
        <Row last data-side-user={side ?? "none"}>
          <RowText title={t("panes.mesh.local.runsAs")} />
          <RowValue className="font-mono">{side ?? t("panes.mesh.local.runsAsNone")}</RowValue>
        </Row>
      </Group>
      <GroupCaption>
        {t("panes.mesh.local.caption")} <HelpLink entry="storage-modes" />
      </GroupCaption>
    </PaneSection>
  );
}

/* A company's own edge may look after LosOS Cloud, LosOS Git and the apps
 * from its own page, once the owner says so here. The switch names the edge
 * in use and is bound to it: a box that moves to another edge is not managed
 * by that one. It takes effect at once, like the Access tokens pane, rather
 * than waiting for Apply, and only an administrator sees it. */
export function ManageGroup({ view, locked }: { view: EdgeView; locked: boolean }) {
  const t = useT();
  const id = React.useId();
  const me = useMe();
  const [busy, setBusy] = React.useState(false);
  const { state } = view;
  if (state.kind !== "known" || state.edge.manage === undefined) return null;
  if (me !== null && !me.admin) return null;
  const manage = state.edge.manage;
  const on = manage.managedBy !== null;
  const name = manage.candidate?.name ?? null;
  const description = on
    ? manage.active
      ? t("panes.mesh.manage.active", { name: name ?? manage.managedBy ?? "" })
      : t("panes.mesh.manage.elsewhere", { url: manage.managedBy ?? "" })
    : manage.refused === "noLocalEdge"
        ? t("panes.mesh.manage.noLocalEdge")
        : manage.refused === "outsideNetwork"
          ? t("panes.mesh.manage.outsideNetwork")
          : t("panes.mesh.manage.offDetail");
  const flip = async (next: boolean): Promise<void> => {
    setBusy(true);
    try {
      await postEdgeManage(next);
      toast.done(next ? t("panes.mesh.manage.turnedOn", { name: name ?? "" }) : t("panes.mesh.manage.turnedOff"));
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      toast.error(t("panes.mesh.manage.failed"), message, { help: "edge-manages-box" });
    } finally {
      setBusy(false);
      view.refresh();
    }
  };
  return (
    <PaneSection>
      <GroupTitle>{t("panes.mesh.manage.title")}</GroupTitle>
      <Group>
        <SwitchRow
          id={id}
          title={name !== null ? t("panes.mesh.manage.switchNamed", { name }) : t("panes.mesh.manage.switch")}
          description={description}
          checked={on}
          disabled={locked || busy || (!on && manage.candidate === null)}
          onCheckedChange={(next) => void flip(next)}
          option="edge-manage"
          last
        />
      </Group>
      <GroupCaption>
        {t("panes.mesh.manage.caption")} <HelpLink entry="edge-manages-box" />
      </GroupCaption>
    </PaneSection>
  );
}

export function MeshPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const edge = useEdge();
  const joinId = React.useId();
  const shareId = React.useId();
  const startId = React.useId();
  const endId = React.useId();
  const windowErrorId = React.useId();

  const draft = form.draft;
  const joined = draft?.clusterEnable ?? false;
  const sharing = draft?.shareCompute ?? false;
  const start = draft?.computeWindowStart ?? "";
  const end = draft?.computeWindowEnd ?? "";

  const disabled = form.locked || !form.ready;
  /* No edge in reach and not joined yet: the daemon would refuse the join,
   * so the switch is greyed and says why. Already joined (as saved) stays
   * live, so the owner can leave the mesh while the edge is away. */
  const joinRefused = edge.blocked && !(form.saved?.clusterEnable ?? false);
  /* The two things modules/cluster.nix asserts before a box may join, read
   * off the draft so that turning the proxy on or renaming the box in
   * Network frees the switch for the same apply. lososd refuses the same
   * apply (`check_mesh`); without this the owner met it as a failed
   * rebuild. Only turning on is held back: a box already joined can leave. */
  const needs =
    joined || (form.saved?.clusterEnable ?? false)
      ? null
      : !(draft?.proxyEnable ?? false)
        ? t("panes.mesh.needsProxy")
        : (draft?.hostName ?? "") === STOCK_NAME
          ? t("panes.mesh.needsName")
          : null;
  const joinDisabled = disabled || joinRefused || needs !== null;
  /* The window only means anything once this box has joined. Greyed rather
   * than hidden, because the shape of what joining gets you is half the
   * decision — and both halves go out in the same apply, so turning join on
   * and setting the hours is still one trip. */
  const windowDisabled = disabled || !joined;

  return (
    <>
      <EdgeGroup view={edge} />
      <ClusterGroup view={edge} locked={form.locked} />
      <LocalShareGroup view={edge} form={form} />
      <ManageGroup view={edge} locked={form.locked} />

      <PaneSection>
        <GroupTitle>{t("panes.mesh.otherBoxes")}</GroupTitle>
        <Group>
          <SwitchRow
            id={joinId}
            title={t("panes.mesh.join")}
            description={
              disabled ? undefined : joinRefused ? t("panes.mesh.edge.needed") : (needs ?? undefined)
            }
            checked={joined}
            disabled={joinDisabled}
            onCheckedChange={(next) => form.set("clusterEnable", next)}
            last
          />
        </Group>
        <GroupCaption>{t("panes.mesh.joinCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.mesh.spareTime")}</GroupTitle>
        <Group>
          <SwitchRow
            id={shareId}
            title={t("panes.mesh.lend")}
            description={windowDisabled && !disabled ? t("panes.mesh.joinFirst") : undefined}
            checked={sharing}
            disabled={windowDisabled}
            onCheckedChange={(next) => form.set("shareCompute", next)}
          />

          <StackRow>
            <HourStrip start={start} end={end} muted={!sharing || windowDisabled} />
            <p className="text-[12.5px] leading-snug text-muted" aria-hidden="true">
              {describeWindow(start, end)}
            </p>
          </StackRow>

          <Row last className="flex-wrap gap-y-2">
            {/* No `htmlFor` here: the row heads a PAIR of fields, and each
                carries its own name below. Pointing this one at the start
                field would name it "Hours Lend from" to a screen reader. */}
            <RowText title={t("panes.mesh.hours")} />
            {/* A shadcn Button Group: from, the word between, until — one
                control with shared edges, not three widgets in a row. */}
            <ButtonGroup aria-label={t("panes.mesh.hours")} className="w-full sm:w-fit">
              <label htmlFor={startId} className="sr-only">
                {t("panes.mesh.lendFrom")}
              </label>
              <Input
                id={startId}
                type="time"
                className="numeric sm:w-[9rem]"
                value={start}
                disabled={windowDisabled}
                aria-invalid={form.problems.computeWindow !== null}
                aria-describedby={
                  form.problems.computeWindow === null ? undefined : windowErrorId
                }
                onChange={(event) => form.set("computeWindowStart", event.target.value)}
              />
              <ButtonGroupText>{t("panes.mesh.until")}</ButtonGroupText>
              <label htmlFor={endId} className="sr-only">
                {t("panes.mesh.lendUntil")}
              </label>
              <Input
                id={endId}
                type="time"
                className="numeric sm:w-[9rem]"
                value={end}
                disabled={windowDisabled}
                aria-invalid={form.problems.computeWindow !== null}
                aria-describedby={
                  form.problems.computeWindow === null ? undefined : windowErrorId
                }
                onChange={(event) => form.set("computeWindowEnd", event.target.value)}
              />
            </ButtonGroup>
          </Row>
        </Group>

        <FieldError id={windowErrorId} className="px-1.5 pt-2">
          {form.problems.computeWindow}
        </FieldError>

        <GroupCaption>
          <Rich k="panes.mesh.windowCaption" vars={{ your: <em>{t("panes.mesh.your")}</em> }} />
        </GroupCaption>
      </PaneSection>
    </>
  );
}
