/* The pane UI's buttons, text fields, filter chips and alerts: the
 * Material3 pieces Panes.kt/SettingsPanes.kt called (Button,
 * OutlinedButton, OutlinedTextField, FilterChip, AlertDialog), rebuilt in
 * the palette and nothing else. The destructive two (factory reset, erase)
 * stayed in the web UI, so these are the only action shapes the box's
 * panes need.
 *
 * The alert is a Modal, not the platform Alert.alert: the Compose dialogs
 * carried a body that tracks state the press changes (a busy flag), which
 * the blocking alert cannot.
 */

import * as React from "react";
import {
  ActivityIndicator,
  Modal,
  Pressable,
  Text,
  TextInput,
  View,
  type TextInputProps,
  type ViewStyle,
} from "react-native";

import { LOSOS_RADIUS, LOSOS_TYPE, type LososTokens } from "../theme/tokens";

/* ── Buttons ─────────────────────────────────────────────────────────────── */

type ButtonKind = "filled" | "outlined";

export function LosButton({
  tokens,
  onPress,
  children,
  kind = "filled",
  enabled = true,
  busy = false,
}: {
  tokens: LososTokens;
  onPress: () => void;
  children: React.ReactNode;
  kind?: ButtonKind;
  enabled?: boolean;
  busy?: boolean;
}) {
  const ink =
    kind === "filled"
      ? tokens.palette.onPrimary
      : enabled
        ? tokens.palette.primary
        : tokens.palette.onSurfaceVariant;

  const shell: ViewStyle = {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    alignSelf: "flex-start",
    borderRadius: LOSOS_RADIUS.large,
    paddingVertical: 10,
    paddingHorizontal: 16,
    overflow: "hidden",
    ...(kind === "filled"
      ? { backgroundColor: enabled ? tokens.palette.primary : tokens.palette.surfaceVariant }
      : {
          backgroundColor: "transparent",
          borderWidth: 1,
          borderColor: enabled ? tokens.palette.outline : tokens.palette.surfaceVariant,
        }),
  };

  return (
    <Pressable onPress={onPress} disabled={!enabled || busy} style={({ pressed }) => [
      shell,
      pressed && enabled && !busy ? { opacity: 0.85 } : null,
    ]}>
      {busy ? <ActivityIndicator size="small" color={kind === "filled" ? ink : tokens.palette.primary} /> : null}
      <Text style={{ ...LOSOS_TYPE.labelSmall, color: ink }}>{children}</Text>
    </Pressable>
  );
}

/* ── Text fields ─────────────────────────────────────────────────────────── */

/** OutlinedTextField with the supporting word the connect screen and the
 *  Settings tab put under theirs. */
export function LosTextField({
  tokens,
  supporting,
  style,
  ...rest
}: TextInputProps & { tokens: LososTokens; supporting?: string }): React.JSX.Element {
  return (
    <View>
      <TextInput
        {...rest}
        autoCapitalize="none"
        autoCorrect={false}
        placeholderTextColor={tokens.palette.onSurfaceVariant}
        style={[
          {
            ...LOSOS_TYPE.bodyMedium,
            color: tokens.palette.onSurface,
            borderRadius: LOSOS_RADIUS.small,
            borderWidth: 1,
            borderColor: tokens.palette.outline,
            paddingHorizontal: 12,
            paddingVertical: 10,
          },
          style,
        ]}
      />
      {supporting !== undefined ? (
        <Text
          style={{
            ...LOSOS_TYPE.bodySmall,
            color: tokens.palette.onSurfaceVariant,
            marginTop: 4,
          }}
        >
          {supporting}
        </Text>
      ) : null}
    </View>
  );
}

/* ── Filter chips ───────────────────────────────────────────────────────── */

export function FilterChip({
  tokens,
  selected,
  onPress,
  label,
}: {
  tokens: LososTokens;
  selected: boolean;
  onPress: () => void;
  label: string;
}) {
  return (
    <Pressable
      onPress={onPress}
      style={{
        borderRadius: LOSOS_RADIUS.large,
        paddingVertical: 8,
        paddingHorizontal: 14,
        backgroundColor: selected
          ? tokens.palette.secondaryContainer
          : tokens.palette.surfaceVariant,
        overflow: "hidden",
      }}
    >
      <Text
        style={{
          ...LOSOS_TYPE.labelSmall,
          color: selected ? tokens.palette.onSecondaryContainer : tokens.palette.onSurfaceVariant,
        }}
      >
        {label}
      </Text>
    </Pressable>
  );
}

/* ── The Compose AlertDialog, as a Modal ─────────────────────────────────── */

export function LosDialog({
  tokens,
  open,
  title,
  body,
  confirmLabel,
  onConfirm,
  onDismiss,
  confirmEnabled = true,
  confirmBusy = false,
  dismissLabel = "Cancel",
}: {
  tokens: LososTokens;
  open: boolean;
  title: string;
  body: React.ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onDismiss: () => void;
  confirmEnabled?: boolean;
  confirmBusy?: boolean;
  dismissLabel?: string;
}) {
  return (
    <Modal visible={open} transparent animationType="fade" onRequestClose={onDismiss}>
      <Pressable
        onPress={onDismiss}
        style={{
          flex: 1,
          backgroundColor: "#00000066",
          justifyContent: "center",
          padding: 24,
        }}
      >
        <Pressable
          onPress={(e) => e.stopPropagation()}
          style={{
            backgroundColor: tokens.palette.surface,
            borderRadius: LOSOS_RADIUS.large,
            padding: 20,
          }}
        >
          <Text style={{ ...LOSOS_TYPE.headlineSmall, color: tokens.palette.onSurface }}>
            {title}
          </Text>
          <View style={{ height: 12 }} />
          {body}
          <View style={{ height: 16 }} />
          <View style={{ flexDirection: "row", justifyContent: "flex-end", gap: 8 }}>
            <LosButton tokens={tokens} kind="outlined" onPress={onDismiss}>
              {dismissLabel}
            </LosButton>
            <LosButton tokens={tokens} onPress={onConfirm} enabled={confirmEnabled} busy={confirmBusy}>
              {confirmLabel}
            </LosButton>
          </View>
        </Pressable>
      </Pressable>
    </Modal>
  );
}
