/* What a box and an edge answer, for the mock server, the pane tests and the
 * screenshots. Shapes are the web UI's (admin-ui/app/src/lib/api.ts). */

import type {
  AppsResponse,
  BackupResponse,
  ConfigResponse,
  EdgeResponse,
  OptionsResponse,
  SettingsResponse,
  StateResponse,
  StatusResponse,
  StorageResponse,
} from "@app/lib/api";

export const SETTINGS: SettingsResponse = {
  sharingMyStorage: false,
  nextcloudMode: "container",
  forgejoMode: "container",
  hostName: "mattbox",
  https: true,
  gpuEnable: false,
  apachePort: 11000,
  proxyEnable: false,
  clusterEnable: true,
  shareCompute: true,
  computeWindowStart: "23:00",
  computeWindowEnd: "07:00",
  hardeningApparmor: false,
  hardeningMalloc: true,
  hardeningNosmt: false,
  hardeningUsbguard: false,
};

export const STATE: StateResponse = { mode: "local", sharing: false };
export const STATUS: StatusResponse = { state: "idle", progress: 0, message: "" };
const GIB = 1024 ** 3;
export const STORAGE: StorageResponse = { totalBytes: 466 * GIB, usedBytes: 131 * GIB, reserveBytes: 48 * GIB };

export const OPTIONS: OptionsResponse = {
  available: true,
  version: 1,
  options: [
    {
      name: "hostName",
      group: "general",
      editor: { kind: "str" },
      nixType: "string",
      description: "The name this box answers to on the local network.",
      default: "mattbox",
      defaultText: "\"mattbox\"",
      current: "mattbox",
      danger: true,
      fixed: null,
      readOnly: false,
      set: "\"mattbox\"",
    },
    {
      name: "sharingMyStorage",
      group: "general",
      editor: { kind: "bool" },
      nixType: "boolean",
      description: "Lend spare disk to the mesh.",
      default: false,
      defaultText: "false",
      current: false,
      danger: false,
      fixed: null,
      readOnly: false,
      set: "false",
    },
    {
      name: "tpm.enable",
      group: "tpm",
      editor: { kind: "bool" },
      nixType: "boolean",
      description: "Unlock the encrypted disk with the TPM.",
      default: true,
      defaultText: "true",
      current: true,
      danger: true,
      fixed: "installer",
      readOnly: true,
      set: null,
    },
    {
      name: "nextcloud.apachePort",
      group: "nextcloud",
      editor: { kind: "int", min: 1024, max: 65535 },
      nixType: "16 bit unsigned integer",
      description: "The port the files app listens on behind the front door.",
      default: 11000,
      defaultText: "11000",
      current: 11000,
      danger: false,
      fixed: null,
      readOnly: false,
      set: "11000",
    },
    {
      name: "nextcloud.defaultPhoneRegion",
      group: "nextcloud",
      editor: { kind: "str" },
      nixType: "string",
      description: "The country phone numbers are read in when they have no prefix.",
      default: "SK",
      defaultText: "\"SK\"",
      current: "SK",
      danger: false,
      fixed: null,
      readOnly: false,
      set: null,
    },
    {
      name: "updates.extraSubstituters",
      group: "updates",
      editor: { kind: "list" },
      nixType: "list of string",
      description: "More binary caches to fetch updates from.",
      default: [],
      defaultText: "[ ]",
      current: ["https://cache.losos.dasmat.us"],
      danger: false,
      fixed: null,
      readOnly: false,
      set: "[ \"https://cache.losos.dasmat.us\" ]",
    },
  ],
  stray: [],
  excluded: {},
};

export const APPS: AppsResponse = {
  available: true,
  sharedAvailable: true,
  apps: [
    {
      release: "jellyfin",
      chart: { repo: "https://jellyfin.github.io/jellyfin-helm", name: "jellyfin", version: "2.3.0" },
      runAs: "notshared",
      values: {},
      valuesYaml: null,
      frontPort: 31080,
      appPort: 8096,
      phase: "running",
      message: null,
      updatedAt: 1791960000,
    },
    {
      release: "vaultwarden",
      chart: { repo: "https://guerzon.github.io/vaultwarden", name: "vaultwarden", version: "0.31.8" },
      runAs: "notshared",
      values: {},
      valuesYaml: null,
      frontPort: 31081,
      appPort: null,
      phase: "installing",
      message: null,
      updatedAt: 1791960100,
    },
  ],
};

export const SEARCH = {
  sources: ["Artifact Hub"],
  results: [
    {
      id: "immich",
      name: "immich",
      source: "Artifact Hub",
      summary: "Self-hosted photo and video backup.",
      version: "0.9.3",
      chart: { repo: "https://immich-app.github.io/immich-charts", name: "immich", version: "0.9.3" },
    },
    {
      id: "jellyfin",
      name: "jellyfin",
      source: "Artifact Hub",
      summary: "The free software media system.",
      version: "2.3.0",
      chart: { repo: "https://jellyfin.github.io/jellyfin-helm", name: "jellyfin", version: "2.3.0" },
    },
  ],
};

export const EDGE: EdgeResponse = {
  reachable: true,
  official: true,
  edges: [{ name: "register.losos.dasmat.us", official: true, url: "https://register.losos.dasmat.us", source: "configured" }],
  lanSearched: true,
  configuredUrl: "https://register.losos.dasmat.us",
  checkedAt: 1791960000,
};

