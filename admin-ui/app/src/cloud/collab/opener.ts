import { UserMultiple02Icon } from "@hugeicons/core-free-icons";
import { registerOpener } from "../openers";
import { editable, editHref } from "./api";

/* Text files open in the editor, where everyone in them types together. */
registerOpener({
  id: "collab",
  label: "collab.open",
  icon: UserMultiple02Icon,
  accepts: (entry) => editable(entry.name, entry.mime),
  href: (entry) => editHref(entry.id),
});
