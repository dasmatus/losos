/* Step 3 — Sign in, without leaving the page.
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
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
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
 * app at all. On a new box this step is reached a few minutes before the
 * app's web server is up (occ finishes installing first, Apache follows), and
 * what the frame gets in between is nginx's own "502 Bad Gateway" page, which
 * nothing ever reloads. Take 7 of the recorded install demo (2026-10-05) sat
 * on that page for fifteen minutes with the step saying "You are signed in". */
const RELOAD_PERIOD_MS = 5000;

/* How long one of those loads may stay in flight before it is given up on
 * and asked for again. A load that is still waiting for its first byte must
 * not be interrupted by the next reload: the frame keeps showing (and the
 * watcher keeps reading) the old error page until the new document commits,
 * so a reload issued every RELOAD_PERIOD_MS on top of a load that needs
 * longer than that to answer cancels it every time, and the app's page never
 * arrives at all. That is what take 9 of the recorded demo did (2026-10-06):
 * Apache was up, its first answer after the claim took longer than five
 * seconds on the busy box, and the step sat on "still starting" for nine
 * minutes. nginx answers for a web server that is down at once (502) and
 * for one that hangs after its own proxy timeout (504), both of which end
 * the load; this bound only covers a connection that stalls short of either. */
const RELOAD_GIVE_UP_MS = 60_000;

/* What the frame is known to hold. It starts out `loading` (nothing has
 * committed yet), becomes `starting` when a document arrived that is not the
 * app's (nginx answering for a web server that is not up), and `app` once a
 * page of the files app itself rendered. Only `app` is shown: until then the
 * frame loads in the background and a quiet panel stands in its place, so
 * the owner never sees a raw "502 Bad Gateway" where their sign-in form
 * should be (take 8 of the recorded demo showed that page for six minutes;
 * an error page as the first thing after setting a password reads as
 * something broke, not as something starting). */
type FrameState = "loading" | "starting" | "app";

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

export function StepFirstSignIn({
  account,
  signedIn,
  onSignedIn,
}: StepFirstSignInProps) {
  const t = useT();
  const frame = React.useRef<HTMLIFrameElement>(null);
  const [watchable, setWatchable] = React.useState(true);
  const [frameState, setFrameState] = React.useState<FrameState>("loading");
  const lastReload = React.useRef(0);
  // Whether a load the watcher asked for has not finished yet (the frame's
  // load event clears it). See RELOAD_GIVE_UP_MS.
  const reloadPending = React.useRef(false);
  // The frame is on screen only once it holds a page of the app, or once
  // the sign-in is done (then it is the app, whatever it shows next).
  const showFrame = signedIn || frameState === "app";

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
          // its 504. Keep the frame hidden, say so, and ask again every few
          // seconds; the page the owner wants appears on its own once the
          // app answers.
          setFrameState("starting");
          const now = Date.now();
          const since = now - lastReload.current;
          const due = reloadPending.current
            ? since >= RELOAD_GIVE_UP_MS
            : since >= RELOAD_PERIOD_MS;
          if (due) {
            lastReload.current = now;
            reloadPending.current = true;
            win.location.replace(FILES_PATH);
          }
          return;
        }
        setFrameState("app");
        if (LOGIN_PATH.test(path)) return;
        // A page of the app that is not the login page is still not proof
        // of a session: a maintenance page is one too. The user stamp is.
        if (!isSignedInPage(doc)) return;
        onSignedIn();
        // Once: the effect is torn down on `signedIn` and not re-run.
        toast.success(t("wizard.first.signedIn.title"), t("wizard.first.signedIn.toast"));
      } catch {
        /* Cross-origin now: the frame followed a redirect off this box. There
         * is nothing further to read, so stop claiming to know. */
        setWatchable(false);
      }
    };

    const timer = window.setInterval(look, WATCH_PERIOD_MS);
    return () => window.clearInterval(timer);
  }, [signedIn, onSignedIn, t]);

  return (
    <div className="relative flex flex-col gap-4">
      {signedIn ? (
        <Callout
          tone="ok"
          icon={CheckmarkCircle02Icon}
          title={t("wizard.first.signedIn.title")}
        >
          <p className="mt-1">{t("wizard.first.signedIn.body")}</p>
        </Callout>
      ) : (
        <StepText>
          {account === null ? (
            t("wizard.first.intro")
          ) : (
            <Rich
              k="wizard.first.introNamed"
              vars={{
                name: <span className="numeric text-ink">{account}</span>,
              }}
            />
          )}
        </StepText>
      )}

      {!watchable && !signedIn && (
        <Callout
          tone="info"
          icon={InformationCircleIcon}
          title={t("wizard.first.lost.title")}
        >
          <p className="mt-1">{t("wizard.first.lost.body")}</p>
        </Callout>
      )}

      {watchable && !showFrame && (
        /* Stands where the frame will be, same height, so nothing jumps when
           the frame takes over. Before the first document: opening. After a
           document that is not the app: starting, with the reason. */
        <div
          role="status"
          className={cn(
            "flex h-[min(68vh,640px)] w-full flex-col items-center justify-center gap-3 px-6 text-center",
            "rounded-control border border-line bg-surface animate-fade-in",
          )}
        >
          <Spinner
            size={22}
            className="text-muted"
            label={t("wizard.first.opening")}
          />
          <p className="text-base font-medium text-ink">
            {frameState === "starting"
              ? t("wizard.first.starting.title")
              : t("wizard.first.opening")}
          </p>
          {frameState === "starting" && (
            <p className="max-w-prose text-sm text-muted">
              {t("wizard.first.starting.body")}
            </p>
          )}
        </div>
      )}

      <iframe
        ref={frame}
        src={FILES_PATH}
        onLoad={() => {
          reloadPending.current = false;
        }}
        title={t("wizard.first.frameTitle")}
        // No sandbox attribute: it is same-origin by design (the CSP note in
        // modules/containers.nix covers what that costs and why it was
        // accepted), and a sandbox without allow-same-origin would give the
        // framed page a null origin — which breaks its own session cookie and
        // means it could never sign anyone in.
        // Kept in the document while hidden: a frame loads and can be read
        // whether or not it is visible, and that reading is what decides
        // when to show it. Hidden means out of the layout, the tab order
        // and the accessibility tree, not merely transparent.
        aria-hidden={!showFrame}
        tabIndex={showFrame ? undefined : -1}
        className={cn(
          "w-full rounded-control border border-line bg-surface",
          showFrame
            ? "h-[min(68vh,640px)] animate-fade-in"
            : "invisible pointer-events-none absolute h-px w-px overflow-hidden",
        )}
      />

      <div className="flex flex-wrap items-center gap-2.5">
        {/* Offered whenever the frame is, not only when the watcher gives
            up: a frame this size is cramped on a phone, and some browsers
            block third-party storage in frames aggressively enough to break
            a login form. Not while the app is still starting: the tab would
            open on the same error page the frame is hiding. */}
        {(showFrame || !watchable) && (
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
        )}

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
