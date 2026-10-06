import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  CheckmarkCircle02Icon,
  InformationCircleIcon,
} from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { reportConfirmationsHeight } from "./corner";
import {
  Toast,
  ToastClose,
  ToastDescription,
  ToastProvider,
  ToastTitle,
  ToastViewport,
} from "./toast-primitives";
import { type ConfirmationTone, dismissConfirmation, useConfirmations } from "./use-toast";

/* shadcn's Toaster: renders the confirmation store (use-toast.ts) through
 * the Toast primitives, newest at the top of the top-right corner. One per
 * shell, mounted by the house <Toaster/> in toast.tsx next to the status
 * stack, whose offset follows this list's height (corner.ts). */

const ICONS: Record<ConfirmationTone, typeof CheckmarkCircle02Icon> = {
  done: InformationCircleIcon,
  success: CheckmarkCircle02Icon,
  error: Alert02Icon,
};

export function Confirmations() {
  const items = useConfirmations();
  const t = useT();
  const viewport = React.useRef<HTMLOListElement>(null);

  React.useEffect(() => {
    const list = viewport.current;
    if (list === null) return;
    const observer = new ResizeObserver(() => {
      reportConfirmationsHeight(list.getBoundingClientRect().height);
    });
    observer.observe(list);
    return () => {
      observer.disconnect();
      reportConfirmationsHeight(0);
    };
  }, []);

  return (
    <ToastProvider swipeDirection="right" label={t("ui.confirmations")}>
      {items.map((item) => (
        <Toast
          key={item.id}
          tone={item.tone}
          open={item.open}
          duration={item.duration}
          onOpenChange={(open) => {
            if (!open) dismissConfirmation(item.id);
          }}
          data-toast=""
          data-tone={item.tone}
        >
          <span aria-hidden="true" className="mt-px shrink-0 text-[var(--tone)]">
            <HugeiconsIcon icon={ICONS[item.tone]} size={20} strokeWidth={1.5} color="currentColor" />
          </span>
          <div className="min-w-0 flex-1 py-px">
            <ToastTitle>{item.title}</ToastTitle>
            {item.description !== undefined && item.description.length > 0 ? (
              <ToastDescription>{item.description}</ToastDescription>
            ) : null}
          </div>
          <ToastClose aria-label={t("ui.dismiss")} />
        </Toast>
      ))}
      <ToastViewport ref={viewport} label={t("ui.confirmations")} />
    </ToastProvider>
  );
}
