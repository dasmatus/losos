import * as React from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort } from "./api";
import { openersFor } from "./openers";

/* /cloud/open/<id>: a file in the app that edits it. Mail, Chat and the
 * rest link here (lib/intents.ts openFileHref) rather than each keeping a
 * list of which app takes which file: the openers are registered on this
 * page, so this page decides. With no opener the file is shown in its
 * folder, marked, as /cloud/files?fileid= does. */
export function OpenFile() {
  const t = useT();
  const navigate = useNavigate();
  const { file } = useParams();
  const [name, setName] = React.useState("");

  React.useEffect(() => {
    const id = Number(file);
    if (!Number.isSafeInteger(id) || id <= 0) {
      navigate("/files", { replace: true });
      return;
    }
    const ctl = new AbortController();
    cloud.byId(id, ctl.signal).then(
      (entry) => {
        setName(entry.name);
        const opener = openersFor(entry)[0];
        if (opener === undefined) {
          navigate(`/files?fileid=${entry.id}`, { replace: true });
          return;
        }
        const href = opener.href(entry);
        // This page's own routes stay in the router; another app's page loads.
        if (href.startsWith("/cloud/")) navigate(href.slice("/cloud".length), { replace: true });
        else window.location.replace(href);
      },
      (err: unknown) => {
        if (!isAbort(err)) navigate("/files", { replace: true });
      },
    );
    return () => ctl.abort();
  }, [file, navigate]);

  return (
    <div className="flex min-h-[40vh] items-center justify-center" data-slot="cloud-open">
      <Spinner label={name === "" ? t("together.open") : t("together.opening", { name })} />
    </div>
  );
}
