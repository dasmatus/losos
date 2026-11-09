/* The box's panes as phone cards, first half (the port of Panes.kt): the
 * terminal client's panes (admin-ui/tui/src/tui/panes.rs) with the reads
 * its `needs()` lists, each one over the same /api/ routes the admin SPA
 * drives (admin-ui/app/src/lib/api.ts). A field that is missing reads as a
 * dash rather than failing the card, in the pages' spirit. A pane keeps
 * its demo shape too; while it is on, every card that carries invented
 * numbers wears the demo chip, and nothing here invents a number without
 * one.
 *
 * Read plumbing (Read/useRead/useBox) is panes/Read.tsx; the five
 * settings panes are panes/SettingsPanes.tsx, Cloud and Settings in
 * panes/CloudTab.tsx and panes/SettingsTab.tsx. Each pane splits its demo
 * and live halves into their own components - Compose keyed `LaunchedEffect`
 * per branch, React counts its hooks per render, so the halves must hook
 * under fixed identities.
 */

import * as React from "react";
import { Text, View } from "react-native";

import { useTokens } from "../theme/useTokens";
import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { Card, CardTitle } from "../components/Card";
import { PaneHeader } from "../components/PaneHeader";
import { PaneRow } from "../components/PaneRow";
import { ReadCard } from "../components/ReadCard";
import { StorageMeter } from "../components/StorageMeter";
import { EdgeCard } from "../components/EdgeCard";
import { SearchCard, type AppJob } from "../components/SearchCard";
import { AppTile, Widget } from "../components/AppTile";
import { LosButton, LosDialog } from "../components/primitives";
import { ApiError, failWord, type BoxApi, type JsonBody } from "../api/api";
import {
  describeWindow,
  encodeQuery,
  formatBytes,
  itemsOf,
  maybeBool,
  maybeDouble,
  maybeObject,
  maybeString,
} from "../api/wire";
import { boxBase, DEMO_STORAGE, updateConfig, useBoxConfig, type BoxConfig } from "../store/config";

/* ── The box API every pane builds on ───────────────────────────────────── */

/** The box API for the config, with the SPA's eviction wired: a 401 drops
 *  the token in the store, and the app shell's needsConnect shows the
 *  connect screen again (MainActivity's update flow, the way Panes.kt
 *  remembered the closure). */
export function useBox(): { box: BoxConfig; api: BoxApi } | null {
  const config = useBoxConfig();
  const host = config?.host ?? "";
  const token = config?.token ?? "";
  const api = React.useMemo(
    () => new BoxApi(host, token, () => updateConfig({ token: "" })),
    [host, token],
  );
  return config === null ? null : { box: config, api };
}

/** What every wrapped pane needs. */
export interface PaneBits {
  tokens: LososTokens;
  box: BoxConfig;
  api: BoxApi;
}

/* ── Dashboard (OverviewPane) ───────────────────────────────────────────── */

export function OverviewPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  return box.demo ? (
    <OverviewDemo tokens={tokens} />
  ) : (
    <OverviewLive tokens={tokens} api={api} />
  );
}

/** The demo half of the Overview: the fake stand-ins, every one chipped
 *  (Panes.kt DemoOverview). */
function OverviewDemo({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  return (
    <View>
      <PaneHeader tokens={tokens} title="Your box" summary="Demo data - nothing phones home" demo />
      <Card tokens={tokens} demo>
        <View style={{ flexDirection: "row", alignItems: "center" }}>
          <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: tokens.extra.ok }} />
          <View style={{ width: 10 }} />
          <View>
            <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>
              Demo mode
            </Text>
            <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
              Link it from the connect screen to read a real box
            </Text>
          </View>
        </View>
      </Card>
      <Card tokens={tokens} demo>
        <CardTitle tokens={tokens} title="Storage" />
        <StorageMeter tokens={tokens} fraction={DEMO_STORAGE.fraction} phrase={DEMO_STORAGE.phrase} />
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
          {DEMO_STORAGE.label}
        </Text>
      </Card>
      <Text style={{ ...paneLabel, color: tokens.palette.onSurface }}>Apps</Text>
      <AppTilesRow tokens={tokens} />
      <Text style={{ ...paneLabel, color: tokens.palette.onSurface }}>Widgets</Text>
      <View style={{ flexDirection: "row", alignItems: "stretch", paddingVertical: 6, marginHorizontal: -3 }}>
        <Widget tokens={tokens} title="Photos" big="1,208" sub="on the box" />
        <Widget tokens={tokens} title="Board" big="Chores" sub="3 left today" />
        <Widget tokens={tokens} title="Mail" big="42" sub="unread" />
      </View>
      <Card tokens={tokens} demo>
        <CardTitle tokens={tokens} title="Lent to the mesh" />
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          1.2 TB spare disk, replicated. Spare CPU 18:00-22:00, only when idle.
        </Text>
      </Card>
      <Tail />
    </View>
  );
}

