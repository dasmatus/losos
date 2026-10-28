/* The first-run wizard.
 *
 * Three steps. A box out of the carton has no password at all, and there
 * is no SSH and no shell login, so nothing (not the owner, not a support
 * call) can sign in until step 2 sets one. The other two are what has to be
 * true around it.
 *
 *   1  Trust this box       install the self-signed certificate, come back on
 *                           https, because step 2 sends a password and a
 *                           passkey cannot be made without a secure context
 *   2  Choose how you sign in   the password (required), the passkey (offered),
 *                           and the spare admin key, shown and printable once
 *   3  Your desktop         signed in to LosOS Login with that password, from
 *                           this page, and the desktop drawn behind the card
 *                           until "Let's go" grows it to the whole window
 *
 * This component owns the sequence and the gates and nothing else. Each step
 * reports upward when its gate is met — `account` for step 2, `signedIn` for
 * step 3 — and the footer reads those. Steps do not move the wizard
 * themselves. The desktop itself is the shell's (App.tsx): `onPreview` says
 * when to draw it.
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
import { t } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { StepRail } from "./wizard/StepRail";
import { StepTrust } from "./wizard/StepTrust";
import { StepSignIn } from "./wizard/StepSignIn";
import { StepDesktop, type Credentials, type DesktopSignIn } from "./wizard/StepDesktop";
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
  /** Called with true once the last step has signed the owner in, and with
   *  false if they go Back from it: the shell draws the desktop behind the
   *  card while it is true. */
  onPreview?: (shown: boolean) => void;
}

export default function Wizard({ onDone, onPreview }: WizardProps) {
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
  const [login, setLogin] = React.useState<string | null>(resumed?.login ?? null);
  const [adminKey, setAdminKey] = React.useState<string | null>(() => resumedKey(resumed));
  const [signedIn, setSignedIn] = React.useState<DesktopSignIn | null>(resumed?.signedIn ?? null);

  /* The password step 2 set, for step 3 to sign in with. A ref, in memory
   * only: it is never in storage, nothing renders it, and it is dropped the
   * moment step 3 is through. */
  const credentials = React.useRef<Credentials | null>(null);
  const readCredentials = React.useCallback(() => credentials.current, []);

  /* Kept from the moment step 2 has set a password: before that the box is
   * still unclaimed and a reload lands in setup anyway. Not once the tab is
   * the person's: it holds no spare key then, and a reload opens the desktop. */
  React.useEffect(() => {
    if (finished || account === null) return;
    if (signedIn === "person") {
      clearProgress();
      return;
    }
    saveProgress({ step, account, login, keyShown: adminKey !== null, signedIn });
  }, [step, account, login, adminKey, signedIn, finished]);

  /* The desktop behind the card: on the last step, once there is someone
   * signed in to show it to. */
  const previewing = step === "finish" && signedIn !== null;
  React.useEffect(() => {
    onPreview?.(previewing);
  }, [previewing, onPreview]);

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

  const onPasswordSet = React.useCallback(
    (user: string, key: string | null, name: string | null, password: string) => {
      setAccount(user);
      // A second password set from this tab goes through set-password and
      // carries no key; the one from the claim stays on screen.
      if (key !== null) setAdminKey(key);
      if (name !== null) setLogin(name);
      const who = name ?? login;
      credentials.current = who === null ? null : { login: who, password };
    },
    [login],
  );
  const onSignedIn = React.useCallback((how: DesktopSignIn) => {
    credentials.current = null;
    setSignedIn(how);
  }, []);

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
      // The shell grows the desktop behind this card to the window on
      // `onDone`; that is what says the sequence ended.
      onDone?.();
      return;
    }
    setDirection("forward");
    setStep(next);
  };

  // With `onDone` the card stays as it was while the shell grows the desktop
  // and takes the card away.
  if (finished && onDone === undefined) return <Closing setup={setup} />;

  return (
    <div
      data-testid="wizard"
      data-docked={previewing ? "true" : undefined}
      className={cn(
        "mx-auto flex w-full max-w-2xl flex-col gap-4 px-4 py-6 sm:px-6",
        // Over the desktop, the card moves to the foot of the window and the
        // desktop has the rest.
        previewing && "wizard-dock fixed inset-x-0 bottom-0 z-40 max-w-xl gap-3 pb-5 sm:pb-8",
      )}
    >
      <div className={cn("flex flex-col gap-3", previewing && "sr-only")}>
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

      <Card className={cn("overflow-hidden", previewing && "shadow-pop")}>
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
              login={login}
              adminKey={adminKey}
              signedIn={signedIn}
              credentials={readCredentials}
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
  login: string | null;
  adminKey: string | null;
  signedIn: DesktopSignIn | null;
  credentials: () => Credentials | null;
  onPasswordSet: (user: string, adminKey: string | null, login: string | null, password: string) => void;
  onSignedIn: (how: DesktopSignIn) => void;
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
        <StepDesktop
          login={props.login}
          credentials={props.credentials}
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
  signedIn: DesktopSignIn | null;
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
 * until this tab is signed in, as the person or, where it cannot be, with the
 * spare key. */
function gateFor(step: StepId, gates: Gates): Gate {
  switch (step) {
    case "trust":
      return { allowed: true, label: t("wizard.nav.continue") };
    case "signin":
      return { allowed: gates.account !== null, label: t("wizard.nav.continue") };
    case "finish":
      return { allowed: gates.signedIn !== null, label: t("wizard.nav.letsGo") };
    default: {
      const unreachable: never = step;
      return unreachable;
    }
  }
}

// ── After the last step ───────────────────────────────────────────────────

/* Shown when `onDone` is not wired. Deliberately a full stop rather than a
 * dashboard: whatever a shell routes to next is its decision, and a wizard
 * that ends in a blank frame looks like a crash. With `onDone` the shell has
 * the desktop on screen already and grows it into the window instead. */
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
