import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ComputerIcon, Moon02Icon, Sun03Icon } from "@hugeicons/core-free-icons";
import {
  getResolvedTheme,
  getTheme,
  isTheme,
  setTheme,
  subscribeTheme,
  THEMES,
  type ResolvedTheme,
  type Theme,
} from "@/lib/theme";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

/** The React binding for the three-state theme store in lib/theme.ts. */
export function useTheme(): {
  theme: Theme;
  resolvedTheme: ResolvedTheme;
  setTheme: (theme: Theme) => void;
} {
  const theme = React.useSyncExternalStore(subscribeTheme, getTheme, () => "auto" as Theme);
  const resolvedTheme = React.useSyncExternalStore(
    subscribeTheme,
    getResolvedTheme,
    () => "light" as ResolvedTheme,
  );
  return { theme, resolvedTheme, setTheme };
}

const OPTIONS: Record<Theme, { label: MessageKey; icon: typeof ComputerIcon }> = {
  auto: { label: "ui.theme.auto", icon: ComputerIcon },
  light: { label: "ui.theme.light", icon: Sun03Icon },
  dark: { label: "ui.theme.dark", icon: Moon02Icon },
};

/* A three-way segmented control: Auto, Light, Dark. Not a two-state toggle —
 * Auto is the default and it is a real answer, not the absence of one. A
 * shadcn Toggle Group (single-select), which renders the radiogroup the old
 * hand-made control did and adds the arrow-key behaviour it lacked. */
export function ThemeToggle({ className }: { className?: string }) {
  const { theme, setTheme: choose } = useTheme();
  const t = useT();

  return (
    <ToggleGroup
      aria-label={t("ui.theme.label")}
      value={theme}
      onValueChange={(next) => {
        if (isTheme(next)) choose(next);
      }}
      size="icon"
      className={className}
    >
      {THEMES.map((option) => {
        const label = t(OPTIONS[option].label);
        const { icon } = OPTIONS[option];
        return (
          <ToggleGroupItem key={option} value={option} aria-label={label} title={label}>
            <HugeiconsIcon
              icon={icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </ToggleGroupItem>
        );
      })}
    </ToggleGroup>
  );
}
