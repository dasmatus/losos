/* The settings panes, second half (the port of SettingsPanes.kt): as the
 * terminal client's Settings screens and the web UI's sidebars draw them.
 * Every one reads its route read-only: a change to these is a rebuild
 * (POST /api/apply), which stays on the box's own pages; the two actions a
 * phone may honestly run, a config sync and a backup run, are the History
 * and Backup cards' buttons.
 */

import * as React from "react";
import { Text, View } from "react-native";

import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { Card, CardTitle } from "../components/Card";
import { PaneHeader } from "../components/PaneHeader";
import { PaneRow, SecurityRow } from "../components/PaneRow";
import { ReadCard } from "../components/ReadCard";
import { OptionRow } from "../components/OptionRow";
import { LosButton } from "../components/primitives";
import { failWord, type BoxApi, type JsonBody } from "../../api/api";
import {
  formatBytes,
  formatStamp,
  formatUnix,
  itemsOf,
  maybeBool,
  maybeDouble,
  maybeNumber,
  maybeObject,
  maybeString,
  shortSha,
} from "../../api/wire";
import { boxBase } from "../../store/config";
import { useBox } from "./Panes";
import { useRead } from "./Read";

const Tail = () => <View style={{ height: 20 }} />;

/* ── Network ───────────────────────────────────────────────────────────── */

export function NetworkPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  if (box.demo) {
    return (
      <View>
        <PaneHeader tokens={tokens} title="Network" summary="The box's name, and how the world reaches it" demo />
        <Card tokens={tokens} demo>
          <CardTitle tokens={tokens} title="Name" />
          <PaneRow tokens={tokens} title="Called" value="demo" />
          <PaneRow tokens={tokens} title="Reached at" value="demo.local" />
        </Card>
        <Tail />
      </View>
    );
  }
  const read = useRead(api, "/api/settings");
  return (
    <View>
      <PaneHeader tokens={tokens} title="Network" summary="The box's name, and how the world reaches it" demo={false} />
      <ReadCard tokens={tokens} title="Name" read={read}>
        {(s) => {
          const name = maybeString(s, "hostName") ?? "";
          return (
            <View>
              <PaneRow
                tokens={tokens}
                title="Called"
                value={name === "" ? "not set" : name}
                detail="A change to the name is a rebuild on the box."
              />
              <PaneRow
                tokens={tokens}
                title="Reached at"
                value={name === "" ? "-" : `${name}.local`}
              />
            </View>
          );
        }}
      </ReadCard>
      <ReadCard tokens={tokens} title="Reaching the box" read={read}>
        {(s) => (
          <PaneRow
            tokens={tokens}
            title="From outside the home network"
            detail="The reverse proxy the box can turn on for names beyond the LAN."
            value={maybeBool(s, "proxyEnable") ? "yes" : "no"}
          />
        )}
      </ReadCard>
      <Tail />
    </View>
  );
}

/* ── Security ─────────────────────────────────────────────────────────── */

export function SecurityPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  if (box.demo) {
    return (
      <View>
        <PaneHeader tokens={tokens} title="Security" summary="Switches that cost something, each a rebuild" demo />
        <Card tokens={tokens} demo>
          <CardTitle tokens={tokens} title="Hardening" />
          <SecurityRow tokens={tokens} name="usbguard" on detail="Blocks USB devices absent at boot." />
          <SecurityRow tokens={tokens} name="malloc" on={false} detail="Hardened malloc; host daemons only, not pods." />
          <SecurityRow tokens={tokens} name="apparmor" on detail="Confines less than it looks like it does." />
          <SecurityRow tokens={tokens} name="nosmt" on={false} detail="Halves the core count." />
        </Card>
        <Tail />
      </View>
    );
  }
  const read = useRead(api, "/api/settings");
  return (
    <View>
      <PaneHeader tokens={tokens} title="Security" summary="Switches that cost something, each a rebuild" demo={false} />
      <ReadCard tokens={tokens} title="Hardening" read={read}>
        {(s) => (
          <View>
            <SecurityRow tokens={tokens} name="USB Guard" on={maybeBool(s, "hardeningUsbguard")} detail="Blocks USB devices absent at boot." />
            <SecurityRow tokens={tokens} name="Hardened malloc" on={maybeBool(s, "hardeningMalloc")} detail="Host daemons only, not pods." />
            <SecurityRow tokens={tokens} name="AppArmor" on={maybeBool(s, "hardeningApparmor")} detail="Confines less than it looks like it does." />
            <SecurityRow tokens={tokens} name="No SMT" on={maybeBool(s, "hardeningNosmt")} detail="Halves the core count." />
          </View>
        )}
      </ReadCard>
      <Card tokens={tokens}>
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
          The protections that cost nothing are always on and appear nowhere. Every switch here is
          off by default; turning one on is a rebuild.
        </Text>
      </Card>
      <Tail />
    </View>
  );
}

