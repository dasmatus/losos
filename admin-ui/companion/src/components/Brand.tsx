/* The LosOS brand: the coho salmon of themes/brand/salmon.png (NOAA
 * Fisheries cutout, credits beside the original), the mark every front end
 * carries. it sits at the head of the connect screen, exactly as the
 * Compose Brand did with the two words under the image.
 */

import * as React from "react";
import { Image, Text, View } from "react-native";

import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";

export function Brand({ tokens }: { tokens: LososTokens }) {
  return (
    <View style={{ flexDirection: "row", alignItems: "center" }}>
      <Image
        source={require("../../assets/images/salmon.png")}
        style={{ width: 52, height: 52 }}
        resizeMode="contain"
        accessibilityRole="image"
        accessibilityLabel="LosOS - a coho salmon"
      />
      <View style={{ width: 14 }} />
      <View>
        <Text style={{ ...LOSOS_TYPE.headlineSmall, color: tokens.palette.onSurface }}>LosOS</Text>
        <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.primary }}>Companion</Text>
      </View>
    </View>
  );
}
