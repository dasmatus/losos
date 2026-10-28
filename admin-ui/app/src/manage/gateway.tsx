import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input, MonoInput } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  ApiError,
  claimGateway,
  clearGatewayUplink,
  forgetGatewayBox,
  getGatewaySettings,
  isAbort,
  leaveGatewayCluster,
  setGatewayCluster,
  setGatewayUplink,
  type GatewayCluster,
  type GatewayMainRow,
  type GatewaySettings,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "@/screens/settings/rows";

/* The gateway's own settings (edge/registrar/src/server/manage/gateway.rs).
 * A gateway has no console login and no SSH, so this page is the only place
 * its owner sets it up: who owns it (their signed settings), the hub it
 * relays through, and the boxes that enrolled on first contact. */

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; settings: GatewaySettings };

function message(error: unknown): string {
  return error instanceof Error ? error.message : "";
}

export function GatewaySection() {
  const t = useT();
  const [state, setState] = React.useState<State>({ kind: "loading" });
  const [round, setRound] = React.useState(0);
  const reload = React.useCallback(() => setRound((n) => n + 1), []);

  React.useEffect(() => {
    const controller = new AbortController();
    getGatewaySettings({ signal: controller.signal })
      .then((settings) => setState({ kind: "ready", settings }))
      .catch((error: unknown) => {
        if (isAbort(error)) return;
        setState({ kind: "failed", message: message(error) });
      });
    return () => controller.abort();
  }, [round]);

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <div className="flex items-center gap-2.5 text-[13px] text-muted">
          <Spinner size={16} label={t("manage.gateway.loading")} />
          {t("manage.gateway.loading")}
        </div>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    // An edge older than these routes answers 404: show the boxes only.
    return null;
  }
  const { settings } = state;
  return (
    <>
      <PaneSection data-testid="gateway">
        <GroupTitle>{t("manage.gateway.title")}</GroupTitle>
        <Group>
          <Row>
            <RowText title={t("manage.gateway.name")} />
            <span className="text-[13px] text-muted">{settings.name}</span>
          </Row>
          {settings.passport !== null && <Passport passport={settings.passport} onClaimed={reload} />}
        </Group>
        <GroupCaption>{t("manage.gateway.noShell")}</GroupCaption>
      </PaneSection>
      {settings.uplink !== null && <Uplink uplink={settings.uplink} onChange={reload} />}
      {settings.cluster != null && <Cluster joined={settings.cluster.joined} onChange={reload} />}
      {(settings.main ?? []).length > 0 && <MainCluster rows={settings.main ?? []} />}
      {settings.enrolled.length > 0 && <Enrolled ids={settings.enrolled} onChange={reload} />}
    </>
  );
}

// ── The owner ─────────────────────────────────────────────────────────────

function Passport({
  passport,
  onClaimed,
}: {
  passport: NonNullable<GatewaySettings["passport"]>;
  onClaimed: () => void;
}) {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  return (
    <>
      <Row>
        <RowText
          title={t("manage.gateway.owner")}
          detail={
            passport.owner !== null
              ? t("manage.gateway.ownerDetail", { fingerprint: passport.owner.fingerprint })
              : t("manage.gateway.unclaimed")
          }
        />
        {passport.owner !== null ? (
          <span className="truncate text-[13px] text-muted">{passport.owner.userId}</span>
        ) : (
          <Badge variant="neutral">{t("manage.gateway.noOwner")}</Badge>
        )}
      </Row>
      <Row>
        <RowText title={t("manage.gateway.edgeKey")} detail={t("manage.gateway.edgeKeyDetail")} />
        <code className="max-w-[45%] truncate font-mono text-[12px] text-muted" title={passport.edgeKey}>
          {passport.edgeKey}
        </code>
      </Row>
      {open ? (
        <ClaimForm onDone={onClaimed} onCancel={() => setOpen(false)} />
      ) : (
        <Row last>
          <RowText
            title={passport.claimed ? t("manage.gateway.change") : t("manage.gateway.claim")}
            detail={t("manage.gateway.claimDetail")}
          />
          <Button size="sm" variant="secondary" onClick={() => setOpen(true)}>
            {passport.claimed ? t("manage.gateway.changeButton") : t("manage.gateway.claimButton")}
          </Button>
        </Row>
      )}
    </>
  );
}

