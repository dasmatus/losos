import * as React from "react";
import { ContextMenu } from "@base-ui/react/context-menu";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  BookOpen01Icon,
  ComputerIcon,
  Copy01Icon,
  LinkSquare02Icon,
  Moon02Icon,
  RefreshIcon,
  Search01Icon,
  Sun03Icon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { useTheme } from "@/components/ui/theme-toggle";
import { popupMotion } from "@/components/ui/motion";
import { toast } from "@/components/ui/toast";
import {
  ariaKeys,
  buildersAt,
  copyText,
  keyCaps,
  onMac,
  pickedText,
  takesText,
  visibleSections,
  type MenuBuilder,
  type MenuCommand,
  type MenuHit,
  type MenuItem,
  type MenuSection,
} from "@/lib/context-menu";
import { HANDBOOK_BASE } from "@/lib/handbook";
import { useT } from "@/lib/i18n-react";
import { isTheme, THEMES } from "@/lib/theme";
import { cn } from "@/lib/utils";

/* The one right-click menu of a page, in place of the browser's.
 *
 * Mounted once at a page's root (the admin shell, LosOS Cloud), it listens
 * on the document: a right click, a long press on a touch screen, and
 * Shift+F10 or the Menu key on whatever has the focus. It asks the element
 * under the pointer for its menu (lib/context-menu.ts, innermost first) and
 * adds what fits around it: Copy and Search for picked text, Open in new tab
 * and Copy link on a link, and the page's own items where nothing closer
 * has a menu. A place that takes text keeps the browser's menu, so spelling
 * and paste still work there, and so does anything with no items at all.
 *
 * Base UI's Context Menu, like the sidebar and the person menu: its
 * positioner and scroll lock write CSSOM only, which the admin CSP allows,
 * and it brings the menu roles, arrow keys, typeahead, submenus and the
 * flip at the window's edge. It is driven here rather than by its Trigger,
 * because the Trigger takes every right click in its area, inputs included,
 * and opens at the pointer even from the keyboard. Inside a native
 * <dialog> (the palette) the popup is portalled into that dialog, which
 * sits in the top layer above anything in <body>. */

const LONG_PRESS_MS = 500;
const LONG_PRESS_SLOP = 10;

interface Shown {
  label: string;
  sections: MenuSection[];
  anchor: { getBoundingClientRect: () => DOMRect };
  container: HTMLElement | undefined;
  keyboard: boolean;
  returnFocus: HTMLElement | null;
}

const ContainerContext = React.createContext<HTMLElement | undefined>(undefined);
const RunContext = React.createContext<(item: MenuCommand) => void>(() => {});

