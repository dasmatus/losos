import * as React from "react";
import { Switch } from "@/components/ui/switch";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* The four extra protections, and what each one costs.
 *
 * Every switch here is off when a box arrives, and the pane exists because
 * before it there was no way to turn any of them on. This box has no shell
 * and no remote login, so an option that is not on this screen is not merely
 * off — it is unreachable. That made all four dead settings on every box
 * anyone installed.
 *
 * They stay four switches rather than one "extra protection" slider because
 * their costs are unrelated and only one of them is free-ish. A single control
 * would let somebody accept a halved processor while reaching for a locked USB
 * port. Each row therefore says its cost in the detail line, not in a footnote
 * — the cost IS the decision.
 *
 * House style, same as every other pane: no emoji, nothing names a container
 * runtime, and every change here waits for the Apply bar like all the rest.
 * Nothing on this screen takes effect until the box rebuilds.
 */

export function SecurityPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const apparmorId = React.useId();
  const mallocId = React.useId();
  const nosmtId = React.useId();
  const usbguardId = React.useId();
  const disabled = form.locked || !form.ready;

  return (
    <PaneSection>
      <GroupTitle>{t("panes.security.group")}</GroupTitle>
      <Group>
        <Row>
          <RowText
            htmlFor={usbguardId}
            title={t("panes.security.usbguard.title")}
            detail={t("panes.security.usbguard.detail")}
          />
          <Switch
            id={usbguardId}
            checked={form.draft?.hardeningUsbguard ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("hardeningUsbguard", next)}
          />
        </Row>
        <Row>
          <RowText
            htmlFor={mallocId}
            title={t("panes.security.malloc.title")}
            detail={t("panes.security.malloc.detail")}
          />
          <Switch
            id={mallocId}
            checked={form.draft?.hardeningMalloc ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("hardeningMalloc", next)}
          />
        </Row>
        <Row>
          <RowText
            htmlFor={apparmorId}
            title={t("panes.security.apparmor.title")}
            detail={t("panes.security.apparmor.detail")}
          />
          <Switch
            id={apparmorId}
            checked={form.draft?.hardeningApparmor ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("hardeningApparmor", next)}
          />
        </Row>
        <Row last>
          <RowText
            htmlFor={nosmtId}
            title={t("panes.security.nosmt.title")}
            detail={t("panes.security.nosmt.detail")}
          />
          <Switch
            id={nosmtId}
            checked={form.draft?.hardeningNosmt ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("hardeningNosmt", next)}
          />
        </Row>
      </Group>
      <GroupCaption>{t("panes.security.caption")}</GroupCaption>
    </PaneSection>
  );
}
