import * as React from "react";
import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ComputerCloudIcon, MinusSignIcon, PlusSignIcon } from "@hugeicons/core-free-icons";
import { Badge, StatusDot, type DotState } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { FieldError, Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOptGroup, NativeSelectOption } from "@/components/ui/native-select";
import { Progress, Spinner } from "@/components/ui/progress";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import type { MarketAccount, MarketOrder, MarketShelfListing, VmCatalogue, VmMachine } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { paneHref } from "@/lib/routes";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { formatBytes, useMachines, type MachinesData } from "./machines";
import { formatDay, formatMoney, isStripePage, minorDigits, toMinorUnits, ORDER_STATUS } from "./market";
import { paneById } from "./panes";

/* Virtual machines on the mesh, sold by the replica.
 *
 * The offer is for boxes that share their disk with the mesh and for no
 * others: lososd answers `notSharing` otherwise and refuses every action
 * (backend/src/losos.rs, cmd_vms), and this pane then draws one greyed
 * sentence saying so and where the switch is. The edge runs the machines on
 * KubeVirt, on boxes that host them, and imports each replica's disk from
 * the catalogue (LosOS and the systems Quickemu knows that ship a cloud
 * image) or from an image this box uploaded.
 *
 * Four groups: start a machine (image, replicas, a host's offer, the price
 * and its half-and-half split), the box's own images, its machines and what
 * each replica is doing, and the host side (payouts and an offer per
 * replica-month). Payment is Stripe's page in a new tab, exactly as on the
 * market pane; nothing here sees a card.
 */

const STATUS_DOT: Record<string, DotState> = {
  Running: "ok",
  Starting: "pending",
  Preparing: "pending",
  Paused: "warn",
  Stopped: "idle",
  Failed: "crit",
};

const STATUS_WORD: Record<string, MessageKey> = {
  Running: "machines.replica.running",
  Starting: "machines.replica.starting",
  Preparing: "machines.replica.preparing",
  Paused: "machines.replica.paused",
  Stopped: "machines.replica.stopped",
  Failed: "machines.replica.failed",
};

function replicaWord(status: string, t: ReturnType<typeof useT>): string {
  const key = STATUS_WORD[status];
  return key === undefined ? status : t(key);
}

