import * as React from "react";
import { ScrollView, Text, View } from "react-native";

import { BoxApi, failWord } from "../../api/api";
import { formatBytes, itemsOf, maybeBool, maybeDouble, maybeString } from "../../api/wire";
import { boxBase, updateConfig, useBoxConfig } from "../../store/config";
import { useRead } from "../panes/Read";
import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { TvPressable } from "./TvPressable";

/* visionOS "Living room" mode: a relaxed, glance-across-the-room dashboard.
 * It shows the same box data the normal surface reads (health/status/storage/
 * apps/settings) but rendered as big, high-contrast tiles with coarse
 * previous/next navigation and no dense editing. The wearer exits back to the
 * normal visionOS surface for anything precise.
 *
 * This is a presentation of existing app state, not a second app: it reuses
 * the same API client, endpoints and Read hook as the panes. The TVos/phone
 * paths never mount it. Spatial window/volume/ornament placement and
 * Persona/presentation APIs are native Swift concerns of the generated
 * visionOS app and/or not exposed by @callstack/react-native-visionos 0.79.6;
 * see the PR's "Notes for the reviewer". */

type PageId = "overview" | "storage" | "mesh" | "apps";

const PAGES: { id: PageId; title: string }[] = [
  { id: "overview", title: "Overview" },
  { id: "storage", title: "Storage" },
  { id: "mesh", title: "Mesh" },
  { id: "apps", title: "Apps" },
];

