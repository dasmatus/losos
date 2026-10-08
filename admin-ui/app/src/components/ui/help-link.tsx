import { HugeiconsIcon } from "@hugeicons/react";
import { BookOpen01Icon } from "@hugeicons/core-free-icons";
import { handbookHref, type HandbookEntry } from "@/lib/handbook";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* "What to do": the link from an error to its handbook page. Rendered at the
 * foot of an error toast (toaster.tsx) and inside the alerts and field
 * errors that stay on screen. A new tab, so the error and whatever the owner
 * was doing stay put; a relative path, so it is the copy on this box. */
export function HelpLink({
  entry,
  label,
  className,
}: {
  entry: HandbookEntry;
  /** What the link says, when the page explains rather than fixes. */
  label?: string;
  className?: string;
}) {
  const t = useT();
  return (
    <a
      href={handbookHref(entry)}
      target="_blank"
      rel="noopener"
      data-help={entry}
      className={cn(
        "inline-flex items-center gap-1 text-[12.5px] font-medium text-accent underline-offset-2 hover:underline",
        className,
      )}
    >
      <HugeiconsIcon icon={BookOpen01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {label ?? t("ui.whatToDo")}
    </a>
  );
}