/** The live Overview (OverviewPane's non-demo arm): health anonymous like
 *  the SPA's own header check (api.ts getHealth), then status, storage,
 *  apps and the lending card. */
function OverviewLive({ tokens, api }: { tokens: LososTokens; api: BoxApi }): React.JSX.Element {
  const health = useRead(api, "/api/health", { anonymous: true });
  const status = useRead(api, "/api/status");
  const storage = useRead(api, "/api/storage");
  const apps = useRead(api, "/api/apps");
  const settings = useRead(api, "/api/settings");
  return (
    <View>
      <PaneHeader
        tokens={tokens}
        title="Your box"
        summary={`${boxBase(api.base)}`}
        demo={false}
      />
      {/* health is anonymous, like the SPA's own header check (api.ts getHealth) */}
      <HealthCard tokens={tokens} read={health} base={boxBase(api.base)} />
      <StatusCard tokens={tokens} read={status} />
      <ReadCard tokens={tokens} title="Storage" read={storage}>
        {(d) => <StorageStrip tokens={tokens} d={d} />}
      </ReadCard>
      <ReadCard tokens={tokens} title="Apps" read={apps}>
        {(d) => <InstalledAppsBody tokens={tokens} d={d} emptyWord="Nothing installed yet." />}
      </ReadCard>
      <ReadCard tokens={tokens} title="Lent to the mesh" read={settings}>
        {(d) => <LendBody tokens={tokens} d={d} />}
      </ReadCard>
      <Tail />
    </View>
  );
}

/** The box's answering dot (Panes.kt HealthCard). */
function HealthCard({
  tokens,
  read,
  base,
}: {
  tokens: LososTokens;
  read: ReturnType<typeof useRead>;
  base: string;
}) {
  const ok = read.kind === "done" && maybeBool(read.data, "ok");
  const state =
    read.kind === "loading"
      ? "Asking…"
      : read.kind === "failed"
        ? `Box unreachable: ${read.reason}`
        : ok
          ? "Answering"
          : "Not answering";
  const dot =
    read.kind === "loading"
      ? tokens.palette.outline
      : read.kind === "failed"
        ? tokens.extra.warn
        : ok
          ? tokens.extra.ok
          : tokens.extra.crit;
  return (
    <Card tokens={tokens}>
      <View style={{ flexDirection: "row", alignItems: "center" }}>
        <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: dot }} />
        <View style={{ width: 10 }} />
        <View>
          <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>{state}</Text>
          <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>{base}</Text>
        </View>
      </View>
    </Card>
  );
}

/** The one card the Overview draws only when the box says it must
 *  (Panes.kt StatusCard: the rebuild state through /api/status). */
function StatusCard({ tokens, read }: { tokens: LososTokens; read: ReturnType<typeof useRead> }) {
  if (read.kind !== "done") return null;
  const message = maybeString(read.data, "message") ?? "";
  const state = maybeString(read.data, "state");
  if (state !== "building" && state !== "failed") return null;
  return (
    <Card tokens={tokens}>
      <Text style={{ ...LOSOS_TYPE.titleMedium, color: state === "building" ? tokens.extra.ok : tokens.extra.crit }}>
        {state === "building" ? "Applying changes" : "The last change failed"}
      </Text>
      {message !== "" ? (
        <>
          <Gap4 />
          <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>{message}</Text>
        </>
      ) : null}
    </Card>
  );
}

/** The storage strip card's body (Panes.kt StorageStripCard). */
function StorageStrip({ tokens, d }: { tokens: LososTokens; d: JsonBody }) {
  const used = maybeDouble(d, "usedBytes");
  const total = maybeDouble(d, "totalBytes");
  if (total === null || total <= 0 || used === null) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        The disk has no size to report yet.
      </Text>
    );
  }
  return (
    <View>
      <StorageMeter tokens={tokens} fraction={used / total} />
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>
        {`${formatBytes(used)} on this box`}
      </Text>
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
        {`of ${formatBytes(total)} total`}
      </Text>
    </View>
  );
}

