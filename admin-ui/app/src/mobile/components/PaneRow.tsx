/* One line of a pane's list (Panes.kt PaneRow): a title at the left, a
 * value word at its right, a detail line under both. `tone` colours title
 * and value together - the ok/warn/crit words of the extra triple.
 */

import { Text, View } from "react-native";

import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { useNativeLook } from "../platform";
export function PaneRow({
  tokens,
  title,
  detail = "",
  value = "",
  tone,
}: {
  tokens: LososTokens;
  title: string;
  detail?: string;
  value?: string;
  tone?: string;
}) {
  const look = useNativeLook();
  const text = tone ?? tokens.palette.onSurface;
  const valueColor = tone ?? tokens.palette.onSurfaceVariant;
  const detailColor = tokens.palette.onSurfaceVariant;
  return (
    <View style={{ width: "100%", paddingVertical: 6, elevation: look.elevation, ...look.shadow }}>
      <View style={{ flexDirection: "row", justifyContent: "space-between" }}>
        <Text
          style={{ ...LOSOS_TYPE.titleMedium, fontFamily: look.fontFamily, color: text }}
          ellipsizeMode="tail"
          numberOfLines={2}
        >
          {title}
        </Text>
        {value !== "" ? (
          <Text style={{ ...LOSOS_TYPE.labelSmall, fontFamily: look.fontFamily, fontWeight: "600", color: valueColor, marginLeft: 10 }}>
            {value}
          </Text>
        ) : null}
      </View>
      {detail !== "" ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, fontFamily: look.fontFamily, color: detailColor }}>{detail}</Text>
      ) : null}
    </View>
  );
}

/** One hardening switch's row (SettingsPanes.kt SecurityRow): a name, its
 *  "on"/"off" in the extra words, and what the switch costs. The off state
 *  renders muted, as the Compose row's onSurfaceVariant tone did. */
export function SecurityRow({
  tokens,
  name,
  on,
  detail,
}: {
  tokens: LososTokens;
  name: string;
  on: boolean;
  detail: string;
}) {
  return (
    <PaneRow
      tokens={tokens}
      title={name}
      detail={detail}
      value={on ? "on" : "off"}
      tone={on ? tokens.extra.ok : tokens.palette.onSurfaceVariant}
    />
  );
}