/* ── Advanced: every option, read only ────────────────────────────────── */

export function AdvancedPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  if (box.demo) {
    return (
      <View>
        <PaneHeader tokens={tokens} title="Advanced" summary="Every option the box declares, read only" demo />
        <Card tokens={tokens} demo>
          <CardTitle tokens={tokens} title="Options" />
          <PaneRow
            tokens={tokens}
            title="losos.demo.example"
            detail="A stand-in for the box's options document."
            value="on"
          />
        </Card>
        <Tail />
      </View>
    );
  }
  const read = useRead(api, "/api/options");
  return (
    <View>
      <PaneHeader tokens={tokens} title="Advanced" summary="Every option the box declares, read only" demo={false} />
      <ReadCard tokens={tokens} title="Options" read={read}>
        {(o) => {
          if (!maybeBool(o, "available")) {
            return (
              <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
                The box has no options document right now.
              </Text>
            );
          }
          // Grouped as the Advanced pane groups them: general first, then
          // the rest in name order, hidden options left out (tui panes.rs).
          const visible = itemsOf(o, "options").filter((opt) => !maybeBool(opt, "hidden"));
          const groups = [...new Set(visible.map((opt) => maybeString(opt, "group") ?? "general"))].sort(
            (a, b) => (a === "general" ? -1 : b === "general" ? 1 : a.localeCompare(b)),
          );
          const strays = itemsOf(o, "stray");
          return (
            <View>
              {groups.map((group) => (
                <View key={group}>
                  <Text style={{ ...paneGroupName, color: tokens.palette.onSurface }}>
                    {group === "general" ? "General" : `losos.${group}`}
                  </Text>
                  {visible
                    .filter((opt) => (maybeString(opt, "group") ?? "general") === group)
                    .map((opt, i) => (
                      <OptionRow key={i} tokens={tokens} option={opt} />
                    ))}
                </View>
              ))}
              {strays.length > 0 ? (
                <View>
                  <Text style={{ ...paneGroupNameCrit, color: tokens.extra.crit, marginTop: 10 }}>
                    Stray assignments
                  </Text>
                  {strays.map((stray, i) => (
                    <PaneRow
                      key={i}
                      tokens={tokens}
                      title={`losos.${maybeString(stray, "key") ?? "-"}`}
                      detail="Overrides a line no option declares."
                      value={maybeString(stray, "value") ?? ""}
                      tone={tokens.extra.crit}
                    />
                  ))}
                </View>
              ) : null}
              <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 8 }}>
                Read only on a phone: an option changes in the web UI's editor, or Apply from losos.
              </Text>
            </View>
          );
        }}
      </ReadCard>
      <Tail />
    </View>
  );
}

const paneGroupName = { ...LOSOS_TYPE.titleMedium, marginTop: 8 } as const;
const paneGroupNameCrit = { ...paneGroupName } as const;

/* ── History ─────────────────────────────────────────────────────────── */

export function HistoryPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  if (box.demo) {
    return (
      <View>
        <PaneHeader tokens={tokens} title="History" summary="The configuration repository, and every change in it" demo />
        <Card tokens={tokens} demo>
          <CardTitle tokens={tokens} title="Commits" />
          <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
            The demo box keeps no history.
          </Text>
        </Card>
        <Tail />
      </View>
    );
  }
  const [refresh, setRefresh] = React.useState(0);
  const read = useRead(api, "/api/config", { refresh });
  return (
    <View>
      <PaneHeader tokens={tokens} title="History" summary="The configuration repository, and every change in it" demo={false} />
      <ReadCard tokens={tokens} title="Repository" read={read}>
        {(c) => repositoryBody(tokens, boxBase(api.base), c)}
      </ReadCard>
      <ReadCard tokens={tokens} title="Sync" read={read}>
        {(c) => <SyncBody tokens={tokens} api={api} c={c} onDone={() => setRefresh((n) => n + 1)} />}
      </ReadCard>
      <ReadCard tokens={tokens} title="Updates" read={read}>
        {(c) => updatesBody(tokens, c)}
      </ReadCard>
      <ReadCard tokens={tokens} title="Commits" read={read}>
        {(c) => commitsBody(tokens, c)}
      </ReadCard>
      <Tail />
    </View>
  );
}

