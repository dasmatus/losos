import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import type { CustomDomain, DomainProblem } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";
import { useDomains } from "./use-domains";

/* A domain the owner already has, pointing at this box.
 *
 * Only an official LosOS edge hands these out, and only to a box whose Stripe
 * account Stripe has checked: that account is the one thing that ties a box
 * to a real, accountable person, and a domain on the edge's certificate
 * resolver is something a stranger could otherwise abuse. The edge gives the
 * box a name in its own DNS zone (`target`), the owner points a CNAME there
 * and publishes one TXT value, and the edge routes the domain once it sees
 * both. Nothing here is a setting of this box, so the section sits outside
 * the Apply bar: every button is its own request to the edge.
 *
 * The records are shown as text to copy into the owner's DNS provider, one
 * per line, in the order a provider's form asks for them. */

const PROBLEM: Record<DomainProblem, MessageKey> = {
  stripeAccount: "panes.network.domains.problem.stripeAccount",
  txtMissing: "panes.network.domains.problem.txtMissing",
  txtWrong: "panes.network.domains.problem.txtWrong",
  notPointing: "panes.network.domains.problem.notPointing",
  takenElsewhere: "panes.network.domains.problem.takenElsewhere",
  lookupFailed: "panes.network.domains.problem.lookupFailed",
};

export function DomainsSection({ locked }: { locked: boolean }) {
  const t = useT();
  const domains = useDomains(!locked);
  const { state } = domains;
  const inputId = React.useId();
  const [draft, setDraft] = React.useState("");

  let body: React.ReactNode;
  if (state.kind === "loading") {
    body = (
      <Group>
        <Row last>
          <RowText title={t("panes.network.domains.loading")} />
          <Spinner size={16} />
        </Row>
      </Group>
    );
  } else if (state.kind === "failed") {
    body = (
      <Group>
        <Row last>
          <RowText title={t("panes.network.domains.unreachable")} detail={state.message} />
          <Button size="sm" variant="secondary" disabled={locked} onClick={domains.refresh}>
            {t("panes.market.refresh")}
          </Button>
        </Row>
      </Group>
    );
  } else if (state.kind === "unavailable") {
    body = (
      <Group>
        <Row last>
          <RowText
            title={t("panes.network.domains.unavailable")}
            detail={t(
              state.reason === "noOfficialEdge"
                ? "panes.network.domains.noOfficialEdge"
                : "panes.network.domains.notOffered",
            )}
          />
        </Row>
      </Group>
    );
  } else if (!state.eligible) {
    body = (
      <Group>
        <Row last>
          <RowText
            title={t("panes.network.domains.needsStripe")}
            detail={t("panes.network.domains.needsStripeDetail")}
          />
        </Row>
      </Group>
    );
  } else {
    const disabled = locked || domains.busy;
    const full = state.domains.length >= state.maxDomains;
    const submit = async () => {
      const name = draft.trim().toLowerCase().replace(/\.$/, "");
      if (name.length === 0) return;
      if (await domains.add(name)) setDraft("");
    };
    body = (
      <Group>
        <Row>
          <RowText
            title={t("panes.network.domains.target")}
            detail={t("panes.network.domains.targetDetail")}
          />
          <RowValue className="numeric select-all">{state.target}</RowValue>
        </Row>
        {state.domains.map((domain) => (
          <DomainRow
            key={domain.domain}
            domain={domain}
            target={state.target ?? ""}
            addresses={state.addresses}
            disabled={disabled}
            onRemove={() => void domains.remove(domain.domain)}
          />
        ))}
        <Row last>
          <RowText
            htmlFor={inputId}
            title={t("panes.network.domains.add")}
            detail={full ? t("panes.network.domains.full", { count: state.maxDomains }) : undefined}
          />
          <form
            className="flex items-center gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              void submit();
            }}
          >
            <Input
              id={inputId}
              value={draft}
              placeholder="cloud.example.org"
              maxLength={253}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              disabled={disabled || full}
              className="w-[13rem]"
              onChange={(event) => setDraft(event.target.value)}
            />
            <Button type="submit" size="sm" disabled={disabled || full || draft.trim().length === 0}>
              {t("panes.network.domains.addButton")}
            </Button>
          </form>
        </Row>
      </Group>
    );
  }

  return (
    <PaneSection data-testid="custom-domains">
      <GroupTitle>{t("panes.network.domains.title")}</GroupTitle>
      {body}
      <GroupCaption>{t("panes.network.domains.caption")}</GroupCaption>
    </PaneSection>
  );
}

function DomainRow({
  domain,
  target,
  addresses,
  disabled,
  onRemove,
}: {
  domain: CustomDomain;
  target: string;
  addresses: string[];
  disabled: boolean;
  onRemove: () => void;
}) {
  const t = useT();
  const live = domain.status === "live";
  return (
    <StackRow data-testid="custom-domain">
      <div className="flex items-center gap-3">
        <p className="min-w-0 flex-1 truncate text-sm text-ink">{domain.domain}</p>
        <Badge variant={live ? "ok" : "warn"}>
          {t(live ? "panes.network.domains.live" : "panes.network.domains.waiting")}
        </Badge>
        <Button size="sm" variant="secondary" disabled={disabled} onClick={onRemove}>
          {t("panes.network.domains.remove")}
        </Button>
      </div>
      {domain.problem !== null && (
        <p className="text-[12.5px] leading-snug text-muted">{t(PROBLEM[domain.problem])}</p>
      )}
      {!live && (
        <dl className="numeric grid grid-cols-[auto_auto_1fr] gap-x-3 gap-y-1 text-[12.5px] leading-snug">
          <Record mark={domain.points_here} name={domain.domain} type="CNAME" value={`${target}.`} />
          <Record
            mark={domain.txt_found}
            name={domain.txt_name}
            type="TXT"
            value={domain.txt_value === null ? "—" : `"${domain.txt_value}"`}
          />
        </dl>
      )}
      {!live && addresses.length > 0 && (
        <p className="text-[12.5px] leading-snug text-muted">
          {t("panes.network.domains.apex", { addresses: addresses.join(", ") })}
        </p>
      )}
    </StackRow>
  );
}

function Record({ mark, name, type, value }: { mark: boolean; name: string; type: string; value: string }) {
  const t = useT();
  return (
    <>
      <dt className="text-muted">{type}</dt>
      <dd className="break-all text-ink select-all">{name}</dd>
      <dd className="break-all text-ink select-all">
        {value}{" "}
        <span className={mark ? "text-ok" : "text-muted"}>
          {t(mark ? "panes.network.domains.seen" : "panes.network.domains.notSeen")}
        </span>
      </dd>
    </>
  );
}