/* What the pool gets from this box, straight out of /api/settings
 * (Panes.kt LendCard; the Mesh pane reuses it). */
export function LendBody({ tokens, d }: { tokens: LososTokens; d: JsonBody }) {
  const sharing = maybeBool(d, "sharingMyStorage");
  const joined = maybeBool(d, "clusterEnable");
  const compute = maybeBool(d, "shareCompute");
  const begin = maybeString(d, "computeWindowStart") ?? "00:00";
  const end = maybeString(d, "computeWindowEnd") ?? "00:00";
  return (
    <View>
      <PaneRow
        tokens={tokens}
        title="Spare disk"
        detail={
          sharing
            ? joined
              ? "Replicated into the pool."
              : "Waiting for a gateway of the box's cluster."
            : "Kept to this box alone."
        }
        value={sharing ? "on" : "off"}
        tone={sharing ? tokens.extra.ok : tokens.palette.onSurfaceVariant}
      />
      {sharing ? (
        <PaneRow
          tokens={tokens}
          title="Spare time"
          detail={describeWindow(begin, end)}
          value={compute ? "lent" : "never"}
          tone={compute ? tokens.extra.ok : tokens.palette.onSurfaceVariant}
        />
      ) : null}
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 6 }}>
        Lending is set on the box's own pages; a phone watches the result.
      </Text>
    </View>
  );
}

/* ── Storage (StoragePane with the guarded reserve-spend) ───────────────── */

export function StoragePane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  return entries.box.demo ? (
    <StorageDemo tokens={tokens} />
  ) : (
    <StorageLive tokens={tokens} api={entries.api} />
  );
}

function StorageDemo({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  return (
    <View>
      <PaneHeader tokens={tokens} title="Storage" summary="The disk as the box reports it" demo />
      <Card tokens={tokens} demo>
        <CardTitle tokens={tokens} title="Disk" />
        <StorageMeter tokens={tokens} fraction={DEMO_STORAGE.fraction} phrase={DEMO_STORAGE.phrase} />
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
          {DEMO_STORAGE.label}
        </Text>
      </Card>
      <Tail />
    </View>
  );
}

function StorageLive({ tokens, api }: { tokens: LososTokens; api: BoxApi }): React.JSX.Element {
  const [refresh, setRefresh] = React.useState(0);
  const read = useRead(api, "/api/storage", { refresh });
  return (
    <View>
      <PaneHeader tokens={tokens} title="Storage" summary="The disk as the box reports it" demo={false} />
      <ReadCard tokens={tokens} title="Disk" read={read}>
        {(d) => {
          const used = maybeDouble(d, "usedBytes");
          const total = maybeDouble(d, "totalBytes");
          if (total === null || total <= 0 || used === null) {
            return (
              <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
                The box does not report the disk's size yet.
              </Text>
            );
          }
          return (
            <StorageMeter
              tokens={tokens}
              fraction={used / total}
              phrase={`${formatBytes(used)} used of ${formatBytes(total)} · ${formatBytes(Math.max(0, total - used))} free`}
            />
          );
        }}
      </ReadCard>
      <ReserveCard tokens={tokens} api={api} read={read} onDone={() => setRefresh((n) => n + 1)} />
      <Tail />
    </View>
  );
}

/** The held-back space, and the guarded action to spend it (Panes.kt
 *  ReserveCard): a confirm dialog, then POST /api/grow - the online grow,
 *  no rebuild. */
function ReserveCard({
  tokens,
  api,
  read,
  onDone,
}: {
  tokens: LososTokens;
  api: BoxApi;
  read: ReturnType<typeof useRead>;
  onDone: () => void;
}) {
  const [busy, setBusy] = React.useState(false);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [grown, setGrown] = React.useState<string | null>(null);
  const [confirm, setConfirm] = React.useState<number | null>(null);

  const reserve = read.kind === "done" ? maybeDouble(read.data, "reserveBytes") : null;
  const hasReserve = reserve !== null && reserve > 0;

  const claim = () => {
    setBusy(true);
    setFailed(null);
    setGrown(null);
    api
      .post("/api/grow")
      .then((result) => {
        if (maybeBool(result, "grew")) {
          setGrown(`Claimed ${formatBytes(maybeDouble(result, "claimedBytes"))}.`);
        } else {
          setFailed("The box had nothing to claim.");
        }
        onDone();
      })
      .catch((e: unknown) => setFailed(failWord(e)))
      .finally(() => {
        setBusy(false);
        setConfirm(null);
      });
  };

  return (
    <>
      <Card tokens={tokens}>
        <CardTitle tokens={tokens} title="Held in reserve" />
        {read.kind === "loading" ? (
          <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
            Asking the box…
          </Text>
        ) : read.kind === "failed" ? (
          <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit }}>
            {`Could not read: ${read.reason}`}
          </Text>
        ) : (
          <View>
            <PaneRow
              tokens={tokens}
              title="Reserve"
              detail={
                reserve === null
                  ? "The box did not report a reserve."
                  : reserve === 0
                    ? "Already spent - the disk is that much bigger now."
                    : "Unallocated disk the box can claim without a repair."
              }
              value={formatBytes(reserve)}
            />
            {grown !== null ? (
              <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.ok, marginTop: 6 }}>
                {grown}
              </Text>
            ) : null}
            <View style={{ height: 8 }} />
            <LosButton
              tokens={tokens}
              onPress={() => setConfirm(reserve ?? 0)}
              enabled={hasReserve && !busy}
              busy={busy}
            >
              Use reserve
            </LosButton>
            {failed !== null ? (
              <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit, marginTop: 6 }}>
                {`Could not claim the reserve: ${failed}`}
              </Text>
            ) : null}
            <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 6 }}>
              Online grow: the box claims the reserve now, with no rebuild.
            </Text>
          </View>
        )}
      </Card>
      <LosDialog
        tokens={tokens}
        open={confirm !== null}
        title="Use the reserve?"
        body={
          <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>
            {confirm === null
              ? "The box will claim the unallocated disk now. This is an online grow and does not rebuild the box."
              : `The box will claim ${formatBytes(confirm)} of unallocated disk now. This is an online grow and does not rebuild the box.`}
          </Text>
        }
        confirmLabel="Use reserve"
        onConfirm={claim}
        onDismiss={() => {
          if (!busy) setConfirm(null);
        }}
        confirmEnabled={!busy}
        confirmBusy={busy}
      />
    </>
  );
}

