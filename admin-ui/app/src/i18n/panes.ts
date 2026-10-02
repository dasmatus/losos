import { defineMessages } from "./define";

/* The settings panes: About, Apps, Hardware, Mesh, Network, Reset, Security
 * and Storage. The shared settings helpers (rows, window, format, the apply
 * bar, the capacity meter) keep their text in settings.ts. */

export default defineMessages({
  // ── About ───────────────────────────────────────────────────────────────
  "panes.about.thisBox": {
    en: "This box",
    sk: "Toto zariadenie",
    de: "Diese Box",
  },
  "panes.about.called": {
    en: "Called",
    sk: "Názov",
    de: "Name",
  },
  "panes.about.reachedAt": {
    en: "Reached at",
    sk: "Adresa",
    de: "Erreichbar unter",
  },
  "panes.about.storage": {
    en: "Storage",
    sk: "Úložisko",
    de: "Speicher",
  },
  "panes.about.storageShared": {
    en: "shared with the mesh",
    sk: "zdieľané so sieťou mesh",
    de: "mit dem Mesh geteilt",
  },
  "panes.about.storageLocal": {
    en: "kept to itself",
    sk: "len pre seba",
    de: "nur für sich",
  },
  "panes.about.reachableOutside": {
    en: "Reachable from outside",
    sk: "Dostupné zvonku",
    de: "Von außen erreichbar",
  },
  "panes.about.yes": {
    en: "yes",
    sk: "áno",
    de: "ja",
  },
  "panes.about.no": {
    en: "no",
    sk: "nie",
    de: "nein",
  },
  "panes.about.spareTime": {
    en: "Spare time",
    sk: "Voľný čas",
    de: "Freie Zeit",
  },
  "panes.about.onMesh": {
    en: "On the mesh",
    sk: "V sieti mesh",
    de: "Im Mesh",
  },
  "panes.about.joined": {
    en: "joined",
    sk: "pripojené",
    de: "beigetreten",
  },
  "panes.about.notJoined": {
    en: "not joined",
    sk: "nepripojené",
    de: "nicht beigetreten",
  },
  "panes.about.lentOut": {
    en: "Lent out",
    sk: "Požičiava sa",
    de: "Verliehen",
  },
  "panes.about.never": {
    en: "never",
    sk: "nikdy",
    de: "nie",
  },
  "panes.about.idle": {
    en: "Nothing on this box is doing work for anyone else.",
    sk: "Nič na tomto zariadení nepracuje pre nikoho iného.",
    de: "Nichts auf dieser Box arbeitet für jemand anderen.",
  },
  "panes.about.current": {
    en: "Keeping itself current",
    sk: "Aktualizácie",
    de: "Sich selbst aktuell halten",
  },
  "panes.about.updates": {
    en: "Looks for updates",
    sk: "Hľadá aktualizácie",
    de: "Sucht nach Updates",
  },
  "panes.about.restarts": {
    en: "Restarts",
    sk: "Reštartuje sa",
    de: "Startet neu",
  },
  "panes.about.everyNightAt": {
    en: "every night at {time}",
    sk: "každú noc o {time}",
    de: "jede Nacht um {time}",
  },
  "panes.about.caption": {
    en: "This box keeps itself up to date on its own and restarts once a night so nothing is left half-applied. If it is switched off at the time, it catches up the next time it is on. Your admin key is remembered only until you close this tab.",
    sk: "Toto zariadenie sa aktualizuje samo a raz za noc sa reštartuje, aby nič nezostalo použité len napoly. Ak je v tom čase vypnuté, dobehne to pri ďalšom zapnutí. Váš správcovský kľúč si pamätá len do zatvorenia tejto karty.",
    de: "Diese Box hält sich selbst aktuell und startet einmal pro Nacht neu, damit nichts halb angewendet bleibt. Ist sie zu der Zeit aus, holt sie es beim nächsten Einschalten nach. Dein Admin-Schlüssel wird nur gemerkt, bis du diesen Tab schließt.",
  },

  // ── Apps ────────────────────────────────────────────────────────────────
  "panes.apps.mode.native": {
    en: "Part of the system",
    sk: "Súčasť systému",
    de: "Teil des Systems",
  },
  "panes.apps.mode.container": {
    en: "Kept separate",
    sk: "Oddelene",
    de: "Abgeschottet",
  },
  "panes.apps.onThisBox": {
    en: "On this box",
    sk: "Na tomto zariadení",
    de: "Auf dieser Box",
  },
  "panes.apps.files.title": {
    en: "Files",
    sk: "Súbory",
    de: "Dateien",
  },
  "panes.apps.files.detail": {
    en: "Your documents, photos and calendars.",
    sk: "Vaše dokumenty, fotky a kalendáre.",
    de: "Deine Dokumente, Fotos und Kalender.",
  },
  "panes.apps.code.title": {
    en: "Code",
    sk: "Kód",
    de: "Code",
  },
  "panes.apps.code.detail": {
    en: "Your repositories and their issues.",
    sk: "Vaše repozitáre a ich úlohy.",
    de: "Deine Repositorys und ihre Issues.",
  },
  "panes.apps.modeCaption": {
    en: "{kept} runs an app walled off from the rest of the box, so a problem inside it stays inside it. That is the setting to leave alone. {native} runs it alongside everything else. It starts faster, and it is how this box worked before. Changing either restarts that app while this box rebuilds itself.",
    sk: "Voľba {kept} spúšťa aplikáciu oddelenú od zvyšku zariadenia, takže problém v nej zostane v nej. Toto nastavenie je najlepšie nechať tak. Voľba {native} ju spúšťa spolu so všetkým ostatným. Štartuje rýchlejšie a takto zariadenie fungovalo predtým. Zmena ktorejkoľvek z nich reštartuje danú aplikáciu, kým sa zariadenie znovu zostavuje.",
    de: "Mit {kept} läuft eine App abgeschottet vom Rest der Box, sodass ein Problem in ihr auch in ihr bleibt. Diese Einstellung lässt du am besten so. Mit {native} läuft sie neben allem anderen. Sie startet schneller, und so hat diese Box früher gearbeitet. Eine Änderung startet die App neu, während sich die Box neu aufbaut.",
  },
  "panes.apps.findMore": {
    en: "Find more",
    sk: "Nájsť ďalšie",
    de: "Mehr finden",
  },
  "panes.apps.searchLabel": {
    en: "Search for apps",
    sk: "Hľadať aplikácie",
    de: "Nach Apps suchen",
  },
  "panes.apps.searchPlaceholder": {
    en: "Search for an app",
    sk: "Hľadať aplikáciu",
    de: "Nach einer App suchen",
  },
  "panes.apps.searching": {
    en: "Searching",
    sk: "Hľadá sa",
    de: "Suche läuft",
  },
  "panes.apps.looking": {
    en: "Looking…",
    sk: "Hľadám…",
    de: "Suche…",
  },
  "panes.apps.unsupported": {
    en: "This box cannot look for new apps yet. When it can, the search will reach out from the box itself. This page can talk to nothing but your own box, on purpose.",
    sk: "Toto zariadenie zatiaľ nevie hľadať nové aplikácie. Keď to bude vedieť, hľadanie pôjde von priamo zo zariadenia. Táto stránka zámerne nekomunikuje s ničím okrem vášho zariadenia.",
    de: "Diese Box kann noch nicht nach neuen Apps suchen. Sobald sie es kann, sucht die Box selbst draußen. Diese Seite spricht absichtlich mit nichts außer deiner eigenen Box.",
  },
  "panes.apps.failed": {
    en: "The search did not come back: {message}",
    sk: "Hľadanie sa nevrátilo: {message}",
    de: "Die Suche kam nicht zurück: {message}",
  },
  "panes.apps.empty": {
    en: "Nothing came back for that.",
    sk: "Na toto sa nič nenašlo.",
    de: "Dazu kam nichts zurück.",
  },
  "panes.apps.from": {
    en: "from {source}",
    sk: "zdroj: {source}",
    de: "von {source}",
  },
  "panes.apps.lookAt": {
    en: "Look at it",
    sk: "Pozrieť",
    de: "Ansehen",
  },
  "panes.apps.footer": {
    en: "Nobody here has checked any of this. Anything you find comes from whoever published it, and installing it puts their code on the same box as your files.",
    sk: "Nikto tu nič z toho nekontroloval. Všetko, čo nájdete, pochádza od toho, kto to zverejnil, a inštalácia dá jeho kód na to isté zariadenie, kde sú vaše súbory.",
    de: "Niemand hier hat irgendetwas davon geprüft. Alles, was du findest, stammt von denen, die es veröffentlicht haben, und eine Installation bringt ihren Code auf dieselbe Box wie deine Dateien.",
  },
  "panes.apps.footerSearched": {
    en: {
      one: "Searched {count} index: {sources}. {footer}",
      other: "Searched {count} indexes: {sources}. {footer}",
    },
    sk: {
      one: "Prehľadaný {count} index: {sources}. {footer}",
      few: "Prehľadané {count} indexy: {sources}. {footer}",
      many: "Prehľadaných {count} indexov: {sources}. {footer}",
      other: "Prehľadaných {count} indexov: {sources}. {footer}",
    },
    de: {
      one: "{count} Index durchsucht: {sources}. {footer}",
      other: "{count} Indizes durchsucht: {sources}. {footer}",
    },
  },

  // ── Hardware ────────────────────────────────────────────────────────────
  "panes.hardware.graphics": {
    en: "Graphics",
    sk: "Grafika",
    de: "Grafik",
  },
  "panes.hardware.gpu.title": {
    en: "Let apps use the graphics chip",
    sk: "Povoliť aplikáciám používať grafický čip",
    de: "Apps den Grafikchip nutzen lassen",
  },
  "panes.hardware.gpu.detail": {
    en: "Only switch this on if this box has one.",
    sk: "Zapnite to, len ak ho toto zariadenie má.",
    de: "Schalte das nur ein, wenn diese Box einen hat.",
  },
  "panes.hardware.caption": {
    en: "Some jobs, like recognising what is in a photo or converting a video, run much faster on a graphics chip than on the main processor. Switching this on tells the apps they may use one. On a box without one it changes nothing except the time the next rebuild takes.",
    sk: "Niektoré úlohy, napríklad rozpoznávanie toho, čo je na fotke, alebo prevod videa, bežia na grafickom čipe oveľa rýchlejšie ako na hlavnom procesore. Zapnutím aplikáciám poviete, že ho smú používať. Na zariadení bez grafického čipu to nezmení nič, len čas, ktorý zaberie ďalšie zostavenie.",
    de: "Manche Aufgaben, etwa erkennen, was auf einem Foto ist, oder ein Video umwandeln, laufen auf einem Grafikchip viel schneller als auf dem Hauptprozessor. Schaltest du das ein, dürfen die Apps einen nutzen. Auf einer Box ohne Grafikchip ändert es nichts außer der Dauer des nächsten Neuaufbaus.",
  },

  // ── Mesh ────────────────────────────────────────────────────────────────
  "panes.mesh.otherBoxes": {
    en: "Other boxes",
    sk: "Iné zariadenia",
    de: "Andere Boxen",
  },
  "panes.mesh.join": {
    en: "Join the mesh",
    sk: "Pripojiť sa k sieti mesh",
    de: "Dem Mesh beitreten",
  },
  "panes.mesh.joinCaption": {
    en: "The mesh is other people's boxes, running the same system as this one. Joining lets them keep copies of your files and lets you keep copies of theirs, and it is what makes the hours below worth anything.",
    sk: "Sieť mesh tvoria zariadenia iných ľudí s rovnakým systémom ako toto. Po pripojení u seba môžu uchovávať kópie vašich súborov a vy kópie ich súborov. Práve vďaka tomu majú hodiny nižšie nejaký zmysel.",
    de: "Das Mesh sind die Boxen anderer Leute, auf denen dasselbe System läuft wie auf dieser. Wenn du beitrittst, behalten sie Kopien deiner Dateien und du Kopien ihrer, und erst dadurch sind die Stunden unten etwas wert.",
  },
  "panes.mesh.spareTime": {
    en: "Spare time",
    sk: "Voľný čas",
    de: "Freie Zeit",
  },
  "panes.mesh.lend": {
    en: "Lend this box while I sleep",
    sk: "Požičiavať toto zariadenie, kým spím",
    de: "Diese Box verleihen, während ich schlafe",
  },
  "panes.mesh.joinFirst": {
    en: "Join the mesh first.",
    sk: "Najprv sa pripojte k sieti mesh.",
    de: "Tritt zuerst dem Mesh bei.",
  },
  "panes.mesh.hours": {
    en: "Hours",
    sk: "Hodiny",
    de: "Stunden",
  },
  "panes.mesh.lendFrom": {
    en: "Lend from",
    sk: "Požičiavať od",
    de: "Verleihen ab",
  },
  "panes.mesh.until": {
    en: "until",
    sk: "do",
    de: "bis",
  },
  "panes.mesh.lendUntil": {
    en: "Lend until",
    sk: "Požičiavať do",
    de: "Verleihen bis",
  },
  "panes.mesh.your": {
    en: "your",
    sk: "vašich",
    de: "deiner",
  },
  "panes.mesh.windowCaption": {
    en: "These are the hours on {your} clock, and they travel with the setting. The box that hands out the work is told which time zone you meant, so the window does not slide by an hour when the clocks change. The end is the moment it stops. Set it to 07:00 and seven o'clock is yours again. An end earlier than the start simply runs through midnight, which is what “while I sleep” usually means. Work already running is left to finish; nothing new starts once the window shuts.",
    sk: "Toto sú hodiny podľa {your} hodín a cestujú spolu s nastavením. Zariadenie, ktoré rozdeľuje prácu, sa dozvie, aké časové pásmo ste mysleli, takže sa okno pri zmene času neposunie o hodinu. Koniec je okamih, keď sa požičiavanie zastaví. Nastavte 07:00 a o siedmej je zariadenie opäť vaše. Koniec skôr ako začiatok jednoducho prechádza cez polnoc, čo „kým spím“ zvyčajne znamená. Práca, ktorá už beží, sa nechá dokončiť; po zatvorení okna sa nič nové nezačne.",
    de: "Das sind die Stunden nach {your} Uhr, und sie reisen mit der Einstellung. Die Box, die die Arbeit verteilt, erfährt, welche Zeitzone du gemeint hast, deshalb verschiebt sich das Zeitfenster bei der Zeitumstellung nicht um eine Stunde. Das Ende ist der Moment, in dem es aufhört. Stell 07:00 ein, und um sieben gehört die Box wieder dir. Ein Ende vor dem Anfang läuft einfach über Mitternacht, und genau das heißt „während ich schlafe“ meistens. Arbeit, die schon läuft, darf fertig werden; sobald das Zeitfenster zu ist, startet nichts Neues.",
  },

  // ── Network ─────────────────────────────────────────────────────────────
  "panes.network.name": {
    en: "Name",
    sk: "Názov",
    de: "Name",
  },
  "panes.network.called": {
    en: "This box is called",
    sk: "Toto zariadenie sa volá",
    de: "Diese Box heißt",
  },
  "panes.network.reachedAt": {
    en: "Reached on your home network at",
    sk: "V domácej sieti dostupné na",
    de: "Im Heimnetz erreichbar unter",
  },
  "panes.network.nameCaption": {
    en: "Letters, digits and hyphens, starting and ending with a letter or a digit, up to 63 characters. Everything about this box hangs off the name: change it and the address you use to reach it changes with it, along with the addresses the apps hand out. There is no other way into this box, so a name it cannot answer to is a box you cannot reach.",
    sk: "Písmená, číslice a pomlčky, na začiatku aj na konci písmeno alebo číslica, najviac 63 znakov. Všetko na tomto zariadení visí na názve: keď ho zmeníte, zmení sa aj adresa, na ktorej ho nájdete, a s ňou aj adresy, ktoré rozdávajú aplikácie. Do zariadenia sa nedá dostať inak, takže názov, na ktorý nevie odpovedať, znamená zariadenie, na ktoré sa nedostanete.",
    de: "Buchstaben, Ziffern und Bindestriche, am Anfang und Ende ein Buchstabe oder eine Ziffer, höchstens 63 Zeichen. An dem Namen hängt alles auf dieser Box: Änderst du ihn, ändert sich die Adresse, unter der du sie erreichst, und mit ihr die Adressen, die die Apps herausgeben. Es gibt keinen anderen Weg in diese Box, also ist ein Name, auf den sie nicht hören kann, eine Box, die du nicht erreichst.",
  },
  "panes.network.reaching": {
    en: "Reaching it",
    sk: "Prístup",
    de: "Erreichbarkeit",
  },
  "panes.network.encrypt": {
    en: "Encrypt the connection",
    sk: "Šifrovať pripojenie",
    de: "Verbindung verschlüsseln",
  },
  "panes.network.encryptDetail": {
    en: "Your browser will want a certificate it trusts.",
    sk: "Váš prehliadač bude chcieť certifikát, ktorému dôveruje.",
    de: "Dein Browser will dann ein Zertifikat, dem er vertraut.",
  },
  "panes.network.outside": {
    en: "Reachable from outside your home",
    sk: "Dostupné aj mimo domova",
    de: "Auch von außerhalb erreichbar",
  },
  "panes.network.port": {
    en: "Port the files app listens on",
    sk: "Port aplikácie na súbory",
    de: "Port der Dateien-App",
  },
  "panes.network.reachCaption": {
    en: "With “reachable from outside” on, this box opens a way out to a small relay so you can get at it from anywhere. Nothing on your router needs opening. The port is an inside detail. The files app answers on it behind the front door, and there is no reason to change it unless something else on this box already wants that number.",
    sk: "Keď je zapnuté „dostupné mimo domova“, zariadenie si otvorí cestu von k malému sprostredkovateľovi, aby ste sa k nemu dostali odkiaľkoľvek. Na routeri netreba nič otvárať. Port je vnútorný detail. Aplikácia na súbory na ňom odpovedá za vstupnými dverami a netreba ho meniť, pokiaľ to isté číslo nechce niečo iné na tomto zariadení.",
    de: "Mit „von außerhalb erreichbar“ öffnet diese Box einen Weg nach draußen zu einem kleinen Relay, sodass du von überall an sie herankommst. An deinem Router muss nichts geöffnet werden. Der Port ist ein internes Detail. Die Dateien-App antwortet darauf hinter der Haustür, und es gibt keinen Grund, ihn zu ändern, außer etwas anderes auf dieser Box will dieselbe Nummer.",
  },

  // ── Reset ───────────────────────────────────────────────────────────────
  "panes.reset.startOver": {
    en: "Start over",
    sk: "Začať odznova",
    de: "Neu anfangen",
  },
  "panes.reset.title": {
    en: "Put every setting back",
    sk: "Vrátiť všetky nastavenia",
    de: "Alle Einstellungen zurücksetzen",
  },
  "panes.reset.detail": {
    en: "The name, the network, the mesh, the apps. All of it, back to the way this box came.",
    sk: "Názov, sieť, mesh, aplikácie. Všetko späť do stavu, v akom zariadenie prišlo.",
    de: "Der Name, das Netzwerk, das Mesh, die Apps. Alles zurück in den Zustand, in dem diese Box kam.",
  },
  "panes.reset.button": {
    en: "Reset…",
    sk: "Reset…",
    de: "Zurücksetzen…",
  },
  "panes.reset.caption": {
    en: "Your files, your photos and your repositories are not touched. This only undoes the choices made on this screen. The box rebuilds itself afterwards and comes back under its original name, so the address you use to reach it will change back too.",
    sk: "Vaše súbory, fotky a repozitáre zostanú nedotknuté. Vráti sa len to, čo ste zvolili na tejto obrazovke. Zariadenie sa potom znovu zostaví a vráti sa pod pôvodným názvom, takže aj adresa, na ktorej ho nájdete, sa zmení späť.",
    de: "Deine Dateien, Fotos und Repositorys werden nicht angerührt. Zurückgenommen wird nur, was auf diesem Bildschirm gewählt wurde. Danach baut sich die Box neu auf und kommt unter ihrem ursprünglichen Namen zurück, also ändert sich auch die Adresse, unter der du sie erreichst, wieder zurück.",
  },
  "panes.reset.dialogTitle": {
    en: "Put every setting back?",
    sk: "Vrátiť všetky nastavenia?",
    de: "Alle Einstellungen zurücksetzen?",
  },
  "panes.reset.dialogBody": {
    en: "This cannot be undone from here, and there is no other way into this box.",
    sk: "Odtiaľto sa to nedá vrátiť a do zariadenia sa nedá dostať inak.",
    de: "Das lässt sich von hier aus nicht rückgängig machen, und es gibt keinen anderen Weg in diese Box.",
  },
  "panes.reset.goes": {
    en: "Every setting goes back to its original value, including the name this box answers to.",
    sk: "Každé nastavenie sa vráti na pôvodnú hodnotu vrátane názvu, na ktorý zariadenie odpovedá.",
    de: "Jede Einstellung geht auf ihren ursprünglichen Wert zurück, auch der Name, auf den diese Box hört.",
  },
  "panes.reset.stays": {
    en: "Your files and repositories stay exactly where they are.",
    sk: "Vaše súbory a repozitáre zostanú presne tam, kde sú.",
    de: "Deine Dateien und Repositorys bleiben genau, wo sie sind.",
  },
  "panes.reset.rebuilds": {
    en: "The box rebuilds itself, which takes a few minutes. It stays reachable while it works.",
    sk: "Zariadenie sa znovu zostaví, čo trvá niekoľko minút. Počas toho zostane dostupné.",
    de: "Die Box baut sich neu auf, das dauert ein paar Minuten. Sie bleibt dabei erreichbar.",
  },
  "panes.reset.keep": {
    en: "Keep my settings",
    sk: "Ponechať moje nastavenia",
    de: "Einstellungen behalten",
  },
  "panes.reset.confirm": {
    en: "Put everything back",
    sk: "Vrátiť všetko",
    de: "Alles zurücksetzen",
  },

  // ── Security ────────────────────────────────────────────────────────────
  "panes.security.group": {
    en: "Extra protection",
    sk: "Ochrana navyše",
    de: "Zusätzlicher Schutz",
  },
  "panes.security.usbguard.title": {
    en: "Ignore USB devices plugged in later",
    sk: "Ignorovať neskôr pripojené USB zariadenia",
    de: "Später eingesteckte USB-Geräte ignorieren",
  },
  "panes.security.usbguard.detail": {
    en: "Anything attached after the box starts is refused. Costs you a keyboard if you ever need one at the machine itself.",
    sk: "Všetko, čo pripojíte po štarte zariadenia, sa odmietne. Stojí vás to klávesnicu, ak by ste ju niekedy potrebovali priamo pri počítači.",
    de: "Alles, was nach dem Start der Box angesteckt wird, wird abgewiesen. Kostet dich eine Tastatur, falls du je eine direkt am Gerät brauchst.",
  },
  "panes.security.malloc.title": {
    en: "Stricter memory handling",
    sk: "Prísnejšia správa pamäte",
    de: "Strengere Speicherverwaltung",
  },
  "panes.security.malloc.detail": {
    en: "Makes a whole family of break-in attempts fail instead of succeed quietly. Costs a little speed and a little memory.",
    sk: "Celá skupina pokusov o prienik zlyhá namiesto toho, aby potichu uspela. Stojí trochu rýchlosti a trochu pamäte.",
    de: "Lässt eine ganze Familie von Einbruchsversuchen scheitern, statt dass sie still gelingen. Kostet etwas Tempo und etwas Speicher.",
  },
  "panes.security.apparmor.title": {
    en: "Confine the programs that face the network",
    sk: "Obmedziť programy, ktoré komunikujú so sieťou",
    de: "Programme mit Netzwerkzugang einschränken",
  },
  "panes.security.apparmor.detail": {
    en: "Limits what each one may touch if it is ever taken over. Only some of what runs here has a rule written for it, so it protects less than it sounds like.",
    sk: "Obmedzí, na čo smie každý z nich siahnuť, ak by ho niekto ovládol. Pravidlo má napísané len časť toho, čo tu beží, takže chráni menej, než to znie.",
    de: "Begrenzt, was jedes davon anfassen darf, falls es je übernommen wird. Nur für einen Teil dessen, was hier läuft, gibt es eine Regel, also schützt es weniger, als es klingt.",
  },
  "panes.security.nosmt.title": {
    en: "Halve the processor to close a leak between jobs",
    sk: "Polovičný procesor na uzavretie úniku medzi úlohami",
    de: "Prozessor halbieren, um ein Leck zwischen Aufgaben zu schließen",
  },
  "panes.security.nosmt.detail": {
    en: "Shuts the door two jobs can otherwise listen through. This is the expensive one: roughly half the speed, and you will notice it converting video.",
    sk: "Zavrie dvere, cez ktoré by sa dve úlohy mohli navzájom počúvať. Toto je tá drahá voľba: zhruba polovičná rýchlosť a pri prevode videa to spoznáte.",
    de: "Schließt die Tür, durch die zwei Aufgaben sonst mithören können. Das ist die teure Option: ungefähr halbe Geschwindigkeit, und beim Umwandeln von Videos merkst du es.",
  },
  "panes.security.caption": {
    en: "This box already protects itself in the ways that cost nothing, and those are always on and not listed here. The four above are the ones with a price, so they are yours to decide. If you lend spare capacity to the mesh, the middle two are the ones worth reading twice: they are what stands between somebody else’s job and yours.",
    sk: "Toto zariadenie sa už chráni všetkými spôsobmi, ktoré nič nestoja; tie sú vždy zapnuté a nie sú tu uvedené. Štyri vyššie niečo stoja, preto je rozhodnutie na vás. Ak požičiavate voľný výkon sieti mesh, prostredné dve si prečítajte dvakrát: práve ony stoja medzi úlohou niekoho iného a vašou.",
    de: "Diese Box schützt sich schon auf alle Arten, die nichts kosten; die sind immer an und hier nicht aufgeführt. Die vier oben haben einen Preis, deshalb entscheidest du. Wenn du freie Rechenleistung ans Mesh verleihst, lohnt es sich, die mittleren beiden zweimal zu lesen: Sie stehen zwischen der Aufgabe von jemand anderem und deiner.",
  },

  // ── Storage ─────────────────────────────────────────────────────────────
  "panes.storage.disk": {
    en: "This box's disk",
    sk: "Disk tohto zariadenia",
    de: "Die Festplatte dieser Box",
  },
  "panes.storage.held.title": {
    en: "Held back for later",
    sk: "Odložené na neskôr",
    de: "Für später zurückgehalten",
  },
  "panes.storage.held.detail": {
    en: "Space this box did not claim when it was set up.",
    sk: "Miesto, ktoré si zariadenie pri nastavení nezabralo.",
    de: "Platz, den diese Box bei der Einrichtung nicht belegt hat.",
  },
  "panes.storage.notReported": {
    en: "not reported",
    sk: "nenahlásené",
    de: "nicht gemeldet",
  },
  "panes.storage.claiming": {
    en: "Claiming the reserve",
    sk: "Zaberá sa rezerva",
    de: "Reserve wird belegt",
  },
  "panes.storage.useReserve": {
    en: "Use reserve…",
    sk: "Použiť rezervu…",
    de: "Reserve nutzen…",
  },
  "panes.storage.diskCaption": {
    en: "This box deliberately left part of its disk unclaimed so it could be made bigger later without opening the case. Using the reserve happens while everything keeps running, and it only goes one way. The disk cannot be made smaller again afterwards.",
    sk: "Toto zariadenie zámerne nechalo časť disku nezabranú, aby sa dalo neskôr zväčšiť bez otvárania skrinky. Použitie rezervy prebehne za chodu všetkého ostatného a ide len jedným smerom. Disk sa potom už nedá znova zmenšiť.",
    de: "Diese Box hat absichtlich einen Teil ihrer Festplatte frei gelassen, damit sie sich später vergrößern lässt, ohne das Gehäuse zu öffnen. Die Reserve wird genutzt, während alles weiterläuft, und das geht nur in eine Richtung. Kleiner machen lässt sich die Festplatte danach nicht mehr.",
  },
  "panes.storage.mesh": {
    en: "The mesh",
    sk: "Sieť mesh",
    de: "Das Mesh",
  },
  "panes.storage.share": {
    en: "Share this box’s disk with the mesh",
    sk: "Zdieľať disk tohto zariadenia so sieťou mesh",
    de: "Die Festplatte dieser Box mit dem Mesh teilen",
  },
  "panes.storage.ifDies": {
    en: "If this disk dies",
    sk: "Ak tento disk zlyhá",
    de: "Wenn diese Festplatte stirbt",
  },
  "panes.storage.rebuilds": {
    en: "rebuilds in about 40 minutes",
    sk: "obnoví sa asi za 40 minút",
    de: "in etwa 40 Minuten wiederhergestellt",
  },
  "panes.storage.notCopied": {
    en: "nothing here is copied anywhere",
    sk: "nič odtiaľto sa nikam nekopíruje",
    de: "nichts hier wird irgendwohin kopiert",
  },
  "panes.storage.meshCaption": {
    en: "Your box lends its spare room to other people's boxes, and copies of your own files are kept on theirs. The two travel together, because a pool you take from but never give to is not a pool. While this is off, the shared half of the disk stays locked and unreadable by anything on this box, including the box itself.",
    sk: "Vaše zariadenie požičiava voľné miesto zariadeniam iných ľudí a kópie vašich súborov sa uchovávajú na ich zariadeniach. Jedno nejde bez druhého, pretože spoločný fond, z ktorého len beriete a nikdy doň nedávate, nie je spoločný fond. Kým je to vypnuté, zdieľaná polovica disku zostáva zamknutá a nečitateľná pre čokoľvek na tomto zariadení vrátane zariadenia samotného.",
    de: "Deine Box verleiht ihren freien Platz an die Boxen anderer Leute, und Kopien deiner eigenen Dateien liegen auf ihren. Beides gehört zusammen, denn ein Pool, aus dem du nur nimmst und in den du nie gibst, ist kein Pool. Solange das aus ist, bleibt die geteilte Hälfte der Festplatte gesperrt und für nichts auf dieser Box lesbar, auch nicht für die Box selbst.",
  },
  "panes.storage.dialogTitle": {
    en: "Use the space held back?",
    sk: "Použiť odložené miesto?",
    de: "Den zurückgehaltenen Platz nutzen?",
  },
  "panes.storage.dialogUnknown": {
    en: "This box has not said how much it is holding back. It will take whatever is there.",
    sk: "Zariadenie neuviedlo, koľko miesta odkladá. Zoberie všetko, čo tam je.",
    de: "Diese Box hat nicht gesagt, wie viel sie zurückhält. Sie nimmt, was da ist.",
  },
  "panes.storage.dialogAdds": {
    en: "This adds about {size} to this box's storage.",
    sk: "Úložisko tohto zariadenia sa tým zväčší asi o {size}.",
    de: "Das fügt dem Speicher dieser Box etwa {size} hinzu.",
  },
  "panes.storage.dialogTail": {
    en: "It happens while everything keeps running, takes a few minutes, and cannot be undone.",
    sk: "Prebehne to za chodu všetkého ostatného, trvá to niekoľko minút a nedá sa to vrátiť.",
    de: "Es passiert, während alles weiterläuft, dauert ein paar Minuten und lässt sich nicht rückgängig machen.",
  },
  "panes.storage.cancel": {
    en: "Cancel",
    sk: "Zrušiť",
    de: "Abbrechen",
  },
  "panes.storage.useIt": {
    en: "Use it",
    sk: "Použiť",
    de: "Nutzen",
  },
  "panes.storage.dismiss": {
    en: "Dismiss",
    sk: "Zavrieť",
    de: "Ausblenden",
  },
  "panes.storage.grew.title": {
    en: "This box has more room",
    sk: "Zariadenie má viac miesta",
    de: "Diese Box hat mehr Platz",
  },
  "panes.storage.grew.detail": {
    en: "{before} before, {after} now.",
    sk: "Predtým {before}, teraz {after}.",
    de: "Vorher {before}, jetzt {after}.",
  },
  "panes.storage.nothing.title": {
    en: "Nothing changed",
    sk: "Nič sa nezmenilo",
    de: "Nichts hat sich geändert",
  },
  "panes.storage.nothing.claimed": {
    en: "{size} was taken from the spare space, but the storage itself did not end up any bigger.",
    sk: "Z voľného miesta sa zabralo {size}, ale samotné úložisko sa nezväčšilo.",
    de: "{size} wurden aus dem freien Platz genommen, aber der Speicher selbst ist dadurch nicht größer geworden.",
  },
  "panes.storage.nothing.none": {
    en: "There was no space left to take. The disk is already using all of itself.",
    sk: "Nezostalo žiadne miesto, ktoré by sa dalo zabrať. Disk sa už využíva celý.",
    de: "Es war kein Platz mehr übrig. Die Festplatte ist bereits ganz in Gebrauch.",
  },
  "panes.storage.failed.title": {
    en: "That did not work",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },
  // ── Market ──────────────────────────────────────────────────────────────
  "panes.market.loading": {
    en: "Looking at the market…",
    sk: "Pozeráme sa na trh…",
    de: "Der Markt wird geladen …",
  },
  "panes.market.failed": {
    en: "The market did not answer",
    sk: "Trh neodpovedal",
    de: "Der Markt hat nicht geantwortet",
  },
  "panes.market.refresh": {
    en: "Refresh",
    sk: "Obnoviť",
    de: "Aktualisieren",
  },
  "panes.market.unavailable.title": {
    en: "The market is not available on this box",
    sk: "Trh na tomto zariadení nie je dostupný",
    de: "Der Markt ist auf dieser Box nicht verfügbar",
  },
  "panes.market.unavailable.detail": {
    en: "It is optional, and it needs the box to be reachable through a master proxy whose operator has switched it on. Nothing else on the box depends on it.",
    sk: "Je voliteľný a vyžaduje, aby bolo zariadenie dostupné cez hlavný proxy server, ktorého prevádzkovateľ trh zapol. Nič iné na zariadení od neho nezávisí.",
    de: "Er ist optional und setzt voraus, dass die Box über einen Master-Proxy erreichbar ist, dessen Betreiber ihn eingeschaltet hat. Nichts sonst auf der Box hängt davon ab.",
  },
  "panes.market.actionFailed": {
    en: "That did not work. Try again in a moment.",
    sk: "Nepodarilo sa to. Skúste to o chvíľu znova.",
    de: "Das hat nicht geklappt. Versuch es gleich noch einmal.",
  },
  "panes.market.yours": {
    en: "What you have bought",
    sk: "Čo ste si kúpili",
    de: "Was du gekauft hast",
  },
  "panes.market.storageBought": {
    en: "Storage",
    sk: "Úložisko",
    de: "Speicher",
  },
  "panes.market.storageBoughtDetail": {
    en: "Capacity on the mesh that you may claim while the purchase lasts.",
    sk: "Kapacita v mesh sieti, ktorú môžete využívať, kým kúpa trvá.",
    de: "Kapazität im Mesh, die du beanspruchen darfst, solange der Kauf läuft.",
  },
  "panes.market.computeBought": {
    en: "Compute",
    sk: "Výpočtový výkon",
    de: "Rechenleistung",
  },
  "panes.market.computeBoughtDetail": {
    en: "Credit only for now: nothing meters or schedules against it yet.",
    sk: "Zatiaľ len kredit: nič ho ešte nemeria ani podľa neho neplánuje.",
    de: "Vorerst nur ein Guthaben: Noch misst oder plant nichts dagegen.",
  },
  "panes.market.gib": {
    en: "{count} GiB",
    sk: "{count} GiB",
    de: "{count} GiB",
  },
  "panes.market.hours": {
    en: "{count} vCPU-hours",
    sk: "{count} vCPU-hodín",
    de: "{count} vCPU-Stunden",
  },
  "panes.market.buy": {
    en: "Buy from other boxes",
    sk: "Kúpiť od iných zariadení",
    de: "Bei anderen Boxen kaufen",
  },
  "panes.market.shelfEmpty": {
    en: "Nobody is selling anything right now.",
    sk: "Momentálne nikto nič nepredáva.",
    de: "Im Moment verkauft niemand etwas.",
  },
  "panes.market.payCaption": {
    en: "You pay on Stripe's own page, which opens in a new tab. This box never sees your card. Come back here and refresh once you have paid.",
    sk: "Platíte na stránke Stripe, ktorá sa otvorí na novej karte. Toto zariadenie vašu kartu nikdy nevidí. Po zaplatení sa vráťte sem a obnovte stránku.",
    de: "Du bezahlst auf der Seite von Stripe, die sich in einem neuen Tab öffnet. Diese Box sieht deine Karte nie. Komm nach der Zahlung hierher zurück und aktualisiere.",
  },
  "panes.market.kind.storage": {
    en: "Storage",
    sk: "Úložisko",
    de: "Speicher",
  },
  "panes.market.kind.compute": {
    en: "Compute",
    sk: "Výpočtový výkon",
    de: "Rechenleistung",
  },
  "panes.market.unit.storage": {
    en: "GiB-month",
    sk: "GiB-mesiac",
    de: "GiB-Monat",
  },
  "panes.market.unit.compute": {
    en: "vCPU-hour",
    sk: "vCPU-hodina",
    de: "vCPU-Stunde",
  },
  "panes.market.status.pending": {
    en: "Awaiting payment",
    sk: "Čaká na platbu",
    de: "Wartet auf Zahlung",
  },
  "panes.market.status.paid": {
    en: "Paid",
    sk: "Zaplatené",
    de: "Bezahlt",
  },
  "panes.market.status.expired": {
    en: "Not paid",
    sk: "Nezaplatené",
    de: "Nicht bezahlt",
  },
  "panes.market.status.lapsed": {
    en: "Ended",
    sk: "Skončilo",
    de: "Abgelaufen",
  },
  "panes.market.until": {
    en: "until {date}",
    sk: "do {date}",
    de: "bis {date}",
  },
  "panes.market.volume": {
    en: "volume {volume}",
    sk: "zväzok {volume}",
    de: "Volume {volume}",
  },
  "panes.market.quantityRange": {
    en: "Enter a whole number from 1 to {max}.",
    sk: "Zadajte celé číslo od 1 do {max}.",
    de: "Gib eine ganze Zahl von 1 bis {max} ein.",
  },
  "panes.market.quantity": {
    en: "Quantity",
    sk: "Množstvo",
    de: "Menge",
  },
  "panes.market.available": {
    en: "{count} available",
    sk: "Dostupné: {count}",
    de: "{count} verfügbar",
  },
  "panes.market.buyButton": {
    en: "Buy",
    sk: "Kúpiť",
    de: "Kaufen",
  },
  "panes.market.noCheckout": {
    en: "The order was made, but no payment page came back. Refresh and try again.",
    sk: "Objednávka vznikla, ale platobná stránka sa nevrátila. Obnovte stránku a skúste to znova.",
    de: "Die Bestellung wurde angelegt, aber es kam keine Zahlungsseite zurück. Aktualisiere und versuch es erneut.",
  },
  "panes.market.popupBlocked": {
    en: "Allow pop-ups for this page, then try again.",
    sk: "Povoľte vyskakovacie okná pre túto stránku a skúste to znova.",
    de: "Erlaube Pop-ups für diese Seite und versuche es erneut.",
  },
  "panes.market.paying": {
    en: "The payment page is open in another tab. Refresh here once you have paid.",
    sk: "Platobná stránka je otvorená na inej karte. Po zaplatení tu obnovte stránku.",
    de: "Die Zahlungsseite ist in einem anderen Tab geöffnet. Aktualisiere hier, sobald du bezahlt hast.",
  },
  "panes.market.sell": {
    en: "Sell what you already share",
    sk: "Predávať to, čo už zdieľate",
    de: "Verkaufen, was du schon teilst",
  },
  "panes.market.payouts": {
    en: "Payouts",
    sk: "Výplaty",
    de: "Auszahlungen",
  },
  "panes.market.payoutsReady": {
    en: "Your Stripe account can receive payments.",
    sk: "Váš účet Stripe môže prijímať platby.",
    de: "Dein Stripe-Konto kann Zahlungen empfangen.",
  },
  "panes.market.payoutsPending": {
    en: "Stripe still needs a few details from you before it can pay you.",
    sk: "Stripe od vás ešte potrebuje niekoľko údajov, aby vám mohol platiť.",
    de: "Stripe braucht noch ein paar Angaben von dir, bevor es dich bezahlen kann.",
  },
  "panes.market.payoutsNone": {
    en: "To be paid you need a Stripe account. Setting it up happens on Stripe's page.",
    sk: "Aby ste mohli dostávať platby, potrebujete účet Stripe. Nastavuje sa na stránke Stripe.",
    de: "Um bezahlt zu werden, brauchst du ein Stripe-Konto. Das richtest du auf der Seite von Stripe ein.",
  },
  "panes.market.payoutsStart": {
    en: "Set up payouts",
    sk: "Nastaviť výplaty",
    de: "Auszahlungen einrichten",
  },
  "panes.market.payoutsContinue": {
    en: "Continue setup",
    sk: "Pokračovať v nastavení",
    de: "Einrichtung fortsetzen",
  },
  "panes.market.nothingShared": {
    en: "Nothing to sell yet. Join the mesh first (and share compute, to sell compute): the market only sells what you already lend.",
    sk: "Zatiaľ nie je čo predávať. Najprv sa pripojte k mesh sieti (a na predaj výkonu zdieľajte výpočtový výkon): trh predáva len to, čo už požičiavate.",
    de: "Noch nichts zu verkaufen. Tritt zuerst dem Mesh bei (und teile Rechenleistung, um sie zu verkaufen): Der Markt verkauft nur, was du ohnehin verleihst.",
  },
  "panes.market.kindLabel": {
    en: "What to sell",
    sk: "Čo predávať",
    de: "Was verkaufen",
  },
  "panes.market.priceLabel": {
    en: "Price per unit ({currency})",
    sk: "Cena za jednotku ({currency})",
    de: "Preis pro Einheit ({currency})",
  },
  "panes.market.priceInvalid": {
    en: "Enter a price above zero, like {example}.",
    sk: "Zadajte cenu vyššiu ako nula, napríklad {example}.",
    de: "Gib einen Preis über null ein, etwa {example}.",
  },
  "panes.market.capacityLabel": {
    en: "Capacity (units)",
    sk: "Kapacita (jednotky)",
    de: "Kapazität (Einheiten)",
  },
  "panes.market.capacityInvalid": {
    en: "Enter a whole number of units, at least 1.",
    sk: "Zadajte celý počet jednotiek, aspoň 1.",
    de: "Gib eine ganze Zahl an Einheiten ein, mindestens 1.",
  },
  "panes.market.listButton": {
    en: "Offer for sale",
    sk: "Ponúknuť na predaj",
    de: "Zum Verkauf anbieten",
  },
  "panes.market.open": {
    en: "on sale",
    sk: "v predaji",
    de: "im Verkauf",
  },
  "panes.market.closed": {
    en: "closed",
    sk: "zatvorené",
    de: "geschlossen",
  },
  "panes.market.closeButton": {
    en: "Stop selling",
    sk: "Prestať predávať",
    de: "Nicht mehr verkaufen",
  },
  "panes.market.sellCaption": {
    en: "The platform keeps {percent}% of each sale to cover running costs; the rest goes to your Stripe account.",
    sk: "Platforma si z každého predaja necháva {percent} % na pokrytie nákladov; zvyšok ide na váš účet Stripe.",
    de: "Die Plattform behält {percent} % jedes Verkaufs für die Betriebskosten; der Rest geht an dein Stripe-Konto.",
  },
  "panes.market.youReceive": {
    en: "you receive {amount}",
    sk: "dostanete {amount}",
    de: "du erhältst {amount}",
  },
});
