/* Step 4 — Sign in, without leaving the page.
 *
 * The sign-in page is embedded rather than linked. `frame-src 'self'` is in
 * the admin CSP for exactly this (modules/containers.nix spells it out, since
 * under CSP3 an absent frame-src falls back to default-src 'none' and would
 * block it), and the two routes proxied through the front vhost are served
 * without X-Frame-Options so the frame is allowed to render.
 *
 * Because the frame is same-origin, its location is readable from here — so
 * the wizard can tell when the sign-in page has been left behind and advance
 * itself. That read is wrapped in try/catch anyway: a redirect off this origin
 * would make it throw, and the honest response to "I can no longer see what
 * the frame is doing" is to stop guessing and offer the manual way out.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  CheckmarkCircle02Icon,
  InformationCircleIcon,
  LinkSquare02Icon,
  Login01Icon,
} from "@hugeicons/core-free-icons";
import { Button, buttonVariants } from "@/components/ui/button";
import { Rich, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Callout, StepText } from "./parts";

/** Where the front vhost proxies the files app. */
const FILES_PATH = "/nextcloud";

/* Covers /nextcloud/login, /nextcloud/index.php/login and the two-factor
 * pages under /nextcloud/login/challenge/*. Anything else under /nextcloud is
 * a signed-in page. */
const LOGIN_PATH = /\/login(\/|$|\?)/;

/* Fast enough that the hand-off does not feel stuck, slow enough that it is
 * not a busy loop; the frame is doing a full page load between reads either
 * way. */
const WATCH_PERIOD_MS = 700;

/* How often the frame is loaded again while what it shows is not the files
 * app at all. On a new box this step is reached a minute or two before the
 * app's web server is up (occ finishes installing first, Apache follows), and
 * what the frame gets in between is nginx's own "502 Bad Gateway" page, which
 * nothing ever reloads. Take 7 of the recorded install demo (2026-10-05) sat
 * on that page for fifteen minutes with the step saying "You are signed in". */
const RELOAD_PERIOD_MS = 5000;

/* Whether the document in the frame is one of the files app's own pages.
 * Nextcloud stamps its request token on <head> of every page it renders,
 * signed in or not; nginx's error pages and a blank frame carry nothing. */
function isFilesAppPage(doc: Document): boolean {
  return doc.head !== null && doc.head.dataset["requesttoken"] !== undefined;
}

/* Whether that page is rendered for a signed-in user: Nextcloud sets
 * `data-user` on <head> only then. The login page, a maintenance page and a
 * two-factor challenge all have the token and no user. */
function isSignedInPage(doc: Document): boolean {
  const user = doc.head?.dataset["user"];
  return typeof user === "string" && user.length > 0;
}

export interface StepFirstSignInProps {
  /** The account name lososd echoed back in step 2, if that step was done. */
  account: string | null;
  signedIn: boolean;
  onSignedIn: () => void;
}

export function StepFirstSignIn({ account, signedIn, onSignedIn }: StepFirstSignInProps) {
  const t = useT();
  const frame = React.useRef<HTMLIFrameElement>(null);
  const [watchable, setWatchable] = React.useState(true);
  // True while the frame shows something that is not the files app (nginx
  // answering for a web server that is not up yet). Drives the note below
  // and the periodic reload.
  const [starting, setStarting] = React.useState(false);
  const lastReload = React.useRef(0);

  React.useEffect(() => {
    if (signedIn) return;

    const look = (): void => {
      const win = frame.current?.contentWindow;
      if (win === null || win === undefined) return;
      try {
        const path = win.location.pathname + win.location.search;
        // about:blank reads as "blank" here, so this also covers the frame
        // before its first document has committed.
        if (!path.startsWith(FILES_PATH)) return;
        const doc = win.document;
        if (doc.readyState !== "complete") return;
        if (!isFilesAppPage(doc)) {
          // Not the app: nginx's 502 while Apache is still coming up, or
          // its 504. Say so, and ask again every few seconds; the page the
          // owner wants appears on its own once the app answers.
          setStarting(true);
          const now = Date.now();
          if (now - lastReload.current >= RELOAD_PERIOD_MS) {
            lastReload.current = now;
            win.location.replace(FILES_PATH);
          }
          return;
        }
        setStarting(false);
        if (LOGIN_PATH.test(path)) return;
        // A page of the app that is not the login page is still not proof
        // of a session: a maintenance page is one too. The user stamp is.
        if (!isSignedInPage(doc)) return;
        onSignedIn();
      } catch {
        /* Cross-origin now: the frame followed a redirect off this box. There
         * is nothing further to read, so stop claiming to know. */
        setWatchable(false);
      }
    };

    const timer = window.setInterval(look, WATCH_PERIOD_MS);
    return () => window.clearInterval(timer);
  }, [signedIn, onSignedIn]);

  return (
    <div className="flex flex-col gap-4">
      {signedIn ? (
        <Callout tone="ok" icon={CheckmarkCircle02Icon} title={t("wizard.first.signedIn.title")}>
          <p className="mt-1">{t("wizard.first.signedIn.body")}</p>
        </Callout>
      ) : (
        <StepText>
          {account === null ? (
            t("wizard.first.intro")
          ) : (
            <Rich
              k="wizard.first.introNamed"
              vars={{ name: <span className="numeric text-ink">{account}</span> }}
            />
          )}
        </StepText>
      )}

      {!watchable && !signedIn && (
        <Callout tone="info" icon={InformationCircleIcon} title={t("wizard.first.lost.title")}>
          <p className="mt-1">{t("wizard.first.lost.body")}</p>
        </Callout>
      )}

      {watchable && starting && !signedIn && (
        <Callout
          tone="info"
          icon={InformationCircleIcon}
          title={t("wizard.first.starting.title")}
        >
          <p className="mt-1">{t("wizard.first.starting.body")}</p>
        </Callout>
      )}

      <iframe
        ref={frame}
        src={FILES_PATH}
        title={t("wizard.first.frameTitle")}
        // No sandbox attribute: it is same-origin by design (the CSP note in
        // modules/containers.nix covers what that costs and why it was
        // accepted), and a sandbox without allow-same-origin would give the
        // framed page a null origin — which breaks its own session cookie and
        // means it could never sign anyone in.
        className={cn(
          "h-[min(68vh,640px)] w-full rounded-control border border-line bg-surface",
          "animate-fade-in",
        )}
      />

      <div className="flex flex-wrap items-center gap-2.5">
        {/* Always offered, not only when the watcher gives up: a frame this
            size is cramped on a phone, and some browsers block third-party
            storage in frames aggressively enough to break a login form. */}
        <a
          href={FILES_PATH}
          target="_blank"
          rel="noopener noreferrer"
          className={cn(buttonVariants({ variant: "secondary", size: "sm" }))}
        >
          <HugeiconsIcon
            icon={LinkSquare02Icon}
            size={16}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {t("wizard.first.newTab")}
        </a>

        {!signedIn && (
          <Button variant="ghost" size="sm" onClick={onSignedIn}>
            <HugeiconsIcon
              icon={Login01Icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.first.manual")}
          </Button>
        )}
      </div>
    </div>
  );
}
