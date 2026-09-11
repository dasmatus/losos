import * as React from "react";
import { Switch } from "@/components/ui/switch";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* One switch, and it is honest about being one switch.
 *
 * There is exactly one hardware knob this box exposes. A pane that padded it
 * out with read-only specifications would be inventing a place for numbers
 * nothing here can measure — the box reports no processor, no memory and no
 * temperature over the admin API, and a row that said "unknown" four times
 * would be worse than a short pane.
 */

export function HardwarePane({ form }: { form: SettingsForm }) {
  const gpuId = React.useId();
  const disabled = form.locked || !form.ready;

  return (
    <PaneSection>
      <GroupTitle>Graphics</GroupTitle>
      <Group>
        <Row last>
          <RowText
            htmlFor={gpuId}
            title="Let apps use the graphics chip"
            detail="Only switch this on if this box has one."
          />
          <Switch
            id={gpuId}
            checked={form.draft?.gpuEnable ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("gpuEnable", next)}
          />
        </Row>
      </Group>
      <GroupCaption>
        Some jobs, like recognising what is in a photo or converting a video, run much faster on a
        graphics chip than on the main processor. Switching this on tells the apps they may use one.
        On a box without one it changes nothing except the time the next rebuild takes.
      </GroupCaption>
    </PaneSection>
  );
}