export function LivingRoomDashboard({
  tokens,
  onExit,
}: {
  tokens: LososTokens;
  onExit: () => void;
}): React.JSX.Element {
  const config = useBoxConfig();
  const [page, setPage] = React.useState(0);

  const api = React.useMemo(() => {
    if (config === null) return null;
    return new BoxApi(config.host, config.token, () => updateConfig({ token: "" }));
  }, [config?.host, config?.token]);

  if (config === null || api === null) {
    return <View style={{ flex: 1, backgroundColor: tokens.palette.background }} />;
  }

  const current = PAGES[page]!;
  const prev = () => setPage((p) => (p + PAGES.length - 1) % PAGES.length);
  const next = () => setPage((p) => (p + 1) % PAGES.length);

  return (
    <View style={{ flex: 1, backgroundColor: tokens.palette.background, padding: 28 }}>
      <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
        <View>
          <Text style={{ ...LOSOS_TYPE.headlineMedium, fontSize: 34, color: tokens.palette.onSurface }}>
            Living room
          </Text>
          <Text style={{ ...LOSOS_TYPE.bodyMedium, fontSize: 18, color: tokens.palette.onSurfaceVariant }}>
            {current.title} · {boxBase(api.base)}
          </Text>
        </View>
        <View style={{ flexDirection: "row", gap: 12 }}>
          <BigAction tokens={tokens} label="Previous" onPress={prev} kind="tonal" />
          <BigAction tokens={tokens} label="Next" onPress={next} kind="tonal" />
          <BigAction tokens={tokens} label="Back to normal" onPress={onExit} kind="filled" />
        </View>
      </View>

      <ScrollView contentContainerStyle={{ gap: 18, paddingBottom: 24 }}>
        {current.id === "overview" ? <OverviewTiles tokens={tokens} api={api} /> : null}
        {current.id === "storage" ? <StorageTiles tokens={tokens} api={api} /> : null}
        {current.id === "mesh" ? <MeshTiles tokens={tokens} api={api} /> : null}
        {current.id === "apps" ? <AppsTiles tokens={tokens} api={api} /> : null}
      </ScrollView>

      <Text style={{ ...LOSOS_TYPE.bodySmall, fontSize: 14, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
        Read-only glance view. Use normal mode for editing, installs, or detailed settings.
      </Text>
    </View>
  );
}

function OverviewTiles({ tokens, api }: { tokens: LososTokens; api: BoxApi }) {
  const health = useRead(api, "/api/health", { anonymous: true });
  const status = useRead(api, "/api/status");
  const storage = useRead(api, "/api/storage");
  const apps = useRead(api, "/api/apps");

  const ok = health.kind === "done" && maybeBool(health.data, "ok");
  const state = status.kind === "done" ? maybeString(status.data, "state") : null;
  const used = storage.kind === "done" ? maybeDouble(storage.data, "usedBytes") : null;
  const total = storage.kind === "done" ? maybeDouble(storage.data, "totalBytes") : null;
  const appCount = apps.kind === "done" ? itemsOf(apps.data, "apps").length : null;
  const fraction = used !== null && total !== null && total > 0 ? used / total : null;

  return (
    <View style={{ gap: 18 }}>
      <TileRow>
        <BigTile
          tokens={tokens}
          title="Box"
          value={health.kind === "loading" ? "…" : ok ? "Answering" : "Not answering"}
          detail={state && state !== "idle" ? `State: ${state}` : "Health check"}
          tone={ok ? tokens.extra.ok : tokens.extra.warn}
        />
        <BigTile
          tokens={tokens}
          title="Storage"
          value={fraction !== null ? `${Math.round(fraction * 100)}% used` : "—"}
          detail={used !== null && total !== null ? `${formatBytes(used)} of ${formatBytes(total)}` : readDetail(storage.kind)}
          tone={fraction !== null && fraction > 0.9 ? tokens.extra.crit : tokens.palette.onSurface}
        />
      </TileRow>
      <TileRow>
        <BigTile
          tokens={tokens}
          title="Apps"
          value={appCount !== null ? String(appCount) : "—"}
          detail={appCount !== null ? "installed records reported" : readDetail(apps.kind)}
          tone={tokens.palette.onSurface}
        />
        <SkiaTile
          tokens={tokens}
          fraction={fraction}
          label={fraction !== null ? "Disk fill" : "No storage reading"}
        />
      </TileRow>
    </View>
  );
}

function StorageTiles({ tokens, api }: { tokens: LososTokens; api: BoxApi }) {
  const storage = useRead(api, "/api/storage");
  const used = storage.kind === "done" ? maybeDouble(storage.data, "usedBytes") : null;
  const total = storage.kind === "done" ? maybeDouble(storage.data, "totalBytes") : null;
  const reserve = storage.kind === "done" ? maybeDouble(storage.data, "reserveBytes") : null;
  const fraction = used !== null && total !== null && total > 0 ? used / total : null;
  return (
    <View style={{ gap: 18 }}>
      <TileRow>
        <BigTile tokens={tokens} title="Used" value={formatBytes(used)} detail="reported used bytes" tone={tokens.palette.onSurface} />
        <BigTile tokens={tokens} title="Total" value={formatBytes(total)} detail="reported disk size" tone={tokens.palette.onSurface} />
      </TileRow>
      <TileRow>
        <BigTile tokens={tokens} title="Free" value={used !== null && total !== null ? formatBytes(Math.max(0, total - used)) : "—"} detail="remaining capacity" tone={tokens.palette.onSurface} />
        <BigTile tokens={tokens} title="Reserve" value={formatBytes(reserve)} detail="held back / reported reserve" tone={tokens.palette.onSurface} />
      </TileRow>
      <SkiaTile tokens={tokens} fraction={fraction} label={fraction !== null ? `${Math.round(fraction * 100)}% of disk in use` : readDetail(storage.kind)} />
    </View>
  );
}

function MeshTiles({ tokens, api }: { tokens: LososTokens; api: BoxApi }) {
  const edge = useRead(api, "/api/edge");
  const settings = useRead(api, "/api/settings");
  const sharing = settings.kind === "done" ? maybeBool(settings.data, "sharingMyStorage") : null;
  const joined = settings.kind === "done" ? maybeBool(settings.data, "clusterEnable") : null;
  const compute = settings.kind === "done" ? maybeBool(settings.data, "shareCompute") : null;
  const reachable = edge.kind === "done" ? maybeBool(edge.data, "reachable") : null;
  return (
    <View style={{ gap: 18 }}>
      <TileRow>
        <BigTile tokens={tokens} title="Edge" value={reachable === null ? readDetail(edge.kind) : reachable ? "Reachable" : "Not reachable"} detail="pool / edge status" tone={reachable ? tokens.extra.ok : tokens.palette.onSurfaceVariant} />
        <BigTile tokens={tokens} title="Sharing disk" value={sharing === null ? "—" : sharing ? "On" : "Off"} detail={joined ? "cluster joined" : "cluster state as reported"} tone={sharing ? tokens.extra.ok : tokens.palette.onSurfaceVariant} />
      </TileRow>
      <TileRow>
        <BigTile tokens={tokens} title="Sharing compute" value={compute === null ? "—" : compute ? "Lent" : "Never"} detail="spare time window follows box settings" tone={compute ? tokens.extra.ok : tokens.palette.onSurfaceVariant} />
        <BigTile tokens={tokens} title="Mode" value="Glance" detail="Coarse navigation only in living room" tone={tokens.palette.primary} />
      </TileRow>
    </View>
  );
}

function AppsTiles({ tokens, api }: { tokens: LososTokens; api: BoxApi }) {
  const apps = useRead(api, "/api/apps");
  if (apps.kind !== "done") {
    return <BigTile tokens={tokens} title="Apps" value={apps.kind === "loading" ? "…" : "Unavailable"} detail={apps.kind === "failed" ? failWord(apps.reason) : "loading"} tone={tokens.palette.onSurfaceVariant} />;
  }
  const list = itemsOf(apps.data, "apps").slice(0, 6);
  const available = maybeBool(apps.data, "available");
  if (available === false) {
    return <BigTile tokens={tokens} title="Apps" value="Unavailable" detail="Apps do not run here while no storage is shared." tone={tokens.palette.onSurfaceVariant} />;
  }
  if (list.length === 0) {
    return <BigTile tokens={tokens} title="Apps" value="None" detail="No installed apps reported." tone={tokens.palette.onSurfaceVariant} />;
  }
  return (
    <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 14 }}>
      {list.map((app, i) => {
        const release = maybeString(app, "release") ?? "app";
        const phase = maybeString(app, "phase") ?? "running";
        return (
          <View key={`${release}-${i}`} style={{ width: "48%", minWidth: 260 }}>
            <BigTile
              tokens={tokens}
              title={release}
              value={phase}
              detail="installed app"
              tone={phase === "failed" ? tokens.extra.crit : phase === "running" ? tokens.extra.ok : tokens.palette.onSurface}
            />
          </View>
        );
      })}
    </View>
  );
}

