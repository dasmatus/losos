/* A card whose body comes from one route's read (Panes.kt ReadCard): a
 * wait word, the box's own error word, or the document itself through
 * children. Every pane that reads a route draws at least one.
 */

import * as React from "react";
import { Text, View } from "react-native";

import { Card } from "./Card";
import { LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { useNativeLook } from "../platform";
import type { JsonBody } from "../../api/api";
import type { Read } from "../panes/Read";
export function ReadCard({
  tokens,
  title,
  read,
  children,
}: {
  tokens: LososTokens;
  title: string;
  read: Read;
  children: (body: JsonBody) => React.ReactNode;
}) {
  const look = useNativeLook();
  return (
    <Card tokens={tokens}>
      <Text style={{ ...LOSOS_TYPE.titleMedium, fontFamily: look.fontFamily, color: tokens.palette.onSurface }}>{title}</Text>
      <View style={{ height: 8 }} />
      {read.kind === "loading" ? (
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          Asking the box…
        </Text>
      ) : read.kind === "failed" ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit }}>
          {`Could not read: ${read.reason}`}
        </Text>
      ) : (
        children(read.data)
      )}
    </Card>
  );
}
