/* The first-run wizard.
 *
 * Three steps, and it exists because of one bug: modules/nextcloud-common.nix
 * mints the first admin password from /dev/urandom, writes it 0600 and shows
 * it to nobody. There is no SSH and no shell login, so on a box out of the
 * carton nothing — not the owner, not a support call — can sign in at all.
 * Step 2 is the fix; the other two are what has to be true around it.
 *
 *   1  Trust this box       install the self-signed certificate, come back on
 *                           https, because step 2 sends a password and a
 *                           passkey cannot be made without a secure context
 *   2  Choose how you sign in   the password (required), the passkey (offered),
 *                           and the spare admin key, shown and printable once
 *   3  Sign in              the files app, embedded, watched
 *
 * There was a recovery-code step between 2 and 3; wizard/steps.ts says why it
 * is hidden and how to bring it back.
 *
 * This component owns the sequence and the gates and nothing else. Each step
 * reports upward when its gate is met — `account` for step 2, `signedIn` for
 * step 3 — and the footer reads those. Steps do not move the wizard
 * themselves.
 *
 * House rules, same as everywhere in this app: no inline style attributes (the
 * appliance CSP refuses them), no emoji, and nothing a user reads names a
 * container runtime.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, ArrowRight01Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { LanguagePicker } from "@/components/ui/language-picker";
import { NoTpmNotice, useRunsWithoutTpm } from "@/components/no-tpm-notice";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { StepRail } from "./wizard/StepRail";
import { StepTrust } from "./wizard/StepTrust";
import { StepSignIn } from "./wizard/StepSignIn";
import { StepFirstSignIn } from "./wizard/StepFirstSignIn";
import { StepText } from "./wizard/parts";
import { boxName, useSetupState, type SetupQuery } from "./wizard/useSetupState";
import { clearProgress, readProgress, resumedKey, saveProgress } from "./wizard/progress";
import {
  nextStep,
  previousStep,
  stepTitle,
  type StepDirection,
  type StepId,
} from "./wizard/steps";
import "./wizard/wizard.css";

export interface WizardProps {
  /** Called once, when the owner leaves the last step. The shell decides what
   *  that means — routing to the overview, clearing a first-run flag. Without
   *  it the wizard shows a closing card and stops. */
  onDone?: () => void;
}

export default function Wizard({ onDone }: WizardProps) {
  const t = useT();
  const setup = useSetupState();

  /* A reload in the middle of setup comes back to the same step with the same
   * gates met (wizard/progress.ts). */
  const [resumed] = React.useState(readProgress);
  const [step, setStep] = React.useState<StepId>(resumed?.step ?? "trust");
  const [direction, setDirection] = React.useState<StepDirection>("forward");
  const [finished, setFinished] = React.useState(false);

  /* The gates. One per step that has one, held here rather than inside the
   * steps so a Back-then-Forward does not reset them — retyping a password
   * because you went back to re-read the fingerprint would be its own bug. */
  const [account, setAccount] = React.useState<string | null>(resumed?.account ?? null);
  const [adminKey, setAdminKey] = React.useState<string | null>(() => resumedKey(resumed));
  const [signedIn, setSignedIn] = React.useState(resumed?.signedIn ?? false);

  /* Kept from the moment step 2 has set a password: before that the box is
   * still unclaimed and a reload lands in setup anyway. */
  React.useEffect(() => {
    if (finished || account === null) return;
    saveProgress({ step, account, keyShown: adminKey !== null, signedIn });
  }, [step, account, adminKey, signedIn, finished]);

  /* The step's own <section> takes focus on a move, not the heading inside
   * it: the section carries the aria-label, so landing on it announces which
   * step this is, and components/ui/card.tsx takes no ref.
   *
   * Skipped on the first render. The page has only just loaded and pulling
   * focus into the middle of it would skip the title and the rail for anyone
   * reading with a keyboard or a screen reader. */
  const panel = React.useRef<HTMLElement>(null);
  const moved = React.useRef(false);

  React.useEffect(() => {
    if (!moved.current) {
      moved.current = true;
      return;
    }
    panel.current?.focus();
  }, [step]);

  const onPasswordSet = React.useCallback((user: string, key: string | null) => {
    setAccount(user);
    // A second password set from this tab goes through set-password and
    // carries no key; the one from the claim stays on screen.
    if (key !== null) setAdminKey(key);
  }, []);
  const onSignedIn = React.useCallback(() => setSignedIn(true), []);

  /* A box installed without a TPM says so here, once the password is set:
   * the claim is what puts the token in this tab, and the owner has just
   * decided how to guard the box, which is when its weak spot belongs on
   * screen. Above the card, so it stays through the last step. */
  const noTpm = useRunsWithoutTpm(account !== null);

  const previous = previousStep(step);
  const next = nextStep(step);
  const gate = gateFor(step, { account, signedIn });

  const goBack = (): void => {
    if (previous === null) return;
    setDirection("back");
    setStep(previous);
  };

  const goForward = (): void => {
    if (next === null) {
      clearProgress();
      setFinished(true);
      // The shell swaps the wizard for the app on `onDone`, so this is the
      // one line that says the sequence ended rather than vanished.
      toast.success(
        t("wizard.closing.toast"),
        setup.kind === "ready"
          ? t("wizard.closing.titleNamed", { name: setup.state.hostName })
          : t("wizard.closing.titleUnnamed"),
      );
      onDone?.();
      return;
    }
    setDirection("forward");
    setStep(next);
  };

  if (finished) return <Closing setup={setup} />;

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-4 py-6 sm:px-6">
      <div className="flex flex-col gap-3">
        {/* The picker sits on the very first screen a new owner sees: someone
            who cannot read the browser's language has to be able to switch
            before step one asks them anything. */}
        <div className="flex items-start justify-between gap-3">
          <h1 className="text-[17px] leading-tight font-semibold tracking-tight text-ink">
            {setup.kind === "ready"
              ? t("wizard.title.named", { name: setup.state.hostName })
              : t("wizard.title.unnamed")}
          </h1>
          <LanguagePicker className="shrink-0" />
        </div>
        <StepRail current={step} />
      </div>

      {noTpm && <NoTpmNotice />}

      <Card className="overflow-hidden">
        {/* Keyed by step so React remounts the subtree and the enter animation
            in wizard.css runs again. The direction class is the only thing
            that says which way the move went; there is no exit half, because
            only one step is ever mounted. */}
        <section
          key={step}
          ref={panel}
          tabIndex={-1}
          aria-label={stepTitle(step)}
          className={direction === "forward" ? "wizard-step-forward" : "wizard-step-back"}
        >
          <CardHeader>
            <CardTitle>{stepTitle(step)}</CardTitle>
          </CardHeader>
          <CardContent>
            <StepBody
              step={step}
              setup={setup}
              account={account}
              adminKey={adminKey}
              signedIn={signedIn}
              onPasswordSet={onPasswordSet}
              onSignedIn={onSignedIn}
            />
          </CardContent>
        </section>

        <CardFooter className="justify-between">
          <div>
            {previous !== null && (
              <Button variant="ghost" onClick={goBack}>
                <HugeiconsIcon
                  icon={ArrowLeft01Icon}
                  size={18}
                  strokeWidth={1.5}
                  color="currentColor"
                  aria-hidden="true"
                />
                {t("wizard.nav.back")}
              </Button>
            )}
          </div>
          <Button onClick={goForward} disabled={!gate.allowed}>
            {gate.label}
            <HugeiconsIcon
              icon={ArrowRight01Icon}
              size={18}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}

