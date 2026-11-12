/* The Settings tab (DashboardScreen.kt SettingsTab): the box this app is
 * pointed at (address, token, save + probe), the theme the app itself
 * wears, and the About card that name-checks version 0.1.0. The probe
 * never carries the token (src/api/probe.ts; the Compose card said so in
 * exactly those words).
 */

import * as React from "react";
import { Text, View } from "react-native";

import { LOSOS_TYPE, type LososTokens, type ThemeMode } from "../theme/tokens";
import { useNativeLook } from "../platform";
import { Card, CardTitle } from "../components/Card";
import { DemoChip } from "../components/DemoChip";
import { FilterChip, LosButton, LosTextField } from "../components/primitives";
import { updateConfig, useBoxConfig, type ProbeResult, type BoxConfig } from "../../store/config";
import { probe } from "../../api/probe";

const Tail = () => <View style={{ height: 20 }} />;
const Gap4 = () => <View style={{ height: 4 }} />;
const Gap8 = () => <View style={{ height: 8 }} />;
const Gap10 = () => <View style={{ height: 10 }} />;

const THEME_MODES = ["system", "light", "dark"] as const;
const THEME_MODE_LABELS: Record<ThemeMode, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

export function SettingsTab({ tokens }: { tokens: LososTokens }): React.JSX.Element {
  const config = useBoxConfig();
  if (config === null) return <View />;
  return <SettingsTabBody tokens={tokens} config={config} />;
}

function SettingsTabBody({
  tokens,
  config,
}: {
  tokens: LososTokens;
  config: NonNullable<ReturnType<typeof useBoxConfig>>;
}): React.JSX.Element {
  const look = useNativeLook();
  // The fields read the config as it was when the tab opened, and Save
  // pushes both at once (DashboardScreen.kt keyed these on the config).
  const [host, setHost] = React.useState(config.host);
  const [token, setToken] = React.useState(config.token);
  const [probeState, setProbeState] = React.useState<ProbeResult | null>(null);

  const save = () => {
    // Save also dismisses a stale probe answer, as the Compose card did.
    setProbeState(null);
    updateConfig({ host: host.trim(), token: token.trim() });
  };
  const probeNow = () => {
    const candidate: BoxConfig = { ...config, host: host.trim(), token: token.trim() };
    updateConfig(candidate);
    setProbeState({ kind: "probing" });
    void probe({ host: candidate.host }).then(setProbeState);
  };

  return (
    <View style={{ ...look.shadow, elevation: look.elevation }}>
      <View style={{ paddingVertical: 12, paddingHorizontal: 4, flexDirection: "row", alignItems: "center" }}>
        <View style={{ flex: 1 }}>
          <Text style={{ ...LOSOS_TYPE.headlineMedium, color: tokens.palette.onSurface }}>Settings</Text>
        </View>
        {config.demo ? <DemoChip tokens={tokens} /> : null}
      </View>

      <Card tokens={tokens} demo={config.demo}>
        <CardTitle tokens={tokens} title="Box" />
        <Gap10 />
        <LosTextField
          tokens={tokens}
          value={host}
          onChangeText={setHost}
          placeholder="Address"
        />
        <Gap8 />
        <LosTextField
          tokens={tokens}
          value={token}
          onChangeText={setToken}
          placeholder="Access token"
        />
        <Gap10 />
        <View style={{ flexDirection: "row", gap: 10 }}>
          <LosButton tokens={tokens} kind="outlined" onPress={save}>
            Save
          </LosButton>
          <LosButton tokens={tokens} onPress={probeNow}>
            Probe now
          </LosButton>
        </View>
        {probeState !== null ? (
          <>
            <Gap8 />
            <Text
              style={{
                ...LOSOS_TYPE.bodySmall,
                color:
                  probeState.kind === "reachable" ? tokens.extra.ok : tokens.palette.onSurfaceVariant,
              }}
            >
              {probeState.kind === "probing"
                ? "Probing…"
                : probeState.kind === "reachable"
                  ? `Box answered in ${probeState.ms} ms`
                  : `No answer: ${probeState.reason}`}
            </Text>
          </>
        ) : null}
      </Card>

      <Card tokens={tokens} demo={config.demo}>
        <CardTitle tokens={tokens} title="Theme" />
        <Gap8 />
        <View style={{ flexDirection: "row", gap: 8, flexWrap: "wrap" }}>
          {THEME_MODES.map((mode) => (
            <FilterChip
              key={mode}
              tokens={tokens}
              selected={config.themeMode === mode}
              onPress={() => updateConfig({ themeMode: mode })}
              label={THEME_MODE_LABELS[mode]}
            />
          ))}
        </View>
      </Card>

      <Card tokens={tokens} demo={config.demo}>
        <CardTitle tokens={tokens} title="About" />
        <Gap4 />
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          LosOS Companion 0.1.0 - the box's panes in a phone's pocket, the same routes the admin
          pages and the losos terminal read. The destructive two, factory reset and erase, stay in
          the web UI.
        </Text>
      </Card>
      <Tail />
    </View>
  );
}
