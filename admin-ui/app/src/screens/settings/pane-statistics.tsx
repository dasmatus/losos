/* Settings › Statistics: what Traefik counted for the apps' LAN ports, drawn
 * by the view the edge's manage page shares (src/statistics). No draft and
 * no Apply bar: the pane only reads. */

import { loadBoxStatistics } from "@/statistics/box";
import { useStatistics } from "@/statistics/use-statistics";
import { StatisticsView } from "@/statistics/view";

export function StatisticsPane() {
  const state = useStatistics(loadBoxStatistics);
  return <StatisticsView state={state} />;
}