// ── Which step is on screen ───────────────────────────────────────────────

interface StepBodyProps {
  step: StepId;
  setup: SetupQuery;
  account: string | null;
  adminKey: string | null;
  signedIn: boolean;
  onPasswordSet: (user: string, adminKey: string | null) => void;
  onSignedIn: () => void;
}

function StepBody(props: StepBodyProps) {
  switch (props.step) {
    case "trust":
      return <StepTrust query={props.setup} />;
    case "signin":
      return (
        <StepSignIn
          boxName={boxName(props.setup)}
          account={props.account}
          adminKey={props.adminKey}
          onPasswordSet={props.onPasswordSet}
        />
      );
    case "finish":
      return (
        <StepFirstSignIn
          account={props.account}
          signedIn={props.signedIn}
          onSignedIn={props.onSignedIn}
        />
      );
    default: {
      // A new StepId without a branch here is a compile error, not a blank card.
      const unreachable: never = props.step;
      return unreachable;
    }
  }
}

// ── The gate on Continue ──────────────────────────────────────────────────

interface Gates {
  account: string | null;
  signedIn: boolean;
}

interface Gate {
  allowed: boolean;
  label: string;
}

/* One place that decides whether Continue is live, and what it says.
 *
 * Step 1 is always passable — the certificate may already be installed, and a
 * box with `losos.tls.enable` off has none to install. Step 2 waits on a
 * password because that is the bug this wizard exists to fix. Step 3 waits
 * on a real session in the embedded files app. */
function gateFor(step: StepId, gates: Gates): Gate {
  switch (step) {
    case "trust":
      return { allowed: true, label: t("wizard.nav.continue") };
    case "signin":
      return { allowed: gates.account !== null, label: t("wizard.nav.continue") };
    case "finish":
      return { allowed: gates.signedIn, label: t("wizard.nav.finish") };
    default: {
      const unreachable: never = step;
      return unreachable;
    }
  }
}

// ── After the last step ───────────────────────────────────────────────────

/* Shown when `onDone` is not wired, and briefly before it takes effect when
 * it is. Deliberately a full stop rather than a dashboard: whatever the shell
 * routes to next is its decision, and a wizard that ends in a blank frame
 * looks like a crash. */
function Closing({ setup }: { setup: SetupQuery }) {
  const t = useT();
  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-4 py-6 sm:px-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HugeiconsIcon
              icon={CheckmarkCircle02Icon}
              size={20}
              strokeWidth={1.5}
              color="currentColor"
              className="text-ok"
              aria-hidden="true"
            />
            {setup.kind === "ready"
              ? t("wizard.closing.titleNamed", { name: setup.state.hostName })
              : t("wizard.closing.titleUnnamed")}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <StepText>
            {t("wizard.closing.body")}
          </StepText>
        </CardContent>
      </Card>
    </div>
  );
}