export const CONFIG: ConfigResponse = {
  enabled: true,
  repository: { owner: "matus", name: "losos-config", url: "/forgejo/matus/losos-config", clone: "http://127.0.0.1:3000/matus/losos-config.git", viaForgejo: true },
  head: { sha: "4f2a9c1e0b7d", branch: "main" },
  log: [
    { sha: "4f2a9c1e0b7d", when: "2026-10-13T21:04:00Z", subject: "Lend compute 23:00 to 07:00" },
    { sha: "b81d3e5f2c90", when: "2026-10-12T18:30:00Z", subject: "Turn on hardened malloc" },
    { sha: "09ac77d1e4b2", when: "2026-10-10T09:12:00Z", subject: "Install the box" },
  ],
  sync: { state: "ok", detail: "Up to date with LosOS Git.", syncedAt: "2026-10-14T08:00:00Z", remoteHead: "4f2a9c1e0b7d" },
  upstream: {
    url: "https://github.com/dasmatus/losos",
    branch: "main",
    report: { state: "ok", detail: "Following the published tree.", syncedAt: "2026-10-14T03:00:00Z", remoteHead: "c0ffee123456" },
  },
};

export const BACKUP: BackupResponse = {
  target: {
    endpoint: "https://s3.eu-central-1.amazonaws.com",
    bucket: "matus-losos",
    prefix: "mattbox",
    region: "eu-central-1",
    accessKeyId: "AKIAEXAMPLE7Q",
    hasSecret: true,
    storageClass: "standard",
  },
  last: { time: 1791932400, snapshot: "a1b2c3d4", files: 48213, bytes: 92 * GIB },
  job: null,
  erase: null,
  lastErase: null,
  graceSeconds: 600,
};

/** The shape `backend-registrar/src/server/operator.rs` answers. */
export const EDGE_OVERVIEW = {
  health: true,
  version: "0.1.0",
  identity: { official: true, name: "losos edge eu-1", url: "https://register.losos.dasmat.us", notAfter: 1806451200 },
  tenants: [
    { id: "mattbox", hostname: "mattbox", seenAgoSecs: 21, via: null },
    { id: "zuzkabox", hostname: "zuzka-nas", seenAgoSecs: 48, via: null },
    { id: "office-01", hostname: "office-01", seenAgoSecs: 3900, via: null },
  ],
  enrolled: 1,
  routes: 2,
  domains: 1,
  optIns: { mesh: true, market: true, builder: false, domains: true, enrolment: false, uplink: false },
};

export const EDGE_SECRETS = [
  { name: "stripe-secret-key", sealed: true, updated: 1791835980 },
  { name: "stripe-webhook-secret", sealed: true, updated: 1791836040 },
  { name: "claude-api-key", sealed: false, updated: null },
];

/** The box's routes, as the mock serves them. */
export function boxRoute(method: string, path: string, body: unknown): { status: number; body: unknown } {
  const p = path.split("?")[0];
  if (method === "GET") {
    switch (p) {
      case "/api/health":
        return { status: 200, body: { ok: true } };
      case "/api/state":
        return { status: 200, body: STATE };
      case "/api/settings":
        return { status: 200, body: SETTINGS };
      case "/api/status":
        return { status: 200, body: STATUS };
      case "/api/storage":
        return { status: 200, body: STORAGE };
      case "/api/options":
        return { status: 200, body: OPTIONS };
      case "/api/apps":
        return { status: 200, body: APPS };
      case "/api/apps/search":
        return { status: 200, body: SEARCH };
      case "/api/apps/chart":
        return { status: 200, body: { chart: SEARCH.results[0]!.chart, release: "immich", valuesYaml: "", values: {}, schema: null } };
      case "/api/edge":
        return { status: 200, body: EDGE };
      case "/api/config":
        return { status: 200, body: CONFIG };
      case "/api/backup":
        return { status: 200, body: BACKUP };
      default:
        return { status: 404, body: { error: `no route ${p}` } };
    }
  }
  if (method === "POST") {
    switch (p) {
      case "/api/apply":
        if (typeof body !== "string" || !body.includes("losos.hostName")) return { status: 400, body: { error: "not overrides.nix" } };
        return { status: 200, body: { job: "job-1" } };
      case "/api/grow":
        return { status: 200, body: { grew: true, beforeBytes: STORAGE.totalBytes, afterBytes: 514 * GIB, claimedBytes: 48 * GIB } };
      case "/api/apps/install":
        return { status: 200, body: { ...APPS.apps[0], release: (body as { release: string }).release, phase: "installing" } };
      case "/api/apps/remove":
        return { status: 200, body: { ...APPS.apps[0], phase: "removing" } };
      case "/api/backup/run":
        return { status: 200, body: { job: "backup-1" } };
      case "/api/config/sync":
        return { status: 200, body: CONFIG };
      default:
        return { status: 404, body: { error: `no route ${p}` } };
    }
  }
  return { status: 405, body: { error: "method" } };
}

export function edgeRoute(method: string, path: string, body: unknown): { status: number; body: unknown } {
  if (method === "GET" && path === "/overview") return { status: 200, body: EDGE_OVERVIEW };
  if (method === "GET" && path === "/secrets") return { status: 200, body: EDGE_SECRETS };
  if (method === "POST" && path === "/secrets") {
    const b = body as { name?: unknown; value?: unknown } | null;
    if (typeof b?.name !== "string" || typeof b.value !== "string") return { status: 400, body: { error: "name and value" } };
    if (!EDGE_SECRETS.some((s) => s.name === b.name)) return { status: 404, body: { error: `no secret ${b.name}` } };
    return { status: 200, body: { sealed: true, name: b.name } };
  }
  if (method === "POST" && path === "/reconcile") return { status: 200, body: { reconciled: true } };
  return { status: 404, body: { error: `no route ${path}` } };
}
