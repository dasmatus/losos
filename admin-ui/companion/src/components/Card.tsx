/* The cards the panes draw (Compose Card2 / DashboardScreen.kt): a full
 * width surface with the palette's own corners and 16pt inset, carrying an
 * optional demo chip in its top-right and children beneath. Every pane
 * reads through ReadCard, above which sits this.
 */

import * as React from "react";
import { Text, View, type StyleProp, type ViewStyle } from "react-native";

import { DemoChip } from "./DemoChip";
import { LOSOS_RADIUS, type LososTokens } from "../theme/tokens";

export function Card({
  tokens,
  demo = false,
  style,
  children,
}: {
  tokens: LososTokens;
  demo?: boolean;
  style?: StyleProp<ViewStyle>;
  children: React.ReactNode;
}) {
  return (
    <View
      style={[
        {
          backgroundColor: tokens.palette.surface,
          borderRadius: LOSOS_RADIUS.medium,
          padding: 16,
          marginVertical: 6,
        },
        style,
      ]}
    >
      {demo ? (
        <View style={{ marginBottom: 6, alignItems: "flex-end" }}>
          <DemoChip tokens={tokens} />
        </View>
      ) : null}
      {children}
    </View>
  );
}

/** A row of pane cards' carried title (ReadCard's first line): the
 *  titleMedium style over the card, in the onSurface ink (Compose's
 *  typography draws that colour from the scheme). */
export function CardTitle({ tokens, title }: { tokens: LososTokens; title: string }) {
  return (
    <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface, marginBottom: 8 }}>
      {title}
    </Text>
  );
}
