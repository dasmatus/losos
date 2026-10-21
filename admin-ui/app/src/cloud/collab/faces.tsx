import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { Presence } from "./editor";
import { initials, RING, STRONG, WASH } from "./people";

/* Who has the file open: a face per person, ringed in the colour their
 * cursor has in the text, the way they appear beside each other on every
 * box. The same person in two tabs is one face. */

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
          return (
            <li key={user.address}>
              <span
                title={`${label}\n${user.address}`}
                className={cn(
                  "grid size-8 place-items-center rounded-full text-[12px] font-semibold select-none",
                  "border-2 border-surface ring-2",
                  WASH[user.accent],
                  STRONG[user.accent],
                  RING[user.accent],
                )}
                data-face={user.address}
              >
                <span aria-hidden="true">{initials(user.name)}</span>
                <span className="sr-only">{label}</span>
              </span>
            </li>
          );
        })}
      </ul>
      {more > 0 && <span className="ml-2 text-[12px] text-muted">+{more}</span>}
    </div>
  );
}
