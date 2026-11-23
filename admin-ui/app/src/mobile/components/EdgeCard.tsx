/* One edge's answer, in the Mesh pane (Panes.kt EdgeCard): what the box
 * found when it last looked for edges - no edge answered, or every edge
 * that did, each with its official mark and where it was found. Read-only:
 * the sharing toggles are an apply (a rebuild) and stay on the box's pages.
 */

import * as React from "react";
import { Text, View } from "react-native";

import { ReadCard } from "./ReadCard";
import { PaneRow } from "./PaneRow";
import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";import type { Read } from "../panes/Read";
import type { JsonBody } from "../../api/api";
import { itemsOf, maybeBool, maybeString } from "../../api/wire";

export function EdgeCard({ tokens, read }: { tokens: LososTokens; read: Read }) {
  return (
    <ReadCard tokens={tokens} title="Edges in reach" read={read}>
      {(e) => body(tokens, e)}
    </ReadCard>
  );
}

function body(tokens: LososTokens, e: JsonBody): React.ReactNode {
  if (!maybeBool(e, "reachable")) {
    const detail =
      !maybeBool(e, "lanSearched")
        ? "The LAN could not be searched."
        : (maybeString(e, "configuredUrl") !== null
            ? `Tried ${maybeString(e, "configuredUrl") as string}; no answer.`
            : "Nothing was tried: no registrar is configured.");
    return (
      <>
        <PaneRow tokens={tokens} title="No edge answered" detail={detail} value="sharing off" tone={tokens.extra.crit} />
      </>
    );
  }
  const edges = itemsOf(e, "edges");
  if (edges.length === 0) {
    return (
      <>
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          The answer names no edge.
        </Text>
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 8 }}>
          Only an edge LosOS runs may process trading; any edge shares the pool.
        </Text>
      </>
    );
  }
  return (
    <View>
      {edges.map((edge, i) => (
        <PaneRow
          key={i}
          tokens={tokens}
          title={`${maybeString(edge, "name") ?? "an edge"}${maybeBool(edge, "official") ? " ✓" : " !"}`}
          detail={[
            maybeString(edge, "url") !== null ? (maybeString(edge, "url") as string) : "",
            maybeBool(edge, "official")
              ? "An official edge - it may process trading."
              : "A company edge; it shares storage only.",
          ]
            .filter((s) => s !== "")
            .join("\n")}
          value={
            maybeString(edge, "source") === "configured" ? "over the internet" : "found on the LAN"
          }
          tone={tokens.extra.ok}
        />
      ))}
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
        Only an edge LosOS runs may process trading; any edge shares the pool.
      </Text>
    </View>
  );
}
