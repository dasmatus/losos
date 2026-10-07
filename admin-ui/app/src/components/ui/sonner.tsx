import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, InformationCircleIcon } from "@hugeicons/core-free-icons";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import { useTheme } from "@/components/ui/theme-toggle";
import { useT } from "@/lib/i18n-react";
import { useConfirmationsHeight } from "./corner";

/* shadcn's Sonner component, on LosOS tokens: the status stack.
 *
 * Sonner carries the status updates (toast.status in toast.tsx): what is
 * happening, what is being waited for. The "done" confirmations are
 * shadcn's Toast (toaster.tsx) and take the top of the same corner; this
 * stack sits under them by their measured height (corner.ts), so the two
 * never land on each other. The `sonner` library's <Toaster/>, themed and
 * positioned here. Where shadcn's copy hands Sonner the palette through a style
 * attribute and reads the theme from next-themes, this one maps the palette
 * in src/styles/index.css (`.toaster`) and reads lib/theme.ts, because the
 * appliance CSP (`style-src 'self'`) is the rule of the house and the theme
 * store is already there.
 *
 * Two things the CSP forces, both in index.css rather than here:
 *
 *   Sonner injects its stylesheet with a <style> element when the module
 *   loads, and `style-src 'self'` refuses that element. main.tsx imports
 *   `sonner/dist/styles.css` into the bundle instead, so the same rules
 *   arrive as a file the policy allows; the refused element is a console
 *   line and nothing more. tests/toasts.browser.mjs serves the bundle
 *   under the real policy and checks the stack is styled.
 *
 *   The per-toast `--index`, `--offset` and heights Sonner sets through
 *   React's style prop are CSSOM writes, which the policy does not block
 *   (it blocks the attribute, not element.style). Nothing to do there.
 *
 * Top right, as a Mac's notification corner: the newest at the top, the
 * older ones collapsed behind it until the pointer rests on the stack, which
 * also pauses every countdown. index.css turns Sonner's drop-from-above into
 * a slide in from the right edge; prefers-reduced-motion switches both off
 * (Sonner's own rule, and the stylesheet's global one).
 */
export function Toaster(props: ToasterProps) {
  const { resolvedTheme } = useTheme();
  const t = useT();
  /* The confirmations above, plus the same gap the stack keeps inside. */
  const above = useConfirmationsHeight();
  const below = above > 0 ? above + 10 : 0;
  return (
    <Sonner
      theme={resolvedTheme}
      position="top-right"
      closeButton
      visibleToasts={5}
      gap={10}
      offset={{ top: 16 + below, right: 16 }}
      mobileOffset={{ top: 12 + below, right: 12, left: 12 }}
      containerAriaLabel={t("ui.notifications")}
      className="toaster"
      icons={{
        info: (
          <HugeiconsIcon icon={InformationCircleIcon} size={20} strokeWidth={1.5} color="currentColor" />
        ),
        close: <HugeiconsIcon icon={Cancel01Icon} size={12} strokeWidth={1.5} color="currentColor" />,
      }}
      toastOptions={{
        closeButtonAriaLabel: t("ui.dismiss"),
      }}
      {...props}
    />
  );
}
