import { defineMessages } from "./define";

/* The Apps pane's two accordions, LosOS Cloud and LosOS Git, each with its
 * settings for everyone. The settings rows themselves keep their text in
 * panes.ts. */

export default defineMessages({
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
});
