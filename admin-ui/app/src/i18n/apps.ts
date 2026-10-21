import { defineMessages } from "./define";

/* The app catalogue (lib/apps.ts): tile names, their tooltips, the notice
 * under a short grid, and the figures lib/apps.ts formats. App names are
 * translated — they are words on a tile, not product names. */
export default defineMessages({
  // ── Catalogue: names ────────────────────────────────────────────────────
  "apps.files.name": { en: "Files", sk: "Súbory", de: "Dateien" },
  "apps.photos.name": { en: "Photos", sk: "Fotky", de: "Fotos" },
  "apps.code.name": { en: "Code", sk: "Kód", de: "Code" },
  "apps.settings.name": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },

  // ── Catalogue: tooltips ─────────────────────────────────────────────────
  "apps.files.note": {
    en: "Everything you keep here",
    sk: "Všetko, čo tu máte",
    de: "Alles, was du hier ablegst",
  },
  "apps.photos.note": { en: "Pictures and albums", sk: "Obrázky a albumy", de: "Bilder und Alben" },
  "apps.code.note": { en: "Your repositories", sk: "Vaše repozitáre", de: "Deine Repositorys" },
  "apps.settings.note": { en: "Run this box", sk: "Správa tohto zariadenia", de: "Diese Box verwalten" },

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
