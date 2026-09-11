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

export interface StepFirstSignInProps {
  /** The account name lososd echoed back in step 2, if that step was done. */
  account: string | null;
  signedIn: boolean;
  onSignedIn: () => void;
}

export function StepFirstSignIn({ account, signedIn, onSignedIn }: StepFirstSignInProps) {
  const frame = React.useRef<HTMLIFrameElement>(null);
  const [watchable, setWatchable] = React.useState(true);

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
        if (LOGIN_PATH.test(path)) return;
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
        <Callout tone="ok" icon={CheckmarkCircle02Icon} title="You are signed in">
          <p className="mt-1">
            That was the last step. Finish below, or carry on in the window here. Nothing is
            waiting on you.
          </p>
        </Callout>
      ) : (
        <StepText>
          Sign in below with the name and password you set{account === null ? "" : " in step 2"}.
          {account === null ? null : (
            <>
              {" "}
              The name is <span className="numeric text-ink">{account}</span>.
            </>
          )}{" "}
          This page notices when you are through and finishes on its own.
        </StepText>
      )}

      {!watchable && !signedIn && (
        <Callout tone="info" icon={InformationCircleIcon} title="Cannot follow along">
          <p className="mt-1">
            The window below went somewhere this page cannot see. Sign in there, then say so
            with the button underneath.
          </p>
        </Callout>
      )}

      <iframe
        ref={frame}
        src={FILES_PATH}
        title="Sign in to your files"
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
          Open in a new tab instead
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
            I have signed in
          </Button>
        )}
      </div>
    </div>
  );
}
