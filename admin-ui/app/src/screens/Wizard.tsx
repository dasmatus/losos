/* The first-run wizard.
 *
 * Four steps, and it exists because of one bug: modules/nextcloud-common.nix
 * mints the first admin password from /dev/urandom, writes it 0600 and shows
 * it to nobody. There is no SSH and no shell login, so on a box out of the
 * carton nothing — not the owner, not a support call — can sign in at all.
 * Step 2 is the fix; the other three are what has to be true around it.
 *
 *   1  Trust this box       install the self-signed certificate, come back on
 *                           https, because step 2 sends a password and a
 *                           passkey cannot be made without a secure context
 *   2  Choose how you sign in   the password (required), the passkey (offered)
 *   3  Recovery code        the one thing that must leave the box
 *   4  Sign in              the files app, embedded, watched
 *
 * This component owns the sequence and the gates and nothing else. Each step
 * reports upward when its gate is met — `account` for step 2, `codeSaved` for
 * step 3, `signedIn` for step 4 — and the footer reads those. Steps do not
 * move the wizard themselves.
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
import { StepRail } from "./wizard/StepRail";
import { StepTrust } from "./wizard/StepTrust";
import { StepSignIn } from "./wizard/StepSignIn";
import { StepRecovery } from "./wizard/StepRecovery";
import { StepFirstSignIn } from "./wizard/StepFirstSignIn";
import { StepText } from "./wizard/parts";
import { boxName, useSetupState, type SetupQuery } from "./wizard/useSetupState";
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
  const setup = useSetupState();

  const [step, setStep] = React.useState<StepId>("trust");
  const [direction, setDirection] = React.useState<StepDirection>("forward");
  const [finished, setFinished] = React.useState(false);

  /* The gates. One per step that has one, held here rather than inside the
   * steps so a Back-then-Forward does not reset them — retyping a password
   * because you went back to re-read the fingerprint would be its own bug. */
  const [account, setAccount] = React.useState<string | null>(null);
  const [codeSaved, setCodeSaved] = React.useState(false);
  const [codeUnavailable, setCodeUnavailable] = React.useState(false);
  const [signedIn, setSignedIn] = React.useState(false);

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

  const onPasswordSet = React.useCallback((user: string) => setAccount(user), []);
  const onCodeSaved = React.useCallback(() => setCodeSaved(true), []);
  const onCodeUnavailable = React.useCallback(() => setCodeUnavailable(true), []);
  const onSignedIn = React.useCallback(() => setSignedIn(true), []);

  const previous = previousStep(step);
  const next = nextStep(step);
  const gate = gateFor(step, { account, codeSaved, codeUnavailable, signedIn });

  const goBack = (): void => {
    if (previous === null) return;
    setDirection("back");
    setStep(previous);
  };

  const goForward = (): void => {
    if (next === null) {
      setFinished(true);
      onDone?.();
      return;
    }
    setDirection("forward");
    setStep(next);
  };

  if (finished) return <Closing name={boxName(setup)} />;

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-4 py-6 sm:px-6">
      <div className="flex flex-col gap-3">
        <h1 className="text-[17px] leading-tight font-semibold tracking-tight text-ink">
          Set up {boxName(setup)}
        </h1>
        <StepRail current={step} />
      </div>

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
              codeSaved={codeSaved}
              signedIn={signedIn}
              onPasswordSet={onPasswordSet}
              onCodeSaved={onCodeSaved}
              onCodeUnavailable={onCodeUnavailable}
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
                Back
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
  codeSaved: boolean;
  signedIn: boolean;
  onPasswordSet: (user: string) => void;
  onCodeSaved: () => void;
  onCodeUnavailable: () => void;
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
          onPasswordSet={props.onPasswordSet}
        />
      );
    case "recovery":
      return (
        <StepRecovery
          boxName={boxName(props.setup)}
          saved={props.codeSaved}
          onSaved={props.onCodeSaved}
          onUnavailable={props.onCodeUnavailable}
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
  codeSaved: boolean;
  codeUnavailable: boolean;
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
 * password because that is the bug this wizard exists to fix. Step 3 waits on
 * the code having been copied or printed, and falls back to an explicit
 * "without a code" when the box cannot produce one: refusing to let anyone
 * past a step whose content failed to load would be a locked door, not a
 * safeguard, and the changed label is what keeps it from reading as done. */
function gateFor(step: StepId, gates: Gates): Gate {
  switch (step) {
    case "trust":
      return { allowed: true, label: "Continue" };
    case "signin":
      return { allowed: gates.account !== null, label: "Continue" };
    case "recovery":
      if (gates.codeSaved) return { allowed: true, label: "Continue" };
      if (gates.codeUnavailable) return { allowed: true, label: "Continue without a code" };
      return { allowed: false, label: "Continue" };
    case "finish":
      return { allowed: gates.signedIn, label: "Finish" };
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
function Closing({ name }: { name: string }) {
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
            {name} is ready
          </CardTitle>
        </CardHeader>
        <CardContent>
          <StepText>
            You can sign in from any device on this network. Keep the recovery code somewhere
            that is not this box.
          </StepText>
        </CardContent>
      </Card>
    </div>
  );
}
