/* ConnectScreen (the port of ConnectScreen.kt).
 *
 * The first screen until the companion knows a box (or the owner picks the
 * demo). Fields mirror the terminal client's `losos login <address>`; the
 * pasted token is the one the API client rides on (src/api/api.ts), and
 * this is also the screen a 401 lands the owner on when the box rejects
 * the old token - the `unlocked` headline below reads the config the
 * eviction left (host set, token gone).
 *
 * The LAN's boxes arrive through the `_losos._tcp` browse (src/api/
 * discovery.ts): a row's tap takes the `.local` address into the field,
 * with an `IP` pill for the fallback when the phone's own lookup is broken.
 * The probe never carries the token (src/api/probe.ts): reachability is
 * not identity.
 */

import * as React from "react";
import { Image, Pressable, ScrollView, Text, TextInput, View } from "react-native";

import { useTokens } from "../theme/useTokens";
import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";
import { probe } from "../../api/probe";
import { browseBoxes, type FoundBox } from "../../api/discovery";
import { useBoxConfig, updateConfig, type BoxConfig, type ProbeResult } from "../../store/config";

export function ConnectScreen(): React.JSX.Element {
  const tokens = useTokens();
  const config: BoxConfig | null = useBoxConfig();
  if (config === null) return <View />;
  return <ConnectScreenBody tokens={tokens} config={config} />;
}

function ConnectScreenBody({
  tokens,
  config,
}: {
  tokens: LososTokens;
  config: BoxConfig;
}): React.JSX.Element {
  const [host, setHost] = React.useState(config.host !== "" ? config.host : "box.local");
  const [token, setToken] = React.useState(config.token);
  const [probeState, setProbeState] = React.useState<ProbeResult>({
    kind: "unreachable",
    reason: "Not probed yet",
  });
  // The LAN's boxes by their `_losos._tcp` announcement (Discovery.kt);
  // null until the owner asks, empty when the browse came back with none.
  const [boxes, setBoxes] = React.useState<FoundBox[] | null>(null);
  const [scanning, setScanning] = React.useState(false);
  // The box threw the token back: this screen is the re-prompt now.
  const unlocked = config.host !== "" && config.token === "";

  const discover = () => {
    if (scanning) return;
    setScanning(true);
    void browseBoxes().then((found) => {
      setBoxes(found);
      setScanning(false);
    });
  };

  const runProbe = () => {
    setProbeState({ kind: "probing" });
    void probe({ host }).then(setProbeState);
  };

  return (
    <ScrollView
      contentContainerStyle={{
        flexGrow: 1,
        justifyContent: "center",
        padding: 28,
        backgroundColor: tokens.palette.background,
      }}
    >
      <Brand tokens={tokens} />
      <View style={{ height: 36 }} />
      <Text style={{ ...LOSOS_TYPE.headlineSmall, color: tokens.palette.onSurface }}>
        {unlocked ? "The box wants its token again" : "Find your box"}
      </Text>
      <View style={{ height: 4 }} />
      <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
        {unlocked
          ? "The one you pasted no longer opens it - paste its current one."
          : "Reach it as <name>.local on the home network, or by the IP printed on the box's screen."}
      </Text>
      <Gap height={20} />
      <LosField
        tokens={tokens}
        value={host}
        onChange={setHost}
        label="Box address"
        supporting="box.local or 192.168.1.42"
      />
      <Gap height={12} />
      <LosField
        tokens={tokens}
        value={token}
        onChange={setToken}
        label="Access token"
        supporting="Settings → Access tokens in the admin pages"
      />
      <Gap height={20} />
      <View style={{ flexDirection: "row", gap: 12 }}>
        <LosPress tokens={tokens} onPress={runProbe} kind="outlined">
          Probe
        </LosPress>
        <LosPress tokens={tokens} onPress={() => updateConfig({ host: host.trim(), token: token.trim(), demo: false })}>
          Link box
        </LosPress>
      </View>
      <Gap height={14} />
      <LosPress tokens={tokens} onPress={() => updateConfig({ host: host.trim(), token: token.trim(), demo: true })}>
        Look around in demo mode
      </LosPress>
      <Gap height={18} />
      <Text
        style={{
          ...LOSOS_TYPE.bodyMedium,
          color: probeState.kind === "reachable" ? tokens.extra.ok : tokens.palette.onSurfaceVariant,
        }}
      >
        {probeState.kind === "probing"
          ? "Probing…"
          : probeState.kind === "reachable"
            ? `Box answered in ${probeState.ms} ms`
            : `No answer: ${probeState.reason}`}
      </Text>

      <Gap height={24} />
      <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>On this network</Text>
      <Gap height={6} />
      <LosPress tokens={tokens} onPress={discover}>
        {boxes === null ? (scanning ? "Browsing…" : "Find boxes") : "Browse again"}
      </LosPress>
      {boxes === null ? null : boxes.length === 0 ? (
        <>
          <Gap height={6} />
          <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
            None announced on this network - a box that is off, or a LAN no box answers on.
          </Text>
        </>
      ) : (
        boxes.map((box) => (
          <BoxRow
            key={`${box.name}:${box.port}`}
            tokens={tokens}
            box={box}
            onPick={setHost}
          />
        ))
      )}
    </ScrollView>
  );
}

