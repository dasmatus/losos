import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Cancel01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { paneMatches, SETTINGS_PANES, type SettingsPaneId } from "./panes";

/* The macOS System Settings sidebar: a search field, then nav rows, each
 * carrying a small rounded-square glyph tile.
 *
 * Three details are the whole look and none of them are decoration:
 *
 *   - The sidebar sits on --sunk while the content sits on --ground. The
 *     recess is what says "this list navigates, it does not contain".
 *   - The glyph tile is 21px and rounded-6, which is a control radius on a
 *     square small enough to read as an icon rather than a button. Its fill
 *     changes with selection; the glyph is always currentColor.
 *   - A selected row is FILLED with the accent. Not tinted, not underlined —
 *     filled, with the text on it in --surface. (The palette's "white": in
 *     the dark theme the accent is a light teal and --surface is the dark
 *     card colour, which is the pair that actually reads. Hard-coding white
 *     would be legible in one theme out of three.)
 *
 * The search filters the rows. It does not search inside panes: this box has
 * eleven settings, the pane summaries and keywords cover all of them, and a
 * full-text index over eleven switches would be a lie about how much is here.
 *
 * A planned pane (`pane.planned`, today the market) keeps its row so an owner
 * sees what is coming, but the row is a disabled button: greyed, carrying a
 * "soon(TM)" badge, skipped by Tab and announced as unavailable. It is not a
 * link to nowhere and not a tooltip trick — `disabled` is the one attribute
 * that gets all three for free.
 */

export interface SidebarProps {
  current: SettingsPaneId;
  onSelect: (pane: SettingsPaneId) => void;
}

export function Sidebar({ current, onSelect }: SidebarProps) {
  const t = useT();
  const locale = useLocale();
  const [search, setSearch] = React.useState("");
  const searchId = React.useId();

  // The locale is a dep: the match reads the labels in the language on screen.
  const visible = React.useMemo(
    () => SETTINGS_PANES.filter((pane) => paneMatches(pane, search)),
    [search, locale],
  );

  /* Enter in the search field goes to the first match that can be opened.
   * With one match left that is the whole interaction: type three letters,
   * press Enter. A planned pane is never that match, however well it fits. */
  const submit = (event: React.FormEvent): void => {
    event.preventDefault();
    const first = visible.find((pane) => !pane.planned);
    if (first !== undefined) onSelect(first.id);
  };

  return (
    <nav
      aria-label={t("settings.sidebar.nav")}
      className={cn(
        "shrink-0 rounded-card border border-line bg-sunk p-2",
        "md:w-[200px] md:self-start",
      )}
    >
      <form onSubmit={submit} role="search" className="relative mb-2">
        <label htmlFor={searchId} className="sr-only">
          {t("settings.sidebar.searchLabel")}
        </label>
        <HugeiconsIcon
          icon={Search01Icon}
          size={15}
          strokeWidth={1.5}
          color="currentColor"
          className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-faint"
          aria-hidden="true"
        />
        <input
          id={searchId}
          type="search"
          value={search}
          placeholder={t("settings.sidebar.searchPlaceholder")}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setSearch(event.target.value)}
          className={cn(
            "h-8 w-full rounded-control border border-line bg-surface pr-7 pl-7",
            "text-[13px] text-ink placeholder:text-faint",
            "transition-[border-color,box-shadow] duration-150",
            "focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/35",
            "focus-visible:outline-none",
            // Safari draws its own clear button on type=search; ours is the
            // one that matches the palette and is reachable by keyboard.
            "[&::-webkit-search-cancel-button]:hidden",
          )}
        />
        {search.length > 0 && (
          <button
            type="button"
            aria-label={t("settings.sidebar.clear")}
            onClick={() => setSearch("")}
            className={cn(
              "absolute top-1/2 right-1 -translate-y-1/2 rounded-control p-1",
              "text-faint transition-colors duration-150 hover:text-ink",
            )}
          >
            <HugeiconsIcon
              icon={Cancel01Icon}
              size={13}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </button>
        )}
      </form>

      {visible.length === 0 ? (
        <p className="animate-fade-in px-2 py-3 text-[12.5px] leading-snug text-muted">
          {t("settings.sidebar.noMatch", { query: search.trim() })}
        </p>
      ) : (
        <ul
          className={cn(
            "flex gap-0.5",
            // A phone gets the same rows in a scrolling strip rather than a
            // drawer: seven destinations, and a drawer would hide all of them
            // behind a tap on the screen this box is most often set up from.
            "max-md:w-max max-md:flex-row max-md:overflow-x-auto",
            "md:flex-col",
          )}
        >
          {visible.map((pane) => (
            <li key={pane.id}>
              <SidebarRow
                label={pane.label}
                icon={pane.icon}
                selected={pane.id === current}
                planned={pane.planned}
                onClick={() => onSelect(pane.id)}
              />
            </li>
          ))}
        </ul>
      )}
    </nav>
  );
}

interface SidebarRowProps {
  label: string;
  icon: IconSvgElement;
  selected: boolean;
  /** Drawn but not open: see the note at the top of the file. */
  planned: boolean;
  onClick: () => void;
}

function SidebarRow({ label, icon, selected, planned, onClick }: SidebarRowProps) {
  const t = useT();
  return (
    <button
      type="button"
      // aria-current, not aria-selected: these are navigation links to panes,
      // not options in a listbox, and a screen reader should say "current
      // page" rather than "selected".
      aria-current={selected ? "page" : undefined}
      // Both: `disabled` takes the row out of the Tab order and off the click
      // path, `aria-disabled` is what assistive tech reads back. A planned
      // row is never the selected one — its id is not an address — so the
      // selected styling below cannot meet this branch.
      disabled={planned}
      aria-disabled={planned ? "true" : undefined}
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left",
        "text-[13px] whitespace-nowrap",
        "transition-colors duration-150",
        planned
          ? "cursor-not-allowed text-faint"
          : selected
            ? "bg-accent font-medium text-surface"
            : "text-ink hover:bg-surface active:bg-surface",
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "flex size-[21px] shrink-0 items-center justify-center rounded-[6px]",
          "transition-colors duration-150",
          planned
            ? "bg-surface text-faint"
            : selected
              ? "bg-accent-wash text-accent"
              : "bg-surface text-muted",
        )}
      >
        <HugeiconsIcon icon={icon} size={13} strokeWidth={1.5} color="currentColor" />
      </span>
      {label}
      {planned && (
        // Part of the button's text, so the accessible name is "Market
        // soon(TM)" and a screen reader hears the reason with the row.
        <span className="ml-auto rounded-[6px] bg-surface px-1.5 py-px text-[10.5px] font-medium tracking-wide text-faint">
          {t("settings.sidebar.soon")}
        </span>
      )}
    </button>
  );
}