function repositoryBody(tokens: LososTokens, base: string, c: JsonBody): React.ReactNode {
  const repo = maybeObject(c, "repository");
  if (repo === null) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        No configuration repository: the box keeps its history to itself.
      </Text>
    );
  }
  const url = maybeString(repo, "url") ?? "";
  const head = maybeObject(repo, "head");
  return (
    <View>
      <PaneRow
        tokens={tokens}
        title="On LosOS Git"
        detail={`${maybeString(repo, "owner") ?? "-"}/${maybeString(repo, "name") ?? "-"}`}
        value={`${base}${url}`}
      />
      {head !== null ? (
        <PaneRow
          tokens={tokens}
          title="Branch"
          value={maybeString(head, "branch") ?? "-"}
          detail={`head ${shortSha(maybeString(head, "sha"))}`}
        />
      ) : null}
      <PaneRow tokens={tokens} title="Clone" detail={`${base}${url}.git`} />
    </View>
  );
}

/** The one honest action a phone carries for History: POST
 *  /api/config/sync (SettingsPanes.kt SyncCard). */
function SyncBody({
  tokens,
  api,
  c,
  onDone,
}: {
  tokens: LososTokens;
  api: BoxApi;
  c: JsonBody;
  onDone: () => void;
}) {
  const [busy, setBusy] = React.useState(false);
  const [failed, setFailed] = React.useState<string | null>(null);
  const repo = maybeObject(c, "repository");
  if (repo === null) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        Nothing to sync: no repository.
      </Text>
    );
  }
  const sync = maybeObject(c, "sync");
  return (
    <View>
      {sync !== null ? (
        <PaneRow
          tokens={tokens}
          title="State"
          detail={[
            maybeString(sync, "detail") ?? "",
            maybeString(sync, "remoteHead") !== null
              ? `remote ${shortSha(maybeString(sync, "remoteHead"))}`
              : "",
          ]
            .filter((w) => w !== "")
            .join(" · ")}
          value={maybeString(sync, "state") ?? "idle"}
        />
      ) : null}
      <View style={{ height: 8 }} />
      <LosButton
        tokens={tokens}
        onPress={() => {
          setBusy(true);
          setFailed(null);
          api
            .post("/api/config/sync")
            .then(onDone)
            .catch((e: unknown) => setFailed(failWord(e)))
            .finally(() => setBusy(false));
        }}
        enabled={!busy}
        busy={busy}
      >
        {busy ? "Syncing…" : "Sync now"}
      </LosButton>
      {failed !== null ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit, marginTop: 6 }}>
          {`Could not sync: ${failed}`}
        </Text>
      ) : null}
    </View>
  );
}

function updatesBody(tokens: LososTokens, c: JsonBody): React.ReactNode {
  const upstream = maybeObject(c, "upstream");
  const url = upstream !== null ? maybeString(upstream, "url") : null;
  if (upstream === null || url === null) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        The box does not follow a published branch.
      </Text>
    );
  }
  const report = maybeObject(upstream, "report");
  return (
    <View>
      <PaneRow
        tokens={tokens}
        title="Source"
        detail={url}
        value={maybeString(upstream, "branch") ?? "-"}
        tone={tokens.palette.primary}
      />
      {report !== null ? (
        <PaneRow
          tokens={tokens}
          title="Last look"
          detail={[
            maybeString(report, "detail") ?? "",
            maybeString(report, "remoteHead") !== null
              ? `remote ${shortSha(maybeString(report, "remoteHead"))}`
              : "",
          ]
            .filter((w) => w !== "")
            .join(" · ")}
          value={maybeString(report, "state") ?? "idle"}
        />
      ) : null}
    </View>
  );
}

function commitsBody(tokens: LososTokens, c: JsonBody): React.ReactNode {
  const log = itemsOf(c, "log");
  if (log.length === 0) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        No commits yet.
      </Text>
    );
  }
  return (
    <View>
      {log.map((entry, i) => (
        <PaneRow
          key={i}
          tokens={tokens}
          title={maybeString(entry, "subject") ?? "-"}
          detail={formatStamp(maybeString(entry, "when"))}
          value={shortSha(maybeString(entry, "sha"))}
        />
      ))}
    </View>
  );
}

