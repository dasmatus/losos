/* The shell (DashboardScreen.kt): one bar with the box's panes, in the
 * phone's order. Each tab is a pane over the routes the admin SPA drives
 * (Panes.kt, SettingsPanes.kt); eleven of them scroll. The bar wears the
 * palette like a NavigationBar but scrolls: the TUI's panes number more
 * than a thumb's width holds at once.
 */

import * as React from "react";
import { Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTokens } from "../theme/useTokens";
import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { SettingsTab } from "../panes/SettingsTab";
import { CloudTab } from "../panes/CloudTab";
import {
  AppsPane,
  MeshPane,
  OverviewPane,
  StoragePane,
} from "../panes/Panes";
import {
  AdvancedPane,
  BackupPane,
  HistoryPane,
  NetworkPane,
  SecurityPane,
} from "../panes/SettingsPanes";

/* The phone's order, exactly the Compose app's tabs list. */
export const TABS = [
  "Dashboard",
  "Storage",
  "Mesh",
  "Apps",
  "Network",
  "Security",
  "Advanced",
  "History",
  "Backup",
  "Cloud",
  "Settings",
] as const;

export type Tab = (typeof TABS)[number];

export function DashboardScreen(): React.JSX.Element {
  const tokens = useTokens();
  const [current, setCurrent] = React.useState<Tab>("Dashboard");
  return (
    <View style={{ flex: 1, backgroundColor: tokens.palette.background }}>
      <KeyboardAwarePanes tokens={tokens} tab={current} />
      <PaneBar tokens={tokens} current={current} pick={setCurrent} />
    </View>
  );
}

/* Eleven on the left, and a scroll to move between them: the panes live
 * in one column and the bar sits at the phone's bottom, the Compose
 * Scaffold's only two children. */
function KeyboardAwarePanes({ tokens, tab }: { tokens: LososTokens; tab: Tab }) {
  return (
    <ScrollView
      contentContainerStyle={{
        paddingHorizontal: 16,
        paddingVertical: 8,
        backgroundColor: tokens.palette.background,
      }}
      keyboardShouldPersistTaps="handled"
    >
      {tab === "Dashboard" ? <OverviewPane tokens={tokens} /> : null}
      {tab === "Storage" ? <StoragePane tokens={tokens} /> : null}
      {tab === "Mesh" ? <MeshPane tokens={tokens} /> : null}
      {tab === "Apps" ? <AppsPane tokens={tokens} /> : null}
      {tab === "Network" ? <NetworkPane tokens={tokens} /> : null}
      {tab === "Security" ? <SecurityPane tokens={tokens} /> : null}
      {tab === "Advanced" ? <AdvancedPane tokens={tokens} /> : null}
      {tab === "History" ? <HistoryPane tokens={tokens} /> : null}
      {tab === "Backup" ? <BackupPane tokens={tokens} /> : null}
      {tab === "Cloud" ? <CloudTab tokens={tokens} /> : null}
      {tab === "Settings" ? <SettingsTab tokens={tokens} /> : null}
    </ScrollView>
  );
}

/* The tab bar (DashboardScreen.kt PaneBar): a dot per pane and the pane's
 * name, the selected one in the secondary container like the Compose bar. */
function PaneBar({
  tokens,
  current,
  pick,
}: {
  tokens: LososTokens;
  current: Tab;
  pick: (tab: Tab) => void;
}) {
  const bottom = useSafeAreaInsets().bottom;
  return (
    <View style={{ backgroundColor: tokens.palette.surface, paddingBottom: bottom }}>
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={{ padding: 6, gap: 2 }}
      >
        {TABS.map((tab) => {
          const selected = tab === current;
          return (
            <Pressable
              key={tab}
              onPress={() => pick(tab)}
              style={({ pressed }) => ({
                padding: 2,
                ...paneCell,
                ...(selected || pressed
                  ? {
                      borderRadius: LOSOS_RADIUS.large,
                      backgroundColor: selected
                        ? tokens.palette.secondaryContainer
                        : tokens.palette.surfaceVariant,
                    }
                  : null),
              })}
            >
              <View style={{ alignItems: "center" }}>
                <View
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: 11,
                    backgroundColor: selected ? tokens.palette.primary : tokens.palette.outline,
                  }}
                />
                <View style={{ height: 2 }} />
                <Text
                  style={{
                    ...LOSOS_TYPE.bodySmall,
                    color: selected
                      ? tokens.palette.onSecondaryContainer
                      : tokens.palette.onSurfaceVariant,
                  }}
                >
                  {tab}
                </Text>
              </View>
            </Pressable>
          );
        })}
      </ScrollView>
    </View>
  );
}

/* The selected pill's cell: the pane's dot and name, tap-wide enough for a
 *  thumb. (Compose PaneBar's horizontalScroll + spacedBy carried over.) */
const paneCell = { paddingVertical: 8, paddingHorizontal: 14 } as const;
