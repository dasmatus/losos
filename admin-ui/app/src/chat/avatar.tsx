import * as React from "react";
import type { MatrixClient } from "matrix-js-sdk";
import { PersonAvatar, type AvatarSize } from "@/components/person";
import { ACCENTS, getProfiles, type Profile } from "@/lib/api";
import { cn } from "@/lib/utils";
import { mediaUrl } from "./media";
import { hue } from "./model";

/* Everyone on this box has one profile (backend/src/profiles.rs): their
 * picture and colour are the ones every other page shows, so chat draws
 * them the same. Rooms and people from other servers keep the Matrix
 * avatar, on a colour picked by the ID's hash. */

let profiles: Promise<Map<string, Profile>> | null = null;

function boxProfiles(): Promise<Map<string, Profile>> {
  profiles ??= getProfiles()
    .then(({ people }) => new Map(people.map((p) => [p.name, p])))
    .catch(() => {
      profiles = null;
      return new Map<string, Profile>();
    });
  return profiles;
}

/** The profile of a Matrix ID on this box's own server, once loaded. */
function useProfile(client: MatrixClient, id: string): Profile | null {
  const [found, setFound] = React.useState<Profile | null>(null);
  React.useEffect(() => {
    setFound(null);
    const match = /^@([^:]+):(.+)$/.exec(id);
    if (match === null || match[2] !== client.getDomain()) return;
    let live = true;
    void boxProfiles().then((all) => {
      if (live) setFound(all.get(match[1] ?? "") ?? null);
    });
    return () => {
      live = false;
    };
  }, [client, id]);
  return found;
}

export interface AvatarProps {
  client: MatrixClient;
  id: string;
  name: string;
  mxc?: string | null | undefined;
  size?: number;
  className?: string;
}

function sizeOf(px: number): AvatarSize {
  return px <= 24 ? "xs" : px <= 32 ? "sm" : px <= 40 ? "md" : "lg";
}

/** A person's or room's picture, or their initials on their colour. */
export function Avatar({ client, id, name, mxc, size = 32, className }: AvatarProps) {
  const profile = useProfile(client, id);
  const [url, setUrl] = React.useState<string | null>(null);
  React.useEffect(() => {
    setUrl(null);
    if (profile !== null || mxc === null || mxc === undefined || mxc === "") return;
    let live = true;
    mediaUrl(client, { url: mxc }, "image/png", Math.max(64, size * 2))
      .then((found) => {
        if (live) setUrl(found);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [client, mxc, size, profile]);

  const person =
    profile !== null
      ? { ...profile, displayName: name || profile.displayName }
      : { name: id, displayName: name.replace(/^[@#!]/u, ""), picture: url, accent: ACCENTS[hue(id) % ACCENTS.length] };
  return <PersonAvatar aria-hidden person={person} size={sizeOf(size)} className={cn(className)} />;
}
