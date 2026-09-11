import * as React from "react";
import { cn } from "@/lib/utils";

interface TabsContextValue {
  value: string;
  setValue: (value: string) => void;
  baseId: string;
}

const TabsContext = React.createContext<TabsContextValue | null>(null);

function useTabs(component: string): TabsContextValue {
  const context = React.useContext(TabsContext);
  if (context === null) throw new Error(`<${component}> must be inside <Tabs>`);
  return context;
}

export interface TabsProps extends Omit<React.ComponentPropsWithoutRef<"div">, "onChange"> {
  /** Controlled value. Omit for an uncontrolled tab set. */
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
}

/** Tabs with the WAI-ARIA keyboard behaviour: roving tabindex, arrows to move,
 *  Home/End to jump, and activation following focus. */
export function Tabs({
  value,
  defaultValue = "",
  onValueChange,
  className,
  children,
  ...props
}: TabsProps) {
  const baseId = React.useId();
  const [internal, setInternal] = React.useState(defaultValue);
  const current = value ?? internal;

  const setValue = React.useCallback(
    (next: string) => {
      if (value === undefined) setInternal(next);
      onValueChange?.(next);
    },
    [value, onValueChange],
  );

  const context = React.useMemo<TabsContextValue>(
    () => ({ value: current, setValue, baseId }),
    [current, setValue, baseId],
  );

  return (
    <TabsContext.Provider value={context}>
      <div className={cn("flex flex-col gap-4", className)} {...props}>
        {children}
      </div>
    </TabsContext.Provider>
  );
}

export function TabsList({ className, ...props }: React.ComponentPropsWithoutRef<"div">) {
  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const tabs = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]:not([disabled])'),
    );
    if (tabs.length === 0) return;
    const index = tabs.findIndex((tab) => tab === document.activeElement);
    let next = index;
    if (event.key === "ArrowLeft") next = index <= 0 ? tabs.length - 1 : index - 1;
    else if (event.key === "ArrowRight") next = index === tabs.length - 1 ? 0 : index + 1;
    else if (event.key === "Home") next = 0;
    else next = tabs.length - 1;
    event.preventDefault();
    tabs[next]?.focus();
    tabs[next]?.click();
  };

  return (
    <div
      role="tablist"
      onKeyDown={onKeyDown}
      className={cn(
        "inline-flex items-center gap-1 rounded-control border border-line bg-sunk p-1",
        className,
      )}
      {...props}
    />
  );
}

export interface TabsTriggerProps extends React.ComponentPropsWithoutRef<"button"> {
  value: string;
}

export function TabsTrigger({ value, className, onClick, ...props }: TabsTriggerProps) {
  const tabs = useTabs("TabsTrigger");
  const selected = tabs.value === value;

  return (
    <button
      type="button"
      role="tab"
      id={`${tabs.baseId}-tab-${value}`}
      aria-controls={`${tabs.baseId}-panel-${value}`}
      aria-selected={selected}
      tabIndex={selected ? 0 : -1}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) tabs.setValue(value);
      }}
      className={cn(
        "rounded-[4px] px-3 py-1.5 text-[13px] font-medium",
        "transition-colors duration-150",
        selected ? "bg-surface text-ink shadow-card" : "text-muted hover:text-ink",
        className,
      )}
      {...props}
    />
  );
}

export interface TabsContentProps extends React.ComponentPropsWithoutRef<"div"> {
  value: string;
}

export function TabsContent({ value, className, ...props }: TabsContentProps) {
  const tabs = useTabs("TabsContent");
  if (tabs.value !== value) return null;
  return (
    <div
      role="tabpanel"
      id={`${tabs.baseId}-panel-${value}`}
      aria-labelledby={`${tabs.baseId}-tab-${value}`}
      tabIndex={0}
      className={cn("animate-fade-in focus-visible:outline-none", className)}
      {...props}
    />
  );
}
