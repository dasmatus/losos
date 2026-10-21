/* Step 3 — Sign in to LosOS Cloud.
 *
 * The owner signs in at LosOS Login in a tab of its own (both it and LosOS
 * Cloud refuse to be framed), and this step notices from here: it asks
 * LosOS Cloud, same-origin, who this browser is every couple of seconds,
 * and moves on by itself once the answer is a person. The session cookie
 * LosOS Login set in the other tab is this tab's too. A button says "done"
 * by hand, for a browser that keeps the two tabs' cookies apart.
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
import { useWaitGame } from "@/components/play-while-waiting";
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import { Rich, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { StepText } from "./parts";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

/** LosOS Cloud's web app. Signed out, nginx sends it to LosOS Login first,
 *  which sends the browser back here once the person is through. */
const FILES_PATH = "/cloud/";

/** Who LosOS Cloud thinks is asking: 200 once LosOS Login has a session in
 *  this browser, 401 before (backend/src/cloud/api.rs). */
const ME_PATH = "/cloud/api/me";

/* Fast enough that the hand-off does not feel stuck, slow enough that it is
 * not a busy loop. */
const WATCH_PERIOD_MS = 2000;

/* What the last answer said: nothing yet, LosOS Cloud is not answering (502,
 * 503 or 504 from nginx while it starts), or it answers and nobody is signed
 * in yet. */
type CloudState = "asking" | "starting" | "signedOut";

async function askWhoIsIn(signal: AbortSignal): Promise<CloudState | "signedIn"> {
  try {
    const response = await fetch(ME_PATH, { cache: "no-store", credentials: "same-origin", signal });
    if (response.ok) return "signedIn";
    if (response.status === 502 || response.status === 503 || response.status === 504) return "starting";
    return "signedOut";
  } catch {
    return "asking";
  }
}

export interface StepFirstSignInProps {
  /** The account name lososd echoed back in step 2, if that step was done. */
  account: string | null;
  signedIn: boolean;
  onSignedIn: () => void;
}

export function StepFirstSignIn({ account, signedIn, onSignedIn }: StepFirstSignInProps) {
  const t = useT();
  const [state, setState] = React.useState<CloudState>("asking");
  // LosOS Cloud starting is the one long wait here.
  const game = useWaitGame(!signedIn && state === "starting");

  React.useEffect(() => {
    if (signedIn) return;
    const controller = new AbortController();
    let live = true;
    const look = async (): Promise<void> => {
      const answer = await askWhoIsIn(controller.signal);
      if (!live) return;
      if (answer !== "signedIn") {
        setState(answer);
        return;
      }
      live = false;
      onSignedIn();
      toast.success(t("wizard.first.signedIn.title"), t("wizard.first.signedIn.toast"));
    };
    void look();
    const timer = window.setInterval(() => void look(), WATCH_PERIOD_MS);
    return () => {
      live = false;
      controller.abort();
      window.clearInterval(timer);
    };
  }, [signedIn, onSignedIn, t]);

  return (
    <div className="flex flex-col gap-4">
      {signedIn ? (
        <Alert variant="ok">
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.first.signedIn.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.first.signedIn.body")}</p>
          </AlertDescription>
        </Alert>
      ) : (
        <StepText>
          {account === null ? (
            t("wizard.first.intro")
          ) : (
            <Rich k="wizard.first.introNamed" vars={{ name: <span className="numeric text-ink">{account}</span> }} />
          )}
        </StepText>
      )}

      {!signedIn && state === "starting" && (
        <Alert variant="default" role="status">
          <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.first.starting.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.first.starting.body")}</p>
          </AlertDescription>
        </Alert>
      )}
      {/* The game is not part of the status: a live region would read out
          every point scored. It stays open once the wait is over, until it
          is closed. */}
      {game.offer}
      {game.panel !== null && <div className="w-full max-w-[560px]">{game.panel}</div>}

      {!signedIn && (
        <div className="flex flex-wrap items-center gap-2.5">
          {/* A tab of its own: LosOS Login and LosOS Cloud both refuse to be
              shown inside another page, and the sign-in is the person's
              first look at their files anyway. */}
          <a
            href={FILES_PATH}
            target="_blank"
            rel="noopener noreferrer"
            aria-disabled={state === "starting"}
            className={cn(
              buttonVariants({ variant: "primary", size: "sm" }),
              state === "starting" && "pointer-events-none opacity-60",
            )}
          >
            <HugeiconsIcon icon={LinkSquare02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("wizard.first.newTab")}
          </a>
          <span className="inline-flex items-center gap-2 text-sm text-muted" role="status">
            <Spinner size={14} label={t("wizard.first.waiting")} />
            {t("wizard.first.waiting")}
          </span>
          <Button variant="ghost" size="sm" onClick={onSignedIn}>
            <HugeiconsIcon icon={Login01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("wizard.first.manual")}
          </Button>
        </div>
      )}
    </div>
  );
}
