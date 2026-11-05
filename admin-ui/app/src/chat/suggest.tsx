import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { SentIcon } from "@hugeicons/core-free-icons";
import type { Room } from "matrix-js-sdk";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { getJson } from "./session";

/* The tag's suggestion, above the composer: the box's one line on whether
 * this room asks for an assistant just now — what looks like questions a
 * person asked with none answered (backend/http.rs `get_tag_suggest`).
 *
 * The pill proposes the built-in assistant only, and only while the box
 * itself does: `suggest` false draws nothing. "No thanks" puts it away
 * for this room until the room's next message moves the tail again. */

export interface Suggestion {
  suggest: boolean;
  assistant: string;
}

export function SuggestPill({ room, seed }: { room: Room; seed: (text: string) => void }) {
  const t = useT();
  const [state, setState] = React.useState<Suggestion | null>(null);
  const [dismissed, setDismissed] = React.useState(true);

  React.useEffect(() => {
    let live = true;
    let timer: number | ReturnType<typeof setTimeout> | null = null;
    const ask = (): void => {
      getJson<Suggestion>(`/api/tag/suggest?room=${encodeURIComponent(room.roomId)}`)
        .then((answer) => {
          if (live) setState(answer);
        })
        .catch(() => {});
      timer = setTimeout(ask, 15_000);
    };
    ask();
    return () => {
      live = false;
      if (timer !== null) clearTimeout(timer);
    };
  }, [room.roomId, room.getLastActiveTimestamp()]);

  if (state === null || state.suggest !== true || dismissed) return null;
  const name = state.assistant.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  return (
    <div className="mb-1.5 flex flex-wrap items-center gap-2 rounded-lg border border-line bg-surface px-3 py-2 text-[12.5px]">
      <span className="min-w-0 flex-1 text-muted">{t("chat.suggest.title")}</span>
      <Button
        size="sm"
        className="gap-1.5"
        onClick={() => {
          setDismissed(true);
          seed(`@LosOS `);
        }}
      >
        <HugeiconsIcon icon={SentIcon} className="size-4" aria-hidden="true" />
        {t("chat.suggest.ask", { name })}
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setDismissed(true)}>
        {t("chat.suggest.dismiss")}
      </Button>
    </div>
  );
}
