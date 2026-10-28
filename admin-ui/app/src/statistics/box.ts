/* The box's own source for the Statistics widget and page: lososd's
 * GET /api/statistics. One function, defined once, so useStatistics sees the
 * same `load` on every render and does not start over. */

import { getStatistics } from "@/lib/api";

export function loadBoxStatistics(signal: AbortSignal): Promise<unknown> {
  return getStatistics({ signal });
}
