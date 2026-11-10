/* A pane's title and its one-line summary (Panes.kt PaneHeader), with the
 * demo chip riding beside it while demo mode reads its numbers.
 */

import { Text, View } from "react-native";

import { DemoChip } from "./DemoChip";
import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";

export function PaneHeader({
  tokens,
  title,
  summary,
  demo,
}: {
  tokens: LososTokens;
  title: string;
  summary: string;
  demo: boolean;
}) {
  return (
    <View style={{ paddingVertical: 12, paddingHorizontal: 4, alignItems: "center", flexDirection: "row" }}>
      <View style={{ flex: 1 }}>
        <Text style={{ ...LOSOS_TYPE.headlineMedium, color: tokens.palette.onSurface }}>{title}</Text>
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          {summary}
        </Text>
      </View>
      {demo ? <DemoChip tokens={tokens} /> : null}
    </View>
  );
}
