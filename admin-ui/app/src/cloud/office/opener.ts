import { Doc01Icon, FileImportIcon, PresentationBarChart01Icon } from "@hugeicons/core-free-icons";
import { registerOpener, registeringOpener } from "../openers";
import { officeTypes, opensInOffice } from "./api";
import { isMarkdown } from "./markdown/new";
import { importsAs, officeFormats } from "./paper/convert";

/* Word, Excel, PowerPoint and OpenDocument files open in LosOS Office, for
 * the types the box's Collabora opens. A box without Office answers the
 * call with 404, and then nothing registers: its documents keep opening as
 * the browser shows them. */
const collabora = officeTypes().then((types) =>
  registerOpener({
    id: "office",
    label: "office.open",
    icon: Doc01Icon,
    accepts: (entry) => opensInOffice(entry.name, types),
    href: (entry) => `/cloud/office/${entry.id}`,
  }),
);
registeringOpener(collabora);

/* A Word or OpenDocument file also opens as a document of LosOS Office's
 * own, where the box's converter is on: it converts the file once, the
 * `.paper` lands beside the original, which stays as it was, and a later
 * open goes to that document while it is the newer of the two. Registered
 * after Collabora's answer, so the file's name keeps opening where it
 * did on a box that has both. */
registeringOpener(
  Promise.allSettled([collabora, officeFormats()]).then(([, formats]) => {
    if (formats.status !== "fulfilled") return;
    const imports = formats.value.imports;
    registerOpener({
      id: "office-paper-import",
      label: "paper.import.open",
      icon: FileImportIcon,
      accepts: (entry) => {
        const f = importsAs(entry.name);
        return f !== null && imports.includes(f);
      },
      href: (entry) => `/cloud/office/${entry.id}?as=paper`,
    });
  }),
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
