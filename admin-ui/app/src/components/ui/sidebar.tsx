import * as React from "react";
import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import { cva, type VariantProps } from "class-variance-authority";
import { HugeiconsIcon } from "@hugeicons/react";
import { PanelLeftIcon } from "@hugeicons/core-free-icons";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/* shadcn/ui's Sidebar from its base registry (Base UI), on the LosOS tokens
 * and the appliance CSP.
 *
 * The parts, their names and their behaviour are shadcn's: SidebarProvider
 * (state, Ctrl/Cmd+B, the two widths as --sidebar-width and
 * --sidebar-width-icon), Sidebar with `collapsible="icon"` (the panel
 * narrows to a rail of icons and each entry's name moves into a Tooltip),
 * SidebarMenuButton/SidebarMenuSubButton/SidebarMenuAction with Base UI's
 * `render` prop (so a route <Link> takes the entry's look and behaviour),
 * Collapsible sub-menus, and a Sheet sliding in from the left on a phone.
 * All of it is Base UI, which positions, measures and locks scroll through
 * CSSOM writes the admin CSP allows and never injects a <style> element,
 * which it refuses; the widths reach the page the same way, as a React
 * style prop.
 *
 * What is LosOS rather than shadcn, each for a reason:
 *   - The panel is the recessed box (--sunk, a hairline, the card radius)
 *     and sits in the page's flow beside the content, sticky under the
 *     header, not fixed to the viewport edge: shadcn's "floating" variant
 *     in spirit, because this is a settings page on a box, not an
 *     application frame.
 *   - A selected entry is FILLED with the accent and its text is --surface,
 *     the macOS System Settings look, and every entry starts with a small
 *     glyph tile (SidebarMenuIcon).
 *   - An entry can be `planned`: greyed, disabled, not a target.
 *   - The expanded/collapsed choice is kept in localStorage, not a cookie:
 *     there is no server render to read it. A browser that refuses storage
 *     gets the default every time, which is fine. */

const SIDEBAR_WIDTH = "15rem";
const SIDEBAR_WIDTH_ICON = "3.25rem";
const SIDEBAR_KEYBOARD_SHORTCUT = "b";
const STORAGE_KEY = "losos-sidebar";
const MOBILE_QUERY = "(max-width: 767px)";

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
  style,
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
      if (
        event.key.toLowerCase() === SIDEBAR_KEYBOARD_SHORTCUT &&
        (event.metaKey || event.ctrlKey) &&
        !event.altKey
      ) {
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
      <TooltipProvider>
        <div
          data-slot="sidebar-wrapper"
          // A CSSOM write (React sets each property on element.style), which
          // style-src 'self' allows; a style="" attribute would be refused.
          style={
            {
              "--sidebar-width": SIDEBAR_WIDTH,
              "--sidebar-width-icon": SIDEBAR_WIDTH_ICON,
              ...style,
            } as React.CSSProperties
          }
          className={cn("group/sidebar-wrapper", className)}
          {...props}
        >
          {children}
        </div>
      </TooltipProvider>
    </SidebarContext.Provider>
  );
}

/* The panel. `collapsible="icon"` lets it narrow to the rail; "none" pins it
 * open. Rendered as <nav> because every entry in it is a destination. On a
 * phone the same children go into a Sheet from the left, opened by a
 * SidebarTrigger placed outside it; `mobileTitle` names that sheet. */
function Sidebar({
  collapsible = "icon",
  mobileTitle,
  mobileDescription,
  closeLabel,
  className,
  children,
  ...props
}: React.ComponentProps<"nav"> & {
  collapsible?: "icon" | "none";
  mobileTitle: string;
  mobileDescription?: string;
  closeLabel: string;
}) {
  const { state, isMobile, openMobile, setOpenMobile } = useSidebar();

  if (isMobile) {
    return (
      <Sheet open={openMobile} onOpenChange={setOpenMobile}>
        <SheetContent side="left" closeLabel={closeLabel} data-sidebar="sidebar" data-mobile="true">
          <SheetHeader className="sr-only">
            <SheetTitle>{mobileTitle}</SheetTitle>
            {mobileDescription !== undefined && <SheetDescription>{mobileDescription}</SheetDescription>}
          </SheetHeader>
          <nav
            data-slot="sidebar"
            data-state="expanded"
            data-collapsible=""
            className={cn("group/sidebar flex min-h-0 flex-1 flex-col overflow-y-auto", className)}
            {...props}
          >
            {children}
          </nav>
        </SheetContent>
      </Sheet>
    );
  }

  const rail = collapsible === "icon" && state === "collapsed";
  return (
    <nav
      data-slot="sidebar"
      data-state={state}
      data-collapsible={rail ? "icon" : ""}
      className={cn(
        "group/sidebar flex shrink-0 flex-col rounded-card border border-line bg-sunk p-2",
        "sticky top-20 max-h-[calc(100dvh-6rem)] self-start overflow-x-hidden overflow-y-auto",
        "w-(--sidebar-width) transition-[width] duration-200 ease-out",
        "data-[collapsible=icon]:w-(--sidebar-width-icon)",
        className,
      )}
      {...props}
    >
      {children}
    </nav>
  );
}

