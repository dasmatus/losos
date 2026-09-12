import * as React from "react";
import { Switch } from "@/components/ui/switch";
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
  const apparmorId = React.useId();
  const mallocId = React.useId();
  const nosmtId = React.useId();
  const usbguardId = React.useId();
  const disabled = form.locked || !form.ready;

  return (
    <PaneSection>
      <GroupTitle>Extra protection</GroupTitle>
      <Group>
        <Row>
          <RowText
            htmlFor={usbguardId}
            title="Ignore USB devices plugged in later"
            detail="Anything attached after the box starts is refused. Costs you a keyboard if you ever need one at the machine itself."
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
            title="Stricter memory handling"
            detail="Makes a whole family of break-in attempts fail instead of succeed quietly. Costs a little speed and a little memory."
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
            title="Confine the programs that face the network"
            detail="Limits what each one may touch if it is ever taken over. Only some of what runs here has a rule written for it, so it protects less than it sounds like."
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
            title="Halve the processor to close a leak between jobs"
            detail="Shuts the door two jobs can otherwise listen through. This is the expensive one: roughly half the speed, and you will notice it converting video."
          />
          <Switch
            id={nosmtId}
            checked={form.draft?.hardeningNosmt ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("hardeningNosmt", next)}
          />
        </Row>
      </Group>
      <GroupCaption>
        This box already protects itself in the ways that cost nothing, and those are always on and
        not listed here. The four above are the ones with a price, so they are yours to decide. If
        you lend spare capacity to the mesh, the middle two are the ones worth reading twice: they
        are what stands between somebody else&rsquo;s job and yours.
      </GroupCaption>
    </PaneSection>
  );
}
