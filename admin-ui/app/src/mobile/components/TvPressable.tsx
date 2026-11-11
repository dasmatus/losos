import * as React from "react";
import { Pressable, type PressableProps, type StyleProp, type ViewStyle } from "react-native";

import { isTV } from "../platform";
import { useTokens } from "../theme/useTokens";
import { LOSOS_RADIUS } from "../theme/tokens";

/** A pressable that carries focus on TV.
 *
 * `react-native-tvos` moves focus with the remote's D-pad and reports it
 * through the `focused` render prop; without an explicit focus treatment
 * a row is reachable but invisible. The focus ring uses the theme's
 * primary colour so it reads the same in light and dark mode. On phone
 * and tablet builds this is a plain Pressable — `Platform.isTV` is false
 * outside the tvos fork's TV targets, so the focus style never applies. */
export function TvPressable({
  style,
  children,
  focusScale = 1.03,
  ...props
}: PressableProps & {
  focusScale?: number;
  children?: React.ReactNode | ((state: { pressed: boolean; focused: boolean }) => React.ReactNode);
}): React.JSX.Element {
  const tokens = useTokens();
  return (
    <Pressable
      accessible
      focusable={isTV || props.focusable}
      {...props}
      style={(state) => {
        const focused = "focused" in state ? Boolean(state.focused) : false;
        const base: StyleProp<ViewStyle> = [
          typeof style === "function" ? style(state) : style,
          focused && isTV
            ? {
                borderColor: tokens.palette.primary,
                borderWidth: 2,
                borderRadius: LOSOS_RADIUS.small,
                transform: [{ scale: focusScale }],
              }
            : null,
        ];
        return base;
      }}
    >
      {(state) => {
        const s = state as { pressed: boolean; focused?: boolean };
        const focused = Boolean(s.focused);
        return typeof children === "function"
          ? (children as (st: { pressed: boolean; focused: boolean }) => React.ReactNode)({
              pressed: s.pressed,
              focused,
            })
          : children;
      }}
    </Pressable>
  );
}