function ClaimForm({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const t = useT();
  const settingsId = React.useId();
  const signatureId = React.useId();
  const [settings, setSettings] = React.useState("");
  const [signature, setSignature] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      await claimGateway(settings.trim() + "\n", signature.trim() + "\n");
      toast.success(t("manage.gateway.accepted"), t("manage.gateway.acceptedDetail"));
      onDone();
    } catch (error) {
      if (error instanceof ApiError && error.status === 403) setProblem(t("manage.gateway.notOwner"));
      else setProblem(t("manage.gateway.refused", { message: message(error) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <StackRow last>
      <form className="flex flex-col gap-3" onSubmit={(event) => void submit(event)} noValidate>
        <div>
          <label htmlFor={settingsId} className="mb-1.5 block text-[13px] font-medium">
            {t("manage.gateway.settingsLabel")}
          </label>
          <Textarea
            id={settingsId}
            rows={5}
            spellCheck={false}
            className="font-mono text-[12px]"
            value={settings}
            onChange={(event) => setSettings(event.target.value)}
          />
        </div>
        <div>
          <label htmlFor={signatureId} className="mb-1.5 block text-[13px] font-medium">
            {t("manage.gateway.signatureLabel")}
          </label>
          <Textarea
            id={signatureId}
            rows={4}
            spellCheck={false}
            className="font-mono text-[12px]"
            placeholder="-----BEGIN PGP SIGNATURE-----"
            value={signature}
            onChange={(event) => setSignature(event.target.value)}
          />
        </div>
        {problem !== null && (
          <p role="alert" className="text-[12.5px] leading-snug text-crit">
            {problem}
          </p>
        )}
        <div className="flex flex-wrap justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
            {t("manage.gateway.cancel")}
          </Button>
          <Button type="submit" disabled={busy || settings.trim() === "" || signature.trim() === ""}>
            {busy && <Spinner size={14} label={t("manage.gateway.save")} />}
            {t("manage.gateway.save")}
          </Button>
        </div>
      </form>
    </StackRow>
  );
}

// ── The hub ───────────────────────────────────────────────────────────────

function Uplink({
  uplink,
  onChange,
}: {
  uplink: NonNullable<GatewaySettings["uplink"]>;
  onChange: () => void;
}) {
  const t = useT();
  const [editing, setEditing] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const set = uplink.set;

  const clear = async () => {
    setBusy(true);
    try {
      await clearGatewayUplink();
      toast.done(t("manage.uplink.cleared"));
      onChange();
    } catch (error) {
      toast.error(t("manage.uplink.failed"), message(error), { help: "index" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <PaneSection data-testid="uplink">
      <GroupTitle>{t("manage.uplink.title")}</GroupTitle>
      <Group>
        {set === null ? (
          <Row last={!editing}>
            <RowText title={t("manage.uplink.none")} detail={t("manage.uplink.noneDetail")} />
            {!editing && (
              <Button size="sm" variant="secondary" onClick={() => setEditing(true)}>
                {t("manage.uplink.set")}
              </Button>
            )}
          </Row>
        ) : (
          <Row last={!editing}>
            <RowText
              title={t("manage.uplink.via", { id: set.id, registrar: set.registrarUrl })}
              detail={t("manage.uplink.relaying", { count: uplink.relaying })}
            />
            {!editing && (
              <div className="flex shrink-0 gap-2">
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => void clear()}>
                  {t("manage.uplink.clear")}
                </Button>
                <Button size="sm" variant="secondary" disabled={busy} onClick={() => setEditing(true)}>
                  {t("manage.uplink.change")}
                </Button>
              </div>
            )}
          </Row>
        )}
        {editing && (
          <UplinkForm
            initial={set}
            onCancel={() => setEditing(false)}
            onSaved={() => {
              setEditing(false);
              onChange();
            }}
          />
        )}
      </Group>
      <GroupCaption>{t("manage.uplink.caption")}</GroupCaption>
    </PaneSection>
  );
}

function UplinkForm({
  initial,
  onCancel,
  onSaved,
}: {
  initial: { id: string; registrarUrl: string; tunnelEndpoint: string } | null;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const t = useT();
  const base = React.useId();
  const [id, setId] = React.useState(initial?.id ?? "");
  const [registrarUrl, setRegistrarUrl] = React.useState(initial?.registrarUrl ?? "");
  const [tunnelEndpoint, setTunnelEndpoint] = React.useState(initial?.tunnelEndpoint ?? "");
  const [token, setToken] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      await setGatewayUplink({
        id: id.trim(),
        token: token.trim(),
        registrarUrl: registrarUrl.trim(),
        tunnelEndpoint: tunnelEndpoint.trim(),
      });
      toast.success(t("manage.uplink.saved"), t("manage.uplink.savedDetail"));
      onSaved();
    } catch (error) {
      setProblem(message(error));
    } finally {
      setBusy(false);
    }
  };

  const field = (
    key: string,
    label: string,
    value: string,
    set: (v: string) => void,
    options: { mono?: boolean; placeholder?: string } = {},
  ) => {
    const Field = options.mono === true ? MonoInput : Input;
    return (
      <div>
        <label htmlFor={`${base}-${key}`} className="mb-1.5 block text-[13px] font-medium">
          {label}
        </label>
        <Field
          id={`${base}-${key}`}
          value={value}
          autoComplete="off"
          spellCheck={false}
          placeholder={options.placeholder}
          onChange={(event) => set(event.target.value)}
          className="w-full"
        />
      </div>
    );
  };

  return (
    <StackRow last>
      <form className="grid gap-3 sm:grid-cols-2" onSubmit={(event) => void submit(event)} noValidate>
        {field("id", t("manage.uplink.id"), id, setId, { placeholder: "acme" })}
        {field("registrar", t("manage.uplink.registrar"), registrarUrl, setRegistrarUrl, {
          placeholder: "https://register.example.com",
        })}
        {field("tunnel", t("manage.uplink.endpoint"), tunnelEndpoint, setTunnelEndpoint, {
          placeholder: "relay.example.com:2333",
        })}
        {field("token", t("manage.uplink.token"), token, setToken, { mono: true })}
        {problem !== null && (
          <Alert variant="crit" className="sm:col-span-2">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>{problem}</AlertDescription>
          </Alert>
        )}
        <div className="flex flex-wrap justify-end gap-2 sm:col-span-2">
          <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
            {t("manage.gateway.cancel")}
          </Button>
          <Button
            type="submit"
            disabled={busy || id.trim() === "" || token.trim() === "" || registrarUrl.trim() === "" || tunnelEndpoint.trim() === ""}
          >
            {busy && <Spinner size={14} label={t("manage.gateway.save")} />}
            {t("manage.gateway.save")}
          </Button>
        </div>
      </form>
    </StackRow>
  );
}

// ── The secondary cluster ─────────────────────────────────────────────────

function Cluster({ joined, onChange }: { joined: GatewayCluster | null; onChange: () => void }) {
  const t = useT();
  const [form, setForm] = React.useState<"start" | "join" | null>(null);
  const [busy, setBusy] = React.useState(false);

  const leave = async () => {
    setBusy(true);
    try {
      await leaveGatewayCluster();
      toast.done(t("manage.cluster.left"), t("manage.cluster.leftDetail"));
      onChange();
    } catch (error) {
      toast.error(t("manage.cluster.failed"), message(error), { help: "index" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <PaneSection data-testid="cluster">
      <GroupTitle>{t("manage.cluster.title")}</GroupTitle>
      <Group>
        {joined === null ? (
          <Row last={form === null}>
            <RowText title={t("manage.cluster.none")} detail={t("manage.cluster.noneDetail")} />
            {form === null && (
              <div className="flex shrink-0 gap-2">
                <Button size="sm" variant="ghost" onClick={() => setForm("join")}>
                  {t("manage.cluster.join")}
                </Button>
                <Button size="sm" variant="secondary" onClick={() => setForm("start")}>
                  {t("manage.cluster.start")}
                </Button>
              </div>
            )}
          </Row>
        ) : (
          <>
            <Row>
              <RowText
                title={t("manage.cluster.in", { name: joined.name })}
                detail={t("manage.cluster.member", { member: joined.member })}
              />
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => void leave()}>
                {busy && <Spinner size={14} label={t("manage.cluster.leave")} />}
                {t("manage.cluster.leave")}
              </Button>
            </Row>
            {joined.gateways.map((g) => (
              <Row key={g.url} data-cluster-gateway={g.member}>
                <RowText title={g.member} detail={g.url} />
              </Row>
            ))}
            <StackRow last>
              <p className="mb-2 text-[13px] text-muted">{t("manage.cluster.addAnother")}</p>
              <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[13px]" data-testid="cluster-join-line">
                <dt className="text-muted">{t("manage.cluster.name")}</dt>
                <dd>{joined.name}</dd>
                <dt className="text-muted">{t("manage.cluster.secret")}</dt>
                <dd className="break-all font-mono">{joined.secret}</dd>
                <dt className="text-muted">{t("manage.cluster.address")}</dt>
                <dd className="break-all">{joined.url}</dd>
              </dl>
            </StackRow>
          </>
        )}
        {joined === null && form !== null && (
          <ClusterForm
            joining={form === "join"}
            onCancel={() => setForm(null)}
            onSaved={() => {
              setForm(null);
              onChange();
            }}
          />
        )}
      </Group>
      <GroupCaption>{t("manage.cluster.caption")}</GroupCaption>
    </PaneSection>
  );
}

function ClusterForm({ joining, onCancel, onSaved }: { joining: boolean; onCancel: () => void; onSaved: () => void }) {
  const t = useT();
  const base = React.useId();
  const [name, setName] = React.useState("");
  const [secret, setSecret] = React.useState("");
  const [peer, setPeer] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      const joined = await setGatewayCluster(
        joining ? { name: name.trim(), secret: secret.trim(), peer: peer.trim() } : { name: name.trim() },
      );
      toast.success(
        t(joining ? "manage.cluster.joined" : "manage.cluster.started", { name: joined.name }),
        t("manage.cluster.syncs"),
      );
      onSaved();
    } catch (error) {
      setProblem(message(error));
    } finally {
      setBusy(false);
    }
  };

  const field = (key: string, label: string, value: string, set: (v: string) => void, mono = false, placeholder?: string) => {
    const Field = mono ? MonoInput : Input;
    return (
      <div>
        <label htmlFor={`${base}-${key}`} className="mb-1.5 block text-[13px] font-medium">
          {label}
        </label>
        <Field
          id={`${base}-${key}`}
          value={value}
          autoComplete="off"
          spellCheck={false}
          placeholder={placeholder}
          onChange={(event) => set(event.target.value)}
          className="w-full"
        />
      </div>
    );
  };

  const ready = name.trim() !== "" && (!joining || (secret.trim() !== "" && peer.trim() !== ""));
  return (
    <StackRow last>
      <form className="grid gap-3 sm:grid-cols-2" onSubmit={(event) => void submit(event)} noValidate>
        {field("name", t("manage.cluster.name"), name, setName, false, "office")}
        {joining && field("peer", t("manage.cluster.address"), peer, setPeer, false, "http://gw-a.local:8443")}
        {joining && <div className="sm:col-span-2">{field("secret", t("manage.cluster.secret"), secret, setSecret, true)}</div>}
        {problem !== null && (
          <Alert variant="crit" className="sm:col-span-2">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>{problem}</AlertDescription>
          </Alert>
        )}
        <div className="flex flex-wrap justify-end gap-2 sm:col-span-2">
          <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
            {t("manage.gateway.cancel")}
          </Button>
          <Button type="submit" disabled={busy || !ready}>
            {busy && <Spinner size={14} label={t("manage.gateway.save")} />}
            {t(joining ? "manage.cluster.join" : "manage.cluster.start")}
          </Button>
        </div>
      </form>
    </StackRow>
  );
}

// ── The main cluster ──────────────────────────────────────────────────────

function MainCluster({ rows }: { rows: GatewayMainRow[] }) {
  const t = useT();
  return (
    <PaneSection data-testid="main-cluster">
      <GroupTitle>{t("manage.main.title")}</GroupTitle>
      <Group>
        {rows.map((r, index) => (
          <Row key={`${r.site}/${r.gateway}`} last={index === rows.length - 1}>
            <RowText
              title={r.gateway === "" ? r.site : `${r.site} · ${r.gateway}`}
              detail={r.cluster !== null ? t("manage.main.inCluster", { cluster: r.cluster }) : undefined}
            />
            <span className="text-[13px] text-muted">{t("manage.main.boxes", { count: r.boxes })}</span>
          </Row>
        ))}
      </Group>
      <GroupCaption>{t("manage.main.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Boxes that enrolled here ──────────────────────────────────────────────

function Enrolled({ ids, onChange }: { ids: string[]; onChange: () => void }) {
  const t = useT();
  const [busy, setBusy] = React.useState<string | null>(null);
  const forget = async (id: string) => {
    setBusy(id);
    try {
      await forgetGatewayBox(id);
      toast.done(t("manage.enrolled.forgot", { id }), t("manage.enrolled.forgotDetail"));
      onChange();
    } catch (error) {
      toast.error(t("manage.enrolled.failed", { id }), message(error), { help: "index" });
    } finally {
      setBusy(null);
    }
  };
  return (
    <PaneSection data-testid="enrolled">
      <GroupTitle>{t("manage.enrolled.title")}</GroupTitle>
      <Group>
        {ids.map((id, index) => (
          <Row key={id} last={index === ids.length - 1}>
            <RowText title={id} />
            <Button size="sm" variant="ghost" disabled={busy !== null} onClick={() => void forget(id)}>
              {busy === id && <Spinner size={14} label={t("manage.enrolled.forget")} />}
              {t("manage.enrolled.forget")}
            </Button>
          </Row>
        ))}
      </Group>
      <GroupCaption>{t("manage.enrolled.caption")}</GroupCaption>
    </PaneSection>
  );
}
