/* The Statistics widget: requests in the last hour, the hour as bars, and the
 * share that failed. The box puts `StatisticsSummary` in an Overview tile and
 * opens Settings › Statistics; the edge's manage page draws the whole
 * `StatisticsWidget` card and opens /manage/statistics. */

import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { SkeletonLine } from "@/components/ui/skeleton";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { fmt } from "@/widgets/format";
import { MinuteChart } from "./chart";
import { lastHour } from "./types";
import type { StatisticsState } from "./use-statistics";

/** A server error rate above this is worth a warning chip. */
const WARN_RATE = 0.01;
const CRIT_RATE = 0.05;

export function StatisticsSummary({ state }: { state: StatisticsState }) {
  const t = useT();
  useLocale();

  if (state.kind === "loading") {
    return (
      <div className="flex flex-col gap-2.5" aria-busy="true">
        <SkeletonLine className="h-8 w-28" />
        <SkeletonLine className="h-12 w-full" />
        <SkeletonLine className="w-40" />
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
      <p className="text-[12.5px] leading-snug text-muted">
        {t("statistics.failed", { message: state.message })}
      </p>
    );
  }
  const { stats } = state;
  if (!stats.available) {
    return (
      <p className="text-[12.5px] leading-snug text-muted">
        {stats.reason ?? t("statistics.unavailable")}
      </p>
    );
  }

  const hour = lastHour(stats);
  const rate = hour.errorRate;
  const tone = rate === null ? null : rate >= CRIT_RATE ? "crit" : rate >= WARN_RATE ? "warn" : "ok";
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span className="numeric text-[34px] leading-none font-semibold tracking-tight text-ink">
          {fmt.number(hour.requests)}
        </span>
        {tone !== null && rate !== null && (
          <Badge variant={tone}>{t("statistics.errorRate", { rate: fmt.percent(rate) })}</Badge>
        )}
      </div>
      <p className="text-[12.5px] leading-snug text-muted">
        {t("statistics.summary", {
          count: hour.requests,
          bytes: fmt.bytes(hour.bytesOut),
        })}
      </p>
      <MinuteChart minutes={stats.perMinute} height={40} className="h-10" />
    </div>
  );
}

export function StatisticsWidget({
  state,
  to,
  className,
}: {
  state: StatisticsState;
  /** Where the card opens: the place's own Statistics page. */
  to: string;
  className?: string;
}) {
  const t = useT();
  return (
    <Card className={cn("p-0", className)}>
      <Link
        to={to}
        className={cn(
          "group flex flex-col gap-3 rounded-card px-5 pt-4 pb-4 outline-none",
          "focus-visible:ring-2 focus-visible:ring-accent",
        )}
      >
        <span className="flex items-center justify-between gap-2">
          <span className="text-[13px] font-semibold tracking-tight">{t("statistics.title")}</span>
          <span className="flex items-center gap-1 text-[12.5px] text-muted group-hover:text-ink">
            {t("statistics.open")}
            <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={1.8} aria-hidden="true" />
          </span>
        </span>
        <StatisticsSummary state={state} />
      </Link>
    </Card>
  );
}
