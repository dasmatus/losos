import * as React from "react";
import { Switch } from "@/components/ui/switch";
import { useT } from "@/lib/i18n-react";
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
  const t = useT();
  const gpuId = React.useId();
  const disabled = form.locked || !form.ready;

  return (
    <PaneSection>
      <GroupTitle>{t("panes.hardware.graphics")}</GroupTitle>
      <Group>
        <Row last>
          <RowText
            htmlFor={gpuId}
            title={t("panes.hardware.gpu.title")}
            detail={t("panes.hardware.gpu.detail")}
          />
          <Switch
            id={gpuId}
            checked={form.draft?.gpuEnable ?? false}
            disabled={disabled}
            onCheckedChange={(next) => form.set("gpuEnable", next)}
          />
        </Row>
      </Group>
      <GroupCaption>{t("panes.hardware.caption")}</GroupCaption>
    </PaneSection>
  );
}
