import * as React from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useT } from "@/lib/i18n-react";
import { PaneHeader } from "@/screens/settings/rows";
import { goSignIn } from "./session";

/* `/chat/vouch?for=<box>&back=<page>`: another LosOS box asks who the
 * person is, so they can sign in to its LosOS Git with their chat address.
 * Asked, this box hands that page a Matrix OpenID token
 * (POST /api/chat/openid, backend/src/chat.rs), which the other box checks
 * with this one's chat server. It learns the person's chat address and
 * display name; the token opens nothing here. The page goes back only to an
 * https address on the box that asked. */

interface Asked {
  host: string;
  back: string;
}

/** A host name, with a port at most. */
const HOST = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*(:[0-9]{1,5})?$/i;

/** What the address asks, or null when it does not ask anything this page
 *  will answer. */
export function readAsked(search: string): Asked | null {
  const q = new URLSearchParams(search);
  const host = (q.get("for") ?? "").toLowerCase();
  const back = q.get("back") ?? "";
  if (!HOST.test(host)) return null;
  let url: URL;
  try {
    url = new URL(back);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.host !== host || url.username !== "" || url.password !== "") return null;
  url.hash = "";
  return { host, back: url.toString() };
}

interface Token {
  access_token: string;
  matrix_server_name: string;
  user_id: string;
  display_name: string;
}

export default function ChatVouch() {
  const t = useT();
  const asked = React.useMemo(() => readAsked(window.location.search), []);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  if (asked === null) {
    return (
      <div className="flex max-w-xl flex-col gap-4">
        <PaneHeader title={t("chat.vouch.title")} summary={t("chat.vouch.nothing")} />
      </div>
    );
  }

  const tell = (): void => {
    setBusy(true);
    setError(null);
    fetch("/api/chat/openid", { method: "POST", credentials: "same-origin", cache: "no-store" })
      .then(async (res) => {
        if (res.status === 401) {
          goSignIn();
          return;
        }
        const body = (await res.json().catch(() => ({}))) as Partial<Token> & { error?: string };
        if (!res.ok || typeof body.access_token !== "string" || typeof body.matrix_server_name !== "string") {
          throw new Error(body.error ?? t("chat.vouch.failed"));
        }
        const hash = new URLSearchParams({
          "chat-token": body.access_token,
          "chat-server": body.matrix_server_name,
          "chat-name": body.display_name ?? "",
        });
        window.location.assign(`${asked.back}#${hash.toString()}`);
      })
      .catch((e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      });
  };

  return (
    <div className="flex max-w-xl flex-col gap-4" data-vouch={asked.host}>
      <PaneHeader title={t("chat.vouch.title")} summary={t("chat.vouch.summary", { host: asked.host })} />
      <Card className="flex flex-col gap-4 p-4">
        <p className="text-[14px] leading-snug">{t("chat.vouch.body", { host: asked.host })}</p>
        {error !== null && (
          <Alert variant="crit">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <div className="flex flex-wrap justify-end gap-2">
          <Button variant="ghost" onClick={() => window.location.assign(asked.back)} disabled={busy}>
            {t("chat.vouch.no")}
          </Button>
          <Button onClick={tell} disabled={busy} data-vouch-yes>
            {t("chat.vouch.yes", { host: asked.host })}
          </Button>
        </div>
      </Card>
    </div>
  );
}
