import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ComputerIcon, Moon02Icon, Sun03Icon } from "@hugeicons/core-free-icons";
import {
  getResolvedTheme,
  getTheme,
  setTheme,
  subscribeTheme,
  THEMES,
  type ResolvedTheme,
  type Theme,
} from "@/lib/theme";
import { cn } from "@/lib/utils";

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

const OPTIONS: Record<Theme, { label: string; icon: typeof ComputerIcon }> = {
  auto: { label: "Match the browser", icon: ComputerIcon },
  light: { label: "Light", icon: Sun03Icon },
  dark: { label: "Dark", icon: Moon02Icon },
};

/* A three-way segmented control: Auto, Light, Dark. Not a two-state toggle —
 * Auto is the default and it is a real answer, not the absence of one. */
export function ThemeToggle({ className }: { className?: string }) {
  const { theme, setTheme: choose } = useTheme();

  return (
    <div
      role="radiogroup"
      aria-label="Appearance"
      className={cn(
        "inline-flex items-center gap-0.5 rounded-control border border-line bg-sunk p-0.5",
        className,
      )}
    >
      {THEMES.map((option) => {
        const { label, icon } = OPTIONS[option];
        const selected = theme === option;
        return (
          <button
            key={option}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={label}
            title={label}
            onClick={() => choose(option)}
            className={cn(
              "inline-flex size-7 items-center justify-center rounded-[4px]",
              "transition-colors duration-150",
              "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
              selected ? "bg-surface text-ink shadow-card" : "text-faint hover:text-ink",
            )}
          >
            <HugeiconsIcon
              icon={icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </button>
        );
      })}
    </div>
  );
}
