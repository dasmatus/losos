import * as React from "react";
import {
  ApiError,
  deleteAccountImage,
  getProfile,
  isAbort,
  isUnauthorized,
  loadMe,
  postAccountProfile,
  putAccountImage,
  type Accent,
  type Profile,
  type ProfileImage,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import type { Outcome } from "./use-account";

/* The signed-in person's own profile (`GET /api/profiles/<name>`), for the
 * Profile section of Your account. Every change is its own request and
 * answers with the profile as it now is; /api/me is asked again after one,
 * so the picture in the top bar follows. */

export interface ProfileData {
  profile: Profile | null;
  busy: boolean;
  saveDetails: (details: { pronouns?: string; about?: string; accent?: Accent | null }) => Promise<Outcome>;
  upload: (kind: ProfileImage, picture: Blob) => Promise<Outcome>;
  remove: (kind: ProfileImage) => Promise<Outcome>;
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

export function useProfile(name: string | null): ProfileData {
  const [profile, setProfile] = React.useState<Profile | null>(null);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (name === null) return;
    const controller = new AbortController();
    getProfile(name, { signal: controller.signal })
      .then((p) => {
        if (!controller.signal.aborted) setProfile(p);
      })
      .catch((error: unknown) => {
        if (isAbort(error) || isUnauthorized(error)) return;
        // An older lososd has no profiles: the section stays away.
        setProfile(null);
      });
    return () => controller.abort();
  }, [name]);

  const attempt = React.useCallback(
    async (send: () => Promise<Profile>, done: string): Promise<Outcome> => {
      setBusy(true);
      try {
        setProfile(await send());
        toast.done(done);
        void loadMe().catch(() => null);
        return { ok: true };
      } catch (error) {
        if (isUnauthorized(error)) return { ok: false, message: "" };
        const said = messageOf(error);
        return { ok: false, message: said.length > 0 ? said : t("profile.failed") };
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const saveDetails = React.useCallback(
    (details: { pronouns?: string; about?: string; accent?: Accent | null }) =>
      attempt(() => postAccountProfile(details), t("profile.saved")),
    [attempt],
  );
  const upload = React.useCallback(
    (kind: ProfileImage, picture: Blob) =>
      attempt(() => putAccountImage(kind, picture), t(`profile.${kind}.saved`)),
    [attempt],
  );
  const remove = React.useCallback(
    (kind: ProfileImage) => attempt(() => deleteAccountImage(kind), t(`profile.${kind}.removed`)),
    [attempt],
  );

  return { profile, busy, saveDetails, upload, remove };
}
