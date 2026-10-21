import { useT } from "@/lib/i18n-react";
import { PersonAvatar } from "@/components/person";
import type { Presence } from "./editor";
import { boxOf, nameOf } from "./people";

/* Who has the file open: each person's picture, or their initials, ringed
 * in the colour their cursor has in the text (components/person.tsx). The
 * same person in two tabs is one face. */

const SHOWN = 5;

export function Faces({ people, me }: { people: readonly Presence[]; me: string }) {
  const t = useT();
  const seen = new Map<string, Presence>();
  for (const p of people) if (!seen.has(p.user.address)) seen.set(p.user.address, p);
  // This person first, then the others in the order they arrived.
  const all = [...seen.values()].sort((a, b) => Number(b.user.address === me) - Number(a.user.address === me));
  const shown = all.slice(0, SHOWN);
  const more = all.length - shown.length;
  return (
    <div className="flex items-center" role="group" aria-label={t("collab.here", { count: all.length })} data-faces>
      <ul className="flex -space-x-1.5">
        {shown.map(({ user }) => {
          const you = user.address === me;
          const label = you ? t("collab.you", { name: user.name }) : user.name;
          // A picture is a URL on its person's own box, so only someone on
          // this box shows theirs; the rest show initials on their colour.
          const picture = boxOf(user.address) === boxOf(me) ? (user.picture ?? null) : null;
          return (
            <li key={user.address} title={`${label}\n${user.address}`} data-face={user.address}>
              <PersonAvatar
                person={{ name: nameOf(user.address), displayName: user.name, accent: user.accent, picture }}
                size="sm"
                className="border-2 border-surface ring-2 ring-person"
              />
              <span className="sr-only">{label}</span>
            </li>
          );
        })}
      </ul>
      {more > 0 && <span className="ml-2 text-[12px] text-muted">+{more}</span>}
    </div>
  );
}
