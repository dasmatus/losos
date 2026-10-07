import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { HugeiconsIcon } from "@hugeicons/react";
import { PanelLeftIcon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";

/* shadcn/ui's Sidebar, on LosOS tokens and the appliance CSP.
 *
 * The parts and their names are shadcn's (SidebarProvider, Sidebar,
 * SidebarHeader, SidebarContent, SidebarMenu, SidebarMenuItem,
 * SidebarMenuButton, SidebarMenuAction, SidebarMenuBadge, SidebarMenuSub,
 * SidebarTrigger, useSidebar) and so is the behaviour that matters: one
 * panel, entries that fold their sub-entries away (with Collapsible), and
 * `collapsible="icon"`, where the whole panel narrows to a rail of icons and
 * Ctrl/Cmd+B toggles it. Four things differ, each for a reason:
 *
 *   - The panel is the recessed box the old settings list was (--sunk, a
 *     line, the card radius) and sits in the page's flow beside the
 *     content, not fixed to the viewport edge: this is a settings page on a
 *     box, not an application frame.
 *   - The width is a class per state, not shadcn's --sidebar-width through
 *     a style prop. That prop would be allowed (a CSSOM write), but there is
 *     nothing to configure, so there is nothing to pass.
 *   - On a phone the panel is not a Sheet (Radix Dialog, whose scroll lock
 *     injects a <style> element the CSP refuses); it stays in the flow above
 *     the content, folded to one row naming where you are, and opens in
 *     place.
 *   - The expanded/collapsed choice is kept in localStorage, not a cookie:
 *     there is no server render to read it. A browser that refuses storage
 *     gets the default every time, which is fine.
 *
 * Links: SidebarMenuButton is a button. A route link takes the same look
 * from `sidebarMenuButtonVariants` (and the sub-entry one from
 * `sidebarMenuSubButtonVariants`), since `asChild` is not supported here
 * (button.tsx). */

type SidebarState = "expanded" | "collapsed";

interface SidebarContextValue {
  state: SidebarState;
  open: boolean;
  setOpen: (open: boolean) => void;
  isMobile: boolean;
  openMobile: boolean;
  setOpenMobile: (open: boolean) => void;
  toggleSidebar: () => void;
}

const SidebarContext = React.createContext<SidebarContextValue | null>(null);

export function useSidebar(): SidebarContextValue {
  const context = React.useContext(SidebarContext);
  if (context === null) throw new Error("useSidebar must be used inside <SidebarProvider>");
  return context;
}

const STORAGE_KEY = "losos-sidebar";
const MOBILE_QUERY = "(max-width: 767px)";

function readStored(fallback: boolean): boolean {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return value === null ? fallback : value === "expanded";
  } catch {
    return fallback;
  }
}

function useIsMobile(): boolean {
  return React.useSyncExternalStore(
    (listener) => {
      const query = window.matchMedia(MOBILE_QUERY);
      query.addEventListener("change", listener);
      return () => query.removeEventListener("change", listener);
    },
    () => window.matchMedia(MOBILE_QUERY).matches,
    () => false,
  );
}

function SidebarProvider({
  defaultOpen = true,
  className,
  children,
  ...props
}: React.ComponentProps<"div"> & { defaultOpen?: boolean }) {
  const isMobile = useIsMobile();
  const [open, setOpenState] = React.useState(() => readStored(defaultOpen));
  const [openMobile, setOpenMobile] = React.useState(false);

  const setOpen = React.useCallback((next: boolean) => {
    setOpenState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next ? "expanded" : "collapsed");
    } catch {
      // Storage refused: the choice lasts as long as the page.
    }
  }, []);

  const toggleSidebar = React.useCallback(() => {
    if (isMobile) setOpenMobile((current) => !current);
    else setOpen(!open);
  }, [isMobile, open, setOpen]);

  // shadcn's shortcut: Ctrl+B, or Cmd+B on a Mac.
  React.useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (event.key.toLowerCase() === "b" && (event.metaKey || event.ctrlKey) && !event.altKey) {
        event.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleSidebar]);

  const value = React.useMemo<SidebarContextValue>(
    () => ({
      state: open ? "expanded" : "collapsed",
      open,
      setOpen,
      isMobile,
      openMobile,
      setOpenMobile,
      toggleSidebar,
    }),
    [open, setOpen, isMobile, openMobile, toggleSidebar],
  );

  return (
    <SidebarContext.Provider value={value}>
      <div data-slot="sidebar-wrapper" className={cn("group/sidebar-wrapper", className)} {...props}>
        {children}
      </div>
    </SidebarContext.Provider>
  );
}

