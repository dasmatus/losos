import { defineMessages } from "./define";

/* The pins on the sidebar (components/app-sidebar.tsx, components/pins-dialog.tsx). */
export default defineMessages({
  "pins.title": { en: "Pinned", sk: "Pripnuté", de: "Angeheftet" },
  "pins.edit": { en: "Edit pins", sk: "Upraviť pripnuté", de: "Angeheftete bearbeiten" },
  "pins.empty": {
    en: "Pin apps, files, repositories and issues here.",
    sk: "Sem si pripnite aplikácie, súbory, repozitáre a úlohy.",
    de: "Hefte hier Apps, Dateien, Repositorys und Issues an.",
  },
  "pins.add": { en: "Add a pin", sk: "Pripnúť", de: "Etwas anheften" },
  "pins.pin": { en: "Pin {name}", sk: "Pripnúť {name}", de: "{name} anheften" },
  "pins.unpin": { en: "Unpin {name}", sk: "Odopnúť {name}", de: "{name} lösen" },
  "pins.pinShort": { en: "Pin", sk: "Pripnúť", de: "Anheften" },
  "pins.unpinShort": { en: "Unpin", sk: "Odopnúť", de: "Lösen" },
  "pins.up": { en: "Move {name} up", sk: "Posunúť {name} vyššie", de: "{name} nach oben" },
  "pins.down": { en: "Move {name} down", sk: "Posunúť {name} nižšie", de: "{name} nach unten" },
  "pins.failed": {
    en: "The box did not keep that change",
    sk: "Zariadenie túto zmenu neuložilo",
    de: "Die Box hat diese Änderung nicht gespeichert",
  },
  "pins.full": {
    en: "The sidebar holds {limit} pins. Unpin one to add another.",
    sk: "Bočný panel unesie {limit} pripnutých položiek. Ak chcete pridať ďalšiu, jednu odopnite.",
    de: "Die Seitenleiste fasst {limit} angeheftete Einträge. Löse einen, um einen weiteren hinzuzufügen.",
  },

  "pins.menu.pin": { en: "Pin to sidebar", sk: "Pripnúť na bočný panel", de: "An die Seitenleiste anheften" },
  "pins.menu.unpin": { en: "Unpin from sidebar", sk: "Odopnúť z bočného panela", de: "Von der Seitenleiste lösen" },
  "pins.menu.newTab": { en: "Open in new tab", sk: "Otvoriť na novej karte", de: "In neuem Tab öffnen" },

  "pins.dialog.title": { en: "Pins", sk: "Pripnuté položky", de: "Angeheftete Einträge" },
  "pins.dialog.body": {
    en: "Pinned items sit in the sidebar under the sections and open in their own app. This box keeps the list, so every browser you sign in from shows it.",
    sk: "Pripnuté položky sú v bočnom paneli pod sekciami a otvárajú sa vo svojej aplikácii. Zoznam si pamätá toto zariadenie, takže ho uvidíte v každom prehliadači, v ktorom sa prihlásite.",
    de: "Angeheftete Einträge stehen in der Seitenleiste unter den Bereichen und öffnen sich in ihrer eigenen App. Die Box speichert die Liste, deshalb zeigt sie jeder Browser, in dem du dich anmeldest.",
  },
  "pins.dialog.none": {
    en: "Nothing is pinned yet. Pick something below.",
    sk: "Zatiaľ nie je nič pripnuté. Vyberte niečo nižšie.",
    de: "Noch ist nichts angeheftet. Wähle unten etwas aus.",
  },
  "pins.dialog.done": { en: "Done", sk: "Hotovo", de: "Fertig" },
  "pins.dialog.choose": { en: "Pin from", sk: "Pripnúť z", de: "Anheften aus" },

  "pins.tab.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "pins.tab.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "pins.tab.repos": { en: "Repositories", sk: "Repozitáre", de: "Repositorys" },
  "pins.tab.issues": { en: "Issues", sk: "Úlohy", de: "Issues" },
  "pins.issues.search": { en: "Find an issue by its words or key", sk: "Nájsť úlohu podľa slov alebo kľúča", de: "Issue nach Wörtern oder Kürzel suchen" },
  "pins.issues.none": {
    en: "No open issue is given to you. Type to find any other.",
    sk: "Nemáte pridelenú žiadnu otvorenú úlohu. Píšte a nájdete akúkoľvek inú.",
    de: "Dir ist kein offenes Issue zugeteilt. Tipp, um ein anderes zu finden.",
  },
  "pins.issues.noMatch": { en: "No issue matches.", sk: "Žiadna úloha nezodpovedá.", de: "Kein Issue passt." },
  "pins.issues.noGit": {
    en: "Issues come from LosOS Git, which needs you signed in with your password.",
    sk: "Úlohy prichádzajú z LosOS Git, ktorý potrebuje prihlásenie heslom.",
    de: "Issues kommen aus LosOS Git, dafür musst du mit deinem Passwort angemeldet sein.",
  },

  "pins.apps.overview": { en: "On the Overview", sk: "Na prehľade", de: "In der Übersicht" },
  "pins.apps.installed": {
    en: "Installed from the search",
    sk: "Nainštalované z vyhľadávania",
    de: "Aus der Suche installiert",
  },
  "pins.apps.noneInstalled": {
    en: "Apps you install from the search on the Apps page show up here.",
    sk: "Aplikácie, ktoré nainštalujete z vyhľadávania na stránke Aplikácie, sa zobrazia tu.",
    de: "Apps, die du über die Suche auf der Seite Apps installierst, erscheinen hier.",
  },

  "pins.files.top": { en: "LosOS Cloud", sk: "LosOS Cloud", de: "LosOS Cloud" },
  "pins.files.up": { en: "Up one folder", sk: "O priečinok vyššie", de: "Einen Ordner höher" },
  "pins.files.open": { en: "Open {name}", sk: "Otvoriť {name}", de: "{name} öffnen" },
  "pins.files.empty": {
    en: "This folder is empty.",
    sk: "Tento priečinok je prázdny.",
    de: "Dieser Ordner ist leer.",
  },
  "pins.files.more": {
    en: "This folder holds more than the list shows. Open a folder inside it to pin from there.",
    sk: "V tomto priečinku je viac, než zoznam ukazuje. Otvorte priečinok v ňom a pripnite odtiaľ.",
    de: "Dieser Ordner enthält mehr, als die Liste zeigt. Öffne einen Ordner darin, um von dort anzuheften.",
  },
  "pins.files.signIn": {
    en: "This box lists your LosOS Cloud files once the owner's password is set again. The handbook's forgotten-password page shows how.",
    sk: "Zariadenie vám tu zobrazí súbory z LosOS Cloud, keď sa heslo vlastníka znova nastaví. Ako na to, ukazuje stránka príručky o zabudnutom hesle.",
    de: "Die Box zeigt hier deine Dateien aus LosOS Cloud, sobald das Passwort des Besitzers neu gesetzt ist. Wie das geht, steht auf der Handbuchseite zum vergessenen Passwort.",
  },
  "pins.files.notUp": {
    en: "LosOS Cloud did not answer. It may still be starting.",
    sk: "LosOS Cloud neodpovedá. Možno sa ešte spúšťa.",
    de: "LosOS Cloud antwortet nicht. Vielleicht startet es noch.",
  },

  "pins.repos.empty": {
    en: "You have no repositories on LosOS Git yet.",
    sk: "Na LosOS Git zatiaľ nemáte žiadne repozitáre.",
    de: "Du hast noch keine Repositorys auf LosOS Git.",
  },
  "pins.repos.noGit": {
    en: "This box does not run LosOS Git.",
    sk: "Na tomto zariadení LosOS Git nebeží.",
    de: "Auf dieser Box läuft kein LosOS Git.",
  },
  "pins.repos.notUp": {
    en: "LosOS Git did not answer. It may still be starting.",
    sk: "LosOS Git neodpovedá. Možno sa ešte spúšťa.",
    de: "LosOS Git antwortet nicht. Vielleicht startet es noch.",
  },
  "pins.repos.private": { en: "private", sk: "súkromný", de: "privat" },

  "pins.loading": { en: "Loading", sk: "Načítavame", de: "Wird geladen" },
  "pins.noAnswer": {
    en: "The box did not answer. Try again in a moment.",
    sk: "Zariadenie neodpovedá. Skúste to o chvíľu znova.",
    de: "Die Box antwortet nicht. Versuche es gleich noch einmal.",
  },
});
