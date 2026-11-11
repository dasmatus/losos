/* The shell (DashboardScreen.kt): one bar with the box's panes, in the
 * phone's order. Each tab is a pane over the routes the admin SPA drives
 * (Panes.kt, SettingsPanes.kt); eleven of them scroll. The bar wears the
 * palette like a NavigationBar but scrolls: the TUI's panes number more
 * than a thumb's width holds at once.
 *
 * On TV the bar is a D-pad focus lane (react-native-tvos reports focus on
 * Pressable) and the panes sit in a wider, roomier layout; on visionOS
 * the content is centred in a generous reading column so a window has
 * space to breathe instead of stretching phone-width rows across the
 * canvas. visionOS also exposes a "Living room" presentation: a
 * glance-across-the-room dashboard that reuses the same pane data with
 * bigger tiles and coarse navigation.
 */

import * as React from "react";
import {
  Pressable,
  ScrollView,
  Text,
  View,
  useWindowDimensions,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTokens } from "../theme/useTokens";
import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { isTV, isVisionOS, tvFocusable } from "../platform";
import { TvPressable } from "../components/TvPressable";
import { LivingRoomDashboard } from "../components/LivingRoomDashboard";
import { SettingsTab } from "../panes/SettingsTab";
import { CloudTab } from "../panes/CloudTab";
import { AppsPane, MeshPane, OverviewPane, StoragePane } from "../panes/Panes";
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

export function DashboardScreen({
  initialTab,
}: {
  initialTab?: Tab;
}): React.JSX.Element {
  const tokens = useTokens();
  const [current, setCurrent] = React.useState<Tab>(initialTab ?? "Dashboard");
  const [livingRoom, setLivingRoom] = React.useState(false);
  useWindowDimensions(); // keep hook available for future spatial/layout tuning

  // visionOS living room is a presentation of the same dashboard data, not a
  // separate app. Anything that needs precise editing routes back to normal.
  if (isVisionOS && livingRoom) {
    return (
      <View style={{ flex: 1, backgroundColor: tokens.palette.background }}>
        <LivingRoomDashboard
          tokens={tokens}
          onExit={() => setLivingRoom(false)}
        />
      </View>
    );
  }

  const showVisionToggle = isVisionOS && current === "Dashboard";
  return (
    <View style={{ flex: 1, backgroundColor: tokens.palette.background }}>
      {showVisionToggle ? (
        <View
          style={{
            paddingHorizontal: isVisionOS ? 28 : 16,
            paddingTop: 10,
            alignItems: "flex-end",
          }}
        >
          <TvPressable
            onPress={() => setLivingRoom(true)}
            style={{
              paddingHorizontal: 14,
              paddingVertical: 10,
              borderRadius: LOSOS_RADIUS.large,
              backgroundColor: tokens.palette.secondaryContainer,
            }}
          >
            <Text
              style={{
                ...LOSOS_TYPE.labelSmall,
                color: tokens.palette.onSecondaryContainer,
              }}
            >
              Living room
            </Text>
          </TvPressable>
        </View>
      ) : null}
      <KeyboardAwarePanes tokens={tokens} tab={current} />
      <PaneBar tokens={tokens} current={current} pick={setCurrent} />
    </View>
  );
}

/* Eleven on the left, and a scroll to move between them: the panes live
 * in one column and the bar sits at the phone's bottom, the Compose
 * Scaffold's only two children. TV uses a wider, leanback-friendly canvas
 * with larger gutters; visionOS centres a roomy reading column. */
function KeyboardAwarePanes({
  tokens,
  tab,
}: {
  tokens: LososTokens;
  tab: Tab;
}) {
  const leanback = isTV;
  const spatial = isVisionOS;
  const contentMaxWidth = leanback ? 1100 : spatial ? 920 : undefined;
  const horizontal = leanback ? 48 : spatial ? 32 : 16;
  return (
    <ScrollView
      contentContainerStyle={{
        paddingHorizontal: horizontal,
        paddingVertical: leanback ? 20 : 8,
        backgroundColor: tokens.palette.background,
        alignItems: contentMaxWidth ? "center" : undefined,
      }}
      keyboardShouldPersistTaps="handled"
    >
      <View
        style={{
          width: "100%",
          maxWidth: contentMaxWidth,
          gap: leanback ? 8 : 0,
        }}
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
      </View>
    </ScrollView>
  );
}

/* The tab bar (DashboardScreen.kt PaneBar): a dot per pane and the pane's
 * name, the selected one in the secondary container like the Compose bar.
 * On TV each tab is a focusable target with a visible ring for D-pad nav. */
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
    <View
      style={{ backgroundColor: tokens.palette.surface, paddingBottom: bottom }}
    >
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={{ padding: isTV ? 12 : 6, gap: isTV ? 8 : 2 }}
      >
        {TABS.map((tab) => {
          const selected = tab === current;
          const Button = isTV ? TvPressable : Pressable;
          return (
            <Button
              key={tab}
              onPress={() => pick(tab)}
              {...tvFocusable}
              style={({
                pressed,
              }: {
                pressed: boolean;
                focused?: boolean;
              }) => ({
                padding: 2,
                ...paneCell,
                minWidth: isTV ? 120 : undefined,
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
                    width: isTV ? 28 : 22,
                    height: isTV ? 28 : 22,
                    borderRadius: isTV ? 14 : 11,
                    backgroundColor: selected
                      ? tokens.palette.primary
                      : tokens.palette.outline,
                  }}
                />
                <View style={{ height: isTV ? 6 : 2 }} />
                <Text
                  style={{
                    ...(isTV ? LOSOS_TYPE.titleMedium : LOSOS_TYPE.bodySmall),
                    color: selected
                      ? tokens.palette.onSecondaryContainer
                      : tokens.palette.onSurfaceVariant,
                  }}
                >
                  {tab}
                </Text>
              </View>
            </Button>
          );
        })}
      </ScrollView>
    </View>
  );
}

/* The selected pill's cell: the pane's dot and name, tap-wide enough for a
 *  thumb. TV targets are larger for D-pad landing. */
const paneCell = { paddingVertical: 8, paddingHorizontal: 14 } as const;
