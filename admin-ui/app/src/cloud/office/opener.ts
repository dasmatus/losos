import { Doc01Icon, FileImportIcon, PresentationBarChart01Icon } from "@hugeicons/core-free-icons";
import { registerOpener, registeringOpener } from "../openers";
import { officeTypes, opensInOffice } from "./api";
import { isMarkdown } from "./markdown/new";
import { isPaper } from "./paper/new";

/* Native LosOS Office capabilities: legacy formats are imported; native
 * .paper/.slides/.md are handled by their own routes/openers. */
registeringOpener(
  officeTypes().then((types) =>
    registerOpener({
      id: "office",
      label: "office.open",
      icon: FileImportIcon,
      accepts: (entry) => {
        if (isMarkdown(entry.name) || isPaper(entry.name)) return false;
        return opensInOffice(entry.name, types);
      },
      href: (entry) => `/cloud/office/${entry.id}`,
    }),
  ),
);

/* Markdown is Office's own: open as a document or presentation. */
registerOpener({
  id: "office-markdown",
  label: "office.md.open",
  icon: PresentationBarChart01Icon,
  accepts: (entry) => isMarkdown(entry.name),
  href: (entry) => `/cloud/office/${entry.id}`,
});

