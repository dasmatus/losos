import { defineMessages } from "./define";

/* The homepage (screens/Home.tsx) and its app grid (components/AppGrid.tsx,
 * components/AppTile.tsx). */
export default defineMessages({
  // ── Header ──────────────────────────────────────────────────────────────
  "home.header.answering": { en: "Answering", sk: "Odpovedá", de: "Antwortet" },
  "home.header.notAnswering": { en: "Not answering", sk: "Neodpovedá", de: "Antwortet nicht" },
  "home.header.checking": { en: "Checking", sk: "Overuje sa", de: "Wird geprüft" },
  "home.header.up": { en: "up {uptime}", sk: "v prevádzke {uptime}", de: "läuft seit {uptime}" },
  "home.header.thisBox": { en: "this box", sk: "toto zariadenie", de: "diese Box" },

  // ── Sections ────────────────────────────────────────────────────────────
  "home.glance.label": { en: "At a glance", sk: "Na prvý pohľad", de: "Auf einen Blick" },

  // ── A change being applied ──────────────────────────────────────────────
  "home.applying.title": {
    en: "Applying a change to this box",
    sk: "Na zariadení sa uplatňuje zmena",
    de: "Eine Änderung wird auf diese Box angewendet",
  },
  "home.applying.short": {
    en: "Applying a change",
    sk: "Uplatňuje sa zmena",
    de: "Änderung wird angewendet",
  },

  // ── Tile attention notes ────────────────────────────────────────────────
  "home.attention.outOfRoom": { en: "Out of room", sk: "Nie je miesto", de: "Kein Platz mehr" },
  "home.attention.almostFull": { en: "Almost full", sk: "Takmer plné", de: "Fast voll" },
  "home.attention.changeFailed": {
    en: "A change failed",
    sk: "Zmena zlyhala",
    de: "Eine Änderung ist fehlgeschlagen",
  },

  // ── Storage strip ───────────────────────────────────────────────────────
  "home.storage.onThisBox": { en: "On this box", sk: "Na tomto zariadení", de: "Auf dieser Box" },
  "home.storage.lent": {
    en: "Lent to other boxes",
    sk: "Požičané iným zariadeniam",
    de: "An andere Boxen verliehen",
  },
  "home.storage.roomLeft": { en: "Room left", sk: "Voľné miesto", de: "Freier Platz" },
  "home.storage.sharing": {
    en: "Everything you keep here stays on this box. Room it is not using is lent to the mesh.",
    sk: "Všetko, čo tu máte, zostáva na tomto zariadení. Miesto, ktoré nevyužíva, požičiava sieti mesh.",
    de: "Alles, was du hier ablegst, bleibt auf dieser Box. Platz, den sie nicht braucht, wird an das Mesh verliehen.",
  },
  "home.storage.local": {
    en: "Everything you keep here stays on this box. Nothing is copied anywhere else.",
    sk: "Všetko, čo tu máte, zostáva na tomto zariadení. Nič sa nekopíruje nikam inam.",
    de: "Alles, was du hier ablegst, bleibt auf dieser Box. Nichts wird anderswohin kopiert.",
  },

  // ── Widget slot ─────────────────────────────────────────────────────────
  "home.widgets.placeholder": {
    en: "Widgets appear here.",
    sk: "Tu sa zobrazia widgety.",
    de: "Hier erscheinen Widgets.",
  },

  // ── App grid ────────────────────────────────────────────────────────────
  "home.grid.label": {
    en: "Apps on this box",
    sk: "Aplikácie na tomto zariadení",
    de: "Apps auf dieser Box",
  },
  "home.grid.measuring": {
    en: "Looking for the apps on this box.",
    sk: "Hľadajú sa aplikácie na tomto zariadení.",
    de: "Die Apps auf dieser Box werden gesucht.",
  },
  "home.setup.title": {
    en: "This box is not set up yet",
    sk: "Toto zariadenie ešte nie je nastavené",
    de: "Diese Box ist noch nicht eingerichtet",
  },
  "home.setup.body": {
    en: "Your files, photos, calendar and the rest appear here once you have finished. It takes a few minutes and you only do it once.",
    sk: "Vaše súbory, fotky, kalendár a všetko ostatné sa tu zobrazí, keď nastavenie dokončíte. Trvá to pár minút a robíte to len raz.",
    de: "Deine Dateien, Fotos, dein Kalender und alles andere erscheinen hier, sobald du fertig bist. Das dauert ein paar Minuten und du machst es nur einmal.",
  },
  "home.setup.action": { en: "Set up this box", sk: "Nastaviť zariadenie", de: "Box einrichten" },
});
