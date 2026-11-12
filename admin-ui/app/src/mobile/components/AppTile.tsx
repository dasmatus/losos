/* The tiles and widgets the demo mode draws, kept from the dashboard's
 * first drafts (Panes.kt's AppTile and Widget): a glyph plate, a labelled
 * big number, and a demo chip on every one.
 */

import { Text, View } from "react-native";

import { DemoChip } from "./DemoChip";
import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { useNativeLook } from "../platform";
export function AppTile({
  tokens,
  glyph,
  name,
  sub,
  demo = true,
  flex = 1,
}: {
  tokens: LososTokens;
  glyph: string;
  name: string;
  sub: string;
  demo?: boolean;
  flex?: number;
}) {
  const look = useNativeLook();
  return (
    <View
      style={{
        flex,
        backgroundColor: tokens.palette.surface,
        borderRadius: LOSOS_RADIUS.medium * look.cornerRadiusScale,
        padding: 12,
        margin: 3,
        elevation: look.elevation,
        ...look.shadow,
      }}
    >
      <View
        style={{
          width: 38,
          height: 38,
          borderRadius: LOSOS_RADIUS.glyph,
          backgroundColor: tokens.palette.primaryContainer,
          alignItems: "center",
          justifyContent: "center",
          marginBottom: 8,
        }}
      >
        <Text
          style={{
            ...LOSOS_TYPE.titleMedium,
            fontFamily: look.fontFamily,
            color: tokens.palette.onPrimaryContainer,
          }}
        >
          {glyph}
        </Text>
      </View>
      <View style={{ flexDirection: "row", alignItems: "center", gap: 4 }}>
        <Text style={{ ...LOSOS_TYPE.labelSmall, fontFamily: look.fontFamily, color: tokens.palette.onSurface }}>{name}</Text>        {demo ? <DemoChip tokens={tokens} /> : null}
      </View>
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>{sub}</Text>
    </View>
  );
}

export function Widget({
  tokens,
  title,
  big,
  sub,
  demo = true,
  flex = 1,
}: {
  tokens: LososTokens;
  title: string;
  big: string;
  sub: string;
  demo?: boolean;
  flex?: number;
}) {
  const look = useNativeLook();
  return (
    <View
      style={{
        flex,
        backgroundColor: tokens.palette.primaryContainer,
        borderRadius: LOSOS_RADIUS.medium * look.cornerRadiusScale,
        padding: 12,
        margin: 3,
        elevation: look.elevation,
        ...look.shadow,
        borderRadius: LOSOS_RADIUS.medium,
        padding: 12,
        margin: 3,
      }}
    >
      <View style={{ flexDirection: "row", alignItems: "center", gap: 4 }}>
        <Text style={{ ...LOSOS_TYPE.labelSmall, color: tokens.palette.onPrimaryContainer }}>
          {title}
        </Text>
        {demo ? <DemoChip tokens={tokens} /> : null}
      </View>
      <Text style={{ ...LOSOS_TYPE.headlineSmall, color: tokens.palette.onPrimaryContainer }}>
        {big}
      </Text>
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onPrimaryContainer }}>{sub}</Text>
    </View>
  );
}
