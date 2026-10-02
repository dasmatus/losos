import { defineMessages } from "./define";

/* The app catalogue (lib/apps.ts): tile names, their tooltips, the notice
 * under a short grid, and the figures lib/apps.ts formats. App names are
 * translated — they are words on a tile, not product names. */
export default defineMessages({
  // ── Catalogue: names ────────────────────────────────────────────────────
  "apps.files.name": { en: "Files", sk: "Súbory", de: "Dateien" },
  "apps.photos.name": { en: "Photos", sk: "Fotky", de: "Fotos" },
  "apps.calendar.name": { en: "Calendar", sk: "Kalendár", de: "Kalender" },
  "apps.contacts.name": { en: "Contacts", sk: "Kontakty", de: "Kontakte" },
  "apps.notes.name": { en: "Notes", sk: "Poznámky", de: "Notizen" },
  "apps.tasks.name": { en: "Tasks", sk: "Úlohy", de: "Aufgaben" },
  "apps.mail.name": { en: "Mail", sk: "Pošta", de: "E-Mail" },
  "apps.music.name": { en: "Music", sk: "Hudba", de: "Musik" },
  "apps.code.name": { en: "Code", sk: "Kód", de: "Code" },
  "apps.settings.name": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },

  // ── Catalogue: tooltips ─────────────────────────────────────────────────
  "apps.files.note": {
    en: "Everything you keep here",
    sk: "Všetko, čo tu máte",
    de: "Alles, was du hier ablegst",
  },
  "apps.photos.note": { en: "Pictures and albums", sk: "Obrázky a albumy", de: "Bilder und Alben" },
  "apps.calendar.note": {
    en: "Dates and reminders",
    sk: "Termíny a pripomienky",
    de: "Termine und Erinnerungen",
  },
  "apps.contacts.note": { en: "Names and addresses", sk: "Mená a adresy", de: "Namen und Adressen" },
  "apps.notes.note": {
    en: "Written down, kept here",
    sk: "Zapísané a uložené tu",
    de: "Aufgeschrieben, hier aufbewahrt",
  },
  "apps.tasks.note": { en: "Lists and due dates", sk: "Zoznamy a termíny", de: "Listen und Fälligkeiten" },
  "apps.mail.note": { en: "Your mail accounts", sk: "Vaše e-mailové účty", de: "Deine Mailkonten" },
  "apps.music.note": { en: "Your own library", sk: "Vaša vlastná knižnica", de: "Deine eigene Sammlung" },
  "apps.code.note": { en: "Your repositories", sk: "Vaše repozitáre", de: "Deine Repositorys" },
  "apps.settings.note": { en: "Run this box", sk: "Správa tohto zariadenia", de: "Diese Box verwalten" },

  // ── Why the grid is short ───────────────────────────────────────────────
  "apps.notice.both": {
    en: "Your files and your code are served under this box's own name, not from this page.",
    sk: "Vaše súbory a váš kód sa poskytujú pod vlastným názvom tohto zariadenia, nie z tejto stránky.",
    de: "Deine Dateien und dein Code werden unter dem eigenen Namen dieser Box ausgeliefert, nicht von dieser Seite.",
  },
  "apps.notice.files": {
    en: "Your files are served under this box's own name, not from this page.",
    sk: "Vaše súbory sa poskytujú pod vlastným názvom tohto zariadenia, nie z tejto stránky.",
    de: "Deine Dateien werden unter dem eigenen Namen dieser Box ausgeliefert, nicht von dieser Seite.",
  },
  "apps.notice.code": {
    en: "Your code is served under this box's own name, not from this page.",
    sk: "Váš kód sa poskytuje pod vlastným názvom tohto zariadenia, nie z tejto stránky.",
    de: "Dein Code wird unter dem eigenen Namen dieser Box ausgeliefert, nicht von dieser Seite.",
  },

  // ── Figures ─────────────────────────────────────────────────────────────
  "apps.format.unknown": { en: "unknown", sk: "neznáme", de: "unbekannt" },
  "apps.format.bytes": {
    en: "{count} bytes",
    sk: { one: "{count} bajt", few: "{count} bajty", many: "{count} bajtov", other: "{count} bajtov" },
    de: "{count} Byte",
  },
  "apps.uptime.days": { en: "{days} d", sk: "{days} d", de: "{days} Tg." },
  "apps.uptime.daysHours": {
    en: "{days} d {hours} h",
    sk: "{days} d {hours} h",
    de: "{days} Tg. {hours} Std.",
  },
  "apps.uptime.hours": { en: "{hours} h", sk: "{hours} h", de: "{hours} Std." },
  "apps.uptime.hoursMinutes": {
    en: "{hours} h {minutes} min",
    sk: "{hours} h {minutes} min",
    de: "{hours} Std. {minutes} Min.",
  },
  "apps.uptime.minutes": { en: "{minutes} min", sk: "{minutes} min", de: "{minutes} Min." },
  "apps.uptime.justNow": { en: "just now", sk: "práve teraz", de: "gerade eben" },
});
