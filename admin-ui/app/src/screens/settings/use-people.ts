import * as React from "react";
import {
  ApiError,
  getPeople,
  isAbort,
  isUnauthorized,
  postPerson,
  postPersonAccess,
  postPersonPassword,
  postPersonRemove,
  postPeopleRules,
  postRole,
  postRoleRemove,
  postPersonFactorsReset,
  clearPersonImage,
  type LoginRules,
  type NewPerson,
  type PeopleResponse,
  type ProfileImage,
  postPersonAge,
  type AgeMark,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { notPresent } from "./http";

/* The people who may sign in to this box (`GET /api/people`), for the Users
 * pane. Like Access tokens, nothing here is part of the box's configuration:
 * no Apply bar, every button is its own request, and nothing polls. */

export type PeopleState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: PeopleResponse };

/** What a change came to: done, or lososd's refusal in its own words.
 *  `status` is the refusal's HTTP status, when there was one. */
export type Outcome = { ok: true } | { ok: false; message: string; status?: number };

export interface PeopleData {
  state: PeopleState;
  busy: boolean;
  refresh: () => void;
  add: (person: NewPerson) => Promise<Outcome>;
  setAccess: (name: string, groups: string[]) => Promise<Outcome>;
  setPassword: (name: string, password: string) => Promise<Outcome>;
  remove: (name: string) => Promise<boolean>;
  setRules: (rules: LoginRules) => Promise<Outcome>;
  resetFactors: (name: string) => Promise<boolean>;
  /** Take a picture or banner off someone's profile. */
  takeDown: (name: string, kind: ProfileImage) => Promise<boolean>;
  /** Save a role of the box's own; `id` changes one already saved. */
  saveRole: (role: { id?: string; name: string; sections: string[] }) => Promise<Outcome>;
  removeRole: (id: string, name: string) => Promise<boolean>;
  /** Mark someone as an adult or a child for pictures, or neither. */
  setAge: (name: string, age: AgeMark["age"] | null) => Promise<boolean>;
}

function viewOf(reply: Partial<PeopleResponse>): PeopleResponse {
  return {
    standalone: reply.standalone === true,
    you: typeof reply.you === "string" ? reply.you : null,
    sections: Array.isArray(reply.sections) ? reply.sections : [],
    ...(reply.rules !== undefined ? { rules: reply.rules } : {}),
    ...(Array.isArray(reply.roles) ? { roles: reply.roles } : {}),
    people: Array.isArray(reply.people) ? reply.people : [],
  };
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

export function usePeople(enabled: boolean): PeopleData {
  const [state, setState] = React.useState<PeopleState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = viewOf(await getPeople({ signal: controller.signal }));
        if (!controller.signal.aborted) setState({ kind: "ready", view });
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState(notPresent(error) ? { kind: "unavailable" } : { kind: "failed", message: messageOf(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  /* One request, then a fresh list. A refusal comes back for the caller to
   * show beside the field or in the dialog it is about. */
  const attempt = React.useCallback(
    async (send: () => Promise<unknown>, fallback: string): Promise<Outcome> => {
      setBusy(true);
      try {
        await send();
        return { ok: true };
      } catch (error) {
        if (isUnauthorized(error)) return { ok: false, message: "" };
        const said = messageOf(error);
        const message = said.length > 0 ? said : fallback;
        return error instanceof ApiError ? { ok: false, message, status: error.status } : { ok: false, message };
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  const add = React.useCallback(
    async (person: NewPerson) => {
      const outcome = await attempt(() => postPerson(person), t("users.form.failed"));
      if (outcome.ok) toast.done(t("users.added", { name: person.name }));
      return outcome;
    },
    [attempt],
  );

  const setAccess = React.useCallback(
    async (name: string, groups: string[]) => {
      const outcome = await attempt(
        () => postPersonAccess(name, groups),
        t("users.accessFailed", { name }),
      );
      if (outcome.ok) toast.done(t("users.accessSaved", { name }));
      return outcome;
    },
    [attempt],
  );

  const setPassword = React.useCallback(
    async (name: string, password: string) => {
      const outcome = await attempt(
        () => postPersonPassword(name, password),
        t("users.form.failed"),
      );
      if (outcome.ok) toast.done(t("users.passwordSaved", { name }));
      return outcome;
    },
    [attempt],
  );

  const remove = React.useCallback(
    async (name: string) => {
      const outcome = await attempt(() => postPersonRemove(name), t("users.removeFailed", { name }));
      if (outcome.ok) toast.done(t("users.removed", { name }));
      else if (outcome.message.length > 0)
        toast.error(t("users.removeFailed", { name }), outcome.message, { help: "sign-in-and-spare-key" });
      return outcome.ok;
    },
    [attempt],
  );

  const setRules = React.useCallback(
    async (rules: LoginRules) => {
      const outcome = await attempt(() => postPeopleRules(rules), t("users.rules.failed"));
      if (outcome.ok) toast.done(t("users.rules.saved"));
      return outcome;
    },
    [attempt],
  );

  const resetFactors = React.useCallback(
    async (name: string) => {
      const outcome = await attempt(
        () => postPersonFactorsReset(name),
        t("users.factorsResetFailed", { name }),
      );
      if (outcome.ok) toast.done(t("users.factorsReset", { name }));
      else if (outcome.message.length > 0)
        toast.error(t("users.factorsResetFailed", { name }), outcome.message, { help: "sign-in-and-spare-key" });
      return outcome.ok;
    },
    [attempt],
  );

  const takeDown = React.useCallback(
    async (name: string, kind: ProfileImage) => {
      const outcome = await attempt(() => clearPersonImage(name, kind), t("profile.failed"));
      if (outcome.ok) toast.done(t("profile.users.takenDown", { name }));
      else if (outcome.message.length > 0) toast.error(t("profile.failed"), outcome.message, { help: "sign-in-and-spare-key" });
      return outcome.ok;
    },
    [attempt],
  );

  const saveRole = React.useCallback(
    async (role: { id?: string; name: string; sections: string[] }) => {
      const outcome = await attempt(() => postRole(role), t("roles.custom.failed"));
      if (outcome.ok) toast.done(t("roles.custom.saved", { name: role.name.trim() }));
      return outcome;
    },
    [attempt],
  );

  const removeRole = React.useCallback(
    async (id: string, name: string) => {
      const outcome = await attempt(() => postRoleRemove(id), t("roles.custom.failed"));
      if (outcome.ok) toast.done(t("roles.custom.removed", { name }));
      else if (outcome.message.length > 0)
        toast.error(t("roles.custom.failed"), outcome.message, { help: "sign-in-and-spare-key" });
      return outcome.ok;
    },
    [attempt],
  );

  const setAge = React.useCallback(
    async (name: string, age: AgeMark["age"] | null) => {
      const outcome = await attempt(() => postPersonAge(name, age), t("pictures.age.failed"));
      if (!outcome.ok && outcome.message.length > 0) toast.error(t("pictures.age.failed"), outcome.message, { help: "sign-in-and-spare-key" });
      return outcome.ok;
    },
    [attempt],
  );

  return {
    state,
    busy,
    refresh,
    add,
    setAccess,
    setPassword,
    remove,
    setRules,
    resetFactors,
    takeDown,
    saveRole,
    removeRole,
    setAge,
  };
}
