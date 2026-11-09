/* Reachability, not identity (BoxStore.kt BoxProbe.probe): GET the box's
 * admin homepage with no token at all. The calls that do carry the token
 * live in src/api/api.ts; this file exists so the connect screen's Probe
 * and the Settings tab's Probe now have their answer reported the same way
 * from one word.
 */

import { boxBase, type BoxConfig, type ProbeResult } from "../store/config";
import { boxFetch, CONNECT_TIMEOUT_MS } from "./transport";

/** The probe's whole deadline: its connect timeout, not the read one - an
 *  offline box answers the question immediately or not at all
 *  (BoxProbe.probe's 4s connect + 4s read). */
const PROBE_TIMEOUT_MS = CONNECT_TIMEOUT_MS;

export async function probe(configLike: Pick<BoxConfig, "host">): Promise<ProbeResult> {
  const host = configLike.host.trim();
  if (host === "") return { kind: "unreachable", reason: "No address set" };
  const started = Date.now();
  try {
    await boxFetch(`${boxBase(host)}/`, { method: "GET", deadlineMs: PROBE_TIMEOUT_MS });
    return { kind: "reachable", ms: Math.max(0, Date.now() - started) };
  } catch (e) {
    const message = (e as Error | undefined)?.message ?? "no answer";
    return { kind: "unreachable", reason: message };
  }
}
