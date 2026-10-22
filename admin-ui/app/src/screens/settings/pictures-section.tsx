import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { FieldError } from "@/components/ui/field";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ApiError, type AgeMark, type PictureBlur } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { savePictureSettings, trustSender, usePictureSettings } from "@/lib/pictures";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow, SwitchRow } from "./rows";

/* Pictures in Chat and Mail (lib/pictures.ts, backend/src/pictures.rs):
 * which arrive blurred, the senders whose pictures always show, the nudity
 * warning and the age mark that decides what may be changed. */

const BLURS: readonly PictureBlur[] = ["everyone", "strangers", "nobody"];

export function ageText(t: ReturnType<typeof useT>, mark: AgeMark | null | undefined): string {
  if (mark === null || mark === undefined) return t("pictures.age.none");
  if (mark.age === "child") return t("pictures.age.child");
  return mark.by === "binance" ? t("pictures.age.adultBinance") : t("pictures.age.adultAdmin");
}

export function PicturesSection({ disabled }: { disabled: boolean }) {
  const t = useT();
  const settings = usePictureSettings();
  const warningId = React.useId();
  const blurId = React.useId();
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  if (settings === null) return null;

  const change = async (next: { blur?: PictureBlur; nudityCheck?: boolean }) => {
    setBusy(true);
    setError(null);
    try {
      await savePictureSettings(next);
    } catch (failure) {
      const code = failure instanceof ApiError ? failure.code : null;
      setError(
        code === "adultRequired"
          ? t("pictures.settings.adultRequired")
          : code === "childAccount"
            ? t("pictures.settings.childAccount")
            : t("pictures.settings.failed"),
      );
    } finally {
      setBusy(false);
    }
  };

  const untrust = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await trustSender(id, false);
    } catch {
      setError(t("pictures.settings.failed"));
    } finally {
      setBusy(false);
    }
  };

  const off = disabled || busy;
  return (
    <PaneSection data-testid="account-pictures">
      <GroupTitle>{t("pictures.settings.title")}</GroupTitle>
      <Group>
        <StackRow>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <RowText id={blurId} title={t("pictures.settings.blur")} />
            <ToggleGroup
              aria-labelledby={blurId}
              value={settings.blur}
              onValueChange={(next) => {
                const blur = BLURS.find((b) => b === next);
                if (blur !== undefined && blur !== settings.blur) void change({ blur });
              }}
              data-testid="pictures-blur"
            >
              {BLURS.map((b) => (
                <ToggleGroupItem
                  key={b}
                  value={b}
                  disabled={off || (b === "nobody" && !settings.mayShowAll)}
                  className="px-2.5"
                >
                  {t(`pictures.settings.blur.${b}`)}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </div>
          <p className="text-[12.5px] leading-snug text-muted">{t(`pictures.settings.blur.${settings.blur}.detail`)}</p>
        </StackRow>
        <SwitchRow
          id={warningId}
          title={t("pictures.settings.warning")}
          description={settings.child ? t("pictures.settings.warningChild") : t("pictures.settings.warningDetail")}
          checked={settings.nudityCheck}
          disabled={off || settings.child}
          onCheckedChange={(on) => void change({ nudityCheck: on })}
        />
        {settings.trusted.length > 0 && (
          <StackRow>
            <RowText title={t("pictures.settings.trusted")} />
            <ul className="flex flex-wrap gap-1.5" data-testid="pictures-trusted">
              {settings.trusted.map((id) => (
                <li key={id} className="flex items-center gap-1 rounded-full border border-line py-0.5 pr-1 pl-2.5 text-[12.5px]">
                  <span className="max-w-56 truncate">{id}</span>
                  <button
                    type="button"
                    disabled={off}
                    onClick={() => void untrust(id)}
                    aria-label={t("pictures.settings.untrust", { name: id })}
                    className="rounded-full p-0.5 text-muted transition-colors hover:bg-sunk hover:text-ink disabled:opacity-45"
                  >
                    <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
                  </button>
                </li>
              ))}
            </ul>
          </StackRow>
        )}
        <Row last>
          <RowText title={t("pictures.settings.age")} />
          <RowValue data-testid="pictures-age">{ageText(t, settings.age)}</RowValue>
        </Row>
      </Group>
      {error !== null && <FieldError className="px-3.5 pt-2">{error}</FieldError>}
      <GroupCaption>{settings.mayShowAll ? t("pictures.settings.captionAdult") : t("pictures.settings.caption")}</GroupCaption>
    </PaneSection>
  );
}