/* ── Backup ─────────────────────────────────────────────────────────── */

export function BackupPane({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  if (box.demo) {
    return (
      <View>
        <PaneHeader tokens={tokens} title="Backup" summary="Where the box copies itself, and the way back" demo />
        <Card tokens={tokens} demo>
          <CardTitle tokens={tokens} title="Target" />
          <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
            The demo box keeps no backups.
          </Text>
        </Card>
        <Tail />
      </View>
    );
  }
  const [refresh, setRefresh] = React.useState(0);
  const read = useRead(api, "/api/backup", { refresh });
  return (
    <View>
      <PaneHeader tokens={tokens} title="Backup" summary="Where the box copies itself, and the way back" demo={false} />
      <ReadCard tokens={tokens} title="Target" read={read}>
        {(b) => targetBody(tokens, b)}
      </ReadCard>
      <ReadCard tokens={tokens} title="Runs" read={read}>
        {(b) => <RunsBody tokens={tokens} b={b} api={api} onDone={() => setRefresh((n) => n + 1)} />}
      </ReadCard>
      <Tail />
    </View>
  );
}

function targetBody(tokens: LososTokens, b: JsonBody): React.ReactNode {
  const target = maybeObject(b, "target");
  if (target === null) {
    return (
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        No target yet: nothing is copied anywhere.
      </Text>
    );
  }
  const bucket = maybeString(target, "bucket") ?? "-";
  const prefix = maybeString(target, "prefix") ?? "";
  return (
    <View>
      <PaneRow tokens={tokens} title="Endpoint" value={maybeString(target, "endpoint") ?? "-"} />
      <PaneRow tokens={tokens} title="Bucket" value={prefix === "" ? bucket : `${bucket}/${prefix}`} />
      <PaneRow
        tokens={tokens}
        title="Class"
        value={maybeString(target, "storageClass") ?? "standard"}
      />
      <PaneRow tokens={tokens} title="Key" value={maybeString(target, "accessKeyId") ?? "-"} />
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
        The secret key stays sealed on the box.
      </Text>
    </View>
  );
}

/** The runs, and the second honest action: POST /api/backup/run
 *  (SettingsPanes.kt BackupRunButton). */
function RunsBody({
  tokens,
  b,
  api,
  onDone,
}: {
  tokens: LososTokens;
  b: JsonBody;
  api: BoxApi;
  onDone: () => void;
}) {
  const [busy, setBusy] = React.useState(false);
  const [failed, setFailed] = React.useState<string | null>(null);
  const hasTarget = maybeObject(b, "target") !== null;
  const job = maybeObject(b, "job");
  const jobState = job !== null && maybeString(job, "kind") === "backup" ? maybeString(job, "state") : null;
  const last = maybeObject(b, "last");
  return (
    <View>
      <PaneRow
        tokens={tokens}
        title="Last run"
        detail={
          last !== null
            ? `${formatBytes(maybeDouble(last, "bytes"))} · ${Math.trunc(maybeDouble(last, "files") ?? 0)} files`
            : "Never"
        }
        value={formatUnix(last !== null ? maybeNumber(last, "time") : null)}
      />
      {jobState === "running" ? (
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.primary, marginTop: 4 }}>
          Copying now…
        </Text>
      ) : null}
      {jobState === "failed" ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit, marginTop: 4 }}>
          {`The last run failed: ${job !== null ? (maybeString(job, "message") ?? "") : ""}`}
        </Text>
      ) : null}
      <View style={{ height: 8 }} />
      <LosButton
        tokens={tokens}
        onPress={() => {
          setBusy(true);
          setFailed(null);
          api
            .post("/api/backup/run")
            .then(onDone)
            .catch((e: unknown) => setFailed(failWord(e)))
            .finally(() => setBusy(false));
        }}
        enabled={hasTarget && jobState !== "running" && !busy}
        busy={busy}
      >
        {busy ? "Asking…" : "Copy now"}
      </LosButton>
      {failed !== null ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit, marginTop: 6 }}>
          {`Could not start: ${failed}`}
        </Text>
      ) : null}
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 8 }}>
        Restores live in the web UI.
      </Text>
    </View>
  );
}
