import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Copy01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { getHidden, isAbort, type HiddenAddresses } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText } from "./rows";
import { BoolRow, type Slot } from "./site-settings";

/* The box on Tor and I2P (modules/hidden-services.nix).
 *
 * Two switches, lines of overrides.nix like the Windows one above, and the
 * addresses they give the box once its Tor and I2P have made them. Making
 * one takes a minute after the rebuild, so while a switch the box runs with
 * is on and its address is not there yet the row says so, and the page asks
 * again every few seconds until it is. The edge the box reports to may give
 * it addresses of its own (modules/edge-hidden.nix); those are listed too,
 * with nothing to switch: the edge decides. */

export const HIDDEN_RECHECK_MS = 10000;

type Net = "tor" | "i2p";

const LABEL: Record<Net, MessageKey> = {
  tor: "panes.network.hidden.torAddress",
  i2p: "panes.network.hidden.i2pAddress",
};

const EDGE_LABEL: Record<Net, MessageKey> = {
  tor: "panes.network.hidden.edgeTor",
  i2p: "panes.network.hidden.edgeI2p",
};

function AddressRow({ label, value, last }: { label: string; value: string; last?: boolean }) {
  const t = useT();
  return (
    <Row last={last}>
      <RowText title={label} />
      <div className="flex min-w-0 items-center gap-1.5">
        <code
          className="min-w-0 truncate font-mono text-[12.5px] text-muted"
          title={value}
          data-testid="hidden-address"
        >
          {value}
        </code>
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={t("panes.network.hidden.copy", { what: label })}
          title={t("panes.network.hidden.copy", { what: label })}
          onClick={() => {
            void navigator.clipboard?.writeText(value).then(() => toast.done(t("panes.network.hidden.copied")));
          }}
        >
          <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </div>
    </Row>
  );
}

/** Whether the box runs with this switch on, whatever the draft says. */
function running(slot: Slot | null): boolean {
  return slot !== null && slot.option.current === true;
}

export function HiddenSection({ tor, i2p, locked }: { tor: Slot | null; i2p: Slot | null; locked: boolean }) {
  const t = useT();
  const torId = React.useId();
  const i2pId = React.useId();
  const [addresses, setAddresses] = React.useState<HiddenAddresses | null>(null);

  const waiting =
    addresses === null ||
    (running(tor) && addresses.box.tor === null) ||
    (running(i2p) && addresses.box.i2p === null);

  React.useEffect(() => {
    if (locked) return;
    const ctl = new AbortController();
    let timer: number | undefined;
    const ask = () => {
      getHidden({ signal: ctl.signal })
        .then(setAddresses)
        .catch((e: unknown) => {
          // A box without the route has no addresses to show.
          if (!isAbort(e)) setAddresses({ box: { tor: null, i2p: null }, edge: { tor: null, i2p: null } });
        });
    };
    ask();
    if (waiting) timer = window.setInterval(ask, HIDDEN_RECHECK_MS);
    return () => {
      ctl.abort();
      if (timer !== undefined) window.clearInterval(timer);
    };
  }, [locked, waiting]);

  const own: { net: Net; slot: Slot | null }[] = [
    { net: "tor", slot: tor },
    { net: "i2p", slot: i2p },
  ];
  const rows: React.ReactNode[] = [];
  for (const { net, slot } of own) {
    const value = addresses?.box[net] ?? null;
    if (value !== null) {
      rows.push(<AddressRow key={net} label={t(LABEL[net])} value={value} />);
    } else if (running(slot)) {
      rows.push(
        <Row key={net}>
          <RowText title={t(LABEL[net])} />
          <span className="text-[13px] text-muted">{t("panes.network.hidden.making")}</span>
        </Row>,
      );
    }
  }
  for (const net of ["tor", "i2p"] as const) {
    const value = addresses?.edge[net] ?? null;
    if (value !== null) rows.push(<AddressRow key={`edge-${net}`} label={t(EDGE_LABEL[net])} value={value} />);
  }

  return (
    <PaneSection data-testid="network-hidden">
      <GroupTitle>{t("panes.network.hidden.title")}</GroupTitle>
      <Group>
        {tor !== null && (
          <BoolRow
            id={torId}
            slot={tor}
            title={t("panes.network.hidden.tor")}
            description={t("panes.network.hidden.torDetail")}
            last={i2p === null}
          />
        )}
        {i2p !== null && (
          <BoolRow
            id={i2pId}
            slot={i2p}
            title={t("panes.network.hidden.i2p")}
            description={t("panes.network.hidden.i2pDetail")}
            last
          />
        )}
      </Group>
      {rows.length > 0 && <Group className="mt-3">{rows}</Group>}
      <GroupCaption>{t("panes.network.hidden.caption")}</GroupCaption>
    </PaneSection>
  );
}