/* The panel. `collapsible="icon"` lets it narrow to a rail; "none" pins it
 * open. Rendered as <nav> because every entry in it is a destination. */
function Sidebar({
  collapsible = "icon",
  className,
  children,
  ...props
}: React.ComponentProps<"nav"> & { collapsible?: "icon" | "none" }) {
  const { state, isMobile, openMobile } = useSidebar();
  const rail = !isMobile && collapsible === "icon" && state === "collapsed";
  return (
    <nav
      data-slot="sidebar"
      data-state={isMobile ? (openMobile ? "expanded" : "collapsed") : state}
      data-collapsible={rail ? "icon" : ""}
      data-mobile={isMobile ? "true" : undefined}
      className={cn(
        "group/sidebar shrink-0 rounded-card border border-line bg-sunk p-2",
        "md:sticky md:top-20 md:self-start md:max-h-[calc(100dvh-6rem)] md:overflow-y-auto",
        "transition-[width] duration-200 ease-out",
        rail ? "md:w-[52px]" : "md:w-[228px]",
        className,
      )}
      {...props}
    >
      {children}
    </nav>
  );
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      className={cn(
        "flex items-center gap-1.5",
        "group-data-[collapsible=icon]/sidebar:flex-col",
        className,
      )}
      {...props}
    />
  );
}

/* Hidden on a phone until the panel is opened, and never hidden on a wider
 * screen (there the rail is the collapsed form). */
function SidebarContent({ className, ...props }: React.ComponentProps<"div">) {
  const { isMobile, openMobile } = useSidebar();
  return (
    <div
      data-slot="sidebar-content"
      hidden={isMobile && !openMobile}
      className={cn("mt-2 flex min-h-0 flex-col gap-2", className)}
      {...props}
    />
  );
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-footer" className={cn("mt-2 flex flex-col gap-2", className)} {...props} />;
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-group" className={cn("flex min-w-0 flex-col", className)} {...props} />;
}

function SidebarGroupLabel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group-label"
      className={cn(
        "px-2 pt-1 pb-1.5 text-[11.5px] font-medium tracking-wide text-faint",
        "group-data-[collapsible=icon]/sidebar:sr-only",
        className,
      )}
      {...props}
    />
  );
}

/* The search box at the top of the panel, on the surface so it reads as a
 * field against the recess. */