export function ContextMenuHost({
  page,
  search,
}: {
  /** The page's own items, where nothing closer has a menu. */
  page?: MenuBuilder;
  /** Search the page's app for picked text; absent where there is no search. */
  search?: (text: string) => void;
}) {
  const t = useT();
  const [shown, setShown] = React.useState<Shown | null>(null);
  const [open, setOpen] = React.useState(false);
  const pending = React.useRef<MenuCommand | null>(null);
  const popup = React.useRef<HTMLDivElement>(null);
  const latest = React.useRef({ t, page, search });
  latest.current = { t, page, search };

  const close = React.useCallback(() => setOpen(false), []);

  React.useEffect(() => {
    let menuKeyAt = -Infinity;
    let touch: { x: number; y: number; target: Element; timer: number } | null = null;
    let pressedAt = -Infinity;
    // The finger that opened the menu is still down, however long it stays.
    let pressing = false;

    /** The sections for a click on `target`, or null to leave the browser's menu. */
    const resolve = (hit: MenuHit): { label: string; sections: MenuSection[] } | null => {
      const { t, page, search } = latest.current;
      if (takesText(hit.target)) return null;

      let primary: { label: string; sections: MenuSection[] } | null = null;
      for (const build of buildersAt(hit.target)) {
        const spec = build(hit);
        const sections = spec === null ? [] : visibleSections(spec.sections);
        if (spec !== null && sections.length > 0) {
          primary = { label: spec.label, sections };
          break;
        }
      }

      const text = pickedText(hit.target);
      const picked: MenuSection =
        text === ""
          ? []
          : [
              {
                id: "copy-text",
                label: t("menu.copy"),
                icon: Copy01Icon,
                keys: ["Mod", "C"],
                onSelect: () => void copied(text),
              },
              {
                id: "search-text",
                label: t("menu.searchFor", { text: text.length > 24 ? `${text.slice(0, 23)}…` : text }),
                icon: Search01Icon,
                hidden: search === undefined,
                onSelect: () => search?.(text),
              },
            ];

      let sections: MenuSection[];
      let label: string;
      if (primary !== null) {
        sections = [picked, ...primary.sections];
        label = primary.label;
      } else {
        const link = hit.target.closest<HTMLAnchorElement>("a[href]");
        const linked: MenuSection =
          link === null
            ? []
            : [
                {
                  id: "link-tab",
                  label: t("menu.newTab"),
                  icon: LinkSquare02Icon,
                  onSelect: () => window.open(link.href, "_blank", "noopener"),
                },
                { id: "link-copy", label: t("menu.copyLink"), icon: Copy01Icon, onSelect: () => void copied(link.href) },
              ];
        const own = page?.(hit);
        sections = [picked, linked, ...(own?.sections ?? [])];
        label = text !== "" ? t("menu.text") : link !== null ? link.textContent?.trim() || link.href : t("menu.page");
      }
      const visible = visibleSections(sections);
      return visible.length === 0 ? null : { label, sections: visible };
    };

    const copied = async (text: string): Promise<void> => {
      const { t } = latest.current;
      if (await copyText(text)) toast.done(t("menu.copied"));
      else toast.error(t("menu.copyFailed"), undefined, { help: "index" });
    };

    const show = (target: Element, x: number, y: number, keyboard: boolean): boolean => {
      const found = resolve({ target, keyboard });
      if (found === null) return false;
      const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      // From the keyboard the menu hangs under what has the focus, not at
      // wherever the pointer was left.
      const at = keyboard ? (focused ?? target).getBoundingClientRect() : null;
      const px = at !== null ? at.left + Math.min(16, at.width / 2) : x;
      const py = at !== null ? at.bottom : y;
      setShown({
        ...found,
        anchor: { getBoundingClientRect: () => DOMRect.fromRect({ x: px, y: py, width: 0, height: 0 }) },
        container: target.closest("dialog[open]") instanceof HTMLDialogElement ? (target.closest("dialog[open]") as HTMLElement) : undefined,
        keyboard,
        returnFocus: focused,
      });
      setOpen(true);
      return true;
    };

    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) menuKeyAt = performance.now();
    };

    const onContextMenu = (event: MouseEvent): void => {
      if (event.defaultPrevented || !(event.target instanceof Element)) return;
      // A long press already opened it; Android sends its own event after.
      if (pressing || performance.now() - pressedAt < 1000) {
        event.preventDefault();
        return;
      }
      cancelPress();
      if (popup.current?.contains(event.target)) {
        event.preventDefault();
        return;
      }
      const keyboard = performance.now() - menuKeyAt < 600 || (event.clientX === 0 && event.clientY === 0);
      const target = keyboard && document.activeElement instanceof Element && document.activeElement !== document.body
        ? document.activeElement
        : event.target;
      if (show(target, event.clientX, event.clientY, keyboard)) event.preventDefault();
    };

    const cancelPress = (): void => {
      if (touch !== null) window.clearTimeout(touch.timer);
      touch = null;
    };

    const onTouchStart = (event: TouchEvent): void => {
      cancelPress();
      pressing = false;
      const first = event.touches[0];
      if (event.touches.length !== 1 || first === undefined || !(event.target instanceof Element)) return;
      if (takesText(event.target)) return;
      const target = event.target;
      const x = first.clientX;
      const y = first.clientY;
      touch = {
        x,
        y,
        target,
        timer: window.setTimeout(() => {
          touch = null;
          if (show(target, x, y, false)) {
            pressedAt = performance.now();
            pressing = true;
            navigator.vibrate?.(10);
          }
        }, LONG_PRESS_MS),
      };
    };

    const onTouchMove = (event: TouchEvent): void => {
      const first = event.touches[0];
      if (touch === null || first === undefined) return;
      if (Math.abs(first.clientX - touch.x) > LONG_PRESS_SLOP || Math.abs(first.clientY - touch.y) > LONG_PRESS_SLOP) cancelPress();
    };

    const onTouchEnd = (event: TouchEvent): void => {
      cancelPress();
      // The finger that opened the menu lifts: that is not a tap on the row.
      if (pressing) event.preventDefault();
      pressing = false;
    };

    const onTouchCancel = (): void => {
      cancelPress();
      pressing = false;
    };

    // The menu hangs at a point on the page; once the page moves, it goes.
    const onScroll = (event: Event): void => {
      if (event.target instanceof Node && popup.current?.contains(event.target)) return;
      setOpen(false);
    };

    document.addEventListener("keydown", onKeyDown, true);
    document.addEventListener("contextmenu", onContextMenu);
    document.addEventListener("touchstart", onTouchStart, { passive: true });
    document.addEventListener("touchmove", onTouchMove, { passive: true });
    document.addEventListener("touchend", onTouchEnd, { passive: false });
    document.addEventListener("touchcancel", onTouchCancel);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onScroll);
    return () => {
      cancelPress();
      document.removeEventListener("keydown", onKeyDown, true);
      document.removeEventListener("contextmenu", onContextMenu);
      document.removeEventListener("touchstart", onTouchStart);
      document.removeEventListener("touchmove", onTouchMove);
      document.removeEventListener("touchend", onTouchEnd);
      document.removeEventListener("touchcancel", onTouchCancel);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onScroll);
    };
  }, []);

  // From the keyboard the first item has the focus, as in a native menu.
  React.useEffect(() => {
    if (!open || shown?.keyboard !== true) return;
    const frame = window.requestAnimationFrame(() => {
      popup.current?.querySelector<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"])')?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, shown]);

  // An item runs once the menu has gone and the focus is back where it was,
  // so a dialog it opens keeps the focus it takes.
  const run = React.useCallback((item: MenuCommand) => {
    pending.current = item;
  }, []);

  // The root stays mounted from the start: a root that mounts already open
  // skips the popup's starting style, and the first menu on a page would
  // appear in one frame instead of growing from the pointer.
  return (
    <ContextMenu.Root
      open={open && shown !== null}
      onOpenChange={(next) => {
        if (!next) close();
      }}
      onOpenChangeComplete={(next) => {
        if (next) return;
        const item = pending.current;
        pending.current = null;
        item?.onSelect();
      }}
    >
      {shown !== null && (
        <ContainerContext.Provider value={shown.container}>
          <RunContext.Provider value={run}>
            <ContextMenu.Portal container={shown.container}>
              <ContextMenu.Positioner
                anchor={shown.anchor}
                // At the pointer the menu overlaps it a little, as a native
                // one does; under a focused row it hangs just below.
                sideOffset={shown.keyboard ? 4 : undefined}
                collisionPadding={8}
                className="isolate z-50 outline-none"
              >
                <ContextMenu.Popup
                  ref={popup}
                  data-slot="context-menu"
                  aria-label={shown.label}
                  finalFocus={{ current: shown.returnFocus }}
                  className={POPUP}
                >
                  <Sections sections={shown.sections} />
                </ContextMenu.Popup>
              </ContextMenu.Positioner>
            </ContextMenu.Portal>
          </RunContext.Provider>
        </ContainerContext.Provider>
      )}
    </ContextMenu.Root>
  );
}

const POPUP = cn(
  "max-h-(--available-height) min-w-52 overflow-y-auto rounded-card border border-line bg-surface p-1 text-ink shadow-md outline-none",
  popupMotion,
);

const ITEM = cn(
  "flex min-h-8 cursor-default items-center gap-2 rounded-control px-2 py-1.5 text-[13px] outline-none select-none",
  "data-highlighted:bg-accent-wash data-highlighted:text-ink data-popup-open:bg-accent-wash",
  "data-disabled:cursor-not-allowed data-disabled:text-faint",
);

function Sections({ sections }: { sections: readonly MenuSection[] }) {
  return sections.map((section, i) => (
    <React.Fragment key={i}>
      {i > 0 && <ContextMenu.Separator className="mx-1 my-1 h-px bg-hair" />}
      <ContextMenu.Group>
        {section.map((item) => (
          <Item key={item.id} item={item} />
        ))}
      </ContextMenu.Group>
    </React.Fragment>
  ));
}

function Icon({ item }: { item: MenuItem }) {
  return item.icon !== undefined ? (
    <HugeiconsIcon icon={item.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
  ) : (
    <span aria-hidden="true" className="size-[15px] shrink-0" />
  );
}

function Item({ item }: { item: MenuItem }) {
  const container = React.useContext(ContainerContext);
  const run = React.useContext(RunContext);

  if (item.kind === "submenu" || item.kind === "choice") {
    return (
      <ContextMenu.SubmenuRoot>
        <ContextMenu.SubmenuTrigger className={ITEM} label={item.label} data-item={item.id}>
          <Icon item={item} />
          <span className="min-w-0 flex-1 truncate">{item.label}</span>
          <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="text-faint rtl:rotate-180" />
        </ContextMenu.SubmenuTrigger>
        <ContextMenu.Portal container={container}>
          <ContextMenu.Positioner alignOffset={-5} sideOffset={2} collisionPadding={8} className="isolate z-50 outline-none">
            <ContextMenu.Popup data-slot="context-menu" aria-label={item.label} className={POPUP}>
              {item.kind === "submenu" ? (
                <Sections sections={[item.items]} />
              ) : (
                <ContextMenu.RadioGroup value={item.value} onValueChange={(value: string) => item.onChange(value)}>
                  {item.options.map((option) => (
                    <ContextMenu.RadioItem key={option.value} value={option.value} closeOnClick className={ITEM} label={option.label}>
                      {option.icon !== undefined ? (
                        <HugeiconsIcon icon={option.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
                      ) : (
                        <span aria-hidden="true" className="size-[15px] shrink-0" />
                      )}
                      <span className="min-w-0 flex-1 truncate">{option.label}</span>
                      <ContextMenu.RadioItemIndicator className="text-accent">
                        <HugeiconsIcon icon={Tick02Icon} size={14} strokeWidth={2} color="currentColor" aria-hidden="true" />
                      </ContextMenu.RadioItemIndicator>
                    </ContextMenu.RadioItem>
                  ))}
                </ContextMenu.RadioGroup>
              )}
            </ContextMenu.Popup>
          </ContextMenu.Positioner>
        </ContextMenu.Portal>
      </ContextMenu.SubmenuRoot>
    );
  }

  return (
    <ContextMenu.Item
      className={cn(ITEM, item.danger === true && "text-crit data-highlighted:bg-crit/10 data-highlighted:text-crit")}
      disabled={item.disabled}
      title={item.title}
      label={item.label}
      data-item={item.id}
      aria-keyshortcuts={item.keys !== undefined ? ariaKeys(item.keys) : undefined}
      onClick={() => run(item)}
    >
      <Icon item={item} />
      <span className="min-w-0 flex-1 truncate">{item.label}</span>
      {item.keys !== undefined && <Keys keys={item.keys} />}
    </ContextMenu.Item>
  );
}

function Keys({ keys }: { keys: readonly string[] }) {
  return (
    <span aria-hidden="true" className="ml-4 inline-flex shrink-0 items-center gap-0.5">
      {keyCaps(keys).map((cap, i) => (
        <kbd
          key={i}
          className="inline-flex min-w-5 items-center justify-center rounded-[4px] border border-line bg-sunk px-1 font-sans text-[11px] leading-[18px] text-muted"
        >
          {cap}
        </kbd>
      ))}
    </span>
  );
}

/** What every page offers where nothing closer has a menu: the browser's
 *  own moves, which the custom menu replaces, and the theme. `extra` is the
 *  page's own section, ahead of them. */
export function usePageMenu(extra: () => MenuSection = () => []): MenuBuilder {
  const t = useT();
  const { theme, setTheme: choose } = useTheme();
  const mac = onMac();
  return () => ({
    label: t("menu.page"),
    sections: [
      extra(),
      [
        { id: "back", label: t("menu.back"), icon: ArrowLeft01Icon, keys: mac ? ["Mod", "["] : ["Alt", "Left"], onSelect: () => history.back() },
        { id: "forward", label: t("menu.forward"), icon: ArrowRight01Icon, keys: mac ? ["Mod", "]"] : ["Alt", "Right"], onSelect: () => history.forward() },
        { id: "reload", label: t("menu.reload"), icon: RefreshIcon, keys: ["Mod", "R"], onSelect: () => location.reload() },
      ],
      [
        {
          kind: "choice",
          id: "theme",
          label: t("ui.theme.label"),
          icon: THEME_ICONS[theme],
          value: theme,
          options: THEMES.map((value) => ({ value, label: t(`ui.theme.${value}`), icon: THEME_ICONS[value] })),
          onChange: (value) => {
            if (isTheme(value)) choose(value);
          },
        },
        { id: "handbook", label: t("menu.handbook"), icon: BookOpen01Icon, onSelect: () => window.open(HANDBOOK_BASE, "_blank", "noopener") },
      ],
    ],
  });
}

const THEME_ICONS = { auto: ComputerIcon, light: Sun03Icon, dark: Moon02Icon } as const;
