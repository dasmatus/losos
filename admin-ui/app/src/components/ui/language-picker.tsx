import { HugeiconsIcon } from "@hugeicons/react";
import { LanguageCircleIcon } from "@hugeicons/core-free-icons";
import { isLocale, LOCALE_NAMES, LOCALES } from "@/lib/i18n";
import { useLocaleChoice, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* The language picker: Browser language (the default), then each language
 * named in itself. A native <select> rather than a custom menu — it is the one
 * control every phone already renders well, and three entries do not need
 * more. Sits beside the theme toggle and matches its height. */
export function LanguagePicker({ className }: { className?: string }) {
  const t = useT();
  const [choice, setChoice] = useLocaleChoice();

  return (
    <label
      className={cn(
        "relative inline-flex h-[34px] items-center gap-1.5 rounded-control border border-line bg-sunk pr-1 pl-2 text-faint",
        "focus-within:ring-2 focus-within:ring-accent/40 hover:text-ink",
        className,
      )}
      title={t("ui.language.label")}
    >
      <HugeiconsIcon
        icon={LanguageCircleIcon}
        size={16}
        strokeWidth={1.5}
        color="currentColor"
        aria-hidden="true"
      />
      <span className="sr-only">{t("ui.language.label")}</span>
      <select
        value={choice}
        onChange={(event) => {
          const next = event.target.value;
          setChoice(isLocale(next) ? next : "auto");
        }}
        className={cn(
          "cursor-pointer appearance-none bg-transparent pr-1 text-[12.5px] text-ink outline-none",
          // On a phone the top bar has no room for a language name: the globe
          // stays, and the select covers it invisibly so a tap still opens it.
          "max-sm:absolute max-sm:inset-0 max-sm:opacity-0",
        )}
      >
        <option value="auto">{t("ui.language.auto")}</option>
        {LOCALES.map((locale) => (
          <option key={locale} value={locale} lang={locale}>
            {LOCALE_NAMES[locale]}
          </option>
        ))}
      </select>
    </label>
  );
}
