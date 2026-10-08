import { defineMessages } from "./define";

/* The settings panes: About, Apps, Backup, Hardware, Mesh, Network, Reset, Security
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
  "panes.about.byName": {
    en: "By name",
    sk: "Podľa názvu",
    de: "Über den Namen",
  },
  "panes.about.addressCaption": {
    en: "The name {name} is found over the local network, by mDNS. A computer that cannot find it, such as the host of a virtual machine, can use the numeric address on the box's screen instead. The box answers to it the same way.",
    sk: "Názov {name} sa hľadá cez lokálnu sieť, cez mDNS. Počítač, ktorý ho nenájde, napríklad hostiteľ virtuálneho stroja, môže použiť číselnú adresu z obrazovky zariadenia. Zariadenie na ňu odpovedá rovnako.",
    de: "Der Name {name} wird über das lokale Netz gefunden, per mDNS. Findet ein Computer ihn nicht, etwa der Host einer virtuellen Maschine, geht die Zahlenadresse vom Bildschirm der Box. Die Box antwortet darauf genauso.",
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
  "panes.about.handbook": {
    en: "Handbook",
    sk: "Príručka",
    de: "Handbuch",
  },
  "panes.about.handbookLink": {
    en: "Open the handbook",
    sk: "Otvoriť príručku",
    de: "Handbuch öffnen",
  },
  "panes.about.handbookCaption": {
    en: "The owner's manual, with a chapter for when something goes wrong. This box carries its own copy, so it opens without the internet.",
    sk: "Príručka vlastníka s kapitolou pre prípad, že niečo nefunguje. Toto zariadenie má vlastnú kópiu, takže sa otvorí aj bez internetu.",
    de: "Das Handbuch für Besitzer, mit einem Kapitel für den Fall, dass etwas nicht funktioniert. Diese Box trägt ihre eigene Kopie, also öffnet es sich auch ohne Internet.",
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
  // The edge proxy group: what lososd found when it last looked (GET
  // /api/edge). The box refuses to turn sharing on without one, so the words
  // here are the words of that refusal, ahead of it.
  "panes.mesh.edge.title": {
    en: "Edge proxy",
    sk: "Okrajový proxy server",
    de: "Edge-Proxy",
  },
  "panes.mesh.edge.looking": {
    en: "Looking for an edge proxy…",
    sk: "Hľadá sa okrajový proxy server…",
    de: "Suche nach einem Edge-Proxy…",
  },
  "panes.mesh.edge.unknown": {
    en: "This box could not say whether an edge proxy is in reach.",
    sk: "Zariadenie nevie povedať, či je okrajový proxy server dostupný.",
    de: "Diese Box konnte nicht sagen, ob ein Edge-Proxy erreichbar ist.",
  },
  "panes.mesh.edge.found": {
    en: "Edge proxy found: {name}",
    sk: "Okrajový proxy server nájdený: {name}",
    de: "Edge-Proxy gefunden: {name}",
  },
  "panes.mesh.edge.viaLan": {
    en: "On this network",
    sk: "V tejto sieti",
    de: "In diesem Netzwerk",
  },
  "panes.mesh.edge.viaInternet": {
    en: "Over the internet",
    sk: "Cez internet",
    de: "Über das Internet",
  },
  "panes.mesh.edge.none": {
    en: "No edge proxy found",
    sk: "Žiadny okrajový proxy server sa nenašiel",
    de: "Kein Edge-Proxy gefunden",
  },
  "panes.mesh.edge.noneDetail": {
    en: "This box searched its network and nothing answered.",
    sk: "Zariadenie prehľadalo svoju sieť a nič neodpovedalo.",
    de: "Diese Box hat ihr Netzwerk durchsucht, und nichts hat geantwortet.",
  },
  "panes.mesh.edge.noneTried": {
    en: "This box searched its network and asked {url}; nothing answered.",
    sk: "Zariadenie prehľadalo svoju sieť a oslovilo {url}; nič neodpovedalo.",
    de: "Diese Box hat ihr Netzwerk durchsucht und {url} gefragt; nichts hat geantwortet.",
  },
  "panes.mesh.edge.lanUnsearched": {
    en: "This box could not search its network, and nothing answered elsewhere.",
    sk: "Zariadenie nemohlo prehľadať svoju sieť a inde nič neodpovedalo.",
    de: "Diese Box konnte ihr Netzwerk nicht durchsuchen, und anderswo hat nichts geantwortet.",
  },
  "panes.mesh.edge.sharingOff": {
    en: "Sharing is off",
    sk: "Zdieľanie je vypnuté",
    de: "Teilen ist aus",
  },
  // The sign beside each edge's name: is this edge one LosOS runs? It
  // decides trading only; the words are the tooltip and the button's name.
  "panes.mesh.edge.officialTip": {
    en: "Official LosOS edge. It proved it is run by LosOS, so it may process storage and compute trading for this box.",
    sk: "Oficiálny okrajový server LosOS. Preukázal, že ho prevádzkuje LosOS, takže môže pre toto zariadenie spracúvať obchodovanie s úložiskom a výpočtami.",
    de: "Offizieller LosOS-Edge. Er hat nachgewiesen, dass LosOS ihn betreibt; er darf für diese Box den Handel mit Speicher und Rechenleistung abwickeln.",
  },
  "panes.mesh.edge.companyTip": {
    en: "Not an official LosOS edge. What you are missing through it:\n• buying storage or compute on the market\n• selling this box's spare storage and compute\nSharing storage through it works.",
    sk: "Nie je oficiálny okrajový server LosOS. Čo cez neho chýba:\n• nákup úložiska alebo výpočtov na trhu\n• predaj voľného úložiska a výpočtov tohto zariadenia\nZdieľanie úložiska cez neho funguje.",
    de: "Kein offizieller LosOS-Edge. Was dir darüber fehlt:\n• Speicher oder Rechenleistung auf dem Markt kaufen\n• freien Speicher und Rechenleistung dieser Box verkaufen\nSpeicher darüber zu teilen funktioniert.",
  },
  "panes.mesh.edge.captionCompany": {
    en: "The edge proxy is the box in the middle: it holds the mesh together and makes yours reachable from outside. Several can be in reach at once; sharing works through any of them. None of these is run by LosOS, so the market (buying and selling storage and compute) stays off until an official edge answers. Your own files and apps work either way.",
    sk: "Okrajový proxy server je zariadenie uprostred: drží sieť mesh pohromade a sprístupňuje to vaše zvonku. Dostupných ich môže byť viac naraz; zdieľanie funguje cez ktorýkoľvek. Žiadny z týchto neprevádzkuje LosOS, takže trh (nákup a predaj úložiska a výpočtov) zostane vypnutý, kým sa neozve oficiálny. Vaše vlastné súbory a aplikácie fungujú tak či tak.",
    de: "Der Edge-Proxy ist die Box in der Mitte: Er hält das Mesh zusammen und macht deine von außen erreichbar. Mehrere können zugleich erreichbar sein; Teilen funktioniert über jeden davon. Keiner davon wird von LosOS betrieben, darum bleibt der Markt (Speicher und Rechenleistung kaufen und verkaufen) aus, bis ein offizieller antwortet. Deine eigenen Dateien und Apps laufen so oder so.",
  },
  "panes.mesh.edge.needed": {
    en: "Needs an edge proxy in reach.",
    sk: "Vyžaduje dostupný okrajový proxy server.",
    de: "Braucht einen erreichbaren Edge-Proxy.",
  },
  "panes.mesh.edge.caption": {
    en: "The edge proxy is the box in the middle: it holds the mesh together and makes yours reachable from outside. This box looks for one on its own network and at its usual address every few seconds, and anything that shares your storage over the network stays off until one answers. Your own files and apps work either way.",
    sk: "Okrajový proxy server je zariadenie uprostred: drží sieť mesh pohromade a sprístupňuje to vaše zvonku. Toto zariadenie ho každých pár sekúnd hľadá vo vlastnej sieti aj na svojej obvyklej adrese, a čokoľvek, čo zdieľa vaše úložisko cez sieť, zostane vypnuté, kým sa niektorý neozve. Vaše vlastné súbory a aplikácie fungujú tak či tak.",
    de: "Der Edge-Proxy ist die Box in der Mitte: Er hält das Mesh zusammen und macht deine von außen erreichbar. Diese Box sucht alle paar Sekunden in ihrem eigenen Netzwerk und an ihrer üblichen Adresse nach einem, und alles, was deinen Speicher über das Netzwerk teilt, bleibt aus, bis einer antwortet. Deine eigenen Dateien und Apps laufen so oder so.",
  },
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
    en: "These are the hours on {your} clock, and they travel with the setting. The box that hands out the work is told which time zone you meant, so the window does not slide by an hour when the clocks change. The end is the moment it stops. Set it to 07:00 and seven o'clock is yours again. An end earlier than the start runs through midnight, which is what \"while I sleep\" usually means. Work already running is left to finish. Nothing new starts once the window shuts.",
    sk: "Toto sú hodiny podľa {your} hodín a cestujú spolu s nastavením. Zariadenie, ktoré rozdeľuje prácu, sa dozvie, aké časové pásmo ste mysleli, takže sa okno pri zmene času neposunie o hodinu. Koniec je okamih, keď sa požičiavanie zastaví. Nastavte 07:00 a o siedmej je zariadenie opäť vaše. Koniec skôr ako začiatok prechádza cez polnoc, čo „kým spím“ zvyčajne znamená. Práca, ktorá už beží, sa nechá dokončiť. Po zatvorení okna sa nič nové nezačne.",
    de: "Das sind die Stunden nach {your} Uhr, und sie reisen mit der Einstellung. Die Box, die die Arbeit verteilt, erfährt, welche Zeitzone du gemeint hast, deshalb verschiebt sich das Zeitfenster bei der Zeitumstellung nicht um eine Stunde. Das Ende ist der Moment, in dem es aufhört. Stell 07:00 ein, und um sieben gehört die Box wieder dir. Ein Ende vor dem Anfang läuft über Mitternacht, und genau das heißt „während ich schlafe“ meistens. Arbeit, die schon läuft, darf fertig werden. Sobald das Zeitfenster zu ist, startet nichts Neues.",
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
    en: "Letters, digits and hyphens, starting and ending with a letter or a digit, up to 63 characters. Everything about this box hangs off the name. Change it and the address you use to reach it changes with it, along with the addresses the apps hand out. There is no other way into this box, so a name it cannot answer to is a box you cannot reach. The name is found over the local network, by mDNS. A computer that cannot find it reaches the box by the numeric address on its screen, which a rename does not change.",
    sk: "Písmená, číslice a pomlčky, na začiatku aj na konci písmeno alebo číslica, najviac 63 znakov. Všetko na tomto zariadení visí na názve. Keď ho zmeníte, zmení sa aj adresa, na ktorej ho nájdete, a s ňou aj adresy, ktoré rozdávajú aplikácie. Do zariadenia sa nedá dostať inak, takže názov, na ktorý nevie odpovedať, znamená zariadenie, na ktoré sa nedostanete. Názov sa hľadá cez lokálnu sieť, cez mDNS. Počítač, ktorý ho nenájde, sa na zariadenie dostane cez číselnú adresu z jeho obrazovky, ktorú premenovanie nemení.",
    de: "Buchstaben, Ziffern und Bindestriche, am Anfang und Ende ein Buchstabe oder eine Ziffer, höchstens 63 Zeichen. An dem Namen hängt alles auf dieser Box. Änderst du ihn, ändert sich die Adresse, unter der du sie erreichst, und mit ihr die Adressen, die die Apps herausgeben. Es gibt keinen anderen Weg in diese Box, also ist ein Name, auf den sie nicht hören kann, eine Box, die du nicht erreichst. Der Name wird über das lokale Netz gefunden, per mDNS. Ein Computer, der ihn nicht findet, erreicht die Box über die Zahlenadresse auf ihrem Bildschirm, die eine Umbenennung nicht ändert.",
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
    en: "With \"reachable from outside\" on, this box opens a way out to a small relay so you can get at it from anywhere. Nothing on your router needs opening. The port is an inside detail. The files app answers on it behind the front door, and there is no reason to change it unless something else on this box already wants that number.",
    sk: "Keď je zapnuté „dostupné mimo domova“, zariadenie si otvorí cestu von k malému sprostredkovateľovi, aby ste sa k nemu dostali odkiaľkoľvek. Na routeri netreba nič otvárať. Port je vnútorný detail. Aplikácia na súbory na ňom odpovedá za vstupnými dverami a netreba ho meniť, pokiaľ to isté číslo nechce niečo iné na tomto zariadení.",
    de: "Mit „von außerhalb erreichbar“ öffnet diese Box einen Weg nach draußen zu einem kleinen Relay, sodass du von überall an sie herankommst. An deinem Router muss nichts geöffnet werden. Der Port ist ein internes Detail. Die Dateien-App antwortet darauf hinter der Haustür, und es gibt keinen Grund, ihn zu ändern, außer etwas anderes auf dieser Box will dieselbe Nummer.",
  },
  "panes.network.domains.title": {
    en: "Your own domain",
    sk: "Vlastná doména",
    de: "Eigene Domain",
  },
  "panes.network.domains.loading": {
    en: "Asking the edge…",
    sk: "Pýtam sa okrajového servera…",
    de: "Frage den Edge…",
  },
  "panes.network.domains.unreachable": {
    en: "The edge did not answer",
    sk: "Okrajový server neodpovedal",
    de: "Der Edge hat nicht geantwortet",
  },
  "panes.network.domains.unavailable": {
    en: "Not offered to this box",
    sk: "Pre toto zariadenie nie je k dispozícii",
    de: "Für diese Box nicht verfügbar",
  },
  "panes.network.domains.noOfficialEdge": {
    en: "Custom domains come from an official LosOS edge, and none is in reach.",
    sk: "Vlastné domény poskytuje oficiálny okrajový server LosOS a žiadny nie je dostupný.",
    de: "Eigene Domains kommen von einem offiziellen LosOS-Edge, und keiner ist erreichbar.",
  },
  "panes.network.domains.notOffered": {
    en: "Turn on \"Reachable from outside your home\" first. If it is on, the edge this box uses does not hand out domain names.",
    sk: "Najprv zapnite „Dostupné aj mimo domova“. Ak je zapnuté, okrajový server tohto zariadenia doménové mená neprideľuje.",
    de: "Schalte zuerst „Auch von außerhalb erreichbar“ ein. Ist es an, vergibt der Edge dieser Box keine Domainnamen.",
  },
  "panes.network.domains.needsStripe": {
    en: "Needs a Stripe account that Stripe has checked",
    sk: "Vyžaduje účet Stripe overený spoločnosťou Stripe",
    de: "Braucht ein von Stripe geprüftes Stripe-Konto",
  },
  "panes.network.domains.needsStripeDetail": {
    en: "The edge gives a domain only to a box whose payout account at Stripe is fully set up. That account is made on the Market pane once the market opens.",
    sk: "Okrajový server pridelí doménu len zariadeniu, ktoré má v Stripe úplne nastavený účet na výplaty. Ten sa zakladá na paneli Trh, keď sa trh otvorí.",
    de: "Der Edge vergibt eine Domain nur an eine Box, deren Auszahlungskonto bei Stripe vollständig eingerichtet ist. Dieses Konto wird im Bereich Markt angelegt, sobald der Markt öffnet.",
  },
  "panes.network.domains.target": {
    en: "This box's name on the edge",
    sk: "Názov tohto zariadenia na okrajovom serveri",
    de: "Name dieser Box auf dem Edge",
  },
  "panes.network.domains.targetDetail": {
    en: "Point your domain here with a CNAME record.",
    sk: "Nasmerujte sem svoju doménu záznamom CNAME.",
    de: "Richte deine Domain mit einem CNAME-Eintrag hierher.",
  },
  "panes.network.domains.add": {
    en: "Add a domain you own",
    sk: "Pridať doménu, ktorú vlastníte",
    de: "Eine eigene Domain hinzufügen",
  },
  "panes.network.domains.addButton": {
    en: "Add",
    sk: "Pridať",
    de: "Hinzufügen",
  },
  "panes.network.domains.full": {
    en: "This box already has {count} domains, the most an edge allows.",
    sk: "Toto zariadenie už má {count} domén, viac okrajový server nedovolí.",
    de: "Diese Box hat schon {count} Domains, mehr erlaubt der Edge nicht.",
  },
  "panes.network.domains.live": {
    en: "Live",
    sk: "Funguje",
    de: "Aktiv",
  },
  "panes.network.domains.waiting": {
    en: "Waiting for DNS",
    sk: "Čaká na DNS",
    de: "Wartet auf DNS",
  },
  "panes.network.domains.remove": {
    en: "Remove",
    sk: "Odstrániť",
    de: "Entfernen",
  },
  "panes.network.domains.seen": {
    en: "✓ seen",
    sk: "✓ nájdené",
    de: "✓ gefunden",
  },
  "panes.network.domains.notSeen": {
    en: "not seen yet",
    sk: "zatiaľ nenájdené",
    de: "noch nicht gefunden",
  },
  "panes.network.domains.apex": {
    en: "For a domain with nothing in front of it, such as example.org itself, a CNAME is not allowed. Use A or AAAA records to {addresses} there instead.",
    sk: "Pri doméne, pred ktorou nič nie je, napríklad samotnej example.org, CNAME nie je dovolený. Použite tam namiesto neho záznamy A alebo AAAA na {addresses}.",
    de: "Für eine Domain, vor der nichts steht, etwa example.org selbst, ist kein CNAME erlaubt. Nimm dort stattdessen A- oder AAAA-Einträge auf {addresses}.",
  },
  "panes.network.domains.caption": {
    en: "Add the two records shown under a domain at the company you bought it from. The edge looks them up itself every half minute and switches the domain on as soon as both are right, with a certificate of its own. LosOS cloud then answers on it from anywhere, at /nextcloud. The TXT value proves the domain is yours. It is made from this box's identity and its Stripe account without revealing either.",
    sk: "Pridajte dva záznamy uvedené pod doménou u firmy, od ktorej ste ju kúpili. Okrajový server si ich každú pol minútu sám overí a doménu zapne, hneď ako sú oba správne, aj s vlastným certifikátom. LosOS cloud potom na nej odpovedá odkiaľkoľvek, na adrese /nextcloud. Hodnota TXT dokazuje, že doména je vaša. Vzniká z identity tohto zariadenia a jeho účtu Stripe bez toho, aby jedno či druhé prezradila.",
    de: "Trage die zwei Einträge unter einer Domain bei der Firma ein, bei der du sie gekauft hast. Der Edge prüft sie jede halbe Minute selbst und schaltet die Domain ein, sobald beide stimmen, mit eigenem Zertifikat. LosOS cloud antwortet dann von überall darauf, unter /nextcloud. Der TXT-Wert beweist, dass die Domain dir gehört. Er entsteht aus der Identität dieser Box und ihrem Stripe-Konto, ohne eines davon preiszugeben.",
  },
  "panes.network.domains.failedTitle": {
    en: "The edge refused",
    sk: "Okrajový server odmietol",
    de: "Der Edge hat abgelehnt",
  },
  "panes.network.domains.failed": {
    en: "The edge did not take the change. Try again in a moment.",
    sk: "Okrajový server zmenu neprijal. Skúste to o chvíľu znova.",
    de: "Der Edge hat die Änderung nicht angenommen. Versuch es gleich noch einmal.",
  },
  "panes.network.domains.addedTitle": {
    en: "Domain added",
    sk: "Doména pridaná",
    de: "Domain hinzugefügt",
  },
  "panes.network.domains.addedBody": {
    en: "Add the two records shown under it at your DNS provider.",
    sk: "Pridajte u poskytovateľa DNS dva záznamy uvedené pod ňou.",
    de: "Trage die zwei Einträge darunter bei deinem DNS-Anbieter ein.",
  },
  "panes.network.domains.removedTitle": {
    en: "{domain} removed",
    sk: "{domain} odstránená",
    de: "{domain} entfernt",
  },
  "panes.network.domains.problem.stripeAccount": {
    en: "The Stripe account behind this domain is no longer ready, so the edge stopped serving it.",
    sk: "Účet Stripe za touto doménou už nie je pripravený, preto ju okrajový server prestal obsluhovať.",
    de: "Das Stripe-Konto hinter dieser Domain ist nicht mehr bereit, deshalb bedient der Edge sie nicht mehr.",
  },
  "panes.network.domains.problem.txtMissing": {
    en: "The TXT record is not there yet. New records can take a few minutes to appear.",
    sk: "Záznam TXT tam ešte nie je. Nové záznamy sa môžu objaviť až o pár minút.",
    de: "Der TXT-Eintrag ist noch nicht da. Neue Einträge können ein paar Minuten brauchen.",
  },
  "panes.network.domains.problem.txtWrong": {
    en: "A TXT record is there, but its value is not the one shown here.",
    sk: "Záznam TXT tam je, ale jeho hodnota nie je tá, ktorá je uvedená tu.",
    de: "Ein TXT-Eintrag ist da, aber sein Wert ist nicht der hier gezeigte.",
  },
  "panes.network.domains.problem.notPointing": {
    en: "The TXT record is right. The domain does not point at this box's name yet.",
    sk: "Záznam TXT je správny. Doména ešte nesmeruje na názov tohto zariadenia.",
    de: "Der TXT-Eintrag stimmt. Die Domain zeigt noch nicht auf den Namen dieser Box.",
  },
  "panes.network.domains.problem.takenElsewhere": {
    en: "Another box already uses this domain on the edge.",
    sk: "Túto doménu už na okrajovom serveri používa iné zariadenie.",
    de: "Eine andere Box nutzt diese Domain schon auf dem Edge.",
  },
  "panes.network.domains.problem.lookupFailed": {
    en: "The edge could not look the domain up. It tries again by itself.",
    sk: "Okrajový server nevedel doménu vyhľadať. Skúsi to znova sám.",
    de: "Der Edge konnte die Domain nicht nachschlagen. Er versucht es von selbst erneut.",
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

  // ── Backup ──────────────────────────────────────────────────────────────
  "panes.backup.loading": {
    en: "Reading the backup settings…",
    sk: "Načítavam nastavenia zálohy…",
    de: "Lese die Sicherungseinstellungen…",
  },
  "panes.backup.unreachable": {
    en: "The box did not answer about its backups.",
    sk: "Zariadenie neodpovedalo na otázku o zálohách.",
    de: "Die Box hat zu ihren Sicherungen nicht geantwortet.",
  },
  "panes.backup.target.title": {
    en: "Where backups go",
    sk: "Kam idú zálohy",
    de: "Wohin Sicherungen gehen",
  },
  "panes.backup.target.endpoint": { en: "Address", sk: "Adresa", de: "Adresse" },
  "panes.backup.target.bucket": { en: "Bucket", sk: "Bucket", de: "Bucket" },
  "panes.backup.target.prefix": { en: "Folder", sk: "Priečinok", de: "Ordner" },
  "panes.backup.target.region": { en: "Region", sk: "Región", de: "Region" },
  "panes.backup.target.key": { en: "Access key", sk: "Prístupový kľúč", de: "Zugriffsschlüssel" },
  "panes.backup.target.secret": { en: "Secret key", sk: "Tajný kľúč", de: "Geheimer Schlüssel" },
  "panes.backup.target.secretKept": {
    en: "Stored. Leave it empty to keep it.",
    sk: "Uložený. Nechajte prázdne, ak ho chcete ponechať.",
    de: "Gespeichert. Leer lassen, um ihn zu behalten.",
  },
  "panes.backup.target.optional": { en: "Optional", sk: "Nepovinné", de: "Optional" },
  "panes.backup.target.change": { en: "Change…", sk: "Zmeniť…", de: "Ändern…" },
  "panes.backup.target.forget": {
    en: "Forget this bucket",
    sk: "Zabudnúť tento bucket",
    de: "Diesen Bucket vergessen",
  },
  "panes.backup.target.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "panes.backup.target.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "panes.backup.target.caption": {
    en: "Any S3-compatible bucket works: Amazon S3, Backblaze B2, Wasabi, or a MinIO on your own network. The box encrypts every backup before it leaves, so the bucket holds only data nobody can read without your recovery code. The secret key is never shown again once saved.",
    sk: "Funguje akýkoľvek bucket kompatibilný s S3: Amazon S3, Backblaze B2, Wasabi alebo MinIO vo vašej vlastnej sieti. Zariadenie každú zálohu pred odoslaním zašifruje, takže v buckete sú len dáta, ktoré bez vášho kódu obnovy nikto neprečíta. Tajný kľúč sa po uložení už nikdy nezobrazí.",
    de: "Jeder S3-kompatible Bucket geht: Amazon S3, Backblaze B2, Wasabi oder ein MinIO im eigenen Netz. Die Box verschlüsselt jede Sicherung, bevor sie hinausgeht, also liegen im Bucket nur Daten, die ohne deinen Wiederherstellungscode niemand lesen kann. Der geheime Schlüssel wird nach dem Speichern nie wieder angezeigt.",
  },
  "panes.backup.code.title": {
    en: "The key to your backups",
    sk: "Kľúč k vašim zálohám",
    de: "Der Schlüssel zu deinen Sicherungen",
  },
  "panes.backup.code.row": { en: "Recovery code", sk: "Kód obnovy", de: "Wiederherstellungscode" },
  "panes.backup.code.detail": {
    en: "Write it down somewhere other than this box.",
    sk: "Zapíšte si ho niekam mimo tohto zariadenia.",
    de: "Schreib ihn irgendwo außerhalb dieser Box auf.",
  },
  "panes.backup.code.failed": {
    en: "The box did not hand out the code. Try again in a moment.",
    sk: "Zariadenie kód nevydalo. Skúste to o chvíľu znova.",
    de: "Die Box hat den Code nicht herausgegeben. Versuch es gleich noch einmal.",
  },
  "panes.backup.code.show": { en: "Show", sk: "Zobraziť", de: "Anzeigen" },
  "panes.backup.code.hide": { en: "Hide", sk: "Skryť", de: "Verbergen" },
  "panes.backup.code.caption": {
    en: "Every backup is encrypted with this code, and nobody can open one without it, LosOS included. Keep it on paper or in a password manager. After a restore the box keeps the code of the backup it came from.",
    sk: "Každá záloha je zašifrovaná týmto kódom a bez neho ju nikto neotvorí, ani LosOS. Uschovajte si ho na papieri alebo v správcovi hesiel. Po obnove si zariadenie ponechá kód zálohy, z ktorej pochádza.",
    de: "Jede Sicherung ist mit diesem Code verschlüsselt, und ohne ihn kann niemand sie öffnen, auch LosOS nicht. Bewahre ihn auf Papier oder in einem Passwortmanager auf. Nach einer Wiederherstellung behält die Box den Code der Sicherung, aus der sie kam.",
  },
  "panes.backup.runs.title": { en: "Backups", sk: "Zálohy", de: "Sicherungen" },
  "panes.backup.runs.last": { en: "Last backup", sk: "Posledná záloha", de: "Letzte Sicherung" },
  "panes.backup.runs.never": {
    en: "None yet.",
    sk: "Zatiaľ žiadna.",
    de: "Noch keine.",
  },
  "panes.backup.runs.lastDetail": {
    en: { one: "{size}, {count} file", other: "{size}, {count} files" },
    sk: {
      one: "{size}, {count} súbor",
      few: "{size}, {count} súbory",
      many: "{size}, {count} súborov",
      other: "{size}, {count} súborov",
    },
    de: { one: "{size}, {count} Datei", other: "{size}, {count} Dateien" },
  },
  "panes.backup.runs.running": {
    en: "Backing up…",
    sk: "Zálohujem…",
    de: "Sichere…",
  },
  "panes.backup.runs.runningDetail": {
    en: "The first backup copies everything. Later ones send only what changed.",
    sk: "Prvá záloha skopíruje všetko. Ďalšie posielajú len to, čo sa zmenilo.",
    de: "Die erste Sicherung kopiert alles. Spätere schicken nur, was sich geändert hat.",
  },
  "panes.backup.runs.failed": {
    en: "The last backup did not finish.",
    sk: "Posledná záloha sa nedokončila.",
    de: "Die letzte Sicherung wurde nicht fertig.",
  },
  "panes.backup.runs.now": { en: "Back up now", sk: "Zálohovať teraz", de: "Jetzt sichern" },
  "panes.backup.runs.needsTarget": {
    en: "Set up a bucket first.",
    sk: "Najprv nastavte bucket.",
    de: "Richte zuerst einen Bucket ein.",
  },
  "panes.backup.runs.button": { en: "Back up", sk: "Zálohovať", de: "Sichern" },
  "panes.backup.runs.caption": {
    en: "A backup holds LosOS cloud's files and database, LosOS Git's repositories, the folder you share with the mesh, your settings and the look of the home page. The bucket keeps the last seven.",
    sk: "Záloha obsahuje súbory a databázu LosOS cloud, repozitáre LosOS Git, priečinok, ktorý zdieľate s mesh sieťou, vaše nastavenia a vzhľad domovskej stránky. Bucket uchováva posledných sedem.",
    de: "Eine Sicherung enthält die Dateien und die Datenbank von LosOS cloud, die Repositorys von LosOS Git, den Ordner, den du mit dem Mesh teilst, deine Einstellungen und das Aussehen der Startseite. Der Bucket behält die letzten sieben.",
  },
  "panes.backup.restore.title": {
    en: "Restore",
    sk: "Obnova",
    de: "Wiederherstellen",
  },
  "panes.backup.restore.running": {
    en: "Restoring…",
    sk: "Obnovujem…",
    de: "Stelle wieder her…",
  },
  "panes.backup.restore.runningDetail": {
    en: "LosOS cloud and LosOS Git are stopped until it is done.",
    sk: "LosOS cloud a LosOS Git sú dovtedy zastavené.",
    de: "LosOS cloud und LosOS Git sind bis dahin angehalten.",
  },
  "panes.backup.restore.done": {
    en: "Restored",
    sk: "Obnovené",
    de: "Wiederhergestellt",
  },
  "panes.backup.restore.doneDetail": {
    en: "The box now keeps the recovery code you typed. The restored settings are applied by themselves.",
    sk: "Zariadenie si teraz ponecháva kód obnovy, ktorý ste zadali. Obnovené nastavenia sa použijú samy.",
    de: "Die Box behält jetzt den Wiederherstellungscode, den du eingegeben hast. Die wiederhergestellten Einstellungen werden von selbst übernommen.",
  },
  "panes.backup.restore.failed": {
    en: "The restore did not finish.",
    sk: "Obnova sa nedokončila.",
    de: "Die Wiederherstellung wurde nicht fertig.",
  },
  "panes.backup.restore.code": {
    en: "Recovery code",
    sk: "Kód obnovy",
    de: "Wiederherstellungscode",
  },
  "panes.backup.restore.button": { en: "Restore…", sk: "Obnoviť…", de: "Wiederherstellen…" },
  "panes.backup.restore.caption": {
    en: "Type the recovery code of the box that made the backup, the one you wrote down from its Backup page. After an erase or a new install, set up the same bucket above first. The newest backup in it comes back.",
    sk: "Zadajte kód obnovy zariadenia, ktoré zálohu vytvorilo, ten, ktorý ste si zapísali z jeho stránky Záloha. Po vymazaní alebo novej inštalácii najprv vyššie nastavte ten istý bucket. Vráti sa najnovšia záloha v ňom.",
    de: "Gib den Wiederherstellungscode der Box ein, die die Sicherung gemacht hat, den du dir auf ihrer Seite Sicherung notiert hast. Nach einem Löschen oder einer Neuinstallation richte oben zuerst denselben Bucket ein. Zurück kommt die neueste Sicherung darin.",
  },
  "panes.backup.restore.dialogTitle": {
    en: "Restore the newest backup?",
    sk: "Obnoviť najnovšiu zálohu?",
    de: "Die neueste Sicherung wiederherstellen?",
  },
  "panes.backup.restore.dialogBody": {
    en: "What is on this box now is replaced by what is in the backup.",
    sk: "To, čo je teraz na zariadení, nahradí obsah zálohy.",
    de: "Was jetzt auf der Box ist, wird durch den Inhalt der Sicherung ersetzt.",
  },
  "panes.backup.restore.replaces": {
    en: "Files, repositories, the shared folder and settings made since the backup are lost.",
    sk: "Súbory, repozitáre, zdieľaný priečinok a nastavenia od vytvorenia zálohy sa stratia.",
    de: "Dateien, Repositorys, der geteilte Ordner und Einstellungen seit der Sicherung gehen verloren.",
  },
  "panes.backup.restore.codeStays": {
    en: "The box keeps the recovery code you typed from now on. Its own code stops working.",
    sk: "Zariadenie si odteraz ponechá kód obnovy, ktorý ste zadali. Jeho vlastný kód prestane platiť.",
    de: "Die Box behält ab jetzt den eingegebenen Wiederherstellungscode. Ihr eigener Code gilt nicht mehr.",
  },
  "panes.backup.restore.password": {
    en: "Sign in afterwards with the password you had when the backup was made.",
    sk: "Potom sa prihláste heslom, ktoré ste mali pri vytvorení zálohy.",
    de: "Melde dich danach mit dem Passwort an, das du beim Erstellen der Sicherung hattest.",
  },
  "panes.backup.restore.keep": {
    en: "Keep this box as it is",
    sk: "Ponechať zariadenie, ako je",
    de: "Box so lassen",
  },
  "panes.backup.restore.confirm": { en: "Restore", sk: "Obnoviť", de: "Wiederherstellen" },
  "panes.backup.toast.targetSaved": {
    en: "Bucket saved",
    sk: "Bucket uložený",
    de: "Bucket gespeichert",
  },
  "panes.backup.toast.targetForgotten": {
    en: "Bucket forgotten. The backups in it are still there.",
    sk: "Bucket zabudnutý. Zálohy v ňom zostali.",
    de: "Bucket vergessen. Die Sicherungen darin sind noch da.",
  },
  "panes.backup.toast.targetFailed": {
    en: "The bucket was not saved",
    sk: "Bucket sa neuložil",
    de: "Der Bucket wurde nicht gespeichert",
  },
  "panes.backup.toast.backupFailed": {
    en: "The backup did not start",
    sk: "Záloha sa nespustila",
    de: "Die Sicherung ist nicht gestartet",
  },
  "panes.backup.toast.restoreFailed": {
    en: "The restore did not start",
    sk: "Obnova sa nespustila",
    de: "Die Wiederherstellung ist nicht gestartet",
  },

  // ── Erase ───────────────────────────────────────────────────────────────
  "panes.erase.title": {
    en: "Erase everything",
    sk: "Vymazať všetko",
    de: "Alles löschen",
  },
  "panes.erase.row": {
    en: "Erase this box",
    sk: "Vymazať toto zariadenie",
    de: "Diese Box löschen",
  },
  "panes.erase.rowDetail": {
    en: "Your files, repositories, settings and password. The box stays installed and greets the next owner with the setup wizard.",
    sk: "Vaše súbory, repozitáre, nastavenia a heslo. Zariadenie zostane nainštalované a ďalšieho majiteľa privíta sprievodcom nastavením.",
    de: "Deine Dateien, Repositorys, Einstellungen und dein Passwort. Die Box bleibt installiert und begrüßt den nächsten Besitzer mit dem Einrichtungsassistenten.",
  },
  "panes.erase.button": { en: "Erase…", sk: "Vymazať…", de: "Löschen…" },
  "panes.erase.caption": {
    en: "The erase waits {grace} before it starts, and Cancel stops it until then. The box then gives up its custom domains, its market listings and its place on the edge, and erases the data when it restarts. With a bucket set up under Backup it backs up first, so your recovery code can bring everything back.",
    sk: "Vymazanie počká {grace}, kým sa začne, a dovtedy ho tlačidlo Zrušiť zastaví. Zariadenie potom uvoľní svoje vlastné domény, ponuky na trhu a miesto na okrajovom serveri a pri reštarte vymaže dáta. Ak máte v sekcii Záloha nastavený bucket, najprv sa zálohuje, takže váš kód obnovy vráti všetko späť.",
    de: "Das Löschen wartet {grace}, bevor es beginnt, und bis dahin hält Abbrechen es auf. Danach gibt die Box ihre eigenen Domains, ihre Marktangebote und ihren Platz auf dem Edge auf und löscht die Daten beim Neustart. Mit einem Bucket unter Sicherung sichert sie vorher, also bringt dein Wiederherstellungscode alles zurück.",
  },
  "panes.erase.countdown": {
    en: "Erasing soon",
    sk: "Čoskoro sa vymaže",
    de: "Wird bald gelöscht",
  },
  "panes.erase.countdownDetail": {
    en: "Nothing has changed yet. Cancel keeps everything as it is.",
    sk: "Zatiaľ sa nič nezmenilo. Zrušiť ponechá všetko, ako je.",
    de: "Noch hat sich nichts geändert. Abbrechen lässt alles, wie es ist.",
  },
  "panes.erase.countdownBackedUp": {
    en: "The backup is in the bucket. Nothing else has changed yet, and Cancel keeps everything as it is.",
    sk: "Záloha je v buckete. Nič iné sa zatiaľ nezmenilo a Zrušiť ponechá všetko, ako je.",
    de: "Die Sicherung liegt im Bucket. Sonst hat sich noch nichts geändert, und Abbrechen lässt alles, wie es ist.",
  },
  "panes.erase.cancel": { en: "Cancel erase", sk: "Zrušiť vymazanie", de: "Löschen abbrechen" },
  "panes.erase.dismiss": { en: "Dismiss", sk: "Zavrieť", de: "Schließen" },
  "panes.erase.phase.backingUp": {
    en: "Backing up before the erase",
    sk: "Zálohujem pred vymazaním",
    de: "Sichere vor dem Löschen",
  },
  "panes.erase.phase.backingUpDetail": {
    en: "The countdown starts when the backup is in the bucket.",
    sk: "Odpočet sa začne, keď bude záloha v buckete.",
    de: "Der Countdown beginnt, wenn die Sicherung im Bucket liegt.",
  },
  "panes.erase.phase.leaving": {
    en: "Leaving the edge",
    sk: "Odchádzam z okrajového servera",
    de: "Verlasse den Edge",
  },
  "panes.erase.phase.noStopping": {
    en: "This can no longer be stopped.",
    sk: "Toto sa už nedá zastaviť.",
    de: "Das lässt sich nicht mehr aufhalten.",
  },
  "panes.erase.phase.resetting": {
    en: "Putting the settings back",
    sk: "Vraciam nastavenia",
    de: "Setze die Einstellungen zurück",
  },
  "panes.erase.phase.restarting": {
    en: "Restarting to erase the data",
    sk: "Reštartujem a mažem dáta",
    de: "Starte neu, um die Daten zu löschen",
  },
  "panes.erase.phase.restartingDetail": {
    en: "This page stops answering while the box restarts.",
    sk: "Počas reštartu táto stránka prestane odpovedať.",
    de: "Während des Neustarts antwortet diese Seite nicht.",
  },
  "panes.erase.phase.gone": {
    en: "The box is restarting",
    sk: "Zariadenie sa reštartuje",
    de: "Die Box startet neu",
  },
  "panes.erase.phase.goneDetail": {
    en: "When it comes back it is empty and opens the setup wizard.",
    sk: "Keď sa vráti, bude prázdne a otvorí sprievodcu nastavením.",
    de: "Wenn sie zurück ist, ist sie leer und öffnet den Einrichtungsassistenten.",
  },
  "panes.erase.phase.failed": {
    en: "The backup before the erase failed",
    sk: "Záloha pred vymazaním zlyhala",
    de: "Die Sicherung vor dem Löschen ist fehlgeschlagen",
  },
  "panes.erase.phase.failedDetail": {
    en: "Nothing was erased. Check the bucket under Backup, or erase without a backup.",
    sk: "Nič sa nevymazalo. Skontrolujte bucket v sekcii Záloha alebo vymažte bez zálohy.",
    de: "Es wurde nichts gelöscht. Prüfe den Bucket unter Sicherung oder lösche ohne Sicherung.",
  },
  "panes.erase.dialogTitle": {
    en: "Erase everything on this box?",
    sk: "Vymazať všetko na tomto zariadení?",
    de: "Alles auf dieser Box löschen?",
  },
  "panes.erase.dialogBody": {
    en: "You have {grace} to change your mind. After that it cannot be stopped.",
    sk: "Máte {grace} na rozmyslenie. Potom sa to už nedá zastaviť.",
    de: "Du hast {grace}, es dir anders zu überlegen. Danach lässt es sich nicht mehr aufhalten.",
  },
  "panes.erase.goes": {
    en: "Your files, repositories, the shared folder, every setting, the password, the spare key and the recovery code are erased.",
    sk: "Vymažú sa vaše súbory, repozitáre, zdieľaný priečinok, všetky nastavenia, heslo, náhradný kľúč a kód obnovy.",
    de: "Gelöscht werden deine Dateien, Repositorys, der geteilte Ordner, jede Einstellung, das Passwort, der Ersatzschlüssel und der Wiederherstellungscode.",
  },
  "panes.erase.outside": {
    en: "Your custom domains, market listings and the box's place on the edge are given up.",
    sk: "Vaše vlastné domény, ponuky na trhu a miesto zariadenia na okrajovom serveri sa uvoľnia.",
    de: "Deine eigenen Domains, Marktangebote und der Platz der Box auf dem Edge werden aufgegeben.",
  },
  "panes.erase.stays": {
    en: "LosOS stays installed. Backups already in your bucket stay there.",
    sk: "LosOS zostane nainštalovaný. Zálohy, ktoré už sú vo vašom buckete, tam zostanú.",
    de: "LosOS bleibt installiert. Sicherungen, die schon in deinem Bucket liegen, bleiben dort.",
  },
  "panes.erase.backupFirst": {
    en: "Back up first",
    sk: "Najprv zálohovať",
    de: "Vorher sichern",
  },
  "panes.erase.backupFirstDetail": {
    en: "Write down the recovery code from Backup first. It is the only key to this backup.",
    sk: "Najprv si zapíšte kód obnovy zo sekcie Záloha. Je to jediný kľúč k tejto zálohe.",
    de: "Notiere dir vorher den Wiederherstellungscode unter Sicherung. Er ist der einzige Schlüssel zu dieser Sicherung.",
  },
  "panes.erase.noTarget": {
    en: "No bucket is set up under Backup, so nothing can be brought back after this erase.",
    sk: "V sekcii Záloha nie je nastavený žiadny bucket, takže po tomto vymazaní sa nič nedá vrátiť.",
    de: "Unter Sicherung ist kein Bucket eingerichtet, also lässt sich nach diesem Löschen nichts zurückholen.",
  },
  "panes.erase.keep": {
    en: "Keep everything",
    sk: "Ponechať všetko",
    de: "Alles behalten",
  },
  "panes.erase.confirm": {
    en: "Start the countdown",
    sk: "Spustiť odpočet",
    de: "Countdown starten",
  },
  "panes.erase.report.title": {
    en: "The last erase",
    sk: "Posledné vymazanie",
    de: "Das letzte Löschen",
  },
  "panes.erase.report.when": { en: "Erased", sk: "Vymazané", de: "Gelöscht" },
  "panes.erase.report.domains": {
    en: "Custom domains removed",
    sk: "Odstránené vlastné domény",
    de: "Entfernte eigene Domains",
  },
  "panes.erase.report.listings": {
    en: "Market listings closed",
    sk: "Zrušené ponuky na trhu",
    de: "Geschlossene Marktangebote",
  },
  "panes.erase.report.edge": {
    en: "Left the edge",
    sk: "Odišlo z okrajového servera",
    de: "Edge verlassen",
  },
  "panes.erase.report.yes": { en: "Yes", sk: "Áno", de: "Ja" },
  "panes.erase.report.no": { en: "No", sk: "Nie", de: "Nein" },
  "panes.erase.report.problems": {
    en: "The edge did not confirm every step, so something may still be listed there under this box.",
    sk: "Okrajový server nepotvrdil každý krok, takže tam pod týmto zariadením môže ešte niečo zostať.",
    de: "Der Edge hat nicht jeden Schritt bestätigt, also kann dort unter dieser Box noch etwas eingetragen sein.",
  },
  "panes.erase.report.noEdge": {
    en: "The box had no edge, so it had nothing to give up outside itself.",
    sk: "Zariadenie nemalo okrajový server, takže mimo seba nemalo čo uvoľniť.",
    de: "Die Box hatte keinen Edge, also gab es außerhalb nichts aufzugeben.",
  },
  "panes.erase.toast.failed": {
    en: "The erase did not start",
    sk: "Vymazanie sa nespustilo",
    de: "Das Löschen ist nicht gestartet",
  },
  "panes.erase.toast.cancelFailed": {
    en: "The erase could not be cancelled",
    sk: "Vymazanie sa nedalo zrušiť",
    de: "Das Löschen ließ sich nicht abbrechen",
  },
  "panes.erase.toast.cancelled": {
    en: "Erase cancelled",
    sk: "Vymazanie zrušené",
    de: "Löschen abgebrochen",
  },
  "panes.erase.toast.cancelledBody": {
    en: "Nothing on the box or outside it was changed.",
    sk: "Na zariadení ani mimo neho sa nič nezmenilo.",
    de: "Weder auf der Box noch außerhalb wurde etwas geändert.",
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
    en: "Shuts the door two jobs can otherwise listen through. This is the expensive one. Roughly half the speed, and you will notice it converting video.",
    sk: "Zavrie dvere, cez ktoré by sa dve úlohy mohli navzájom počúvať. Toto je tá drahá voľba. Zhruba polovičná rýchlosť a pri prevode videa to spoznáte.",
    de: "Schließt die Tür, durch die zwei Aufgaben sonst mithören können. Das ist die teure Option. Ungefähr halbe Geschwindigkeit, und beim Umwandeln von Videos merkst du es.",
  },
  "panes.security.caption": {
    en: "This box already protects itself in the ways that cost nothing, and those are always on and not listed here. The four above are the ones with a price, so they are yours to decide. If you lend spare capacity to the mesh, the middle two are the ones worth reading twice. They are what stands between somebody else's job and yours.",
    sk: "Toto zariadenie sa už chráni všetkými spôsobmi, ktoré nič nestoja. Tie sú vždy zapnuté a nie sú tu uvedené. Štyri vyššie niečo stoja, preto je rozhodnutie na vás. Ak požičiavate voľný výkon sieti mesh, prostredné dve si prečítajte dvakrát. Práve ony stoja medzi úlohou niekoho iného a vašou.",
    de: "Diese Box schützt sich schon auf alle Arten, die nichts kosten. Die sind immer an und hier nicht aufgeführt. Die vier oben haben einen Preis, deshalb entscheidest du. Wenn du freie Rechenleistung ans Mesh verleihst, lohnt es sich, die mittleren beiden zweimal zu lesen. Sie stehen zwischen der Aufgabe von jemand anderem und deiner.",
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
  // The disk-sharing group, moved here from Storage with the switch.
  "panes.market.share.title": {
    en: "The mesh",
    sk: "Sieť mesh",
    de: "Das Mesh",
  },
  "panes.market.share.switch": {
    en: "Share this box's disk with the mesh",
    sk: "Zdieľať disk tohto zariadenia so sieťou mesh",
    de: "Die Festplatte dieser Box mit dem Mesh teilen",
  },
  "panes.market.share.ifDies": {
    en: "If this disk dies",
    sk: "Ak tento disk zlyhá",
    de: "Wenn diese Festplatte stirbt",
  },
  "panes.market.share.rebuilds": {
    en: "rebuilds in about 40 minutes",
    sk: "obnoví sa asi za 40 minút",
    de: "in etwa 40 Minuten wiederhergestellt",
  },
  "panes.market.share.notCopied": {
    en: "nothing here is copied anywhere",
    sk: "nič odtiaľto sa nikam nekopíruje",
    de: "nichts hier wird irgendwohin kopiert",
  },
  "panes.market.share.caption": {
    en: "Your box lends its spare room to other people's boxes, and copies of your own files are kept on theirs. The two travel together, because a pool you take from but never give to is not a pool. While this is off, the shared half of the disk stays locked and unreadable by anything on this box, including the box itself.",
    sk: "Vaše zariadenie požičiava voľné miesto zariadeniam iných ľudí a kópie vašich súborov sa uchovávajú na ich zariadeniach. Jedno nejde bez druhého, pretože spoločný fond, z ktorého len beriete a nikdy doň nedávate, nie je spoločný fond. Kým je to vypnuté, zdieľaná polovica disku zostáva zamknutá a nečitateľná pre čokoľvek na tomto zariadení vrátane zariadenia samotného.",
    de: "Deine Box verleiht ihren freien Platz an die Boxen anderer Leute, und Kopien deiner eigenen Dateien liegen auf ihren. Beides gehört zusammen, denn ein Pool, aus dem du nur nimmst und in den du nie gibst, ist kein Pool. Solange das aus ist, bleibt die geteilte Hälfte der Festplatte gesperrt und für nichts auf dieser Box lesbar, auch nicht für die Box selbst.",
  },
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
  "panes.market.unavailable.noOfficialEdge": {
    en: "The edge proxy this box found is not run by LosOS. Sharing storage through it works; trading needs an official LosOS edge in reach.",
    sk: "Okrajový proxy server, ktorý toto zariadenie našlo, neprevádzkuje LosOS. Zdieľanie úložiska cez neho funguje; obchodovanie vyžaduje dostupný oficiálny okrajový server LosOS.",
    de: "Der Edge-Proxy, den diese Box gefunden hat, wird nicht von LosOS betrieben. Speicher darüber zu teilen funktioniert; Handel braucht einen erreichbaren offiziellen LosOS-Edge.",
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
    en: "Credit only, for now. Nothing meters or schedules against it yet.",
    sk: "Zatiaľ len kredit. Nič ho ešte nemeria ani podľa neho neplánuje.",
    de: "Vorerst nur ein Guthaben. Noch misst oder plant nichts dagegen.",
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
    en: "Nothing to sell yet. The market only sells what you already lend, so join the mesh first. To sell compute, share compute too.",
    sk: "Zatiaľ nie je čo predávať. Trh predáva len to, čo už požičiavate, preto sa najprv pripojte k mesh sieti. Ak chcete predávať výpočtový výkon, zdieľajte ho tiež.",
    de: "Noch nichts zu verkaufen. Der Markt verkauft nur, was du ohnehin verleihst, also tritt zuerst dem Mesh bei. Um Rechenleistung zu verkaufen, teile sie auch.",
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
    en: "The platform keeps {percent}% of each sale to cover running costs. The rest goes to your Stripe account.",
    sk: "Platforma si z každého predaja necháva {percent} % na pokrytie nákladov. Zvyšok ide na váš účet Stripe.",
    de: "Die Plattform behält {percent} % jedes Verkaufs für die Betriebskosten. Der Rest geht an dein Stripe-Konto.",
  },
  "panes.market.youReceive": {
    en: "you receive {amount}",
    sk: "dostanete {amount}",
    de: "du erhältst {amount}",
  },

  // ── Toasts the panes raise ────────────────────────────────────────────────
  "panes.market.actionFailedTitle": {
    en: "That did not work",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },
  "panes.market.orderPlaced": {
    en: "Order placed",
    sk: "Objednávka je vytvorená",
    de: "Bestellung aufgegeben",
  },
  "panes.market.paidTitle": {
    en: "Payment received",
    sk: "Platba prijatá",
    de: "Zahlung eingegangen",
  },
  "panes.market.paidBody": {
    en: "{kind}, {quantity} {unit}, is yours now.",
    sk: "{kind}, {quantity} {unit}, je teraz vaše.",
    de: "{kind}, {quantity} {unit}, gehört jetzt dir.",
  },
  "panes.market.listedTitle": {
    en: "Offered for sale",
    sk: "Ponúknuté na predaj",
    de: "Zum Verkauf angeboten",
  },
  "panes.market.listedBody": {
    en: "Other boxes on the mesh can buy it from now on.",
    sk: "Ostatné zariadenia v sieti si to odteraz môžu kúpiť.",
    de: "Andere Boxen im Mesh können es ab jetzt kaufen.",
  },
  "panes.market.closedTitle": {
    en: "No longer for sale",
    sk: "Už sa nepredáva",
    de: "Nicht mehr im Angebot",
  },
  "panes.market.payoutsOpened": {
    en: "Payout setup opened",
    sk: "Nastavenie výplat je otvorené",
    de: "Auszahlungs-Einrichtung geöffnet",
  },
  "panes.market.payoutsOpenedBody": {
    en: "Finish it in the other tab, then refresh here.",
    sk: "Dokončite ho na druhej karte a potom tu obnovte stránku.",
    de: "Schließ sie im anderen Tab ab und aktualisiere dann hier.",
  },
  // ── Table headers (shadcn Table in the apps catalogue and the market) ──
  "panes.apps.col.app": {
    en: "App",
    sk: "Aplikácia",
    de: "App",
  },
  "panes.apps.col.source": {
    en: "Published by",
    sk: "Zverejnil",
    de: "Veröffentlicht von",
  },
  "panes.market.col.item": {
    en: "Bought",
    sk: "Kúpené",
    de: "Gekauft",
  },
  "panes.market.col.quantity": {
    en: "Quantity",
    sk: "Množstvo",
    de: "Menge",
  },
  "panes.market.col.until": {
    en: "Until",
    sk: "Do",
    de: "Bis",
  },
  "panes.market.col.status": {
    en: "Status",
    sk: "Stav",
    de: "Status",
  },
});
