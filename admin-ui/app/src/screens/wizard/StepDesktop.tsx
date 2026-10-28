/* Step 3 — Your desktop.
 *
 * The owner chose a name and a password in step 2, and this step signs them
 * in to LosOS Login with exactly those, from this page: the same call LosOS
 * Login's own sign-in page makes (`signInWithPassword`), same origin, so the
 * session cookie is this tab's. Then the tab lets go of the spare key the
 * claim gave it (`adoptSession`) and is signed in as the person. Once it is,
 * the shell draws the real desktop behind this card, small and out of reach,
 * and "Let's go" grows it to the whole window (App.tsx, `desktop-stage`).
 *
 * The password is held by the wizard in memory only (Wizard.tsx), never in
 * storage, and dropped once the step is done, so a tab reloaded on this step
 * asks for it once more. LosOS Login
 * starts only after the claim wrote the people file, so on a new box the
 * first answers are nginx's 502 and the step waits, with a game on offer.
 *
 * Three boxes cannot sign anyone in this way, and the step says so and lets
 * them on with the spare key, which is a sign-in too: a box without LosOS
 * Login (`standalone` false in GET /api/me), a page on plain http (LosOS
 * Login's cookie is https only), and an address LosOS Login has no cookie
 * domain for, where the sign-in succeeds and the cookie never comes back.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { InformationCircleIcon, Login01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { useWaitGame } from "@/components/play-while-waiting";
import { adoptSession, isAbort, loadMe, signInPageReady, signInWithPassword } from "@/lib/api";
import { Rich, useT } from "@/lib/i18n-react";
import { StepText } from "./parts";

/** A name and the password set for it in step 2. */
export interface Credentials {
  login: string;
  password: string;
}

/** How this tab is signed in once the step is done: as the person, or with
 *  the spare key and the reason it could not be the person. */
export type DesktopSignIn = "person" | "plainHttp" | "noLogin" | "address";

/* Asked again this often while LosOS Login starts. */
const RETRY_MS = 2000;

type Phase =
  | { kind: "asking" }
  | { kind: "starting" }
  | { kind: "password"; refused: boolean }
  | { kind: "done"; how: DesktopSignIn };

export interface StepDesktopProps {
  /** The LosOS Login name step 2 made, or null when this tab does not know it. */
  login: string | null;
  /** What step 2 set, while the wizard still holds it. */
  credentials: () => Credentials | null;
  /** How this tab is signed in, once it is. */
  signedIn: DesktopSignIn | null;
  onSignedIn: (how: DesktopSignIn) => void;
}

