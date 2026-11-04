/* Settings › Statistics: what Traefik counted for the apps' LAN ports, drawn
 * by the view the edge's manage page shares (src/statistics). No draft and
 * no Apply bar: the pane only reads. The pane is
 * src/pages/settings.statistics.mdx (src/pages/README.md). */

import { pageOf } from "@/pages/page";
import { loadBoxStatistics } from "@/statistics/box";
import { useStatistics } from "@/statistics/use-statistics";
import { StatisticsView } from "@/statistics/view";

export function StatisticsPane() {
  const state = useStatistics(loadBoxStatistics);
  return <StatisticsPage facts={state} />;
}

const StatisticsPage = pageOf<"settings.statistics", ReturnType<typeof useStatistics>>("settings.statistics", {
  Counts: ({ facts }) => <StatisticsView state={facts} />,
});
