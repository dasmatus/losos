import { HugeiconsIcon } from "@hugeicons/react";
import { ViewOffSlashIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { mailHtmlUrl, type MailAddress } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { trustSender, usePictureSettings, useRevealed, useSenderShown } from "@/lib/pictures";

/* A message's HTML, with its pictures held back until the person shows
 * them. lososd serves the frame with `img-src 'none'` unless the page asks
 * with `?pictures=1`, so a hidden picture never loads, in an <img> or in
 * the message's CSS. A sender the person trusts, or (with `strangers`) one
 * on this box or in their contacts, shows at once. */
export function MailHtml({ id, from, title, own }: { id: string; from: MailAddress | undefined; title: string; own: boolean }) {
  const t = useT();
  usePictureSettings();
  const answer = useSenderShown(own ? null : (from?.email ?? null));
  const [revealed, reveal] = useRevealed(`mail:${id}`);
  const shown = own || revealed || answer?.shown === true;
  const name = from?.name ?? from?.email ?? "";
  return (
    <>
      {!shown && answer !== null && (
        <div
          data-mail-pictures-hidden
          className="mx-5 mb-2 flex flex-wrap items-center gap-x-3 gap-y-1 rounded-control border border-line bg-sunk px-3 py-2 text-[13px]"
        >
          <HugeiconsIcon icon={ViewOffSlashIcon} className="size-4 shrink-0 text-muted" aria-hidden />
          <span className="min-w-0 flex-1 text-muted">{t("pictures.mail.hidden")}</span>
          <Button variant="secondary" size="xs" onClick={reveal}>
            {t("pictures.mail.show")}
          </Button>
          {from !== undefined && (
            <Button variant="ghost" size="xs" onClick={() => void trustSender(from.email).catch(reveal)}>
              {t("pictures.hidden.trust", { name })}
            </Button>
          )}
        </div>
      )}
      {/* White, as senders lay their HTML out for it, and inset like a card
          so it does not read as a hole in the dark theme. */}
      <div className="mx-5 mb-5 flex min-h-80 flex-1 overflow-hidden rounded-card border border-line">
        <iframe
          key={shown ? "shown" : "hidden"}
          title={title}
          src={mailHtmlUrl(id, shown)}
          sandbox="allow-popups allow-popups-to-escape-sandbox"
          referrerPolicy="no-referrer"
          className="w-full flex-1 bg-white"
        />
      </div>
    </>
  );
}