function SidebarTrigger({ className, label, onClick, ...props }: React.ComponentProps<"button"> & { label: string }) {
  const { toggleSidebar, state, isMobile, openMobile } = useSidebar();
  const expanded = isMobile ? openMobile : state === "expanded";
  return (
    <button
      type="button"
      data-slot="sidebar-trigger"
      aria-label={label}
      title={label}
      aria-expanded={expanded}
      onClick={(event) => {
        onClick?.(event);
        toggleSidebar();
      }}
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

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      className={cn("flex items-center gap-1.5 group-data-[collapsible=icon]/sidebar:flex-col", className)}
      {...props}
    />
  );
}

function SidebarContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-content" className={cn("mt-2 flex min-h-0 flex-col gap-2", className)} {...props} />;
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

const sidebarMenuButtonVariants = cva(
  cn(
    "peer/menu-button flex w-full min-w-0 items-center gap-2 overflow-hidden rounded-control px-2 py-1.5 text-left",
    "text-[13px] whitespace-nowrap outline-none",
    "transition-[background-color,color,width,padding] duration-150",
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

/* An entry. A button by default; `render={<Link to=… />}` makes it a route
 * link with the same look. `tooltip` is the name shown beside the icon on
 * the rail; it never shows while the panel is expanded or on a phone. */
function SidebarMenuButton({
  render,
  isActive = false,
  planned = false,
  tooltip,
  className,
  ...props
}: useRender.ComponentProps<"button"> &
  VariantProps<typeof sidebarMenuButtonVariants> & {
    tooltip?: React.ReactNode;
  }) {
  const { isMobile, state } = useSidebar();
  const element = useRender({
    defaultTagName: "button",
    render,
    props: mergeProps<"button">(
      {
        type: render === undefined ? "button" : undefined,
        className: cn(sidebarMenuButtonVariants({ isActive, planned }), className),
      },
      {
        ...props,
        // Data attributes after the caller's props, so they always reflect
        // the variants the entry was drawn with.
        ...({ "data-slot": "sidebar-menu-button", "data-active": isActive ? "true" : undefined } as object),
      },
    ),
  });

  if (tooltip === undefined || tooltip === null) return element;
  return (
    <Tooltip disabled={state !== "collapsed" || isMobile}>
      <TooltipTrigger render={element} />
      <TooltipContent side="right" align="center" sideOffset={10}>
        {tooltip}
      </TooltipContent>
    </Tooltip>
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

/* A second button on an entry (shadcn's SidebarMenuAction), at its right
 * edge, so the entry's label still navigates; used as the fold toggle of an
 * entry that is also a link. Gone on the rail. */
function SidebarMenuAction({ render, className, ...props }: useRender.ComponentProps<"button">) {
  return useRender({
    defaultTagName: "button",
    render,
    props: mergeProps<"button">(
      {
        type: render === undefined ? "button" : undefined,
        className: cn(
          "absolute top-1 right-1 flex size-6 items-center justify-center rounded-[6px]",
          "text-muted transition-[background-color,color] duration-150 hover:bg-surface hover:text-ink",
          "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
          "peer-data-[active=true]/menu-button:text-surface peer-data-[active=true]/menu-button:hover:bg-accent-wash peer-data-[active=true]/menu-button:hover:text-accent",
          "group-data-[collapsible=icon]/sidebar:hidden",
          className,
        ),
      },
      { ...props, ...({ "data-slot": "sidebar-menu-action" } as object) },
    ),
  });
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
  return (
    <li data-slot="sidebar-menu-sub-item" className={cn("group/menu-sub-item relative", className)} {...props} />
  );
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
  render,
  isActive = false,
  planned = false,
  className,
  ...props
}: useRender.ComponentProps<"button"> & VariantProps<typeof sidebarMenuSubButtonVariants>) {
  return useRender({
    defaultTagName: "button",
    render,
    props: mergeProps<"button">(
      {
        type: render === undefined ? "button" : undefined,
        className: cn(sidebarMenuSubButtonVariants({ isActive, planned }), className),
      },
      {
        ...props,
        ...({ "data-slot": "sidebar-menu-sub-button", "data-active": isActive ? "true" : undefined } as object),
      },
    ),
  });
}

export {
  SidebarProvider,
  Sidebar,
  SidebarTrigger,
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
  sidebarMenuButtonVariants,
  sidebarMenuSubButtonVariants,
};
