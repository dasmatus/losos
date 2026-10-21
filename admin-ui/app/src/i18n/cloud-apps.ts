import { defineMessages } from "./define";

/* The Apps pane's two accordions: LosOS Cloud (its apps, the app store and
 * its settings for everyone) and LosOS Git (its settings for everyone). The
 * settings rows themselves keep their text in panes.ts. */

export default defineMessages({
  "cloudApps.cloud.detail": {
    en: {
      one: "{count} app on, and the settings for everyone",
      other: "{count} apps on, and the settings for everyone",
    },
    sk: {
      one: "{count} zapnutá aplikácia a nastavenia pre všetkých",
      few: "{count} zapnuté aplikácie a nastavenia pre všetkých",
      many: "{count} zapnutých aplikácií a nastavenia pre všetkých",
      other: "{count} zapnutých aplikácií a nastavenia pre všetkých",
    },
    de: {
      one: "{count} App an und die Einstellungen für alle",
      other: "{count} Apps an und die Einstellungen für alle",
    },
  },
  "cloudApps.cloud.detailPlain": {
    en: "Its apps and the settings for everyone",
    sk: "Jeho aplikácie a nastavenia pre všetkých",
    de: "Seine Apps und die Einstellungen für alle",
  },
  "cloudApps.git.detail": {
    en: "The settings for everyone",
    sk: "Nastavenia pre všetkých",
    de: "Die Einstellungen für alle",
  },
  "cloudApps.settings.title": {
    en: "Settings for everyone",
    sk: "Nastavenia pre všetkých",
    de: "Einstellungen für alle",
  },

  "cloudApps.federation.cloud": {
    en: "Share with other Nextcloud servers",
    sk: "Zdieľať s inými servermi Nextcloud",
    de: "Mit anderen Nextcloud-Servern teilen",
  },
  "cloudApps.federation.cloudDetail": {
    en: "Accounts can share files and calendars with people on other servers, by their user@server address.",
    sk: "Účty môžu zdieľať súbory a kalendáre s ľuďmi na iných serveroch, cez ich adresu používateľ@server.",
    de: "Konten können Dateien und Kalender mit Leuten auf anderen Servern teilen, über deren Adresse nutzer@server.",
  },
  "cloudApps.federation.git": {
    en: "Federate with other Forgejo servers",
    sk: "Federovať s inými servermi Forgejo",
    de: "Mit anderen Forgejo-Servern föderieren",
  },
  "cloudApps.federation.gitDetail": {
    en: "People on other servers can star your repositories and follow accounts here.",
    sk: "Ľudia na iných serveroch môžu dávať hviezdičky vašim repozitárom a sledovať účty tu.",
    de: "Leute auf anderen Servern können deine Repositorys mit Sternen versehen und Konten hier folgen.",
  },
  "cloudApps.federation.needsSharing": {
    en: "Runs only while the box shares its disk, and sharing is off, so this does nothing for now.",
    sk: "Beží len vtedy, keď zariadenie zdieľa disk, a zdieľanie je vypnuté, takže toto zatiaľ nič nerobí.",
    de: "Läuft nur, solange die Box ihre Platte teilt, und Teilen ist aus, also bewirkt das vorerst nichts.",
  },

  // ── The apps ────────────────────────────────────────────────────────────
  "cloudApps.apps.title": {
    en: "Apps",
    sk: "Aplikácie",
    de: "Apps",
  },
  "cloudApps.apps.filterLabel": {
    en: "Filter the apps",
    sk: "Filtrovať aplikácie",
    de: "Apps filtern",
  },
  "cloudApps.apps.filterPlaceholder": {
    en: "Filter by name",
    sk: "Filtrovať podľa názvu",
    de: "Nach Namen filtern",
  },
  "cloudApps.apps.loading": {
    en: "Asking LosOS Cloud…",
    sk: "Pýtam sa LosOS Cloud…",
    de: "Frage LosOS Cloud…",
  },
  "cloudApps.apps.unavailable": {
    en: "The apps show here once LosOS Cloud is running. {reason}",
    sk: "Aplikácie sa tu zobrazia, keď bude LosOS Cloud bežať. {reason}",
    de: "Die Apps erscheinen hier, sobald LosOS Cloud läuft. {reason}",
  },
  "cloudApps.apps.failed": {
    en: "The app list did not come back: {message}",
    sk: "Zoznam aplikácií sa nevrátil: {message}",
    de: "Die App-Liste kam nicht zurück: {message}",
  },
  "cloudApps.apps.noMatch": {
    en: "No app matches that.",
    sk: "Tomu nezodpovedá žiadna aplikácia.",
    de: "Dazu passt keine App.",
  },
  "cloudApps.apps.showCore": {
    en: {
      one: "Show the {count} app built into LosOS Cloud",
      other: "Show the {count} apps built into LosOS Cloud",
    },
    sk: {
      one: "Zobraziť {count} aplikáciu zabudovanú v LosOS Cloud",
      few: "Zobraziť {count} aplikácie zabudované v LosOS Cloud",
      many: "Zobraziť {count} aplikácií zabudovaných v LosOS Cloud",
      other: "Zobraziť {count} aplikácií zabudovaných v LosOS Cloud",
    },
    de: {
      one: "Die {count} in LosOS Cloud eingebaute App zeigen",
      other: "Die {count} in LosOS Cloud eingebauten Apps zeigen",
    },
  },
  "cloudApps.apps.hideCore": {
    en: "Hide the apps built into LosOS Cloud",
    sk: "Skryť aplikácie zabudované v LosOS Cloud",
    de: "Die in LosOS Cloud eingebauten Apps ausblenden",
  },
  "cloudApps.apps.fromStore": {
    en: "From the app store",
    sk: "Z obchodu s aplikáciami",
    de: "Aus dem App-Store",
  },
  "cloudApps.apps.switchLabel": {
    en: "{name} on",
    sk: "{name} zapnuté",
    de: "{name} an",
  },
  "cloudApps.apps.changing": {
    en: "Changing",
    sk: "Mení sa",
    de: "Wird geändert",
  },
  "cloudApps.apps.remove": {
    en: "Remove",
    sk: "Odstrániť",
    de: "Entfernen",
  },
  "cloudApps.apps.removeLabel": {
    en: "Remove {name}",
    sk: "Odstrániť {name}",
    de: "{name} entfernen",
  },
  "cloudApps.apps.changeFailed": {
    en: "That did not change",
    sk: "Nezmenilo sa to",
    de: "Das hat sich nicht geändert",
  },
  "cloudApps.apps.caption": {
    en: "An app turned off is gone from every account, and what was made in it is kept for when it comes back. It stays off after the box restarts. The owner's account in LosOS Cloud is a standard one, so apps are added and turned on or off here, not in LosOS Cloud.",
    sk: "Vypnutá aplikácia zmizne zo všetkých účtov a to, čo sa v nej vytvorilo, zostane pre jej návrat. Zostane vypnutá aj po reštarte zariadenia. Účet vlastníka v LosOS Cloud je bežný účet, takže aplikácie sa pridávajú, zapínajú a vypínajú tu, nie v LosOS Cloud.",
    de: "Eine ausgeschaltete App verschwindet aus allen Konten, und was darin entstanden ist, bleibt für ihre Rückkehr erhalten. Sie bleibt auch nach einem Neustart der Box aus. Das Konto des Besitzers in LosOS Cloud ist ein normales Konto, deshalb werden Apps hier hinzugefügt und ein- oder ausgeschaltet, nicht in LosOS Cloud.",
  },

  // ── Why an app cannot be changed ────────────────────────────────────────
  "cloudApps.locked.nextcloud": {
    en: "LosOS Cloud does not run without it.",
    sk: "LosOS Cloud bez nej nebeží.",
    de: "LosOS Cloud läuft nicht ohne sie.",
  },
  "cloudApps.locked.signIn": {
    en: "The box needs it for signing in.",
    sk: "Zariadenie ju potrebuje na prihlasovanie.",
    de: "Die Box braucht sie für die Anmeldung.",
  },
  "cloudApps.locked.sharing": {
    en: "On only while the box shares its disk.",
    sk: "Zapnutá len vtedy, keď zariadenie zdieľa disk.",
    de: "Nur an, solange die Box ihre Platte teilt.",
  },
  "cloudApps.locked.look": {
    en: "It gives LosOS Cloud its look.",
    sk: "Dáva LosOS Cloud jeho vzhľad.",
    de: "Sie gibt LosOS Cloud sein Aussehen.",
  },
  "cloudApps.locked.wizard": {
    en: "Kept off: it would open inside the setup wizard.",
    sk: "Vypnutá: otvorila by sa v sprievodcovi nastavením.",
    de: "Bleibt aus: Sie würde sich im Einrichtungsassistenten öffnen.",
  },

  // ── Removing one ────────────────────────────────────────────────────────
  "cloudApps.remove.title": {
    en: "Remove {name}?",
    sk: "Odstrániť {name}?",
    de: "{name} entfernen?",
  },
  "cloudApps.remove.body": {
    en: "Its code leaves the box. What accounts made in it stays in LosOS Cloud, so adding it again from the app store finds it.",
    sk: "Jej kód zo zariadenia zmizne. To, čo v nej účty vytvorili, zostane v LosOS Cloud, takže keď ju znova pridáte z obchodu s aplikáciami, nájde to.",
    de: "Ihr Code verschwindet von der Box. Was Konten darin angelegt haben, bleibt in LosOS Cloud, und wenn du sie wieder aus dem App-Store hinzufügst, findet sie es.",
  },
  "cloudApps.remove.cancel": {
    en: "Cancel",
    sk: "Zrušiť",
    de: "Abbrechen",
  },
  "cloudApps.remove.confirm": {
    en: "Remove",
    sk: "Odstrániť",
    de: "Entfernen",
  },

  // ── The app store ───────────────────────────────────────────────────────
  "cloudApps.store.title": {
    en: "Add from the app store",
    sk: "Pridať z obchodu s aplikáciami",
    de: "Aus dem App-Store hinzufügen",
  },
  "cloudApps.store.searchLabel": {
    en: "Search the app store",
    sk: "Hľadať v obchode s aplikáciami",
    de: "Im App-Store suchen",
  },
  "cloudApps.store.searchPlaceholder": {
    en: "Search Nextcloud's app store",
    sk: "Hľadať v obchode s aplikáciami Nextcloud",
    de: "Im App-Store von Nextcloud suchen",
  },
  "cloudApps.store.looking": {
    en: "Looking…",
    sk: "Hľadám…",
    de: "Suche…",
  },
  "cloudApps.store.empty": {
    en: "Nothing came back for that.",
    sk: "Na toto sa nič nenašlo.",
    de: "Dazu kam nichts zurück.",
  },
  "cloudApps.store.failed": {
    en: "The search did not come back: {message}",
    sk: "Hľadanie sa nevrátilo: {message}",
    de: "Die Suche kam nicht zurück: {message}",
  },
  "cloudApps.store.add": {
    en: "Add",
    sk: "Pridať",
    de: "Hinzufügen",
  },
  "cloudApps.store.addLabel": {
    en: "Add {name} to LosOS Cloud",
    sk: "Pridať {name} do LosOS Cloud",
    de: "{name} zu LosOS Cloud hinzufügen",
  },
  "cloudApps.store.adding": {
    en: "Adding",
    sk: "Pridáva sa",
    de: "Wird hinzugefügt",
  },
  "cloudApps.store.added": {
    en: "In LosOS Cloud",
    sk: "V LosOS Cloud",
    de: "In LosOS Cloud",
  },
  "cloudApps.store.lookAt": {
    en: "Look at it",
    sk: "Pozrieť",
    de: "Ansehen",
  },
  "cloudApps.store.caption": {
    en: "The box searches Nextcloud's app store itself, so this needs the internet. Nobody on the LosOS side has checked these apps. Each one comes from whoever published it, and adding it puts their code on the same box as your files.",
    sk: "Zariadenie hľadá v obchode s aplikáciami Nextcloud samo, takže to potrebuje internet. Nikto na strane LosOS tieto aplikácie nekontroloval. Každá pochádza od toho, kto ju zverejnil, a jej pridanie dá jeho kód na to isté zariadenie, kde sú vaše súbory.",
    de: "Die Box durchsucht den App-Store von Nextcloud selbst, deshalb braucht das hier Internet. Niemand auf der Seite von LosOS hat diese Apps geprüft. Jede stammt von denen, die sie veröffentlicht haben, und das Hinzufügen bringt ihren Code auf dieselbe Box wie deine Dateien.",
  },
});
