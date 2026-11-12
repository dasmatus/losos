/* The word that keeps demo data honest: while demo mode is on, every card
 * that carries invented numbers wears this chip, next to the title or the
 * tile that reads as a number. Where a pane shows no chip while it is up,
 * the pane reads the box (Panes.kt's DemoChip; the same rule named in the
 * README of the terminal client's panes).
 */

import { Text, type StyleProp, type TextStyle } from "react-native";

import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { useNativeLook } from "../platform";
export function DemoChip({ tokens, style }: { tokens: LososTokens; style?: StyleProp<TextStyle> }) {
  const look = useNativeLook();
  return (
    <Text
      style={[
        {
          ...LOSOS_TYPE.labelSmall,
          fontFamily: look.fontFamily,
          color: tokens.extra.crit,
          backgroundColor: withWash(tokens.extra.crit),
          alignSelf: "flex-start",
          borderRadius: LOSOS_RADIUS.chip * look.cornerRadiusScale,
          overflow: "hidden",
          paddingVertical: 2,
          paddingHorizontal: 6,
        },
        style,
      ]}
    >
      demo
    </Text>
  );
}

/** The chip's wash: the same alpha the Compose chip carried
 *  (crit.copy(alpha = 0.12f) on the card's ground). A wash over an ink
 *  colour matches what the chip looked like on both white cards and in
 *  dark mode's surface. */
function withWash(crit: string): string {
  return `${crit}20`;
}
