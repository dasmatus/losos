import { Doc01Icon } from "@hugeicons/core-free-icons";
import { registerOpener } from "../../openers";
import { isPaper } from "./new";

/* Documents and presentations of LosOS Office's own open in its editor.
 * Imported before the text editor's opener, so a document's name opens it
 * as it looks and "Edit together" on its source stays in the selection
 * bar. */
registerOpener({
  id: "office-paper",
  label: "paper.open",
  icon: Doc01Icon,
  accepts: (entry) => isPaper(entry.name),
  href: (entry) => `/cloud/office/${entry.id}`,
});
