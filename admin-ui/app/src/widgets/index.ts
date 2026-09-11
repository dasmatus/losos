/* The widget system's public face.
 *
 * Everything the rest of the app should need is re-exported here; nothing
 * outside widgets/ should have to know that the renderers, the sandbox and
 * the expression interpreter exist.
 *
 * WIRING, in short:
 *
 *   <WidgetBoard />                     on the Overview screen.
 *   startUptimeProbe()                  once in the app shell, if the uptime
 *                                       heatmap should fill in from every
 *                                       screen and not only from the board.
 *   recordStatus(status)                from the status poller's onStatus, so
 *                                       the Changes widget has a history.
 *   recordGrow(response)                from the Storage screen's grow flow,
 *                                       so the Room widget has real sizes.
 *   invalidateMetrics()                 after anything that changes the box.
 */

export { WidgetBoard, type WidgetBoardProps } from "./board";
export { WidgetGallery } from "./gallery";
export { WidgetTile, WidgetBody } from "./tile";

export { CATALOGUE, catalogueEntry, type CatalogueEntry, type WidgetSpan } from "./catalogue";

export {
  invalidateMetrics,
  recordGrow,
  recordProbe,
  recordStatus,
  recordUsedBytes,
  startUptimeProbe,
  stopUptimeProbe,
} from "./metrics";

export { createSandbox } from "./sandbox";
export { blankSpec, compileSpec, parseSpec, type WidgetSpec } from "./spec";
export { fmt, stat } from "./format";

export type {
  LososApi,
  MetricName,
  WidgetFn,
  WidgetKind,
  WidgetResult,
} from "./types";
export { WidgetError } from "./types";
