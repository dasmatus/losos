import { BookOpen01Icon, Globe02Icon, NoteIcon } from "@hugeicons/core-free-icons";
import { registerFolderAction, registerOpener } from "../openers";
import { PublishDialog } from "./publish";
import { reloadSites, siteFor, sites } from "./sites";

/* Notes in LosOS Cloud's files: a Markdown file opens as a note in its
 * vault, a folder opens as a vault, and a folder can be published as a
 * knowledge base while the box shares and is in a mesh. Imported before
 * the editor's opener, so a note's name opens Notes and "Edit together"
 * stays one click away in the selection bar. */

registerOpener({
  id: "notes",
  label: "notes.open",
  icon: NoteIcon,
  accepts: (entry) => /\.(md|markdown)$/i.test(entry.name),
  href: (entry) => `/cloud/notes/open?file=${encodeURIComponent(entry.path)}`,
});

registerFolderAction({
  id: "notes-open",
  label: () => "notes.openVault",
  icon: BookOpen01Icon,
  href: (folder) => `/notes?vault=${encodeURIComponent(folder.path)}`,
});

// Asked once at start, so the first right click already knows the answer.
void reloadSites();

registerFolderAction({
  id: "kb-publish",
  label: (folder) => (siteFor(folder.id) === undefined ? "notes.publish.action" : "notes.publish.manage"),
  icon: Globe02Icon,
  // Hidden on a box that does not share or is in no mesh: the sites are
  // off there, and a menu item that can only refuse is noise.
  offers: () => sites().available,
  Dialog: PublishDialog,
});
