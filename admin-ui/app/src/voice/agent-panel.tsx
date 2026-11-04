/* The call's assistant, over the call: what it is doing, what it heard
 * asked and answered, what it may look at, and how to make it stop. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, LockKeyIcon, VolumeOffIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { useT } from "@/lib/i18n-react";
import type { Caption, Phase } from "./agent";

export interface AgentPanelProps {
  phase: Phase;
  /* 0 to 1 while the engine and the models are coming. */
  load: number;
  captions: Caption[];
  grounded: boolean;
  onGrounded(on: boolean): void;
  onHush(): void;
  /* Take the assistant out of the call. */
  onRemove(): void;
  onClose(): void;
}

export function AgentPanel(p: AgentPanelProps) {
  const t = useT();
  const log = React.useRef<HTMLOListElement>(null);
  React.useEffect(() => {
    log.current?.scrollTo({ top: log.current.scrollHeight });
  }, [p.captions.length]);

  return (
    <section
      role="dialog"
      aria-label={t("calls.agent")}
      data-agent-panel=""
      data-phase={p.phase}
      className="absolute inset-x-3 top-16 z-10 mx-auto flex max-w-xl flex-col gap-4 rounded-card border border-line bg-surface p-4 shadow-pop sm:inset-x-6"
    >
      <header className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-medium text-ink">{t("calls.agent")}</h3>
          <p className="text-[13px] text-muted" data-agent-status="">
            {t(`calls.agentPhase.${p.phase}`)}
          </p>
        </div>
        <Button variant="ghost" size="sm" className="size-8 p-0" aria-label={t("calls.close")} onClick={p.onClose}>
          <HugeiconsIcon icon={Cancel01Icon} size={16} />
        </Button>
      </header>

      {p.phase === "loading" ? <Progress value={Math.round(p.load * 100)} label={t("calls.agentLoading")} /> : null}

      <ol ref={log} aria-live="polite" data-agent-captions="" className="flex max-h-56 flex-col gap-2 overflow-y-auto text-[14px]">
        {p.captions.length === 0 ? <li className="text-muted">{t("calls.agentHint")}</li> : null}
        {p.captions.map((c, i) => (
          <li key={i} data-agent-caption={c.agent ? "agent" : "person"} className={c.agent ? "text-ink" : "text-muted"}>
            <span className="font-medium">{c.who}:</span> {c.text}
          </li>
        ))}
      </ol>

      <label className="flex items-start justify-between gap-4">
        <span>
          <span className="block text-[14px] font-medium text-ink">{t("calls.agentGrounded")}</span>
          <span className="block text-[13px] text-muted">{t("calls.agentGroundedDetail")}</span>
        </span>
        <Switch aria-label={t("calls.agentGrounded")} isSelected={p.grounded} onChange={p.onGrounded} />
      </label>

      <Alert>
        <HugeiconsIcon icon={LockKeyIcon} />
        <AlertDescription>{t("calls.agentPrivacy")}</AlertDescription>
      </Alert>

      <div className="flex justify-between gap-2">
        <Button variant="ghost" disabled={p.phase !== "speaking" && p.phase !== "thinking"} onClick={p.onHush}>
          <HugeiconsIcon icon={VolumeOffIcon} size={16} />
          {t("calls.agentHush")}
        </Button>
        <Button variant="ghost" onClick={p.onRemove}>
          {t("calls.agentRemove")}
        </Button>
      </div>
    </section>
  );
}