/* ── Mesh (MeshPane) ───────────────────────────────────────────────────── */

export function MeshPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  return entries.box.demo ? (
    <MeshDemo tokens={tokens} />
  ) : (
    <MeshLive tokens={tokens} api={entries.api} />
  );
}

function MeshDemo({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  return (
    <View>
      <PaneHeader tokens={tokens} title="Mesh" summary="The pool this box lends to and buys from" demo />
      <Card tokens={tokens} demo>
        <CardTitle tokens={tokens} title="Edges in reach" />
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          The demo box sees no edges; sharing is off.
        </Text>
      </Card>
      <Tail />
    </View>
  );
}

function MeshLive({ tokens, api }: { tokens: LososTokens; api: BoxApi }): React.JSX.Element {
  const edge = useRead(api, "/api/edge");
  const settings = useRead(api, "/api/settings");
  return (
    <View>
      <PaneHeader tokens={tokens} title="Mesh" summary="The pool this box lends to and buys from" demo={false} />
      <EdgeCard tokens={tokens} read={edge} />
      <ReadCard tokens={tokens} title="Lent to the mesh" read={settings}>
        {(d) => <LendBody tokens={tokens} d={d} />}
      </ReadCard>
      <Tail />
    </View>
  );
}

/* ── Apps (AppsPane, with the guarded install/remove) ──────────────────── */

export function AppsPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  return entries.box.demo ? (
    <AppsDemo tokens={tokens} />
  ) : (
    <AppsLive tokens={tokens} api={entries.api} />
  );
}

