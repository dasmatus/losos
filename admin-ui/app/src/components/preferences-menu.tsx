import { Menu } from "@base-ui/react/menu";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowRight01Icon,
  ComputerIcon,
  LanguageCircleIcon,
  Moon02Icon,
  PaintBoardIcon,
  Sun03Icon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { popupMotion } from "@/components/ui/motion";
import { useTheme } from "@/components/ui/theme-toggle";
import { isLocale, LOCALE_NAMES, LOCALES, type MessageKey } from "@/lib/i18n";
import { useLocaleChoice, useT } from "@/lib/i18n-react";
import { isTheme, THEMES, type Theme } from "@/lib/theme";
import { cn } from "@/lib/utils";

/* The theme and the language, as two rows of a menu: the person menu in the
 * top bar (components/app-bar.tsx) holds them under the person's name, and
 * a page nobody is signed in to holds them in `DisplayMenu`, one button in
 * its bar. Each row opens a submenu of choices, the shape the context
 * menu's Appearance already has, with the current choice named on the row
 * so it reads without opening.
 *
 * Both stay this browser's: the theme in lib/theme.ts, the language in
 * lib/i18n.ts, each in localStorage and stamped on <html> before the first
 * paint. The no-JavaScript pages draw the same two rows as disclosures
 * (backend/plugins/page/src/shell.rs). */

const THEME_OPTIONS: Record<Theme, { label: MessageKey; icon: IconSvgElement }> = {
  auto: { label: "ui.theme.auto", icon: ComputerIcon },
  light: { label: "ui.theme.light", icon: Sun03Icon },
  dark: { label: "ui.theme.dark", icon: Moon02Icon },
};

export const menuItemClass = cn(
  "flex min-h-8 cursor-default items-center gap-2 rounded-control px-2 py-1.5 text-[13px] text-ink outline-none select-none",
  "transition-colors duration-100 data-highlighted:bg-accent-wash data-popup-open:bg-accent-wash",
);

const popupClass = cn(
  "min-w-52 max-w-[calc(100vw-2rem)] rounded-card border border-line bg-surface p-1 text-ink shadow-pop outline-none",
  popupMotion,
);

function Glyph({ icon, className }: { icon: IconSvgElement; className?: string }) {
  return (
    <HugeiconsIcon
      icon={icon}
      size={14}
      strokeWidth={1.5}
      color="currentColor"
      aria-hidden="true"
      className={cn("shrink-0", className)}
    />
  );
}

/** A row that opens a list of choices beside the menu. */
function ChoiceRow<V extends string>({
  slot,
  icon,
  label,
  current,
  value,
  options,
  onChange,
}: {
  slot: string;
  icon: IconSvgElement;
  label: string;
  current: string;
  value: V;
  options: readonly { value: V; label: string; icon?: IconSvgElement; lang?: string }[];
  onChange: (value: string) => void;
}) {
  return (
    <Menu.SubmenuRoot>
      <Menu.SubmenuTrigger className={menuItemClass} label={label} data-slot={slot}>
        <Glyph icon={icon} />
        <span className="flex-1">{label}</span>
        <span className="max-w-[8rem] truncate text-[12px] text-muted">{current}</span>
        <Glyph icon={ArrowRight01Icon} className="text-faint rtl:rotate-180" />
      </Menu.SubmenuTrigger>
      <Menu.Portal>
        <Menu.Positioner alignOffset={-5} sideOffset={2} collisionPadding={8} className="isolate z-50 outline-none">
          <Menu.Popup aria-label={label} data-slot={`${slot}-choices`} className={popupClass}>
            <Menu.RadioGroup value={value} onValueChange={(next: string) => onChange(next)}>
              {options.map((option) => (
                <Menu.RadioItem
                  key={option.value}
                  value={option.value}
                  closeOnClick
                  label={option.label}
                  className={menuItemClass}
                  lang={option.lang}
                >
                  {option.icon !== undefined ? (
                    <Glyph icon={option.icon} />
                  ) : (
                    <span aria-hidden="true" className="size-[14px] shrink-0" />
                  )}
                  <span className="min-w-0 flex-1 truncate">{option.label}</span>
                  <Menu.RadioItemIndicator className="text-accent">
                    <HugeiconsIcon icon={Tick02Icon} size={14} strokeWidth={2} color="currentColor" aria-hidden="true" />
                  </Menu.RadioItemIndicator>
                </Menu.RadioItem>
              ))}
            </Menu.RadioGroup>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.SubmenuRoot>
  );
}

/** The two rows, for inside a Base UI menu's popup. */
export function PreferenceItems() {
  const t = useT();
  const { theme, setTheme } = useTheme();
  const [locale, setLocale] = useLocaleChoice();
  const themes = THEMES.map((value) => ({ value, label: t(THEME_OPTIONS[value].label), icon: THEME_OPTIONS[value].icon }));
  const languages = [
    { value: "auto", label: t("ui.language.auto") },
    ...LOCALES.map((value) => ({ value, label: LOCALE_NAMES[value], lang: value })),
  ];
  return (
    <Menu.Group>
      <ChoiceRow
        slot="theme-menu"
        icon={THEME_OPTIONS[theme].icon}
        label={t("ui.theme.label")}
        current={t(THEME_OPTIONS[theme].label)}
        value={theme}
        options={themes}
        onChange={(next) => {
          if (isTheme(next)) setTheme(next);
        }}
      />
      <ChoiceRow
        slot="language-menu"
        icon={LanguageCircleIcon}
        label={t("ui.language.label")}
        current={locale === "auto" ? t("ui.language.auto") : LOCALE_NAMES[locale]}
        value={locale}
        options={languages}
        onChange={(next) => setLocale(isLocale(next) ? next : "auto")}
      />
    </Menu.Group>
  );
}

/* For a bar with nobody signed in to it (a wiki read by a visitor, LosOS Git
 * before sign-in): one button that opens the same two rows. */
export function DisplayMenu({ className }: { className?: string }) {
  const t = useT();
  return (
    <Menu.Root>
      <Menu.Trigger
        data-testid="display-menu"
        aria-label={t("ui.display.label")}
        title={t("ui.display.label")}
        className={cn(
          "inline-flex size-[34px] shrink-0 items-center justify-center rounded-control text-faint",
          "hover:bg-sunk hover:text-ink data-popup-open:bg-sunk data-popup-open:text-ink",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          className,
        )}
      >
        <HugeiconsIcon icon={PaintBoardIcon} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner side="bottom" align="end" sideOffset={6} className="isolate z-50 outline-none">
          <Menu.Popup data-slot="display-menu" className={cn(popupClass, "w-64")}>
            <PreferenceItems />
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
