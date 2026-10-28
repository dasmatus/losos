import * as React from "react";
import { FieldError, Input } from "@/components/ui/input";
import { useT } from "@/lib/i18n-react";
import { DomainsSection } from "./domains-section";
import { HiddenSection } from "./hidden-section";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, SwitchRow } from "./rows";
import { BoolRow, slotFor } from "./site-settings";
import type { SettingsForm } from "./use-settings-form";

/* The name this box answers to, and how it is reached.
 *
 * The name is the single most dangerous field on this screen. It becomes the
 * system hostname, the name announced on the local network, the address the
 * files app trusts and the address the code app builds its links from. A
 * space or a slash fails the rebuild; a leading hyphen or a sixty-fourth
 * character gives a box that comes back up unreachable — and there is no way
 * in but this page. So the rule is stated under the group before it is
 * broken, the field says what is wrong the moment it is wrong, and Apply
 * stays dead until it is right.
 */

export function NetworkPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const nameId = React.useId();
  const nameErrorId = React.useId();
  const proxyId = React.useId();
  const windowsId = React.useId();
  // Drawn only on a box whose option document declares it.
  const windows = slotFor(form, "cloud.windows.enable");
  const tor = slotFor(form, "hidden.tor.enable");
  const i2p = slotFor(form, "hidden.i2p.enable");

  const draft = form.draft;
  const disabled = form.locked || !form.ready;
  const name = draft?.hostName ?? "";
  const nameProblem = form.problems.hostName;

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.network.name")}</GroupTitle>
        <Group>
          <Row>
            <RowText htmlFor={nameId} title={t("panes.network.called")} />
            <Input
              id={nameId}
              value={name}
              maxLength={63}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              disabled={disabled}
              className="w-[13rem]"
              aria-invalid={nameProblem !== null}
              aria-describedby={nameProblem === null ? undefined : nameErrorId}
              onChange={(event) => form.set("hostName", event.target.value)}
            />
          </Row>
          <Row last>
            <RowText title={t("panes.network.reachedAt")} />
            <RowValue>{name.length > 0 ? `${name}.local` : "—"}</RowValue>
          </Row>
        </Group>

        <FieldError id={nameErrorId} className="px-1.5 pt-2">
          {nameProblem}
        </FieldError>

        <GroupCaption>{t("panes.network.nameCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.network.reaching")}</GroupTitle>
        <Group>
          <SwitchRow
            id={proxyId}
            title={t("panes.network.outside")}
            checked={draft?.proxyEnable ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("proxyEnable", next)}
            last
          />
        </Group>

        <GroupCaption>{t("panes.network.reachCaption")}</GroupCaption>
      </PaneSection>

      {windows !== null && (
        <PaneSection data-testid="network-windows">
          <GroupTitle>{t("panes.network.windows.title")}</GroupTitle>
          <Group>
            <BoolRow
              id={windowsId}
              slot={windows}
              title={t("panes.network.windows.show")}
              description={t("panes.network.windows.showDetail")}
              last
            />
          </Group>
          <GroupCaption>
            {t("panes.network.windows.caption", { address: `\\\\${name || "box"}\\` })}
          </GroupCaption>
        </PaneSection>
      )}

      {(tor !== null || i2p !== null) && <HiddenSection tor={tor} i2p={i2p} locked={form.locked} />}

      <DomainsSection locked={form.locked} />
    </>
  );
}