function AppsDemo({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  return (
    <View>
      <PaneHeader tokens={tokens} title="Apps" summary="What the box runs, and what it could" demo />
      <AppTilesRow tokens={tokens} />
      <Tail />
    </View>
  );
}

function AppsLive({ tokens, api }: { tokens: LososTokens; api: BoxApi }): React.JSX.Element {
  const [refresh, setRefresh] = React.useState(0);
  const [pendingRemove, setPendingRemove] = React.useState<string | null>(null);
  const [pendingInstall, setPendingInstall] = React.useState<JsonBody | null>(null);
  const [busy, setBusy] = React.useState<AppJob | null>(null);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [notice, setNotice] = React.useState<string | null>(null);
  const read = useRead(api, "/api/apps", { refresh });

  /* App action: remove. */
  const remove = (release: string) => {
    setBusy({ release, action: "remove" });
    setFailed(null);
    setNotice(null);
    api
      .postJson("/api/apps/remove", { release })
      .then(() => {
        setNotice(`Removing ${release}…`);
        setRefresh((n) => n + 1);
      })
      .catch((e: unknown) => setFailed(failWord(e)))
      .finally(() => {
        setBusy(null);
        setPendingRemove(null);
      });
  };

  /* App action: install (Panes.kt's install arm). It fetches the box's
   * chart details, confirms the release name, then POSTs the install with
   * the notshared user and the values left at defaults. */
  const install = (hit: JsonBody) => {
    const chart = maybeObject(hit, "chart");
    if (chart === null) {
      setPendingInstall(null);
      return;
    }
    const repo = maybeString(chart, "repo") ?? "";
    const name = maybeString(chart, "name") ?? "";
    const version = maybeString(chart, "version") ?? "";
    setBusy({ release: `${repo}/${name}`, action: "install" });
    setFailed(null);
    setNotice(null);
    api
      .get(
        `/api/apps/chart?repo=${encodeQuery(repo)}&name=${encodeQuery(name)}&version=${encodeQuery(version)}`,
      )
      .then((prepared) => {
        const release = maybeString(prepared, "release");
        if (release === null) throw new ApiError(0, "The box did not return a release name.");
        const chartRef = maybeObject(prepared, "chart") ?? chart;
        return api
          .postJson("/api/apps/install", {
            release,
            chart: {
              repo: maybeString(chartRef, "repo"),
              name: maybeString(chartRef, "name"),
              version: maybeString(chartRef, "version"),
            },
            runAs: "notshared",
            values: {},
            valuesYaml: null,
          })
          .then(() => {
            setNotice(`Installing ${release}…`);
            setRefresh((n) => n + 1);
          });
      })
      .catch((e: unknown) => setFailed(failWord(e)))
      .finally(() => {
        setBusy(null);
        setPendingInstall(null);
      });
  };

  const installChart = pendingInstall !== null ? maybeObject(pendingInstall, "chart") : null;
  const installOk =
    installChart !== null &&
    (maybeString(installChart, "repo") ?? "") !== "" &&
    (maybeString(installChart, "name") ?? "") !== "" &&
    (maybeString(installChart, "version") ?? "") !== "";

  return (
    <View>
      <PaneHeader tokens={tokens} title="Apps" summary="What the box runs, and what it could" demo={false} />
      <ReadCard tokens={tokens} title="Installed" read={read}>
        {(d) => (
          <InstalledAppsBody
            tokens={tokens}
            d={d}
            emptyWord="Nothing installed yet."
            busy={busy}
            onRemove={setPendingRemove}
          />
        )}
      </ReadCard>
      <SearchCard tokens={tokens} api={api} busy={busy} onInstall={setPendingInstall} />

      {notice !== null ? (
        <Text
          style={{
            ...LOSOS_TYPE.bodySmall,
            color: tokens.extra.ok,
            paddingHorizontal: 4,
            paddingVertical: 2,
          }}
        >
          {notice}
        </Text>
      ) : null}
      {failed !== null ? (
        <Text
          style={{
            ...LOSOS_TYPE.bodySmall,
            color: tokens.extra.crit,
            paddingHorizontal: 4,
            paddingVertical: 2,
          }}
        >
          {`App action failed: ${failed}`}
        </Text>
      ) : null}

      {pendingRemove !== null ? (
        <LosDialog
          tokens={tokens}
          open
          title={`Remove ${pendingRemove}?`}
          body={
            <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>
              The app will be removed from the box. Its data files stay in the user's app data
              directory, as the box normally keeps them.
            </Text>
          }
          confirmLabel="Remove"
          onConfirm={() => remove(pendingRemove)}
          onDismiss={() => {
            if (busy === null) setPendingRemove(null);
          }}
          confirmEnabled={busy === null}
        />
      ) : null}

      {pendingInstall !== null && installChart !== null ? (
        <LosDialog
          tokens={tokens}
          open
          title={`Install ${maybeString(pendingInstall, "name") ?? "app"}?`}
          body={
            <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>
              {!installOk
                ? "This result does not include a chart the box can install."
                : `The box will fetch chart details, confirm the release name, and install ${
                    maybeString(installChart, "name") ?? ""
                  } ${maybeString(installChart, "version") ?? ""} as the notshared user (values left at defaults).`}
            </Text>
          }
          confirmLabel="Install"
          onConfirm={() => install(pendingInstall)}
          onDismiss={() => {
            if (busy === null) setPendingInstall(null);
          }}
          confirmEnabled={busy === null && installOk}
        />
      ) : null}
      <Tail />
    </View>
  );
}

/* ── The shared cards' bodies ──────────────────────────────────────────── */

/** The tiles the demo mode draws, kept from the dashboard's first drafts
 *  (Panes.kt AppTilesRow): every one chipped. */
export function AppTilesRow({ tokens }: { tokens: LososTokens }) {
  return (
    <View style={{ flexDirection: "row", paddingVertical: 6, marginHorizontal: -3 }}>
      <AppTile tokens={tokens} glyph="Cloud" name="LosOS Cloud" sub="1,204 files" />
      <AppTile tokens={tokens} glyph="Git" name="LosOS Git" sub="12 repos" />
      <AppTile tokens={tokens} glyph="Admin" name="Admin pages" sub="LAN only" />
    </View>
  );
}

/** The rows the Apps pane and the Overview both draw out of /api/apps
 *  (Panes.kt InstalledAppsBody); onRemove guards each row's phase. */
export function InstalledAppsBody({
  tokens,
  d,
  emptyWord,
  busy,
  onRemove,
}: {
  tokens: LososTokens;
  d: JsonBody;
  emptyWord: string;
  busy?: AppJob | null;
  onRemove?: (release: string) => void;
}) {
  if (!maybeBool(d, "available")) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        Apps do not run here while no storage is shared.
      </Text>
    );
  }
  const records = itemsOf(d, "apps");
  return (
    <View>
      {records.length === 0 ? (
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>{emptyWord}</Text>
      ) : null}
      {records.map((app, i) => (
        <InstalledAppRow
          key={i}
          tokens={tokens}
          app={app}
          busy={busy ?? null}
          onRemove={onRemove ?? null}
        />
      ))}
    </View>
  );
}

