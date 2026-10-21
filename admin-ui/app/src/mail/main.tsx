/* LosOS Mail's entry: mail/index.html loads this. Mail is its own page, like
 * the Lab, because it exists only while the box shares its storage: nginx
 * serves /mail/ then and not otherwise (modules/containers.nix), and the
 * admin SPA links it from the sidebar only when /api/me lists "mail" among
 * the shared apps. It shares the SPA's components, palette, theme and
 * language. */

import { createRoot } from "react-dom/client";
import { applyFavicon } from "@/lib/logo";
import { applyStoredTheme } from "@/lib/theme";
/* The same two stylesheets in the same order as src/main.tsx; see
 * src/lab/main.tsx for why the order matters to the shared CSS chunk. */
import "sonner/dist/styles.css";
import "@/styles/index.css";
import { MailRoot } from "./app";

applyStoredTheme();
applyFavicon();

const container = document.getElementById("mail");
if (container === null) throw new Error("#mail is missing from mail/index.html");

createRoot(container).render(<MailRoot />);