function TileRow({ children }: { children: React.ReactNode }) {
  return <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 14 }}>{children}</View>;
}

function BigTile({
  tokens,
  title,
  value,
  detail,
  tone,
}: {
  tokens: LososTokens;
  title: string;
  value: string;
  detail: string;
  tone: string;
}) {
  return (
    <View
      style={{
        flexGrow: 1,
        flexBasis: 320,
        minHeight: 180,
        borderRadius: 28,
        padding: 22,
        backgroundColor: tokens.palette.surface,
        borderWidth: 1,
        borderColor: tokens.palette.outline,
        justifyContent: "space-between",
      }}
    >
      <Text style={{ ...LOSOS_TYPE.titleMedium, fontSize: 20, color: tokens.palette.onSurfaceVariant }}>{title}</Text>
      <Text style={{ ...LOSOS_TYPE.headlineMedium, fontSize: 44, color: tone }}>{value}</Text>
      <Text style={{ ...LOSOS_TYPE.bodyMedium, fontSize: 18, color: tokens.palette.onSurfaceVariant }}>{detail}</Text>
    </View>
  );
}

function SkiaTile({
  tokens,
  fraction,
  label,
}: {
  tokens: LososTokens;
  fraction: number | null;
  label: string;
}) {
  // Skia is bundled for native targets (old arch). We render a simple,
  // dependency-light bar visual when Canvas is available; otherwise fall back
  // to a plain RN bar so typecheck/web paths stay safe.
  let SkiaCanvas: React.ComponentType<any> | null = null;
  let SkiaRect: React.ComponentType<any> | null = null;
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const skia = require("@shopify/react-native-skia") as {
      Canvas?: React.ComponentType<any>;
      Rect?: React.ComponentType<any>;
    };
    SkiaCanvas = skia.Canvas ?? null;
    SkiaRect = skia.Rect ?? null;
  } catch {
    SkiaCanvas = null;
    SkiaRect = null;
  }

  const pct = fraction !== null ? Math.max(0, Math.min(1, fraction)) : null;
  return (
    <View
      style={{
        flexGrow: 1,
        flexBasis: 320,
        minHeight: 180,
        borderRadius: 28,
        padding: 22,
        backgroundColor: tokens.palette.surface,
        borderWidth: 1,
        borderColor: tokens.palette.outline,
      }}
    >
      <Text style={{ ...LOSOS_TYPE.titleMedium, fontSize: 20, color: tokens.palette.onSurfaceVariant, marginBottom: 12 }}>
        Visual
      </Text>
      <View style={{ height: 64, borderRadius: 16, overflow: "hidden", backgroundColor: tokens.palette.surfaceVariant }}>
        {SkiaCanvas && SkiaRect && pct !== null ? (
          <SkiaCanvas style={{ flex: 1 }}>
            <SkiaRect x={0} y={0} width={900 * pct} height={64} color={pct > 0.9 ? tokens.extra.crit : tokens.palette.primary} />
          </SkiaCanvas>
        ) : (
          <View style={{ flex: 1, justifyContent: "center" }}>
            <View
              style={{
                height: 28,
                marginHorizontal: 10,
                borderRadius: 14,
                backgroundColor: tokens.palette.surface,
                overflow: "hidden",
              }}
            >
              <View
                style={{
                  height: "100%",
                  width: pct !== null ? `${Math.round(pct * 100)}%` : "0%",
                  backgroundColor: pct !== null && pct > 0.9 ? tokens.extra.crit : tokens.palette.primary,
                }}
              />
            </View>
          </View>
        )}
      </View>
      <Text style={{ ...LOSOS_TYPE.bodyMedium, fontSize: 18, color: tokens.palette.onSurface, marginTop: 12 }}>{label}</Text>
      <Text style={{ ...LOSOS_TYPE.bodySmall, fontSize: 14, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
        Skia-backed on native targets when available; plain fallback otherwise.
      </Text>
    </View>
  );
}

function BigAction({
  tokens,
  label,
  onPress,
  kind,
}: {
  tokens: LososTokens;
  label: string;
  onPress: () => void;
  kind: "filled" | "tonal";
}) {
  const bg = kind === "filled" ? tokens.palette.primary : tokens.palette.secondaryContainer;
  const fg = kind === "filled" ? tokens.palette.onPrimary : tokens.palette.onSecondaryContainer;
  return (
    <TvPressable
      onPress={onPress}
      style={{
        minWidth: 150,
        paddingHorizontal: 18,
        paddingVertical: 14,
        borderRadius: LOSOS_RADIUS.large,
        backgroundColor: bg,
        alignItems: "center",
      }}
    >
      <Text style={{ ...LOSOS_TYPE.titleMedium, fontSize: 18, color: fg }}>{label}</Text>
    </TvPressable>
  );
}

function readDetail(kind: string): string {
  if (kind === "loading") return "loading…";
  if (kind === "failed") return "could not read";
  return "reported by the box";
}
