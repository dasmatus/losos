/* The Cloud tab (DashboardScreen.kt CloudTab): the top of the files' root
 * listing only - deeper browsing stays in LosOS Cloud for now - and the
 * folders-on-the-box word it carried. Every demo row wears the chip.
 */

import * as React from "react";
import { Text, View } from "react-native";

import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { useNativeLook } from "../platform";
import { Card } from "../components/Card";
import { ReadCard } from "../components/ReadCard";
import { DemoChip } from "../components/DemoChip";
import type { BoxApi } from "../../api/api";
import { formatBytes, itemsOf, maybeBool, maybeDouble, maybeString } from "../../api/wire";
import { useBox } from "./Panes";
import { useRead } from "./Read";

const Tail = () => <View style={{ height: 20 }} />;
const Gap4 = () => <View style={{ height: 4 }} />;

export function CloudTab({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const look = useNativeLook();
  const entries = useBox();
  if (entries === null) return <View />;
  const { box, api } = entries;
  return (
    <View style={{ ...look.shadow, elevation: look.elevation }}>
      <View style={{ paddingVertical: 12, paddingHorizontal: 4, flexDirection: "row", alignItems: "center" }}>
        <View style={{ flex: 1 }}>
          <Text style={{ ...LOSOS_TYPE.headlineMedium, color: tokens.palette.onSurface }}>
            LosOS Cloud
          </Text>
        </View>
        {box.demo ? <DemoChip tokens={tokens} /> : null}
      </View>
      <Text style={{ ...cloudLead, color: tokens.palette.onSurfaceVariant }}>
        Your files, photos, calendars and boards live here.
      </Text>
      {box.demo ? (
        <Card tokens={tokens} demo>
          {([
            ["Documents", "108 files · 2.3 GB"],
            ["Photos", "1,208 · last import today"],
            ["Music", "213 · 1.1 GB"],
            ["Boards", "Chore chart · 3 left today"],
          ] as Array<[string, string]>).map(([name, meta]) => (
            <CloudRow key={name} tokens={tokens} name={name} meta={meta} />
          ))}
        </Card>
      ) : (
        <CloudFilesCard tokens={tokens} api={api} />
      )}
      <Card tokens={tokens} demo={box.demo}>
        <Text style={{ ...LOSOS_TYPE.labelSmall, color: tokens.palette.onSurface }}>
          Folders kept on the box only:
        </Text>
        <Gap4 />
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurface }}>
          Deleted from /persist the day you wipe the box, kept in every daily snapshot.
        </Text>
      </Card>
      <Tail />
    </View>
  );
}

const cloudLead = {
  ...LOSOS_TYPE.bodyMedium,
  marginTop: 0,
  marginBottom: 6,
  marginHorizontal: 4,
} as const;

/** GET /api/cloud/files?path=%2F, the root listing, folders first
 *  (DashboardScreen.kt CloudFilesCard). */
function CloudFilesCard({ tokens, api }: { tokens: LososTokens; api: BoxApi }): React.JSX.Element {
  const read = useRead(api, "/api/cloud/files?path=%2F");
  return (
    <ReadCard tokens={tokens} title="Files" read={read}>
      {(c) => {
        if (!maybeBool(c, "available")) {
          return (
            <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
              {maybeString(c, "reason") === "signIn"
                ? "Your files unlock the first time you sign in on the box's pages."
                : "LosOS Cloud is not answering right now."}
            </Text>
          );
        }
        const entries = itemsOf(c, "entries");
        return (
          <View>
            {entries.length === 0 ? (
              <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
                Nothing at the top yet.
              </Text>
            ) : null}
            {entries.map((entry, i) => (
              <CloudRow
                key={i}
                tokens={tokens}
                name={maybeString(entry, "name") ?? "-"}
                meta={maybeBool(entry, "folder") ? "folder" : formatBytes(maybeDouble(entry, "size"))}
              />
            ))}
            {maybeBool(c, "more") ? (
              <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
                There are more; deeper browsing stays in LosOS Cloud for now.
              </Text>
            ) : null}
          </View>
        );
      }}
    </ReadCard>
  );
}

/** One row (DashboardScreen.kt CloudRow): the glyph plate, the name, the
 *  meta word. */
function CloudRow({ tokens, name, meta }: { tokens: LososTokens; name: string; meta: string }) {
  return (
    <View style={{ width: "100%", paddingVertical: 8, flexDirection: "row", alignItems: "center" }}>
      <View
        style={{
          width: 34,
          height: 34,
          borderRadius: LOSOS_RADIUS.glyph,
          backgroundColor: tokens.palette.surfaceVariant,
          padding: 8,
          justifyContent: "center",
        }}
      >
        <View style={{ height: 2, backgroundColor: tokens.palette.outline, borderRadius: 1 }} />
      </View>
      <View style={{ width: 12 }} />
      <View style={{ flex: 1 }}>
        <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>{name}</Text>
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>{meta}</Text>
      </View>
    </View>
  );
}

/* failWord is a live name here: the ReadCard owns the failed word itself,
 * so this file carries none. */
