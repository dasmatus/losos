/* The Statistics tile on the box's Overview: the shared summary, fed by
 * lososd's GET /api/statistics. The tile around it (widgets/tile.tsx) makes
 * it open Settings › Statistics. */

import { loadBoxStatistics } from "./box";
import { useStatistics } from "./use-statistics";
import { StatisticsSummary } from "./widget";

export function BoxStatisticsSummary() {
  const state = useStatistics(loadBoxStatistics);
  return <StatisticsSummary state={state} />;
}
