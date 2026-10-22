import { Doc01Icon, PresentationBarChart01Icon } from "@hugeicons/core-free-icons";
import { registerOpener } from "../openers";
import { officeTypes, opensInOffice } from "./api";
import { isMarkdown } from "./markdown/new";

/* Word, Excel, PowerPoint and OpenDocument files open in LosOS Office, for
 * the types the box's Collabora opens. A box without Office answers the
 * call with 404, and then nothing registers: its documents keep opening as
 * the browser shows them. */
officeTypes().then(
  (types) =>
    registerOpener({
      id: "office",
      label: "office.open",
      icon: Doc01Icon,
      accepts: (entry) => opensInOffice(entry.name, types),
      href: (entry) => `/cloud/office/${entry.id}`,
    }),
  () => {},
);

/* Markdown is Office's own, with or without Collabora: a .md opens in
 * Notes by default, and as a document or a presentation from here. */
registerOpener({
  id: "office-markdown",
  label: "office.md.open",
  icon: PresentationBarChart01Icon,
  accepts: (entry) => isMarkdown(entry.name),
  href: (entry) => `/cloud/office/${entry.id}`,
});