export function MachinesPane({ locked }: { locked: boolean }) {
  const t = useT();
  const machines = useMachines(!locked);
  const { state } = machines;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("machines.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("machines.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={machines.refresh}>
              {t("machines.refresh")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return <Unavailable reason={state.reason} />;
  }

  const disabled = locked || machines.busy || machines.uploading !== null;
  return (
    <>
      <StartSection
        catalogue={state.catalogue}
        listings={state.listings}
        account={state.account}
        machines={machines}
        disabled={disabled}
      />
      <MineSection account={state.account} catalogue={state.catalogue} statuses={state.machines} />
      <ImagesSection catalogue={state.catalogue} account={state.account} machines={machines} disabled={disabled} />
      <HostSection catalogue={state.catalogue} account={state.account} machines={machines} disabled={disabled} />
    </>
  );
}

/* Greyed: the whole pane is one sentence while machines are not offered. */
function Unavailable({ reason }: { reason: "notSharing" | "noOfficialEdge" | "notOffered" }) {
  const t = useT();
  const market = paneById("market");
  return (
    <PaneSection>
      <Group data-testid="machines-unavailable" aria-disabled="true">
        <Empty className="border-0 p-5 opacity-70 md:p-8">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={ComputerCloudIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t(`machines.unavailable.${reason}.title`)}</EmptyTitle>
            <EmptyDescription>
              {t(`machines.unavailable.${reason}.detail`)}
              {reason === "notSharing" && market.planned && ` ${t("machines.unavailable.notSharing.planned")}`}
            </EmptyDescription>
          </EmptyHeader>
          {reason === "notSharing" && !market.planned && (
            <EmptyContent>
              <Link to={paneHref("market")} className={buttonVariants({ variant: "secondary", size: "sm" })}>
                {t("machines.unavailable.notSharing.open")}
              </Link>
            </EmptyContent>
          )}
        </Empty>
      </Group>
    </PaneSection>
  );
}

// ── Start a machine ───────────────────────────────────────────────────────

interface Choice {
  id: string;
  name: string;
  cloudInit: boolean;
  diskGib: number;
  memoryMib: number;
}

function choices(catalogue: VmCatalogue, account: MarketAccount): { offered: Choice[]; own: Choice[] } {
  const { shape } = catalogue;
  return {
    offered: catalogue.images.map((image) => ({
      id: image.id,
      name: image.name,
      cloudInit: image.cloud_init,
      diskGib: image.disk_gib,
      memoryMib: image.memory_mib,
    })),
    own: (account.uploads ?? [])
      .filter((upload) => upload.stored)
      .map((upload) => ({
        id: upload.id,
        name: upload.name,
        // An uploaded image may read cloud-init or not; the box cannot
        // tell, so the field stays offered and does nothing if it does not.
        cloudInit: true,
        diskGib: Math.max(shape.disk_gib, upload.min_disk_gib),
        memoryMib: shape.memory_mib,
      })),
  };
}

function StartSection({
  catalogue,
  listings,
  account,
  machines,
  disabled,
}: {
  catalogue: VmCatalogue;
  listings: MarketShelfListing[];
  account: MarketAccount;
  machines: MachinesData;
  disabled: boolean;
}) {
  const t = useT();
  const imageId = React.useId();
  const nameId = React.useId();
  const hostId = React.useId();
  const replicasId = React.useId();
  const userDataId = React.useId();
  const problemId = React.useId();
  const { offered, own } = choices(catalogue, account);
  const all = [...offered, ...own];
  const [image, setImage] = React.useState(offered[0]?.id ?? "");
  const [name, setName] = React.useState("");
  const [listingId, setListingId] = React.useState(listings[0]?.id ?? "");
  const [replicas, setReplicas] = React.useState(1);
  const [userData, setUserData] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);

  const chosen = all.find((c) => c.id === image) ?? all[0];
  const listing = listings.find((l) => l.id === listingId) ?? listings[0];
  const most = Math.max(1, Math.min(catalogue.max_replicas, listing?.available ?? 1));
  const count = Math.min(replicas, most);
  const half = catalogue.fee_bps / 100;

  const start = async () => {
    const trimmed = name.trim();
    if (chosen === undefined || listing === undefined) return;
    if (trimmed.length === 0 || trimmed.length > 40) {
      setProblem(t("machines.start.nameInvalid"));
      return;
    }
    setProblem(null);
    const checkoutTab = window.open("about:blank", "_blank");
    if (checkoutTab === null) {
      toast.error(t("machines.actionFailedTitle"), t("panes.market.popupBlocked"), { help: "index" });
      return;
    }
    checkoutTab.opener = null;
    const reply = await machines.order({
      listingId: listing.id,
      replicas: count,
      image: chosen.id,
      name: trimmed,
      userData: chosen.cloudInit ? userData : "",
    });
    if (reply !== null && isStripePage(reply.checkout_url)) {
      checkoutTab.location.href = reply.checkout_url;
      toast.status(t("panes.market.orderPlaced"), t("machines.start.paying"), {
        id: ORDER_STATUS,
        duration: Infinity,
      });
      setName("");
      setUserData("");
    } else {
      checkoutTab.close();
      if (reply !== null) {
        toast.error(t("machines.actionFailedTitle"), t("panes.market.noCheckout"), { help: "index" });
      }
    }
  };

  return (
    <PaneSection>
      <GroupTitle>{t("machines.start.title")}</GroupTitle>
      <Group data-testid="machines-start">
        <Row>
          <RowText
            title={t("machines.start.image")}
            htmlFor={imageId}
            detail={
              chosen === undefined
                ? undefined
                : t("machines.start.shape", {
                    cpu: catalogue.shape.cpu,
                    memory: Math.round(chosen.memoryMib / 1024),
                    disk: chosen.diskGib,
                  })
            }
          />
          <NativeSelect
            id={imageId}
            className="w-56 max-w-full"
            value={chosen?.id ?? ""}
            disabled={disabled}
            onChange={(event) => setImage(event.target.value)}
          >
            <NativeSelectOptGroup label={t("machines.start.offered")}>
              {offered.map((c) => (
                <NativeSelectOption key={c.id} value={c.id}>
                  {c.name}
                </NativeSelectOption>
              ))}
            </NativeSelectOptGroup>
            {own.length > 0 && (
              <NativeSelectOptGroup label={t("machines.start.own")}>
                {own.map((c) => (
                  <NativeSelectOption key={c.id} value={c.id}>
                    {c.name}
                  </NativeSelectOption>
                ))}
              </NativeSelectOptGroup>
            )}
          </NativeSelect>
        </Row>
        <Row>
          <RowText title={t("machines.start.name")} htmlFor={nameId} />
          <Input
            id={nameId}
            className="w-56 max-w-full"
            value={name}
            maxLength={40}
            placeholder={t("machines.start.namePlaceholder")}
            disabled={disabled}
            aria-invalid={problem !== null}
            aria-describedby={problem === null ? undefined : problemId}
            onChange={(event) => setName(event.target.value)}
          />
        </Row>
        <Row>
          <RowText
            title={t("machines.start.host")}
            htmlFor={hostId}
            detail={listings.length === 0 ? t("machines.start.noHosts") : undefined}
          />
          {listings.length > 0 && (
            <NativeSelect
              id={hostId}
              className="w-56 max-w-full"
              value={listing?.id ?? ""}
              disabled={disabled}
              onChange={(event) => setListingId(event.target.value)}
            >
              {listings.map((l) => (
                <NativeSelectOption key={l.id} value={l.id}>
                  {t("machines.start.offer", {
                    price: formatMoney(l.unit_price, l.currency),
                    count: l.available,
                  })}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          )}
        </Row>
        <Row>
          <RowText title={t("machines.start.replicas")} detail={t("machines.start.replicasDetail", { max: most })} />
          {/* A shadcn Button Group: the count between its two steps. */}
          <ButtonGroup>
            <Button
              variant="secondary"
              size="icon"
              aria-label={t("machines.start.fewer")}
              disabled={disabled || count <= 1}
              onClick={() => setReplicas(Math.max(1, count - 1))}
            >
              <HugeiconsIcon icon={MinusSignIcon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
            <label htmlFor={replicasId} className="sr-only">
              {t("machines.start.replicas")}
            </label>
            <Input
              id={replicasId}
              inputMode="numeric"
              className="numeric w-14 text-center"
              value={String(count)}
              disabled={disabled}
              onChange={(event) => {
                const n = Number(event.target.value);
                if (Number.isInteger(n) && n >= 1) setReplicas(Math.min(n, most));
              }}
            />
            <Button
              variant="secondary"
              size="icon"
              aria-label={t("machines.start.more")}
              disabled={disabled || count >= most}
              onClick={() => setReplicas(Math.min(most, count + 1))}
            >
              <HugeiconsIcon icon={PlusSignIcon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
          </ButtonGroup>
        </Row>
        {chosen?.cloudInit === true && (
          <StackRow>
            <RowText
              title={t("machines.start.userData")}
              htmlFor={userDataId}
              detail={t("machines.start.userDataDetail")}
            />
            <Textarea
              id={userDataId}
              className="mt-2 font-mono text-[12.5px]"
              rows={4}
              spellCheck={false}
              placeholder={"#cloud-config\n"}
              value={userData}
              disabled={disabled}
              onChange={(event) => setUserData(event.target.value)}
            />
          </StackRow>
        )}
        <Row last>
          <RowText
            title={
              listing === undefined
                ? t("machines.start.noPrice")
                : t("machines.start.total", {
                    total: formatMoney(listing.unit_price * count, listing.currency),
                    count,
                  })
            }
            detail={t("machines.start.split", { percent: 100 - half, ours: half })}
          />
          <Button
            disabled={disabled || listing === undefined || chosen === undefined}
            onClick={() => void start()}
          >
            {t("machines.start.button")}
          </Button>
        </Row>
      </Group>
      <FieldError id={problemId}>{problem}</FieldError>
      <GroupCaption>
        {t("machines.start.caption")}
        {catalogue.installer_only.length > 0 &&
          ` ${t("machines.start.installerOnly", { systems: catalogue.installer_only.join(", ") })}`}
      </GroupCaption>
    </PaneSection>
  );
}

// ── This box's machines ───────────────────────────────────────────────────

function MineSection({
  account,
  catalogue,
  statuses,
}: {
  account: MarketAccount;
  catalogue: VmCatalogue;
  statuses: VmMachine[];
}) {
  const t = useT();
  const mine = account.purchases.filter((order) => order.kind === "vm" && order.vm != null);
  if (mine.length === 0) return null;
  return (
    <PaneSection>
      <GroupTitle>{t("machines.mine.title")}</GroupTitle>
      <Group data-testid="machines-mine">
        {mine.map((order, i) => (
          <MachineRow
            key={order.id}
            order={order}
            catalogue={catalogue}
            account={account}
            status={statuses.find((s) => s.order_id === order.id)}
            last={i === mine.length - 1}
          />
        ))}
      </Group>
      <GroupCaption>{t("machines.mine.caption")}</GroupCaption>
    </PaneSection>
  );
}

function MachineRow({
  order,
  catalogue,
  account,
  status,
  last,
}: {
  order: MarketOrder;
  catalogue: VmCatalogue;
  account: MarketAccount;
  status: VmMachine | undefined;
  last: boolean;
}) {
  const t = useT();
  const vm = order.vm;
  if (vm == null) return null;
  const imageName =
    catalogue.images.find((i) => i.id === vm.image)?.name ??
    (account.uploads ?? []).find((u) => u.id === vm.image)?.name ??
    vm.image;
  const state =
    order.status === "pending"
      ? t("machines.mine.awaitingPayment")
      : order.status === "expired"
        ? t("panes.market.status.expired")
        : vm.halted || order.expired
          ? t("machines.mine.lapsed")
          : order.expires_at !== null
            ? t("machines.mine.until", { date: formatDay(order.expires_at) })
            : t("panes.market.status.paid");
  return (
    <StackRow last={last}>
      <div className="flex items-start justify-between gap-4">
        <RowText
          title={vm.name}
          detail={`${imageName} · ${t("machines.mine.replicas", { count: order.quantity })} · ${formatMoney(
            order.amount,
            order.currency,
          )}`}
        />
        <span className="shrink-0 text-[13px] text-muted">{state}</span>
      </div>
      {status !== undefined && status.replicas.length > 0 && (
        <ul className="mt-2 flex flex-wrap gap-1.5" aria-label={t("machines.mine.replicaList")}>
          {status.replicas.map((replica) => (
            <li key={replica.name}>
              <Badge variant="outline" className="numeric gap-1.5">
                <StatusDot state={STATUS_DOT[replica.status] ?? "idle"} />
                <span>{replica.name}</span>
                <span className="text-faint">{replicaWord(replica.status, t)}</span>
              </Badge>
            </li>
          ))}
        </ul>
      )}
      {vm.address !== null && !vm.halted && (
        <p className="mt-2 text-[13px]">
          <a className="text-accent underline-offset-2 hover:underline" href={vm.address} target="_blank" rel="noreferrer">
            {vm.address}
          </a>
        </p>
      )}
    </StackRow>
  );
}

// ── This box's images ─────────────────────────────────────────────────────

function ImagesSection({
  catalogue,
  account,
  machines,
  disabled,
}: {
  catalogue: VmCatalogue;
  account: MarketAccount;
  machines: MachinesData;
  disabled: boolean;
}) {
  const t = useT();
  const fileId = React.useId();
  const nameId = React.useId();
  const efiId = React.useId();
  const problemId = React.useId();
  // Bumped to clear the file field after an upload: a file input's value
  // can only be emptied, and a new element is the simple way to do it.
  const [fileField, setFileField] = React.useState(0);
  const [file, setFile] = React.useState<File | null>(null);
  const [name, setName] = React.useState("");
  const [efi, setEfi] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const uploads = account.uploads ?? [];
  const limit = formatBytes(catalogue.max_upload_bytes);

  const pick = (picked: File | null) => {
    setFile(picked);
    setProblem(null);
    if (picked !== null && name.trim().length === 0) {
      setName(picked.name.replace(/\.(qcow2|img)$/i, "").slice(0, 40));
    }
  };

  const send = async () => {
    const trimmed = name.trim();
    if (file === null) return setProblem(t("machines.images.pickFirst"));
    if (file.size > catalogue.max_upload_bytes) return setProblem(t("machines.images.tooBig", { limit }));
    if (trimmed.length === 0 || trimmed.length > 40) return setProblem(t("machines.start.nameInvalid"));
    setProblem(null);
    if (await machines.upload(file, trimmed, efi)) {
      setFile(null);
      setName("");
      setEfi(false);
      setFileField((n) => n + 1);
    }
  };

  return (
    <PaneSection>
      <GroupTitle>{t("machines.images.title")}</GroupTitle>
      <Group data-testid="machines-images">
        <StackRow last={uploads.length === 0 && machines.uploading === null}>
          <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
            <div>
              <label htmlFor={fileId} className="sr-only">
                {t("machines.images.file")}
              </label>
              <Input
                key={fileField}
                id={fileId}
                type="file"
                accept=".qcow2,.img,application/octet-stream"
                disabled={disabled}
                onChange={(event) => pick(event.target.files?.[0] ?? null)}
              />
            </div>
            <div>
              <label htmlFor={nameId} className="sr-only">
                {t("machines.images.name")}
              </label>
              <Input
                id={nameId}
                value={name}
                maxLength={40}
                placeholder={t("machines.images.name")}
                disabled={disabled}
                aria-invalid={problem !== null}
                aria-describedby={problem === null ? undefined : problemId}
                onChange={(event) => setName(event.target.value)}
              />
            </div>
            <Button disabled={disabled || file === null} onClick={() => void send()}>
              {t("machines.images.upload")}
            </Button>
          </div>
          <label htmlFor={efiId} className="mt-2 flex items-center gap-2 text-[13px] text-muted">
            <input
              id={efiId}
              type="checkbox"
              className="size-4 accent-accent"
              checked={efi}
              disabled={disabled}
              onChange={(event) => setEfi(event.target.checked)}
            />
            {t("machines.images.efi")}
          </label>
          <FieldError id={problemId}>{problem}</FieldError>
        </StackRow>
        {machines.uploading !== null && (
          <StackRow last={uploads.length === 0}>
            <p className="text-[13px] text-ink">
              {t("machines.images.sending", {
                name: machines.uploading.name,
                percent: Math.floor(machines.uploading.percent),
              })}
            </p>
            <Progress
              className="mt-2"
              value={machines.uploading.percent}
              label={t("machines.images.sendingLabel")}
            />
          </StackRow>
        )}
        {uploads.map((upload, i) => (
          <Row key={upload.id} last={i === uploads.length - 1}>
            <RowText
              title={upload.name}
              detail={
                upload.stored && upload.size !== null
                  ? t("machines.images.stored", {
                      size: formatBytes(upload.size),
                      disk: upload.min_disk_gib,
                    })
                  : t("machines.images.notStored")
              }
            />
            <Button size="sm" variant="secondary" disabled={disabled} onClick={() => void machines.remove(upload.id)}>
              {t("machines.images.remove")}
            </Button>
          </Row>
        ))}
      </Group>
      <GroupCaption>{t("machines.images.caption", { limit })}</GroupCaption>
    </PaneSection>
  );
}

// ── Hosting machines on this box ──────────────────────────────────────────

function HostSection({
  catalogue,
  account,
  machines,
  disabled,
}: {
  catalogue: VmCatalogue;
  account: MarketAccount;
  machines: MachinesData;
  disabled: boolean;
}) {
  const t = useT();
  const priceId = React.useId();
  const capId = React.useId();
  const problemId = React.useId();
  const [price, setPrice] = React.useState("");
  const [capacity, setCapacity] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const own = account.listings.filter((l) => l.kind === "vm");
  const sales = account.sales.filter((o) => o.kind === "vm");
  const half = catalogue.fee_bps / 100;
  const currency = account.currency.toUpperCase();

  const onboard = async () => {
    const onboardingTab = window.open("about:blank", "_blank");
    if (onboardingTab === null) {
      toast.error(t("machines.actionFailedTitle"), t("panes.market.popupBlocked"), { help: "index" });
      return;
    }
    onboardingTab.opener = null;
    const reply = await machines.onboard();
    if (reply !== null && isStripePage(reply.url)) {
      onboardingTab.location.href = reply.url;
      toast.status(t("panes.market.payoutsOpened"), t("panes.market.payoutsOpenedBody"), { duration: 12000 });
    } else onboardingTab.close();
  };

  const submit = async () => {
    const minor = toMinorUnits(price, account.currency);
    const cap = Number(capacity);
    if (minor === null) {
      const digits = minorDigits(account.currency);
      return setProblem(t("panes.market.priceInvalid", { example: (5 / 10 ** digits).toFixed(digits) }));
    }
    if (!Number.isInteger(cap) || cap < 1 || cap > 50) {
      return setProblem(t("machines.host.capacityInvalid"));
    }
    setProblem(null);
    if (await machines.list(minor, cap)) {
      setPrice("");
      setCapacity("");
    }
  };

  return (
    <PaneSection>
      <GroupTitle>{t("machines.host.title")}</GroupTitle>
      <Group data-testid="machines-host">
        <Row last={!account.seller_ready && own.length === 0}>
          <RowText
            title={t("panes.market.payouts")}
            detail={
              account.seller_ready
                ? t("panes.market.payoutsReady")
                : account.seller_onboarded
                  ? t("panes.market.payoutsPending")
                  : t("panes.market.payoutsNone")
            }
          />
          {!account.seller_ready && (
            <Button size="sm" disabled={disabled} onClick={() => void onboard()}>
              {account.seller_onboarded ? t("panes.market.payoutsContinue") : t("panes.market.payoutsStart")}
            </Button>
          )}
        </Row>
        {account.seller_ready && (
          <StackRow last={own.length === 0}>
            {account.can_host_vms !== true ? (
              <p className="text-[13px] text-muted">{t("machines.host.notHosting")}</p>
            ) : (
              <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
                <div>
                  <label htmlFor={priceId} className="sr-only">
                    {t("machines.host.price", { currency })}
                  </label>
                  <Input
                    id={priceId}
                    inputMode="decimal"
                    placeholder={t("machines.host.price", { currency })}
                    value={price}
                    disabled={disabled}
                    aria-invalid={problem !== null}
                    aria-describedby={problem === null ? undefined : problemId}
                    onChange={(event) => setPrice(event.target.value)}
                  />
                </div>
                <div>
                  <label htmlFor={capId} className="sr-only">
                    {t("machines.host.capacity")}
                  </label>
                  <Input
                    id={capId}
                    inputMode="numeric"
                    placeholder={t("machines.host.capacity")}
                    value={capacity}
                    disabled={disabled}
                    onChange={(event) => setCapacity(event.target.value)}
                  />
                </div>
                <Button disabled={disabled} onClick={() => void submit()}>
                  {t("machines.host.list")}
                </Button>
              </div>
            )}
            <FieldError id={problemId}>{problem}</FieldError>
          </StackRow>
        )}
        {own.map((l, i) => (
          <Row key={l.id} last={i === own.length - 1}>
            <RowText
              title={t("machines.host.offer", { price: formatMoney(l.unit_price, account.currency) })}
              detail={`${t("machines.host.free", { count: l.available, capacity: l.capacity })} · ${
                l.active ? t("panes.market.open") : t("panes.market.closed")
              }`}
            />
            {l.active && (
              <Button size="sm" variant="secondary" disabled={disabled} onClick={() => void machines.close(l.id)}>
                {t("panes.market.closeButton")}
              </Button>
            )}
          </Row>
        ))}
      </Group>
      <GroupCaption>{t("machines.host.caption", { percent: 100 - half, ours: half })}</GroupCaption>
      {sales.length > 0 && (
        <Group className="mt-3">
          {sales.map((o, i) => (
            <Row key={o.id} last={i === sales.length - 1}>
              <RowText
                title={`${o.vm?.name ?? t("panes.market.kind.vm")} · ${t("machines.mine.replicas", { count: o.quantity })}`}
                detail={t("panes.market.youReceive", { amount: formatMoney(o.seller_net, o.currency) })}
              />
              <span className="text-[13px] text-muted">{t(`panes.market.status.${o.status}` as MessageKey)}</span>
            </Row>
          ))}
        </Group>
      )}
    </PaneSection>
  );
}
