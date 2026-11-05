import { defineMessages } from "./define";

/* LosOS Tag (src/screens/settings/pane-tag.tsx, src/chat/): the assistant
 * people tag in LosOS Chat, the assistants and skills anyone makes, and
 * the confidentiality levels that decide what a tag reads. */
export default defineMessages({
  "tag.loading": {
    en: "Reading the box's tag",
    sk: "Načítava sa značka tohto boxu",
    de: "Die Box-Einblendungen werden gelesen",
  },
  "tag.failed": {
    en: "The tag could not be read just now",
    sk: "Značku sa teraz nepodarilo prečítať",
    de: "Die Einblendung konnte gerade nicht gelesen werden",
  },
  "tag.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  "tag.add": { en: "Add", sk: "Pridať", de: "Hinzufügen" },
  "tag.delete": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },

  "tag.clearance.title": {
    en: "Your own clearance",
    sk: "Vaše vlastné oprávnenie",
    de: "Deine eigene Freigabe",
  },
  "tag.clearance.summary": {
    en: "What the tag reads as you, including the companion in your AI tools. The higher the level, the more private it may read.",
    sk: "Čo značka číta vo vašom mene, vrátane spoločníka vo vašich AI nástrojoch. Čím vyššia úroveň, tým súkromnejšie môže čítať.",
    de: "Was die Tag in deinem Namen liest, auch der Begleiter in deinen KI-Werkzeugen. Je höher die Stufe, desto Privateres darf sie lesen.",
  },
  "tag.clearance.row": { en: "Reads at", sk: "Číta do úrovne", de: "Liest bis" },

  "tag.levels.title": { en: "Levels", sk: "Úrovne", de: "Stufen" },
  "tag.levels.summary": {
    en: "The box's words for how private a piece of data is, least private first. An area carries one; a tag reads an area only when its clearance reaches the level.",
    sk: "Výrazy tohto boxu pre súkromie dát, od najmenej súkromných. Oblasť nesie jednu; značka číta oblasť len keď jej oprávnenie úroveň dosahuje.",
    de: "Die Wörter dieser Box dafür, wie privat Daten sind, am wenigsten privat zuerst. Ein Bereich trägt eine Stufe; eine Tag liest ihn nur, wenn ihre Freigabe sie erreicht.",
  },
  "tag.levels.name": {
    en: "level (letters, digits, -)",
    sk: "úroveň (písmená, číslice, -)",
    de: "Stufe (Buchstaben, Ziffern, -)",
  },
  "tag.levels.description": { en: "One line, optional", sk: "Jeden riadok, voliteľné", de: "Eine Zeile, optional" },
  "tag.levels.rank": {
    en: "rank {count, plural, =0 {least private} one {#} other {#}}",
    sk: "poradie {count, plural, =0 {najmenej súkromné} few {#} other {#}}",
    de: "Rang {count, plural, =0 {am wenigsten privat} other {#}}",
  },

  "tag.areas.title": { en: "Areas", sk: "Oblasti", de: "Bereiche" },
  "tag.areas.summary": {
    en: "One piece of the box's data, graded by a level: a folder in your files, a repository, a room, a mailbox, a journal unit. Yours to grade; an administrator grades the box's own.",
    sk: "Jedna dátová časť boxu s úrovňou: priečinok vo vašich súboroch, repozitár, miestnosť, poštový priečinok, jednotka žurnálu. Svoje označíte sami; správca označí tie boxové.",
    de: "Ein Stück der Box-Daten, mit einer Stufe bewertet: ein Ordner in deinen Dateien, ein Repository, ein Raum, ein Postfach, eine Journaleinheit. Deine bewertest du; die der Box bewertet der Administrator.",
  },
  "tag.areas.empty": {
    en: "Nothing is graded yet: everything the tag reads is at the least private level.",
    sk: "Nič ešte nie je označené: všetko, čo značka číta, je na najmenej súkromnej úrovni.",
    de: "Noch ist nichts bewertet: alles, was die Tag liest, liegt auf der am wenigsten privaten Stufe.",
  },
  "tag.areas.kind": { en: "which", sk: "čo", de: "Welche" },
  "tag.areas.ref": { en: "what (path, owner/repo, room, mailbox)", sk: "čo presne (cesta, vlastník/repo, miestnosť, priečinok)", de: "Was (Pfad, Eigentümer/Repo, Raum, Postfach)" },
  "tag.areas.owner": { en: "whose (blank for the box)", sk: "čie (prázdne pre box)", de: "Wessen (leer für die Box)" },
  "tag.areas.level": { en: "at which level", sk: "na ktorej úrovni", de: "Auf welcher Stufe" },

  "tag.assistants.title": { en: "Assistants", sk: "Assistenti", de: "Assistenten" },
  "tag.assistants.summary": {
    en: "The built-in tag, and your own assistants beside it: real chat participants you tag and that tag each other, each reading only up to its own clearance.",
    sk: "Vstavaná značka a vaši vlastní asistenti: skutoční účastníci chatu, ktorých označíte a ktorí sa označujú navzájom; každý číta len do vlastného oprávnenia.",
    de: "Die eingebaute Tag und deine eigenen Assistenten: echte Chat-Beteiligte, die du ansprichst und die einander ansprechen; jeder liest nur bis zu seiner Freigabe.",
  },
  "tag.assistants.slug": { en: "id (letters, digits, -)", sk: "id (písmená, číslice, -)", de: "Kennung (Buchstaben, Ziffern, -)" },
  "tag.assistants.display": { en: "name shown", sk: "zobrazené meno", de: "Angezeigter Name" },
  "tag.assistants.clearance": { en: "reads at", sk: "číta do", de: "Liest bis" },
  "tag.assistants.prompt": {
    en: "what it adds to every answer (optional)",
    sk: "čo pridáva ku každej odpovedi (voliteľné)",
    de: "Was es jeder Antwort hinzufügt (optional)",
  },
  "tag.assistants.on": { en: "ANSWERING", sk: "ODPOVEDÁ", de: "ANTWORTET" },
  "tag.assistants.off": { en: "PAUSED", sk: "ZASTAVENÝ", de: "PAUSIERT" },
  "tag.assistants.disable": { en: "Pause", sk: "Zastaviť", de: "Pause" },

  "tag.skills.title": { en: "Skills", sk: "Zručnosti", de: "Fähigkeiten" },
  "tag.skills.summary": {
    en: "Instructions an ask can take. The seeded ones come with the box; your own are yours to write, and none of them raises a clearance or grants a tool.",
    sk: "Pokyny, ktoré si dopyt môže vziať. Vstavané prichádzajú s boxom; vaše si píšete sami a žiadna nezvyšuje oprávnenie ani nedarí nástroj.",
    de: "Anweisungen, die eine Anfrage mitnimmt. Die mitgelieferten kommen mit der Box; deine schreibst du selbst, und keine hebt eine Freigabe oder vergibt ein Werkzeug.",
  },
  "tag.skills.slug": { en: "id (letters, digits, -)", sk: "id (písmená, číslice, -)", de: "Kennung (Buchstaben, Ziffern, -)" },
  "tag.skills.display": { en: "name shown", sk: "zobrazené meno", de: "Angezeigter Name" },
  "tag.skills.prompt": {
    en: "what it says (at most 2000 characters)",
    sk: "čo hovorí (najviac 2000 znakov)",
    de: "Was sie sagt (höchstens 2000 Zeichen)",
  },
  "tag.skills.builtin": { en: "LosOS", sk: "LosOS", de: "LosOS" },
  "tag.skills.own": { en: "yours", sk: "vaše", de: "Deine" },

  "tag.asks.title": { en: "Asks and conversations", sk: "Dopyty a rozhovory", de: "Anfragen und Gespräche" },
  "tag.asks.summary": {
    en: "Every ask on the queue, then the ones that answered, newest first.",
    sk: "Každý dopyt vo fronte, potom zodpovedané, najnovšie prednostne.",
    de: "Jede Anfrage in der Warteschlange, dann die beantworteten, neueste zuerst.",
  },
  "tag.asks.empty": {
    en: "Nothing asked yet.",
    sk: "Zatiaľ nič nebolo opýtané.",
    de: "Noch nichts gefragt.",
  },
  "tag.asks.done": { en: "answered", sk: "zodpovedané", de: "beantwortet" },
  "tag.jobs.row": { en: "on the queue", sk: "vo fronte", de: "In der Warteschlange" },

  // In the chat room, above the composer.
  "chat.suggest.title": {
    en: "Unanswered questions here — ask the tag?",
    sk: "Nezodpovedané otázky — opýtať sa značky?",
    de: "Offene Fragen hier — die Tag fragen?",
  },
  "chat.suggest.ask": {
    en: "Ask {name}",
    sk: "Opýtať {name}",
    de: "{name} fragen",
  },
  "chat.suggest.dismiss": {
    en: "No thanks",
    sk: "Nie, ďakujem",
    de: "Nein, danke",
  },
});