/* One answer of the browse (ConnectScreen.kt BoxRow): the box's `.local`
 * name with its release tag; tapping takes it into the address, the `IP`
 * pill (present when resolution answered) takes the address the LAN gave
 * for phones whose own `.local` lookup is broken. */
function BoxRow({
  tokens,
  box,
  onPick,
}: {
  tokens: LososTokens;
  box: FoundBox;
  onPick: (address: string) => void;
}) {
  const local =
    box.port === 443
      ? box.address
      : box.port === 80
        ? `http://${box.address}`
        : `https://${box.address}:${box.port}`;
  return (
    <Pressable
      onPress={() => onPick(local)}
      style={({ pressed }) => ({
        width: "100%",
        marginTop: 6,
        backgroundColor: pressed ? tokens.palette.outline : tokens.palette.surfaceVariant,
        borderRadius: LOSOS_RADIUS.small,
        paddingVertical: 10,
        paddingHorizontal: 12,
        flexDirection: "row",
        alignItems: "center",
      })}
    >
      <View style={{ flex: 1 }}>
        <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>{box.name}</Text>
        {box.version !== null ? (
          <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant }}>
            {`LosOS ${box.version}`}
          </Text>
        ) : null}
      </View>
      {box.ip !== null ? (
        <Pressable
          hitSlop={8}
          onPress={() => {
            const ip = box.ip as string;
            onPick(box.port === 443 ? ip : `https://${ip}:${box.port}`);
          }}
          style={{
            borderRadius: LOSOS_RADIUS.extraSmall,
            paddingVertical: 4,
            paddingHorizontal: 10,
          }}
        >
          <Text style={{ ...LOSOS_TYPE.labelSmall, color: tokens.palette.primary }}>IP</Text>
        </Pressable>
      ) : null}
    </Pressable>
  );
}

/* ── Local atoms ────────────────────────────────────────────────────────── */

/** The two Material buttons (Button / OutlinedButton), named as the
 *  screens used them: filled for the actions that change the config. */
function LosPress({
  tokens,
  onPress,
  kind = "filled",
  children,
}: {
  tokens: LososTokens;
  onPress: () => void;
  kind?: "filled" | "outlined";
  children: React.ReactNode;
}) {
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => ({
        alignSelf: "flex-start",
        borderRadius: LOSOS_RADIUS.large,
        paddingVertical: 10,
        paddingHorizontal: 16,
        overflow: "hidden",
        ...(kind === "filled"
          ? { backgroundColor: tokens.palette.primary }
          : {
              borderWidth: 1,
              borderColor: tokens.palette.outline,
              backgroundColor: pressed ? tokens.palette.surfaceVariant : "transparent",
            }),
        opacity: pressed && kind === "filled" ? 0.85 : 1,
      })}
    >
      <Text
        style={{
          ...LOSOS_TYPE.labelSmall,
          fontWeight: "600",
          color: kind === "filled" ? tokens.palette.onPrimary : tokens.palette.primary,
        }}
      >
        {children}
      </Text>
    </Pressable>
  );
}

/** An OutlinedTextField: the label word above, the supporting word under
 *  it, exactly the two fields the Compose connect screen carried. */
function LosField({
  tokens,
  value,
  onChange,
  label,
  supporting,
}: {
  tokens: LososTokens;
  value: string;
  onChange: (text: string) => void;
  label: string;
  supporting: string;
}) {
  return (
    <View style={{ width: "100%" }}>
      <Text
        style={{
          ...LOSOS_TYPE.labelSmall,
          color: tokens.palette.onSurfaceVariant,
          marginBottom: 4,
        }}
      >
        {label}
      </Text>
      <TextInput
        value={value}
        onChangeText={onChange}
        placeholder={label}
        placeholderTextColor={tokens.palette.onSurfaceVariant}
        autoCapitalize="none"
        autoCorrect={false}
        style={{
          ...LOSOS_TYPE.bodyMedium,
          color: tokens.palette.onSurface,
          width: "100%",
          borderRadius: LOSOS_RADIUS.small,
          borderWidth: 1,
          borderColor: tokens.palette.outline,
          paddingHorizontal: 12,
          paddingVertical: 10,
        }}
      />
      <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
        {supporting}
      </Text>
    </View>
  );
}

const Gap = ({ height }: { height: number }) => <View style={{ height }} />;

/** The LosOS brand (ConnectScreen.kt Brand): the coho salmon of
 *  themes/brand/salmon.png and the two words under the image. */
export function Brand({ tokens }: { tokens: LososTokens }) {
  return (
    <View style={{ flexDirection: "row", alignItems: "center" }}>
      <Image
        source={require("../../../assets/images/salmon.png")}
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