export function StepDesktop({ login, credentials, signedIn, onSignedIn }: StepDesktopProps) {
  const t = useT();
  const [phase, setPhase] = React.useState<Phase>(() =>
    signedIn === null ? { kind: "asking" } : { kind: "done", how: signedIn },
  );
  const [attempt, setAttempt] = React.useState<Credentials | null>(null);
  const game = useWaitGame(phase.kind === "starting");

  /* Settles who this tab is: once on arrival with what step 2 set, and again
   * for each password typed here. */
  React.useEffect(() => {
    if (signedIn !== null) return;
    const controller = new AbortController();
    const { signal } = controller;
    let timer = 0;
    const done = (how: DesktopSignIn): void => {
      setPhase({ kind: "done", how });
      onSignedIn(how);
    };

    const run = async (given: Credentials | null): Promise<void> => {
      // Whether this box has LosOS Login at all. The spare key answers
      // /api/me too, and says.
      const me = await loadMe({ signal }).catch((error: unknown) => {
        if (isAbort(error)) throw error;
        return null;
      });
      if (me !== null && me.standalone === false) return done("noLogin");
      if (window.location.protocol !== "https:") return done("plainHttp");
      if (given === null) {
        setPhase({ kind: "password", refused: false });
        return;
      }
      const tryOnce = async (): Promise<void> => {
        if (!(await signInPageReady(signal))) {
          setPhase({ kind: "starting" });
          timer = window.setTimeout(() => void tryOnce().catch(ignoreAbort), RETRY_MS);
          return;
        }
        const answer = await signInWithPassword(given.login, given.password, signal);
        if (answer === "notYet") {
          setPhase({ kind: "starting" });
          timer = window.setTimeout(() => void tryOnce().catch(ignoreAbort), RETRY_MS);
          return;
        }
        if (answer === "refused") {
          setPhase({ kind: "password", refused: true });
          return;
        }
        done((await adoptSession(signal)) ? "person" : "address");
      };
      await tryOnce();
    };

    void run(attempt ?? credentials()).catch(ignoreAbort);
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [attempt, signedIn, onSignedIn, credentials]);

  if (phase.kind === "done") {
    return phase.how === "person" ? (
      <StepText data-testid="desktop-ready">
        {login === null ? (
          t("wizard.desktop.ready")
        ) : (
          <Rich k="wizard.desktop.readyNamed" vars={{ name: <span className="numeric text-ink">{login}</span> }} />
        )}
      </StepText>
    ) : (
      <Alert variant="default" data-testid="desktop-spare-key">
        <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <AlertTitle>{t("wizard.desktop.spare.title")}</AlertTitle>
        <AlertDescription>
          <p>{t(SPARE_REASON[phase.how])}</p>
        </AlertDescription>
      </Alert>
    );
  }

  if (phase.kind === "password") {
    return (
      <PasswordAgain
        login={login}
        refused={phase.refused}
        onSubmit={(credentials) => {
          setPhase({ kind: "asking" });
          setAttempt(credentials);
        }}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <span className="inline-flex items-center gap-2 text-sm text-muted">
        <Spinner size={14} />
        {login === null ? t("wizard.desktop.signingIn") : t("wizard.desktop.signingInNamed", { name: login })}
      </span>
      {phase.kind === "starting" && (
        <Alert variant="default" role="status">
          <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.desktop.starting.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.desktop.starting.body")}</p>
          </AlertDescription>
        </Alert>
      )}
      {/* Not part of the status: a live region would read out every point
          scored. It stays open once the wait is over, until it is closed. */}
      {game.offer}
      {game.panel !== null && <div className="w-full max-w-[560px]">{game.panel}</div>}
    </div>
  );
}

const SPARE_REASON = {
  plainHttp: "wizard.desktop.spare.plainHttp",
  noLogin: "wizard.desktop.spare.noLogin",
  address: "wizard.desktop.spare.address",
} as const;

function ignoreAbort(error: unknown): void {
  if (!isAbort(error)) throw error;
}

/* A tab reloaded on this step has forgotten the password, and a refused one
 * is asked again: the name too when this tab never knew it. */
function PasswordAgain({
  login,
  refused,
  onSubmit,
}: {
  login: string | null;
  refused: boolean;
  onSubmit: (credentials: Credentials) => void;
}) {
  const t = useT();
  const [name, setName] = React.useState(login ?? "");
  const [password, setPassword] = React.useState("");
  const nameId = React.useId();
  const passwordId = React.useId();
  const errorId = React.useId();
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        if (name.trim() === "" || password === "") return;
        onSubmit({ login: name.trim(), password });
      }}
    >
      <StepText>{t(refused ? "wizard.desktop.password.refused" : "wizard.desktop.password.again")}</StepText>
      <FieldGroup>
        {login === null && (
          <Field>
            <FieldLabel htmlFor={nameId}>{t("wizard.desktop.password.name")}</FieldLabel>
            <Input
              id={nameId}
              name="username"
              autoComplete="username"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
        )}
        <Field data-invalid={refused ? "true" : undefined}>
          <FieldLabel htmlFor={passwordId}>{t("wizard.desktop.password.label")}</FieldLabel>
          <Input
            id={passwordId}
            name="current-password"
            type="password"
            autoComplete="current-password"
            autoFocus
            aria-invalid={refused}
            aria-describedby={refused ? errorId : undefined}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
          {refused && <FieldError id={errorId}>{t("wizard.desktop.password.wrong")}</FieldError>}
        </Field>
      </FieldGroup>
      <div>
        <Button type="submit" size="sm" disabled={name.trim() === "" || password === ""}>
          <HugeiconsIcon icon={Login01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("wizard.desktop.password.submit")}
        </Button>
      </div>
    </form>
  );
}
