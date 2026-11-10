/* The meter the dashboard's storage card has always drawn (Panes.kt
 * StorageMeter and its StorageStripCard twin): a rounded progress bar in
 * the primary colour over the sunk ground, the fraction below it in words.
 */

import { Text, View } from "react-native";

import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";

export function StorageMeter({
  tokens,
  fraction,
  phrase,
}: {
  tokens: LososTokens;
  fraction: number;
  /** The words under the bar; skipped when the caller draws its own. */
  phrase?: string;
}) {
  const clamped = Math.min(Math.max(fraction, 0), 1);
  return (
    <View>
      <View
        style={{
          width: "100%",
          height: 8,
          borderRadius: LOSOS_RADIUS.meter,
          backgroundColor: tokens.palette.surfaceVariant,
          overflow: "hidden",
        }}
      >
        <View
          style={{
            width: `${Math.round(clamped * 100)}%`,
            height: 8,
            borderRadius: LOSOS_RADIUS.meter,
            backgroundColor: tokens.palette.primary,
          }}
        />
      </View>
      {phrase !== undefined ? (
        <Text style={{ ...LOSOS_TYPE.bodyMedium, marginTop: 8 }}>{phrase}</Text>
      ) : null}
    </View>
  );
}
