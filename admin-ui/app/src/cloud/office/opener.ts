import { Doc01Icon } from "@hugeicons/core-free-icons";
import { registerOpener } from "../openers";
import { officeTypes, opensInOffice } from "./api";

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
