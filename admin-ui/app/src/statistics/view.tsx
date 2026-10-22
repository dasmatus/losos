/* The Statistics page's body: the last hour, how Traefik answered, over which
 * protocol, and per route. The box wraps it in Settings › Statistics, the
 * edge in /manage/statistics; each adds its own PaneHeader. */

import * as React from "react";
import { Card } from "@/components/ui/card";
import { Spinner } from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  Group,
  GroupCaption,
  GroupTitle,
  PaneSection,
  Row,
  RowText,
  RowValue,
} from "@/screens/settings/rows";
import { DASH, fmt } from "@/widgets/format";
import { MinuteChart } from "./chart";
import {
  CODE_CLASSES,
  lastHour,
  PROTOCOLS,
  type CodeClass,
  type Protocol,
  type Statistics,
  type StatisticsRoute,
} from "./types";
import type { StatisticsState } from "./use-statistics";

const CODE_FILL: Record<CodeClass, string> = {
  "2xx": "fill-ok",
  "3xx": "fill-faint",
  "4xx": "fill-warn",
  "5xx": "fill-crit",
};

const CODE_DOT: Record<CodeClass, string> = {
  "2xx": "bg-ok",
  "3xx": "bg-faint",
  "4xx": "bg-warn",
  "5xx": "bg-crit",
};

const PROTOCOL_NAME: Record<Protocol, string> = {
  "http/1.1": "HTTP/1.1",
  "http/2": "HTTP/2",
  "http/3": "HTTP/3",
};

const PROTOCOL_FILL: Record<Protocol, string> = {
  "http/1.1": "fill-faint",
  "http/2": "fill-accent/60",
  "http/3": "fill-accent",
};

const PROTOCOL_DOT: Record<Protocol, string> = {
  "http/1.1": "bg-faint",
  "http/2": "bg-accent/60",
  "http/3": "bg-accent",
};

function ms(value: number | null): string {
  if (value === null) return DASH;
  return value < 1000
    ? `${fmt.number(value, value < 10 ? 1 : 0)} ms`
    : `${fmt.number(value / 1000, 2)} s`;
}

/** One bar split into parts, drawn as SVG rects so no width is a style. */
function SplitBar({ parts }: { parts: readonly { value: number; fill: string; key: string }[] }) {
  const total = parts.reduce((sum, part) => sum + part.value, 0);
  let x = 0;
  return (
    <svg viewBox="0 0 100 6" preserveAspectRatio="none" aria-hidden="true" className="block h-2 w-full">
      <rect x={0} y={0} width={100} height={6} rx={3} className="fill-sunk" />
      {total > 0 &&
        parts.map((part) => {
          const width = (part.value / total) * 100;
          const rect = (
            <rect key={part.key} x={x} y={0} width={width} height={6} className={part.fill} />
          );
          x += width;
          return rect;
        })}
    </svg>
  );
}

function Figure({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <Card className="flex flex-col gap-1 px-4 py-3.5">
      <span className="text-[12.5px] text-muted">{label}</span>
      <span className="numeric text-[24px] leading-tight font-semibold tracking-tight">{value}</span>
      {detail !== undefined && <span className="text-[12px] text-muted">{detail}</span>}
    </Card>
  );
}

function routeAddress(route: StatisticsRoute, t: ReturnType<typeof useT>): string {
  if (route.host !== null) return route.host;
  if (route.port !== undefined) return t("statistics.routes.port", { port: route.port });
  return DASH;
}

export function StatisticsView({ state }: { state: StatisticsState }) {
  const t = useT();
  useLocale();

  if (state.kind === "loading") {
    return (
      <div className="flex items-center gap-2.5 text-[13px] text-muted">
        <Spinner size={16} label={t("statistics.loading")} />
        {t("statistics.loading")}
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
      <Group>
        <Row last>
          <RowText title={t("statistics.failed", { message: state.message })} />
        </Row>
      </Group>
    );
  }
  if (!state.stats.available) {
    return (
      <Group>
        <Row last>
          <RowText
            title={t("statistics.unavailable")}
            detail={state.stats.reason ?? undefined}
          />
        </Row>
      </Group>
    );
  }
  return <Ready stats={state.stats} />;
}

