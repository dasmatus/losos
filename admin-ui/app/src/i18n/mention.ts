import { defineMessages } from "./define";

/* @ tags (components/mention-picker.tsx): people on this box and files in
 * LosOS Cloud, offered while typing @ in a text box. */
export default defineMessages({
  "mention.label": { en: "People and files", sk: "Ľudia a súbory", de: "Personen und Dateien" },
  "mention.people": { en: "People", sk: "Ľudia", de: "Personen" },
  "mention.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "mention.issues": { en: "Issues", sk: "Úlohy", de: "Issues" },
  "mention.start": {
    en: "Type a name or a file name",
    sk: "Napíšte meno alebo názov súboru",
    de: "Tippe einen Namen oder Dateinamen",
  },
  "mention.none": { en: "No people or files match", sk: "Nezhodujú sa žiadni ľudia ani súbory", de: "Keine passenden Personen oder Dateien" },
});