function InstalledAppRow({
  tokens,
  app,
  busy,
  onRemove,
}: {
  tokens: LososTokens;
  app: JsonBody;
  busy: AppJob | null;
  onRemove: ((release: string) => void) | null;
}) {
  const release = maybeString(app, "release") ?? "-";
  const phase = maybeString(app, "phase") ?? "running";
  const chart = maybeObject(app, "chart");
  const detail = [
    chart !== null ? (maybeString(chart, "name") ?? "-") : "-",
    maybeString(app, "runAs") ?? "notshared",
    ...(phase === "failed" ? [maybeString(app, "message") ?? ""] : []),
  ]
    .filter((s) => s !== "")
    .join(" · ");
  const removing = busy !== null && busy.release === release && busy.action === "remove";
  const canRemove =
    onRemove !== null &&
    phase !== "installing" &&
    phase !== "removing" &&
    busy === null &&
    release !== "-";
  return (
    <View style={{ width: "100%", paddingVertical: 2 }}>
      <PaneRow
        tokens={tokens}
        title={release}
        detail={detail}
        value={phaseWord(phase)}
        tone={
          phase === "failed"
            ? tokens.extra.crit
            : phase === "running"
              ? tokens.extra.ok
              : tokens.palette.onSurfaceVariant
        }
      />
      {onRemove !== null && (canRemove || removing) ? (
        <LosButton
          tokens={tokens}
          kind="outlined"
          onPress={() => onRemove?.(release)}
          enabled={!removing}
          busy={removing}
        >
          {removing ? "Removing…" : "Remove"}
        </LosButton>
      ) : null}
    </View>
  );
}

/** An install phase, named the pages' way (Panes.kt phaseWord). */
export function phaseWord(phase: string): string {
  switch (phase) {
    case "installing":
      return "installing";
    case "removing":
      return "removing";
    case "failed":
      return "failed";
    default:
      return "running";
  }
}

/* ── Local atoms ───────────────────────────────────────────────────────── */

const paneLabel = {
  ...LOSOS_TYPE.titleMedium,
  marginTop: 10,
  marginBottom: 4,
  marginHorizontal: 4,
} as const;

const Gap4 = () => <View style={{ height: 4 }} />;

/** The 20pt tail every pane closes with (Compose Spacer height 20). */
const Tail = () => <View style={{ height: 20 }} />;