function SidebarInput({ className, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      data-slot="sidebar-input"
      className={cn(
        "h-8 w-full min-w-0 rounded-control border border-line bg-surface pr-7 pl-7",
        "text-[13px] text-ink placeholder:text-faint",
        "transition-[border-color,box-shadow] duration-150",
        "focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/35 focus-visible:outline-none",
        "[&::-webkit-search-cancel-button]:hidden",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"ul">) {
  return <ul data-slot="sidebar-menu" className={cn("flex min-w-0 flex-col gap-0.5", className)} {...props} />;
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<"li">) {
  return <li data-slot="sidebar-menu-item" className={cn("group/menu-item relative", className)} {...props} />;
}

/* An entry. Selected is FILLED with the accent and the text on it is
 * --surface, the macOS System Settings look the old list had; a planned
 * entry is greyed and not a target. On the rail only the 21px glyph tile
 * stays, centred. */
const sidebarMenuButtonVariants = cva(
  cn(
    "peer/menu-button flex w-full min-w-0 items-center gap-2 rounded-control px-2 py-1.5 text-left",
    "text-[13px] whitespace-nowrap outline-none",
    "transition-colors duration-150",
    "focus-visible:ring-2 focus-visible:ring-accent/40",
    "[&>span:last-child]:truncate",
    "group-data-[collapsible=icon]/sidebar:size-9 group-data-[collapsible=icon]/sidebar:justify-center group-data-[collapsible=icon]/sidebar:px-0",
    "group-has-data-[slot=sidebar-menu-action]/menu-item:pr-8",
  ),
  {
    variants: {
      isActive: {
        true: "bg-accent font-medium text-surface",
        false: "text-ink hover:bg-surface active:bg-surface",
      },
      planned: {
        true: "cursor-not-allowed text-faint hover:bg-transparent",
        false: "",
      },
    },
    defaultVariants: { isActive: false, planned: false },
  },
);

function SidebarMenuButton({
  isActive = false,
  planned = false,
  className,
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof sidebarMenuButtonVariants>) {
  return (
    <button
      type="button"
      data-slot="sidebar-menu-button"
      data-active={isActive ? "true" : undefined}
      className={cn(sidebarMenuButtonVariants({ isActive, planned }), className)}
      {...props}
    />
  );
}

/* The rounded-square glyph tile at the start of an entry. Its fill follows
 * selection; the glyph is always currentColor. */
function SidebarMenuIcon({
  active = false,
  planned = false,
  className,
  children,
}: {
  active?: boolean;
  planned?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "flex size-[21px] shrink-0 items-center justify-center rounded-[6px] transition-colors duration-150",
        planned ? "bg-surface text-faint" : active ? "bg-accent-wash text-accent" : "bg-surface text-muted",
        className,
      )}
    >
      {children}
    </span>
  );
}

/* The fold toggle on an entry that is also a link (shadcn's
 * SidebarMenuAction): a separate button at the right edge, so the label
 * still navigates. Gone on the rail, where there is nothing to unfold. */
const sidebarMenuActionClassName = cn(
  "absolute top-1 right-1 flex size-6 items-center justify-center rounded-[6px]",
  "text-muted transition-[background-color,color,transform] duration-150 hover:bg-surface hover:text-ink",
  "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
  "peer-data-[active=true]/menu-button:text-surface peer-data-[active=true]/menu-button:hover:bg-accent-wash peer-data-[active=true]/menu-button:hover:text-accent",
  "group-data-[collapsible=icon]/sidebar:hidden",
);

function SidebarMenuAction({ className, ...props }: React.ComponentProps<"button">) {
  return (
    <button
      type="button"
      data-slot="sidebar-menu-action"
      className={cn(sidebarMenuActionClassName, className)}
      {...props}
    />
  );
}

function SidebarMenuBadge({ className, ...props }: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="sidebar-menu-badge"
      className={cn(
        "ml-auto shrink-0 rounded-[6px] bg-surface px-1.5 py-px text-[10.5px] font-medium tracking-wide text-faint",
        "group-data-[collapsible=icon]/sidebar:hidden",
        className,
      )}
      {...props}
    />
  );
}

/* Sub-entries: indented under their parent behind a hairline, shadcn's
 * look. Hidden on the rail. */
function SidebarMenuSub({ className, ...props }: React.ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu-sub"
      className={cn(
        "mt-0.5 ml-[18px] flex min-w-0 flex-col gap-0.5 border-l border-line py-0.5 pl-2",
        "group-data-[collapsible=icon]/sidebar:hidden",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenuSubItem({ className, ...props }: React.ComponentProps<"li">) {
  return <li data-slot="sidebar-menu-sub-item" className={cn("group/menu-sub-item relative", className)} {...props} />;
}

const sidebarMenuSubButtonVariants = cva(
  cn(
    "flex w-full min-w-0 items-center gap-2 rounded-control px-2 py-1 text-left",
    "text-[13px] whitespace-nowrap outline-none transition-colors duration-150",
    "focus-visible:ring-2 focus-visible:ring-accent/40",
  ),
  {
    variants: {
      isActive: {
        true: "bg-accent font-medium text-surface",
        false: "text-ink hover:bg-surface active:bg-surface",
      },
      planned: {
        true: "cursor-not-allowed text-faint hover:bg-transparent",
        false: "",
      },
    },
    defaultVariants: { isActive: false, planned: false },
  },
);

function SidebarMenuSubButton({
  isActive = false,
  planned = false,
  className,
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof sidebarMenuSubButtonVariants>) {
  return (
    <button
      type="button"
      data-slot="sidebar-menu-sub-button"
      className={cn(sidebarMenuSubButtonVariants({ isActive, planned }), className)}
      {...props}
    />
  );
}

function SidebarTrigger({ className, label, ...props }: React.ComponentProps<"button"> & { label: string }) {
  const { toggleSidebar, state, isMobile, openMobile } = useSidebar();
  const expanded = isMobile ? openMobile : state === "expanded";
  return (
    <button
      type="button"
      data-slot="sidebar-trigger"
      aria-label={label}
      title={label}
      aria-expanded={expanded}
      onClick={toggleSidebar}
      className={cn(
        "flex size-8 shrink-0 items-center justify-center rounded-control text-muted",
        "transition-colors duration-150 hover:bg-surface hover:text-ink",
        "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
        className,
      )}
      {...props}
    >
      <HugeiconsIcon icon={PanelLeftIcon} size={17} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
    </button>
  );
}

export {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarInput,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarMenuIcon,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuSub,
  SidebarMenuSubItem,
  SidebarMenuSubButton,
  SidebarTrigger,
  sidebarMenuActionClassName,
  sidebarMenuButtonVariants,
  sidebarMenuSubButtonVariants,
};
