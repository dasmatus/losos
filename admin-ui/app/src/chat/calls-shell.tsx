import * as React from "react";
import { CallsProvider } from "./calls";
import { type Signed, session, signedNow, signedOutListeners } from "./session";

/* Calls belong to the page, not to the chat screen: a call goes on while its
 * people look at Settings or Mail in the same document. App.tsx wraps the
 * screens in this on a document loaded at /chat/ (chatDocument), the only
 * one that can hold chat's session. The chat screen reports a refused
 * session; this only waits for one. */
export default function CallsShell({ children }: { children: React.ReactNode }) {
  const [signed, setSigned] = React.useState<Signed | null>(signedNow);

  React.useEffect(() => {
    let live = true;
    const settle = (): void => {
      session()
        .then((s) => {
          if (live) setSigned(s);
        })
        .catch(() => {
          if (live) setSigned(null);
        });
    };
    const gone = (): void => {
      setSigned(null);
      settle();
    };
    settle();
    signedOutListeners.add(gone);
    return () => {
      live = false;
      signedOutListeners.delete(gone);
    };
  }, []);

  return signed === null ? <>{children}</> : <CallsProvider client={signed.client}>{children}</CallsProvider>;
}