function Ready({ stats }: { stats: Statistics }) {
  const t = useT();
  const hour = lastHour(stats);
  const p95 = React.useMemo(() => {
    const values = stats.perMinute.flatMap((m) => (m.p95Ms === null ? [] : [m.p95Ms]));
    return values.length === 0 ? null : Math.max(...values);
  }, [stats.perMinute]);
  const codeTotal = CODE_CLASSES.reduce((sum, c) => sum + stats.codes[c], 0);
  const protocolTotal = PROTOCOLS.reduce((sum, p) => sum + stats.protocols[p], 0);
  const routes = [...stats.routes].sort((a, b) => b.requests - a.requests);

  return (
    <>
      <PaneSection>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Figure label={t("statistics.figure.requests")} value={fmt.number(hour.requests)} />
          <Figure
            label={t("statistics.figure.errors")}
            value={fmt.number(hour.serverErrors)}
            {...(hour.errorRate !== null
              ? { detail: t("statistics.errorRate", { rate: fmt.percent(hour.errorRate) }) }
              : {})}
          />
          <Figure label={t("statistics.figure.sent")} value={fmt.bytes(hour.bytesOut)} />
          <Figure
            label={t("statistics.figure.connections")}
            value={fmt.number(stats.totals.openConnections)}
          />
        </div>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("statistics.hour.title")}</GroupTitle>
        <Group className="px-4 pt-4 pb-3">
          <MinuteChart minutes={stats.perMinute} height={96} className="h-32" />
          <div className="mt-2 flex justify-between text-[12px] text-muted">
            <span>{t("statistics.hour.start")}</span>
            <span>{t("statistics.hour.p95", { time: ms(p95) })}</span>
            <span>{t("statistics.hour.now")}</span>
          </div>
        </Group>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("statistics.codes.title")}</GroupTitle>
        <Group>
          <Row>
            <div className="w-full py-1.5">
              <SplitBar
                parts={CODE_CLASSES.map((c) => ({ key: c, value: stats.codes[c], fill: CODE_FILL[c] }))}
              />
            </div>
          </Row>
          {CODE_CLASSES.map((c, index) => (
            <Row key={c} last={index === CODE_CLASSES.length - 1}>
              <span className="flex min-w-0 flex-1 items-center gap-2.5 text-sm">
                <span className={cn("size-2 shrink-0 rounded-full", CODE_DOT[c])} aria-hidden="true" />
                {t(`statistics.codes.${c}`)}
              </span>
              <RowValue>
                {fmt.number(stats.codes[c])}
                {codeTotal > 0 && (
                  <span className="ml-2 text-faint">{fmt.percent(stats.codes[c] / codeTotal)}</span>
                )}
              </RowValue>
            </Row>
          ))}
        </Group>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("statistics.protocols.title")}</GroupTitle>
        <Group>
          <Row>
            <div className="w-full py-1.5">
              <SplitBar
                parts={PROTOCOLS.map((p) => ({
                  key: p,
                  value: stats.protocols[p],
                  fill: PROTOCOL_FILL[p],
                }))}
              />
            </div>
          </Row>
          {PROTOCOLS.map((p, index) => (
            <Row key={p} last={index === PROTOCOLS.length - 1}>
              <span className="flex min-w-0 flex-1 items-center gap-2.5 text-sm">
                <span className={cn("size-2 shrink-0 rounded-full", PROTOCOL_DOT[p])} aria-hidden="true" />
                {PROTOCOL_NAME[p]}
              </span>
              <RowValue>
                {fmt.number(stats.protocols[p])}
                {protocolTotal > 0 && (
                  <span className="ml-2 text-faint">
                    {fmt.percent(stats.protocols[p] / protocolTotal)}
                  </span>
                )}
              </RowValue>
            </Row>
          ))}
        </Group>
        <GroupCaption>{t("statistics.protocols.caption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("statistics.routes.title")}</GroupTitle>
        <Group>
          {routes.length === 0 ? (
            <Row last>
              <RowText title={t("statistics.routes.none")} />
            </Row>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("statistics.routes.name")}</TableHead>
                  <TableHead className="text-right">{t("statistics.routes.requests")}</TableHead>
                  <TableHead className="text-right">{t("statistics.routes.errors")}</TableHead>
                  <TableHead className="text-right">{t("statistics.routes.sent")}</TableHead>
                  <TableHead className="text-right">{t("statistics.routes.p95")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {routes.map((route) => (
                  <TableRow key={route.name}>
                    <TableCell>
                      <span className="block text-sm text-ink">{route.name}</span>
                      <span className="block font-mono text-[12px] text-muted">
                        {routeAddress(route, t)}
                      </span>
                    </TableCell>
                    <TableCell className="numeric text-right">{fmt.number(route.requests)}</TableCell>
                    <TableCell
                      className={cn("numeric text-right", route.serverErrors > 0 && "text-crit")}
                    >
                      {fmt.number(route.serverErrors)}
                    </TableCell>
                    <TableCell className="numeric text-right">{fmt.bytes(route.bytesOut)}</TableCell>
                    <TableCell className="numeric text-right">{ms(route.p95Ms)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </Group>
        {stats.since !== null && (
          <GroupCaption>
            {t("statistics.since", { date: fmt.date(stats.since), time: fmt.time(stats.since) })}
          </GroupCaption>
        )}
      </PaneSection>
    </>
  );
}
