import { defineMessages } from "./define";

/* The company edge's manage page (src/manage/), served by the edge at
 * /manage/: its key, the boxes it routes, and one box's LosOS Cloud, LosOS
 * Git and apps. The rows of a box reuse the Apps pane's own messages. */

export default defineMessages({
  "manage.title": { en: "LosOS edge", sk: "LosOS edge", de: "LosOS-Edge" },
  "manage.signOut": { en: "Sign out", sk: "Odhlásiť sa", de: "Abmelden" },

  // ── The key ───────────────────────────────────────────────────────────
  "manage.key.title": {
    en: "Manage the boxes behind this edge",
    sk: "Spravujte zariadenia za týmto edge",
    de: "Boxen hinter dieser Edge verwalten",
  },
  "manage.key.summary": {
    en: "Paste the edge's manage key. On the edge, losos-edge manage-key prints it.",
    sk: "Vložte kľúč na správu z edge. Na edge ho vypíše losos-edge manage-key.",
    de: "Fügen Sie den Verwaltungsschlüssel der Edge ein. Auf der Edge gibt losos-edge manage-key ihn aus.",
  },
  "manage.key.label": { en: "Manage key", sk: "Kľúč na správu", de: "Verwaltungsschlüssel" },
  "manage.key.submit": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "manage.key.shape": {
    en: "The key is 64 characters, 0 to 9 and a to f.",
    sk: "Kľúč má 64 znakov, 0 až 9 a a až f.",
    de: "Der Schlüssel hat 64 Zeichen, 0 bis 9 und a bis f.",
  },
  "manage.key.wrong": {
    en: "This edge does not know that key.",
    sk: "Tento edge taký kľúč nepozná.",
    de: "Diese Edge kennt diesen Schlüssel nicht.",
  },
  "manage.key.off": {
    en: "This edge does not manage boxes. Turn on losos.edge.manage.enable on the edge.",
    sk: "Tento edge nespravuje zariadenia. Na edge zapnite losos.edge.manage.enable.",
    de: "Diese Edge verwaltet keine Boxen. Schalten Sie auf der Edge losos.edge.manage.enable ein.",
  },
  "manage.key.failed": {
    en: "The edge did not answer: {message}",
    sk: "Edge neodpovedal: {message}",
    de: "Die Edge hat nicht geantwortet: {message}",
  },

  // ── The boxes ─────────────────────────────────────────────────────────
  "manage.boxes.title": { en: "Boxes", sk: "Zariadenia", de: "Boxen" },
  "manage.boxes.summary": {
    en: "The boxes this edge routes. You can manage a box once its owner switches it on, on the box's Mesh page.",
    sk: "Zariadenia, ktoré tento edge smeruje. Zariadenie môžete spravovať, keď to jeho vlastník zapne na stránke Mesh zariadenia.",
    de: "Die Boxen, die diese Edge weiterleitet. Sie können eine Box verwalten, sobald ihr Besitzer das auf der Mesh-Seite der Box einschaltet.",
  },
  "manage.boxes.loading": {
    en: "Loading the boxes…",
    sk: "Načítavam zariadenia…",
    de: "Lade die Boxen…",
  },
  "manage.boxes.failed": {
    en: "Could not load the boxes: {message}",
    sk: "Zariadenia sa nepodarilo načítať: {message}",
    de: "Die Boxen konnten nicht geladen werden: {message}",
  },
  "manage.boxes.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "manage.boxes.empty": {
    en: "No box has joined this edge yet.",
    sk: "K tomuto edge sa zatiaľ nepripojilo žiadne zariadenie.",
    de: "Noch keine Box hat sich mit dieser Edge verbunden.",
  },
  "manage.boxes.seen": {
    en: "Last heard from {ago}",
    sk: "Naposledy ozvalo {ago}",
    de: "Zuletzt gemeldet {ago}",
  },
  "manage.boxes.justNow": { en: "just now", sk: "práve teraz", de: "gerade eben" },
  "manage.boxes.minutes": {
    en: { one: "a minute ago", other: "{count} minutes ago" },
    sk: { one: "pred minútou", few: "pred {count} minútami", other: "pred {count} minútami" },
    de: { one: "vor einer Minute", other: "vor {count} Minuten" },
  },
  "manage.boxes.managed": { en: "Managed here", sk: "Spravované tu", de: "Hier verwaltet" },
  "manage.boxes.notManaged": {
    en: "Not switched on",
    sk: "Nezapnuté",
    de: "Nicht eingeschaltet",
  },
  "manage.boxes.open": { en: "Manage", sk: "Spravovať", de: "Verwalten" },
  "manage.boxes.notManagedDetail": {
    en: "Its owner has not let this edge manage it.",
    sk: "Vlastník tomuto edge nedovolil spravovať ho.",
    de: "Ihr Besitzer hat dieser Edge die Verwaltung nicht erlaubt.",
  },

  // ── One box ───────────────────────────────────────────────────────────
  "manage.box.back": { en: "All boxes", sk: "Všetky zariadenia", de: "Alle Boxen" },
  "manage.box.summary": {
    en: "LosOS Cloud, LosOS Git and the apps on this box. Everything else stays with its owner.",
    sk: "LosOS Cloud, LosOS Git a aplikácie na tomto zariadení. Všetko ostatné ostáva jeho vlastníkovi.",
    de: "LosOS Cloud, LosOS Git und die Apps auf dieser Box. Alles andere bleibt bei ihrem Besitzer.",
  },
  "manage.box.refused": {
    en: "The box refused this edge. Its owner may have switched management off.",
    sk: "Zariadenie tento edge odmietlo. Vlastník mohol správu vypnúť.",
    de: "Die Box hat diese Edge abgelehnt. Ihr Besitzer hat die Verwaltung vielleicht ausgeschaltet.",
  },

  // ── Its settings ──────────────────────────────────────────────────────
  "manage.site.loading": {
    en: "Loading the box's settings…",
    sk: "Načítavam nastavenia zariadenia…",
    de: "Lade die Einstellungen der Box…",
  },
  "manage.site.failed": {
    en: "Could not load the box's settings: {message}",
    sk: "Nastavenia zariadenia sa nepodarilo načítať: {message}",
    de: "Die Einstellungen der Box konnten nicht geladen werden: {message}",
  },
  "manage.site.none": {
    en: "This box has no settings of LosOS Cloud or LosOS Git to change from here.",
    sk: "Toto zariadenie nemá nastavenia LosOS Cloud ani LosOS Git, ktoré by sa dali zmeniť odtiaľto.",
    de: "Diese Box hat keine Einstellungen von LosOS Cloud oder LosOS Git, die sich von hier ändern lassen.",
  },
  "manage.site.applying": {
    en: "Applying the box's new settings",
    sk: "Používam nové nastavenia zariadenia",
    de: "Wende die neuen Einstellungen der Box an",
  },
  "manage.site.didNotStart": {
    en: "The box did not take the change",
    sk: "Zariadenie zmenu neprijalo",
    de: "Die Box hat die Änderung nicht angenommen",
  },
  "manage.site.done": {
    en: "The box runs its new settings",
    sk: "Zariadenie beží s novými nastaveniami",
    de: "Die Box läuft mit den neuen Einstellungen",
  },
  "manage.site.buildFailed": {
    en: "The box could not apply the change",
    sk: "Zariadenie nedokázalo zmenu použiť",
    de: "Die Box konnte die Änderung nicht anwenden",
  },
});
