import * as React from "react";
import type { Accent } from "@/lib/api";
import { cn } from "@/lib/utils";

/* A person as every page draws them: their picture, or their initials on
 * their colour, and the card with their banner that Your account and the
 * apps show (backend/src/profiles.rs).
 *
 * The colour is a `data-accent` attribute that index.css turns into the
 * `--person` and `--person-wash` tokens, never a style, so it holds under
 * the admin page's `style-src 'self'`. A picture that does not load (the
 * spare key's tab, which carries no cookie for an <img>) falls back to the
 * initials without a broken-image icon. */

export interface PersonLike {
  name: string;
  displayName: string;
  picture?: string | null | undefined;
  accent?: Accent | undefined;
}

const SIZES = {
  xs: "size-6 text-[10px]",
  sm: "size-8 text-xs",
  md: "size-10 text-sm",
  lg: "size-16 text-xl",
  xl: "size-24 text-3xl",
} as const;

export type AvatarSize = keyof typeof SIZES;

/** Up to two letters: the first of the first and last words. */
export function initials(displayName: string | undefined, name: string | undefined): string {
  const words = ((displayName ?? "").trim() || (name ?? "")).split(/\s+/u).filter((w) => w.length > 0);
  const first = [...(words[0] ?? "?")][0] ?? "?";
  const last = words.length > 1 ? ([...(words[words.length - 1] ?? "")][0] ?? "") : "";
  return (first + last).toLocaleUpperCase();
}

export function PersonAvatar({
  person,
  size = "md",
  className,
  ...props
}: {
  person: PersonLike;
  size?: AvatarSize;
} & Omit<React.ComponentPropsWithoutRef<"span">, "children">) {
  const picture = person.picture ?? null;
  const [failed, setFailed] = React.useState<string | null>(null);
  const showPicture = picture !== null && failed !== picture;
  return (
    <span
      data-accent={person.accent}
      data-slot="avatar"
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full",
        "bg-person-wash font-medium text-person select-none",
        SIZES[size],
        className,
      )}
      {...props}
    >
      {showPicture ? (
        <img
          src={picture}
          alt=""
          draggable={false}
          className="size-full object-cover"
          onError={() => setFailed(picture)}
        />
      ) : (
        <span aria-hidden="true">{initials(person.displayName, person.name)}</span>
      )}
    </span>
  );
}

export interface ProfileLike extends PersonLike {
  pronouns?: string;
  about?: string;
  banner?: string | null | undefined;
}

/* The card: banner, picture over its lower edge, then name, sign-in name,
 * pronouns and the line about them. The sign-in name is always shown
 * beside the picture, so a picture someone picked can never stand in for
 * who they are. `actions` sit to the right of the picture. */
export function ProfileCard({
  profile,
  actions,
  className,
  ...props
}: {
  profile: ProfileLike;
  actions?: React.ReactNode;
} & Omit<React.ComponentPropsWithoutRef<"div">, "children">) {
  const banner = profile.banner ?? null;
  const [failed, setFailed] = React.useState<string | null>(null);
  return (
    <div
      data-accent={profile.accent}
      data-slot="profile-card"
      className={cn("overflow-hidden rounded-card border border-line bg-surface shadow-card", className)}
      {...props}
    >
      <div className="relative aspect-[3/1] w-full bg-person-wash" data-slot="profile-banner">
        {banner !== null && failed !== banner && (
          <img
            src={banner}
            alt=""
            draggable={false}
            className="absolute inset-0 size-full object-cover"
            onError={() => setFailed(banner)}
          />
        )}
      </div>
      <div className="px-4 pb-4">
        <div className="flex items-end justify-between gap-3">
          <PersonAvatar
            person={profile}
            size="xl"
            className="-mt-12 ring-4 ring-surface max-sm:-mt-10 max-sm:size-20"
          />
          {actions !== undefined && <div className="flex flex-wrap justify-end gap-2 pt-3">{actions}</div>}
        </div>
        <div className="mt-2 min-w-0">
          <p className="truncate text-lg leading-snug font-medium text-ink" data-testid="profile-card-name">
            {profile.displayName.trim() || profile.name}
          </p>
          <p className="truncate text-[13px] leading-snug text-muted" data-testid="profile-card-handle">
            @{profile.name}
            {(profile.pronouns ?? "").length > 0 && ` · ${profile.pronouns}`}
          </p>
          {(profile.about ?? "").length > 0 && (
            <p className="mt-2 text-sm leading-snug text-ink" data-testid="profile-card-about">
              {profile.about}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
