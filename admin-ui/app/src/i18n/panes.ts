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
  "panes.about.storageWaiting": {
    en: "sharing on, waiting for a mesh",
    sk: "zdieľanie zapnuté, čaká na sieť mesh",
    de: "Teilen an, wartet auf ein Mesh",
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
  "panes.about.tour": { en: "Tour", sk: "Prehliadka", de: "Tour" },
  "panes.about.tourLink": { en: "Take the tour", sk: "Spustiť prehliadku", de: "Tour starten" },
  "panes.about.tourCaption": {
    en: "A few short steps through what this box can do. It opened by itself once, after setup.",
    sk: "Niekoľko krátkych krokov po tom, čo toto zariadenie dokáže. Raz sa otvorila sama, po nastavení.",
    de: "Ein paar kurze Schritte durch das, was diese Box kann. Nach der Einrichtung hat sie sich einmal von selbst geöffnet.",
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
  "panes.apps.onThisBox": {
    en: "On this box",
    sk: "Na tomto zariadení",
    de: "Auf dieser Box",
  },

  // ── Apps: each app's box-wide settings (site-settings.tsx) ───────────
  "panes.apps.site.trash": {
    en: "Empty the trash after",
    sk: "Vysypať kôš po",
    de: "Papierkorb leeren nach",
  },
  "panes.apps.site.days": {
    en: "days",
    sk: "dňoch",
    de: "Tagen",
  },
  "panes.apps.site.trashDefault": {
    en: "30 by default",
    sk: "Predvolene 30",
    de: "Standardmäßig 30",
  },
  "panes.apps.site.cloudCaption": {
    en: "This holds for every account in LosOS Cloud. An empty field keeps a deleted file for 30 days.",
    sk: "Toto platí pre každý účet v LosOS Cloud. Prázdne pole nechá zmazaný súbor 30 dní.",
    de: "Das gilt für jedes Konto in LosOS Cloud. Ein leeres Feld behält eine gelöschte Datei 30 Tage.",
  },
  "panes.apps.site.signIn": {
    en: "Only signed-in accounts see anything",
    sk: "Obsah vidia len prihlásené účty",
    de: "Nur angemeldete Konten sehen etwas",
  },
  "panes.apps.site.signInDetail": {
    en: "Public repositories too.",
    sk: "Aj verejné repozitáre.",
    de: "Auch öffentliche Repositorys.",
  },
  "panes.apps.site.private": {
    en: "A new repository starts out",
    sk: "Nový repozitár je na začiatku",
    de: "Ein neues Repository ist zuerst",
  },
  "panes.apps.site.private.last": {
    en: "Like the account's last one",
    sk: "Ako posledný repozitár účtu",
    de: "Wie das letzte des Kontos",
  },
  "panes.apps.site.private.private": {
    en: "Private",
    sk: "Súkromný",
    de: "Privat",
  },
  "panes.apps.site.private.public": {
    en: "Public",
    sk: "Verejný",
    de: "Öffentlich",
  },
  "panes.apps.site.push": {
    en: "Create a repository by pushing to it",
    sk: "Vytvoriť repozitár odoslaním (push) doň",
    de: "Repository durch einen Push anlegen",
  },
  "panes.apps.site.gitCaption": {
    en: "These hold for all of LosOS Git. What each account sets for itself stays in LosOS Git.",
    sk: "Tieto nastavenia platia pre celý LosOS Git. Čo si každý účet nastaví sám pre seba, zostáva v LosOS Git.",
    de: "Diese gelten für ganz LosOS Git. Was jedes Konto für sich selbst einstellt, bleibt in LosOS Git.",
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
  "panes.mesh.edge.paused": {
    en: "Sharing paused since {date}, {time}",
    sk: "Zdieľanie pozastavené od {date}, {time}",
    de: "Teilen pausiert seit {date}, {time}",
  },
  "panes.mesh.edge.pausedDetail": {
    en: "No edge answered for a while, so this box turned sharing off by itself. It turns sharing back on a minute after an edge answers again.",
    sk: "Dlhší čas sa neozval žiadny edge, preto toto zariadenie zdieľanie samo vyplo. Znova ho zapne minútu po tom, ako sa edge opäť ozve.",
    de: "Eine Weile hat kein Edge geantwortet, also hat diese Box das Teilen selbst ausgeschaltet. Sie schaltet es wieder ein, eine Minute nachdem ein Edge wieder antwortet.",
  },
  "panes.mesh.edge.waiting": {
    en: "Waiting for an edge",
    sk: "Čaká sa na edge",
    de: "Wartet auf ein Edge",
  },
  "panes.mesh.edge.resuming": {
    en: "Turning back on",
    sk: "Znova sa zapína",
    de: "Wird wieder eingeschaltet",
  },
  // Whether the edge can read what passes through it (modules/public-tls.nix).
  "panes.mesh.tls.edgeReads": {
    en: "The edge can read this box's traffic",
    sk: "Edge môže čítať prevádzku tohto zariadenia",
    de: "Das Edge kann den Verkehr dieser Box lesen",
  },
  "panes.mesh.tls.edgeReadsDetail": {
    en: "This box has no certificate of its own for its public name yet, so the edge ends TLS and passes requests on unencrypted. The box asks for one every hour.",
    sk: "Zariadenie zatiaľ nemá vlastný certifikát pre svoje verejné meno, preto TLS končí na edgi a požiadavky idú ďalej nešifrované. Zariadenie oň žiada každú hodinu.",
    de: "Diese Box hat noch kein eigenes Zertifikat für ihren öffentlichen Namen, also beendet das Edge TLS und reicht Anfragen unverschlüsselt weiter. Die Box fragt jede Stunde danach.",
  },
  "panes.mesh.tls.readable": {
    en: "Readable",
    sk: "Čitateľná",
    de: "Lesbar",
  },
  "panes.mesh.tls.sealed": {
    en: "The edge cannot read this box's traffic",
    sk: "Edge nemôže čítať prevádzku tohto zariadenia",
    de: "Das Edge kann den Verkehr dieser Box nicht lesen",
  },
  "panes.mesh.tls.sealedDetail": {
    en: "This box holds its own certificate for {names}. The edge passes the encrypted connection through without opening it.",
    sk: "Zariadenie má vlastný certifikát pre {names}. Edge šifrované spojenie len preposiela a neotvára ho.",
    de: "Diese Box hat ein eigenes Zertifikat für {names}. Das Edge reicht die verschlüsselte Verbindung durch, ohne sie zu öffnen.",
  },
  "panes.mesh.tls.checked": {
    en: "Checked from outside at {time}.",
    sk: "Overené zvonka o {time}.",
    de: "Von außen geprüft um {time}.",
  },
  "panes.mesh.tls.unreachable": {
    en: "The last check could not reach {names} from outside.",
    sk: "Posledná kontrola sa zvonka nedostala k {names}.",
    de: "Die letzte Prüfung hat {names} von außen nicht erreicht.",
  },
  "panes.mesh.tls.encrypted": {
    en: "Encrypted to this box",
    sk: "Šifrované až po zariadenie",
    de: "Bis zur Box verschlüsselt",
  },
  "panes.mesh.tls.foreign": {
    en: "Someone else answers for {name}",
    sk: "Za {name} odpovedá niekto iný",
    de: "Jemand anderes antwortet für {name}",
  },
  "panes.mesh.tls.foreignDetail": {
    en: "Looking at {name} from outside, this box found a certificate it did not make. Something between visitors and this box ends their TLS and can read what they send.",
    sk: "Pri pohľade na {name} zvonka našlo zariadenie certifikát, ktorý nevytvorilo. Niečo medzi návštevníkmi a zariadením ukončuje ich TLS a môže čítať, čo posielajú.",
    de: "Beim Blick von außen auf {name} hat diese Box ein Zertifikat gefunden, das sie nicht erstellt hat. Etwas zwischen den Besuchern und dieser Box beendet ihr TLS und kann lesen, was sie senden.",
  },
  "panes.mesh.tls.intercepted": {
    en: "Intercepted",
    sk: "Zachytávané",
    de: "Abgefangen",
  },
  "panes.mesh.tls.lapsed": {
    en: "This box's certificate has lapsed",
    sk: "Certifikát tohto zariadenia vypršal",
    de: "Das Zertifikat dieser Box ist abgelaufen",
  },
  "panes.mesh.tls.lapsedDetail": {
    en: "Visitors see a certificate warning until the box renews it. The edge does not take TLS back, so it still cannot read the traffic.",
    sk: "Návštevníci uvidia varovanie o certifikáte, kým ho zariadenie neobnoví. Edge TLS späť nepreberá, takže prevádzku ani teraz čítať nemôže.",
    de: "Besucher sehen eine Zertifikatswarnung, bis die Box es erneuert. Das Edge übernimmt TLS nicht wieder und kann den Verkehr weiterhin nicht lesen.",
  },
  "panes.mesh.tls.renewing": {
    en: "Renewing",
    sk: "Obnovuje sa",
    de: "Wird erneuert",
  },
  "panes.mesh.edge.sharingOff": {
    en: "Sharing is off",
    sk: "Zdieľanie je vypnuté",
    de: "Teilen ist aus",
  },
  // The sign beside each edge's name: did this edge pass the gateway checks?
  // It decides trading only; the words are the tooltip and the button's name.
  "panes.mesh.edge.officialTip": {
    en: "Checked gateway, run by {owner}. Its TPM shows the published image {image}, LosOS's fee addresses and nothing changed since it started, so it may process storage and compute trading for this box.",
    sk: "Overená brána, prevádzkuje ju {owner}. Jej TPM ukazuje zverejnený obraz {image}, adresy poplatkov LosOS a že sa od spustenia nič nezmenilo, takže môže pre toto zariadenie spracúvať obchodovanie s úložiskom a výpočtami.",
    de: "Geprüftes Gateway, betrieben von {owner}. Sein TPM zeigt das veröffentlichte Image {image}, die Gebührenadressen von LosOS und dass sich seit dem Start nichts geändert hat; es darf für diese Box den Handel mit Speicher und Rechenleistung abwickeln.",
  },
  "panes.mesh.edge.companyTip": {
    en: "This edge shows no gateway passport, so this box cannot check it. What you are missing through it:\n• buying storage or compute on the market\n• selling this box's spare storage and compute\nSharing storage through it works.",
    sk: "Tento okrajový server neukazuje pas brány, takže ho toto zariadenie nemôže overiť. Čo cez neho chýba:\n• nákup úložiska alebo výpočtov na trhu\n• predaj voľného úložiska a výpočtov tohto zariadenia\nZdieľanie úložiska cez neho funguje.",
    de: "Dieser Edge zeigt keinen Gateway-Pass, darum kann diese Box ihn nicht prüfen. Was dir darüber fehlt:\n• Speicher oder Rechenleistung auf dem Markt kaufen\n• freien Speicher und Rechenleistung dieser Box verkaufen\nSpeicher darüber zu teilen funktioniert.",
  },
  "panes.mesh.edge.refusedTip": {
    en: "Not trusted with trading: {reason}\nThis box checks again on its next scan. Sharing storage through it works.",
    sk: "Nie je dôveryhodný na obchodovanie: {reason}\nToto zariadenie ho overí znova pri ďalšom hľadaní. Zdieľanie úložiska cez neho funguje.",
    de: "Für den Handel nicht vertrauenswürdig: {reason}\nDiese Box prüft beim nächsten Suchen erneut. Speicher darüber zu teilen funktioniert.",
  },
  "panes.mesh.edge.disabledTip": {
    en: "Tampered with: {reason}\nThis box no longer uses this edge for anything, and will not again.",
    sk: "Bol pozmenený: {reason}\nToto zariadenie tento okrajový server už na nič nepoužíva a ani nebude.",
    de: "Manipuliert: {reason}\nDiese Box nutzt diesen Edge für nichts mehr, und das bleibt so.",
  },
  "panes.mesh.edge.disabled": {
    en: "Not used",
    sk: "Nepoužíva sa",
    de: "Nicht genutzt",
  },
  "panes.mesh.edge.ownedBy": {
    en: "{url} · run by {owner}",
    sk: "{url} · prevádzkuje {owner}",
    de: "{url} · betrieben von {owner}",
  },
  "panes.mesh.edge.captionCompany": {
    en: "The edge proxy is the box in the middle. It holds the mesh together and makes yours reachable from outside. Several can be in reach at once; sharing works through any of them. None of these passed the gateway checks, so the market (buying and selling storage and compute) stays off until a checked gateway answers. Your own files and apps work either way.",
    sk: "Okrajový proxy server je zariadenie uprostred: drží sieť mesh pohromade a sprístupňuje to vaše zvonku. Dostupných ich môže byť viac naraz; zdieľanie funguje cez ktorýkoľvek. Žiadny z týchto neprešiel overením brány, takže trh (nákup a predaj úložiska a výpočtov) zostane vypnutý, kým sa neozve overená brána. Vaše vlastné súbory a aplikácie fungujú tak či tak.",
    de: "Der Edge-Proxy ist die Box in der Mitte: Er hält das Mesh zusammen und macht deine von außen erreichbar. Mehrere können zugleich erreichbar sein; Teilen funktioniert über jeden davon. Keiner davon hat die Gateway-Prüfung bestanden, darum bleibt der Markt (Speicher und Rechenleistung kaufen und verkaufen) aus, bis ein geprüftes Gateway antwortet. Deine eigenen Dateien und Apps laufen so oder so.",
  },
  // Which tier a link is (handbook in-depth/edge-federation, "Three tiers").
  "panes.mesh.tier.primary": {
    en: "Primary",
    sk: "Primárne",
    de: "Primär",
  },
  "panes.mesh.tier.secondary": {
    en: "Secondary",
    sk: "Sekundárne",
    de: "Sekundär",
  },
  "panes.mesh.tier.tertiary": {
    en: "Tertiary",
    sk: "Terciárne",
    de: "Tertiär",
  },
  "panes.mesh.edge.inUse": {
    en: "{tier}, in use",
    sk: "{tier}, používa sa",
    de: "{tier}, in Gebrauch",
  },
  "panes.mesh.edge.where": {
    en: "{via} · {url}",
    sk: "{via} · {url}",
    de: "{via} · {url}",
  },
  "panes.mesh.edge.inCluster": {
    en: "{via} · cluster {cluster} · {url}",
    sk: "{via} · klaster {cluster} · {url}",
    de: "{via} · Cluster {cluster} · {url}",
  },
  "panes.mesh.link.secondary": {
    en: "Secondary connection through {gateway}",
    sk: "Sekundárne spojenie cez {gateway}",
    de: "Sekundäre Verbindung über {gateway}",
  },
  "panes.mesh.link.secondaryDetail": {
    en: "A local gateway of the cluster {cluster}, on this network. If it stops answering, the box moves to another gateway of the same cluster.",
    sk: "Lokálna brána klastra {cluster} v tejto sieti. Ak prestane odpovedať, zariadenie prejde na inú bránu toho istého klastra.",
    de: "Ein lokales Gateway des Clusters {cluster} in diesem Netzwerk. Antwortet es nicht mehr, wechselt die Box zu einem anderen Gateway desselben Clusters.",
  },
  "panes.mesh.link.secondaryLone": {
    en: "A local gateway on this network.",
    sk: "Lokálna brána v tejto sieti.",
    de: "Ein lokales Gateway in diesem Netzwerk.",
  },
  "panes.mesh.link.tertiary": {
    en: "Tertiary connection to {name}",
    sk: "Terciárne spojenie s {name}",
    de: "Tertiäre Verbindung zu {name}",
  },
  "panes.mesh.link.tertiaryDetail": {
    en: "Straight to an official edge over the internet, because no local gateway is in reach.",
    sk: "Priamo na oficiálny okrajový server cez internet, pretože žiadna lokálna brána nie je dostupná.",
    de: "Direkt über das Internet zu einem offiziellen Edge, weil kein lokales Gateway erreichbar ist.",
  },
  "panes.mesh.link.tertiaryFallback": {
    en: "Straight to an official edge over the internet, because no gateway of the cluster {cluster} answers. The box goes back to one as soon as it does.",
    sk: "Priamo na oficiálny okrajový server cez internet, pretože žiadna brána klastra {cluster} neodpovedá. Keď niektorá odpovie, zariadenie sa k nej vráti.",
    de: "Direkt über das Internet zu einem offiziellen Edge, weil kein Gateway des Clusters {cluster} antwortet. Sobald eines antwortet, kehrt die Box dorthin zurück.",
  },
  "panes.mesh.link.held": {
    en: "Your custom domain is served through this edge, so the box stays on it even when another one would answer.",
    sk: "Vaša vlastná doména je obsluhovaná cez tento okrajový server, preto na ňom zariadenie zostane, aj keď by odpovedal iný.",
    de: "Deine eigene Domain läuft über diesen Edge, darum bleibt die Box bei ihm, auch wenn ein anderer antworten würde.",
  },
  "panes.mesh.link.leftBanned": {
    en: "The box moved here because {url} is banned.",
    sk: "Zariadenie sa sem presunulo, pretože {url} je zablokovaný.",
    de: "Die Box ist hierher gewechselt, weil {url} gesperrt ist.",
  },
  "panes.mesh.cluster.title": {
    en: "Secondary cluster",
    sk: "Sekundárny klaster",
    de: "Sekundärer Cluster",
  },
  "panes.mesh.cluster.gateways": {
    en: "Gateways: {gateways}",
    sk: "Brány: {gateways}",
    de: "Gateways: {gateways}",
  },
  "panes.mesh.cluster.noneAnswers": {
    en: "None of its gateways answers right now.",
    sk: "Žiadna z jeho brán práve neodpovedá.",
    de: "Keines seiner Gateways antwortet gerade.",
  },
  "panes.mesh.cluster.mine": {
    en: "This box's cluster",
    sk: "Klaster tohto zariadenia",
    de: "Cluster dieser Box",
  },
  "panes.mesh.cluster.move": {
    en: "Move here",
    sk: "Presunúť sem",
    de: "Hierher wechseln",
  },
  "panes.mesh.cluster.moved": {
    en: "This box is in the cluster {cluster} now",
    sk: "Zariadenie je teraz v klastri {cluster}",
    de: "Diese Box ist jetzt im Cluster {cluster}",
  },
  "panes.mesh.cluster.failed": {
    en: "Could not move this box to that cluster",
    sk: "Zariadenie sa nepodarilo presunúť do toho klastra",
    de: "Konnte die Box nicht in diesen Cluster verschieben",
  },
  "panes.mesh.cluster.caption": {
    en: "A secondary cluster is one or more local gateways that serve the same boxes, so a box keeps working when one gateway goes away. Gateways are linked to each other and to the official edges in the main cluster. This box stays in its cluster until you move it.",
    sk: "Sekundárny klaster tvorí jedna alebo viac lokálnych brán, ktoré obsluhujú tie isté zariadenia, takže zariadenie funguje ďalej, aj keď jedna brána vypadne. Brány sú navzájom a s oficiálnymi okrajovými servermi prepojené v hlavnom klastri. Zariadenie zostane vo svojom klastri, kým ho nepresuniete.",
    de: "Ein sekundärer Cluster besteht aus einem oder mehreren lokalen Gateways, die dieselben Boxen bedienen, damit eine Box weiterläuft, wenn ein Gateway ausfällt. Die Gateways sind untereinander und mit den offiziellen Edges im Hauptcluster verbunden. Diese Box bleibt in ihrem Cluster, bis du sie verschiebst.",
  },
  "panes.mesh.cluster.captionHeld": {
    en: "Your custom domain is served through the edge this box uses, so the box cannot move to another cluster. It moves on its own only if that edge is banned, and the domain goes with it.",
    sk: "Vaša vlastná doména je obsluhovaná cez okrajový server, ktorý zariadenie používa, preto ho nemožno presunúť do iného klastra. Samo sa presunie, iba ak je ten server zablokovaný, a doména ide s ním.",
    de: "Deine eigene Domain läuft über den Edge, den diese Box nutzt, darum kann sie nicht in einen anderen Cluster wechseln. Sie wechselt nur von selbst, wenn dieser Edge gesperrt wird, und die Domain zieht mit.",
  },
  "panes.mesh.local.title": {
    en: "This network",
    sk: "Táto sieť",
    de: "Dieses Netzwerk",
  },
  "panes.mesh.local.switch": {
    en: "Share with this network's gateways",
    sk: "Zdieľať s bránami tejto siete",
    de: "Mit den Gateways dieses Netzwerks teilen",
  },
  "panes.mesh.local.needed": {
    en: "Needs a local gateway in reach. An official edge is not enough.",
    sk: "Vyžaduje dostupnú lokálnu bránu. Oficiálny okrajový server nestačí.",
    de: "Braucht ein erreichbares lokales Gateway. Ein offizieller Edge reicht nicht.",
  },
  "panes.mesh.local.gateways": {
    en: "Local gateways",
    sk: "Lokálne brány",
    de: "Lokale Gateways",
  },
  "panes.mesh.local.none": {
    en: "None in reach",
    sk: "Žiadna nie je dostupná",
    de: "Keines erreichbar",
  },
  "panes.mesh.local.runsAs": {
    en: "Chat, mail, calls and Mesh CI run as",
    sk: "Chat, pošta, hovory a Mesh CI bežia ako",
    de: "Chat, Mail, Anrufe und Mesh CI laufen als",
  },
  "panes.mesh.local.runsAsNone": {
    en: "Off",
    sk: "Vypnuté",
    de: "Aus",
  },
  "panes.mesh.local.paused": {
    en: "Sharing with this network paused since {date}, {time}",
    sk: "Zdieľanie s touto sieťou pozastavené od {date}, {time}",
    de: "Teilen mit diesem Netzwerk pausiert seit {date}, {time}",
  },
  "panes.mesh.local.pausedDetail": {
    en: "No local gateway answered for a while, so this box turned it off by itself. It turns it back on a minute after one answers again.",
    sk: "Dlhší čas sa neozvala žiadna lokálna brána, preto ho toto zariadenie samo vyplo. Znova ho zapne minútu po tom, ako sa niektorá opäť ozve.",
    de: "Eine Weile hat kein lokales Gateway geantwortet, also hat diese Box es selbst ausgeschaltet. Sie schaltet es wieder ein, eine Minute nachdem eines wieder antwortet.",
  },
  "panes.mesh.local.waiting": {
    en: "Waiting for a gateway",
    sk: "Čaká sa na bránu",
    de: "Wartet auf ein Gateway",
  },
  "panes.mesh.local.caption": {
    en: "Kinda shared: the box keeps a third account, kindashared, with its own encrypted folder, for the gateways of the network it sits on, a company's or your own. Nothing in it goes to an official edge or the wider mesh, and nothing in it is sold on the market. While it is on, chat, mail, calls and Mesh CI run as kindashared when the box is not in the mesh, and apps can be installed as kindashared. Your own files stay apart either way.",
    sk: "Tak trochu zdieľané: zariadenie má tretí účet, kindashared, s vlastným šifrovaným priečinkom, pre brány siete, v ktorej je, firemnej alebo vašej. Nič z neho nejde na oficiálny okrajový server ani do širšej siete mesh a nič z neho sa nepredáva na trhu. Kým je zapnuté, chat, pošta, hovory a Mesh CI bežia ako kindashared, ak zariadenie nie je v sieti mesh, a aplikácie možno inštalovať ako kindashared. Vaše vlastné súbory zostávajú oddelené tak či tak.",
    de: "Ein bisschen geteilt: Die Box führt ein drittes Konto, kindashared, mit einem eigenen verschlüsselten Ordner, für die Gateways des Netzwerks, in dem sie steht, das einer Firma oder dein eigenes. Nichts darin geht an einen offiziellen Edge oder ins weitere Mesh, und nichts darin wird auf dem Markt verkauft. Solange es an ist, laufen Chat, Mail, Anrufe und Mesh CI als kindashared, wenn die Box nicht im Mesh ist, und Apps lassen sich als kindashared installieren. Deine eigenen Dateien bleiben so oder so getrennt.",
  },
  "panes.mesh.manage.title": {
    en: "Managed from the edge",
    sk: "Správa z okrajového servera",
    de: "Verwaltung durch den Edge",
  },
  "panes.mesh.manage.switch": {
    en: "Let the company edge manage this box",
    sk: "Nechať firemný okrajový server spravovať toto zariadenie",
    de: "Den Firmen-Edge diese Box verwalten lassen",
  },
  "panes.mesh.manage.switchNamed": {
    en: "Let {name} manage this box",
    sk: "Nechať {name} spravovať toto zariadenie",
    de: "{name} diese Box verwalten lassen",
  },
  "panes.mesh.manage.offDetail": {
    en: "Its admins could then change LosOS Cloud, LosOS Git and the apps here from the edge's page.",
    sk: "Jeho správcovia by potom mohli meniť LosOS Cloud, LosOS Git a aplikácie tu zo stránky okrajového servera.",
    de: "Seine Admins könnten dann LosOS Cloud, LosOS Git und die Apps hier von der Seite des Edge aus ändern.",
  },
  "panes.mesh.manage.active": {
    en: "{name} can change LosOS Cloud, LosOS Git and the apps here. It cannot see your files, add people or change anything else.",
    sk: "{name} môže meniť LosOS Cloud, LosOS Git a aplikácie tu. Nevidí vaše súbory, nepridáva ľudí a nič iné nemení.",
    de: "{name} kann LosOS Cloud, LosOS Git und die Apps hier ändern. Er sieht deine Dateien nicht, fügt keine Personen hinzu und ändert sonst nichts.",
  },
  "panes.mesh.manage.elsewhere": {
    en: "On for {url}, which this box does not use right now, so nothing manages it.",
    sk: "Zapnuté pre {url}, ktorý toto zariadenie práve nepoužíva, takže ho nikto nespravuje.",
    de: "An für {url}, den diese Box gerade nicht nutzt, also verwaltet sie niemand.",
  },
  "panes.mesh.manage.noLocalEdge": {
    en: "Only an edge on this network can manage the box, and this box uses none right now.",
    sk: "Zariadenie môže spravovať iba okrajový server v tejto sieti a toto zariadenie teraz žiadny nepoužíva.",
    de: "Nur ein Edge in diesem Netz kann die Box verwalten, und diese Box nutzt gerade keinen.",
  },
  "panes.mesh.manage.outsideNetwork": {
    en: "The edge this box uses is not on its own network, so it may not manage the box. Only an edge on the same local network can.",
    sk: "Okrajový server, ktorý toto zariadenie používa, nie je v jeho vlastnej sieti, takže ho nesmie spravovať. Môže to iba server v tej istej lokálnej sieti.",
    de: "Der Edge, den diese Box nutzt, ist nicht in ihrem eigenen Netz und darf sie daher nicht verwalten. Das kann nur ein Edge im selben lokalen Netz.",
  },
  "panes.mesh.manage.caption": {
    en: "For a company's own edge. The switch is tied to the edge it was turned on for, so a different edge never inherits it. Turn it off and the edge loses access at once.",
    sk: "Pre vlastný okrajový server firmy. Prepínač patrí k serveru, pre ktorý bol zapnutý, takže iný server ho nikdy nezdedí. Po vypnutí server stratí prístup okamžite.",
    de: "Für den eigenen Edge einer Firma. Der Schalter gilt nur für den Edge, für den er eingeschaltet wurde, ein anderer erbt ihn nie. Ausgeschaltet verliert der Edge den Zugriff sofort.",
  },
  "panes.mesh.manage.turnedOn": {
    en: "{name} can manage this box now",
    sk: "{name} teraz môže spravovať toto zariadenie",
    de: "{name} kann diese Box jetzt verwalten",
  },
  "panes.mesh.manage.turnedOff": {
    en: "No edge manages this box now",
    sk: "Toto zariadenie teraz nespravuje žiadny okrajový server",
    de: "Kein Edge verwaltet diese Box mehr",
  },
  "panes.mesh.manage.failed": {
    en: "Could not change who manages this box",
    sk: "Nepodarilo sa zmeniť, kto spravuje toto zariadenie",
    de: "Konnte nicht ändern, wer diese Box verwaltet",
  },
  "panes.mesh.edge.needed": {
    en: "Needs an edge proxy in reach.",
    sk: "Vyžaduje dostupný okrajový proxy server.",
    de: "Braucht einen erreichbaren Edge-Proxy.",
  },
  "panes.mesh.edge.caption": {
    en: "The edge proxy is the box in the middle. It holds the mesh together and makes yours reachable from outside. This box looks for one on its own network and at its usual address every few seconds, and anything that shares your storage over the network stays off until one answers. Your own files and apps work either way.",
    sk: "Okrajový proxy server je zariadenie uprostred: drží sieť mesh pohromade a sprístupňuje to vaše zvonku. Toto zariadenie ho každých pár sekúnd hľadá vo vlastnej sieti aj na svojej obvyklej adrese, a čokoľvek, čo zdieľa vaše úložisko cez sieť, zostane vypnuté, kým sa niektorý neozve. Vaše vlastné súbory a aplikácie fungujú tak či tak.",
    de: "Der Edge-Proxy ist die Box in der Mitte: Er hält das Mesh zusammen und macht deine von außen erreichbar. Diese Box sucht alle paar Sekunden in ihrem eigenen Netzwerk und an ihrer üblichen Adresse nach einem, und alles, was deinen Speicher über das Netzwerk teilt, bleibt aus, bis einer antwortet. Deine eigenen Dateien und Apps laufen so oder so.",
  },
  "panes.mesh.otherBoxes": {
    en: "Other boxes",
    sk: "Iné zariadenia",
    de: "Andere Boxen",
  },
  "panes.mesh.needsProxy": {
    en: "Turn on \"Reachable from outside your home\" in Network first. The box signs in to the edge with the same key.",
    sk: "Najprv v Sieti zapnite „Dostupné aj mimo domova“. Zariadenie sa na okrajový server prihlasuje tým istým kľúčom.",
    de: "Schalte zuerst unter Netzwerk „Auch von außerhalb erreichbar“ ein. Die Box meldet sich mit demselben Schlüssel beim Edge an.",
  },
  "panes.mesh.needsName": {
    en: "Give this box a name of its own in Network first. Every box starts as \"mattbox\", and the edge turns away a second one by that name.",
    sk: "Najprv v Sieti dajte zariadeniu vlastný názov. Každé zariadenie začína ako „mattbox“ a okrajový server druhé s rovnakým názvom odmietne.",
    de: "Gib der Box zuerst unter Netzwerk einen eigenen Namen. Jede Box heißt anfangs „mattbox“, und der Edge weist eine zweite mit demselben Namen ab.",
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
  "panes.network.outside": {
    en: "Reachable from outside your home",
    sk: "Dostupné aj mimo domova",
    de: "Auch von außerhalb erreichbar",
  },
  "panes.network.reachCaption": {
    en: "With \"reachable from outside\" on, this box opens a way out to a small relay so you can get at it from anywhere. Nothing on your router needs opening.",
    sk: "Keď je zapnuté „dostupné mimo domova“, zariadenie si otvorí cestu von k malému sprostredkovateľovi, aby ste sa k nemu dostali odkiaľkoľvek. Na routeri netreba nič otvárať.",
    de: "Mit „von außerhalb erreichbar“ öffnet diese Box einen Weg nach draußen zu einem kleinen Relay, sodass du von überall an sie herankommst. An deinem Router muss nichts geöffnet werden.",
  },
  "panes.network.windows.title": {
    en: "Windows, Mac and Linux",
    sk: "Windows, Mac a Linux",
    de: "Windows, Mac und Linux",
  },
  "panes.network.windows.show": {
    en: "Show under Network on computers nearby",
    sk: "Zobraziť v Sieti na počítačoch nablízku",
    de: "Unter Netzwerk auf Computern in der Nähe zeigen",
  },
  "panes.network.windows.showDetail": {
    en: "File Explorer, Finder and Linux file managers list this box and open each person's LosOS Cloud files with their LosOS name and password.",
    sk: "Prieskumník, Finder a správcovia súborov v Linuxe zobrazia toto zariadenie a otvoria každému jeho súbory v LosOS Cloud po zadaní mena a hesla LosOS.",
    de: "Explorer, Finder und Linux-Dateimanager zeigen diese Box und öffnen jeder Person ihre Dateien in LosOS Cloud mit ihrem LosOS-Namen und -Passwort.",
  },
  "panes.network.windows.caption": {
    en: "Each person finds their files at {address} followed by their name. Only computers on your local network get in, never through the relay. Each person's Account page says whether Windows takes their password yet.",
    sk: "Každý nájde svoje súbory na adrese {address} a za ňou svoje meno. Dostanú sa sem len počítače v lokálnej sieti, nikdy nie cez sprostredkovateľa. Na stránke Účet každý vidí, či Windows už berie jeho heslo.",
    de: "Jede Person findet ihre Dateien unter {address} gefolgt von ihrem Namen. Hinein kommen nur Computer in deinem lokalen Netz, nie über das Relay. Auf der Seite Konto sieht jede Person, ob Windows ihr Passwort schon annimmt.",
  },
  "panes.network.hidden.title": {
    en: "Tor and I2P",
    sk: "Tor a I2P",
    de: "Tor und I2P",
  },
  "panes.network.hidden.tor": {
    en: "Reachable on Tor",
    sk: "Dostupné cez Tor",
    de: "Über Tor erreichbar",
  },
  "panes.network.hidden.torDetail": {
    en: "An .onion address that Tor Browser opens.",
    sk: "Adresa .onion, ktorú otvorí Tor Browser.",
    de: "Eine .onion-Adresse, die der Tor Browser öffnet.",
  },
  "panes.network.hidden.i2p": {
    en: "Reachable on I2P",
    sk: "Dostupné cez I2P",
    de: "Über I2P erreichbar",
  },
  "panes.network.hidden.i2pDetail": {
    en: "A .b32.i2p address that any browser set up for I2P opens.",
    sk: "Adresa .b32.i2p, ktorú otvorí prehliadač nastavený pre I2P.",
    de: "Eine .b32.i2p-Adresse, die jeder für I2P eingerichtete Browser öffnet.",
  },
  "panes.network.hidden.torAddress": {
    en: "Tor address",
    sk: "Adresa v sieti Tor",
    de: "Tor-Adresse",
  },
  "panes.network.hidden.i2pAddress": {
    en: "I2P address",
    sk: "Adresa v sieti I2P",
    de: "I2P-Adresse",
  },
  "panes.network.hidden.edgeTor": {
    en: "Tor, through your edge",
    sk: "Tor cez váš edge",
    de: "Tor, über deinen Edge",
  },
  "panes.network.hidden.edgeI2p": {
    en: "I2P, through your edge",
    sk: "I2P cez váš edge",
    de: "I2P, über deinen Edge",
  },
  "panes.network.hidden.making": {
    en: "Being made…",
    sk: "Vytvára sa…",
    de: "Wird erstellt…",
  },
  "panes.network.hidden.copy": {
    en: "Copy the {what}",
    sk: "Kopírovať: {what}",
    de: "{what} kopieren",
  },
  "panes.network.hidden.copied": {
    en: "Copied",
    sk: "Skopírované",
    de: "Kopiert",
  },
  "panes.network.hidden.caption": {
    en: "These addresses open LosOS Cloud, LosOS Git and sign-in, and stay the same after a restart. The admin pages stay on your local network, and nothing opens on your router.",
    sk: "Tieto adresy otvoria LosOS Cloud, LosOS Git a prihlásenie a po reštarte sa nemenia. Stránky správy zostávajú len vo vašej lokálnej sieti a na routeri sa nič neotvára.",
    de: "Diese Adressen öffnen LosOS Cloud, LosOS Git und die Anmeldung und bleiben nach einem Neustart gleich. Die Verwaltungsseiten bleiben in deinem lokalen Netz, und an deinem Router öffnet sich nichts.",
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
    en: "Custom domains come from a checked LosOS gateway, and none is in reach.",
    sk: "Vlastné domény poskytuje overená brána LosOS a žiadna nie je dostupná.",
    de: "Eigene Domains kommen von einem geprüften LosOS-Gateway, und keines ist erreichbar.",
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
    en: "Add the two records shown under a domain at the company you bought it from. The edge looks them up itself every half minute and switches the domain on as soon as both are right, with a certificate of its own. LosOS Cloud then answers on it from anywhere, at /cloud. The TXT value proves the domain is yours. It is made from this box's identity and its Stripe account without revealing either.",
    sk: "Pridajte dva záznamy uvedené pod doménou u firmy, od ktorej ste ju kúpili. Okrajový server si ich každú pol minútu sám overí a doménu zapne, hneď ako sú oba správne, aj s vlastným certifikátom. LosOS Cloud potom na nej odpovedá odkiaľkoľvek, na adrese /cloud. Hodnota TXT dokazuje, že doména je vaša. Vzniká z identity tohto zariadenia a jeho účtu Stripe bez toho, aby jedno či druhé prezradila.",
    de: "Trage die zwei Einträge unter einer Domain bei der Firma ein, bei der du sie gekauft hast. Der Edge prüft sie jede halbe Minute selbst und schaltet die Domain ein, sobald beide stimmen, mit eigenem Zertifikat. LosOS Cloud antwortet dann von überall darauf, unter /cloud. Der TXT-Wert beweist, dass die Domain dir gehört. Er entsteht aus der Identität dieser Box und ihrem Stripe-Konto, ohne eines davon preiszugeben.",
  },
  "panes.network.wallet.title": {
    en: "Paid by name",
    sk: "Platby na názov",
    de: "Bezahlt per Name",
  },
  "panes.network.wallet.noNames": {
    en: "No name to bind yet",
    sk: "Zatiaľ niet názvu na prepojenie",
    de: "Noch kein Name zum Verknüpfen",
  },
  "panes.network.wallet.noNamesDetail": {
    en: "The edge has no name for this box in its zone yet. It gives one once Stripe has checked this box's account.",
    sk: "Okrajový server zatiaľ nemá v svojej zóne pre toto zariadenie názov. Pridelí ho, keď Stripe overí účet tohto zariadenia.",
    de: "Der Edge hat in seiner Zone noch keinen Namen für diese Box. Er vergibt einen, sobald Stripe das Konto dieser Box geprüft hat.",
  },
  "panes.network.wallet.paysTo": {
    en: "Wallets that send to ENS names pay this wallet",
    sk: "Peňaženky, ktoré posielajú na názvy ENS, platia do tejto peňaženky",
    de: "Wallets, die an ENS-Namen senden, zahlen an diese Wallet",
  },
  "panes.network.wallet.published.found": {
    en: "Wallets find this name and pay this wallet",
    sk: "Peňaženky tento názov nájdu a platia do tejto peňaženky",
    de: "Wallets finden diesen Namen und zahlen an diese Wallet",
  },
  "panes.network.wallet.published.unsigned": {
    en: "Wallets do not trust this name yet: the zone above the edge's has not vouched for its signatures",
    sk: "Peňaženky tomuto názvu zatiaľ neveria: zóna nad zónou okrajového servera ešte neručí za jej podpisy",
    de: "Wallets vertrauen diesem Namen noch nicht: Die Zone über der des Edge bürgt noch nicht für ihre Signaturen",
  },
  "panes.network.wallet.published.missing": {
    en: "Wallets do not find this name yet: public DNS does not reach the edge's zone",
    sk: "Peňaženky tento názov zatiaľ nenájdu: verejné DNS sa k zóne okrajového servera nedostane",
    de: "Wallets finden diesen Namen noch nicht: Das öffentliche DNS erreicht die Zone des Edge nicht",
  },
  "panes.network.wallet.published.other": {
    en: "Public DNS still shows an older record here. It catches up within minutes",
    sk: "Verejné DNS tu ešte ukazuje starší záznam. Dobehne ho do niekoľkých minút",
    de: "Das öffentliche DNS zeigt hier noch einen älteren Eintrag. Es holt in wenigen Minuten auf",
  },
  "panes.network.wallet.published.checking": {
    en: "Checking whether wallets find this name",
    sk: "Overuje sa, či peňaženky tento názov nájdu",
    de: "Es wird geprüft, ob Wallets diesen Namen finden",
  },
  "panes.network.wallet.bound": {
    en: "Bound to this box's wallet",
    sk: "Prepojené s peňaženkou tohto zariadenia",
    de: "Mit der Wallet dieser Box verknüpft",
  },
  "panes.network.wallet.boundDetail": {
    en: "Signed by {address}. The address is public at these names.",
    sk: "Podpísané adresou {address}. Pod týmito názvami je adresa verejná.",
    de: "Signiert von {address}. Unter diesen Namen ist die Adresse öffentlich.",
  },
  "panes.network.wallet.unbind": {
    en: "Unbind",
    sk: "Zrušiť prepojenie",
    de: "Trennen",
  },
  "panes.network.wallet.bind": {
    en: "Let people pay this box by name",
    sk: "Nechajte ľudí platiť tomuto zariadeniu na názov",
    de: "Lass Leute diese Box per Name bezahlen",
  },
  "panes.network.wallet.bindDetail": {
    en: "Wallets that send to ENS names, such as MetaMask, then pay {names} to this box's wallet. The wallet signs once, and its address becomes public.",
    sk: "Peňaženky, ktoré posielajú na názvy ENS, napríklad MetaMask, potom platia na {names} do peňaženky tohto zariadenia. Peňaženka raz podpíše a jej adresa sa stane verejnou.",
    de: "Wallets, die an ENS-Namen senden, etwa MetaMask, zahlen dann an {names} in die Wallet dieser Box. Die Wallet signiert einmal, und ihre Adresse wird öffentlich.",
  },
  "panes.network.wallet.bindButton": {
    en: "Bind wallet",
    sk: "Prepojiť peňaženku",
    de: "Wallet verknüpfen",
  },
  "panes.network.wallet.password": {
    en: "Password of the wallet on this box",
    sk: "Heslo peňaženky na tomto zariadení",
    de: "Passwort der Wallet auf dieser Box",
  },
  "panes.network.wallet.passwordDetail": {
    en: "Signs as {address}.",
    sk: "Podpíše ako {address}.",
    de: "Signiert als {address}.",
  },
  "panes.network.wallet.sign": {
    en: "Sign",
    sk: "Podpísať",
    de: "Signieren",
  },
  "panes.network.wallet.cancel": {
    en: "Cancel",
    sk: "Zrušiť",
    de: "Abbrechen",
  },
  "panes.network.wallet.caption": {
    en: "The edge publishes the wallet's address at each of these names as an ENS record, in a zone it signs with DNSSEC, and only after checking the wallet's signature. The address is the same on Ethereum, Base, Arbitrum and OP. To be paid at a domain of your own as well, add the TXT record shown under it; your DNS provider has to sign the domain with DNSSEC.",
    sk: "Okrajový server zverejní adresu peňaženky pod každým z týchto názvov ako záznam ENS v zóne, ktorú podpisuje cez DNSSEC, a až po overení podpisu peňaženky. Adresa je rovnaká na sieťach Ethereum, Base, Arbitrum a OP. Ak chcete platby aj na vlastnú doménu, pridajte záznam TXT uvedený pod ňou; váš poskytovateľ DNS musí doménu podpisovať cez DNSSEC.",
    de: "Der Edge veröffentlicht die Adresse der Wallet unter jedem dieser Namen als ENS-Eintrag, in einer Zone, die er mit DNSSEC signiert, und erst nachdem er die Signatur der Wallet geprüft hat. Die Adresse ist auf Ethereum, Base, Arbitrum und OP dieselbe. Willst du auch unter einer eigenen Domain bezahlt werden, trag den TXT-Eintrag darunter ein; dein DNS-Anbieter muss die Domain mit DNSSEC signieren.",
  },
  "panes.network.wallet.failedTitle": {
    en: "Wallet not bound",
    sk: "Peňaženka nie je prepojená",
    de: "Wallet nicht verknüpft",
  },
  "panes.network.wallet.failed": {
    en: "The wallet did not sign, or the edge refused. Try again in a moment.",
    sk: "Peňaženka nepodpísala alebo okrajový server odmietol. Skúste to o chvíľu znova.",
    de: "Die Wallet hat nicht signiert, oder der Edge hat abgelehnt. Versuch es gleich noch einmal.",
  },
  "panes.network.wallet.noWallet": {
    en: "There is no wallet in this browser. Make or connect one on the Market pane first.",
    sk: "V tomto prehliadači nie je žiadna peňaženka. Najprv ju vytvorte alebo pripojte na paneli Trh.",
    de: "In diesem Browser gibt es keine Wallet. Leg zuerst im Bereich Markt eine an oder verbinde eine.",
  },
  "panes.network.wallet.boundTitle": {
    en: "Wallet bound",
    sk: "Peňaženka prepojená",
    de: "Wallet verknüpft",
  },
  "panes.network.wallet.boundBody": {
    en: "Wallets find it at this box's names within a few minutes.",
    sk: "Peňaženky ju pod názvami tohto zariadenia nájdu do pár minút.",
    de: "Wallets finden sie in ein paar Minuten unter den Namen dieser Box.",
  },
  "panes.network.wallet.unboundTitle": {
    en: "Wallet unbound",
    sk: "Prepojenie peňaženky zrušené",
    de: "Wallet getrennt",
  },
  "panes.network.wallet.seen.pending": {
    en: "The edge looks for this record at the domain on its next check.",
    sk: "Okrajový server tento záznam pri doméne vyhľadá pri najbližšej kontrole.",
    de: "Der Edge sucht diesen Eintrag bei der Domain bei der nächsten Prüfung.",
  },
  "panes.network.wallet.seen.unsigned": {
    en: "The record is there, but the domain is not signed with DNSSEC, so wallets cannot read it. Turn DNSSEC on at your DNS provider.",
    sk: "Záznam tam je, ale doména nie je podpísaná cez DNSSEC, takže ho peňaženky neprečítajú. Zapnite DNSSEC u svojho poskytovateľa DNS.",
    de: "Der Eintrag ist da, aber die Domain ist nicht mit DNSSEC signiert, deshalb können Wallets ihn nicht lesen. Schalte DNSSEC bei deinem DNS-Anbieter ein.",
  },
  "panes.network.wallet.seen.cname": {
    en: "This domain is a CNAME, which can carry no other record. Only a domain with A or AAAA records can be paid by name.",
    sk: "Táto doména je CNAME, ktorý nemôže niesť iný záznam. Na názov sa dá platiť len doméne so záznamami A alebo AAAA.",
    de: "Diese Domain ist ein CNAME, der keinen anderen Eintrag tragen kann. Nur eine Domain mit A- oder AAAA-Einträgen kann per Name bezahlt werden.",
  },
  "panes.network.wallet.seen.other": {
    en: "A record at this domain names another wallet.",
    sk: "Záznam pri tejto doméne uvádza inú peňaženku.",
    de: "Ein Eintrag bei dieser Domain nennt eine andere Wallet.",
  },
  "panes.network.wallet.seen.missing": {
    en: "To be paid at this domain too, add this TXT record at the domain itself.",
    sk: "Ak chcete platby aj na túto doménu, pridajte tento záznam TXT priamo k doméne.",
    de: "Um auch unter dieser Domain bezahlt zu werden, trag diesen TXT-Eintrag direkt bei der Domain ein.",
  },
  "panes.network.wallet.seen.found": {
    en: "Wallets pay {domain} to this box's wallet.",
    sk: "Peňaženky platia na {domain} do peňaženky tohto zariadenia.",
    de: "Wallets zahlen an {domain} in die Wallet dieser Box.",
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
  "panes.reset.waitErase": {
    en: "The box is being erased, which resets the settings too.",
    sk: "Zariadenie sa práve maže, čím sa obnovia aj nastavenia.",
    de: "Die Box wird gerade gelöscht, das setzt auch die Einstellungen zurück.",
  },
  "panes.reset.waitBackup": {
    en: "A backup or restore is running. Reset when it is done.",
    sk: "Beží záloha alebo obnova. Obnovte nastavenia, keď skončí.",
    de: "Eine Sicherung oder Wiederherstellung läuft. Setze zurück, wenn sie fertig ist.",
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
  "panes.backup.target.class": { en: "Storage", sk: "Úložisko", de: "Speicherklasse" },
  "panes.backup.target.classStandard": { en: "Standard", sk: "Štandardné", de: "Standard" },
  "panes.backup.target.classStandardDetail": {
    en: "The bucket's own default.",
    sk: "Predvolené nastavenie bucketu.",
    de: "Die Voreinstellung des Buckets.",
  },
  "panes.backup.target.classInstantDetail": {
    en: "Cheaper to keep. A restore starts at once, and AWS charges for each read.",
    sk: "Lacnejšie uloženie. Obnova začne hneď a AWS si účtuje každé čítanie.",
    de: "Günstiger zu lagern. Eine Wiederherstellung beginnt sofort, und AWS berechnet jeden Abruf.",
  },
  "panes.backup.target.classGlacierDetail": {
    en: "Cheaper still. A restore waits 3 to 5 hours while AWS thaws the backup, and AWS charges for it.",
    sk: "Ešte lacnejšie. Obnova čaká 3 až 5 hodín, kým AWS zálohu sprístupní, a AWS si to účtuje.",
    de: "Noch günstiger. Eine Wiederherstellung wartet 3 bis 5 Stunden, bis AWS die Sicherung bereitstellt, und AWS berechnet das.",
  },
  "panes.backup.target.classDeepDetail": {
    en: "The cheapest. A restore waits up to 12 hours while AWS thaws the backup, and AWS charges for it.",
    sk: "Najlacnejšie. Obnova čaká až 12 hodín, kým AWS zálohu sprístupní, a AWS si to účtuje.",
    de: "Am günstigsten. Eine Wiederherstellung wartet bis zu 12 Stunden, bis AWS die Sicherung bereitstellt, und AWS berechnet das.",
  },
  "panes.backup.target.classAwsOnly": {
    en: "Glacier is on Amazon S3 only. Enter an amazonaws.com address to choose it.",
    sk: "Glacier je len na Amazon S3. Ak ho chcete zvoliť, zadajte adresu amazonaws.com.",
    de: "Glacier gibt es nur bei Amazon S3. Gib dafür eine amazonaws.com-Adresse ein.",
  },
  "panes.backup.target.change": { en: "Change…", sk: "Zmeniť…", de: "Ändern…" },
  "panes.backup.target.forget": {
    en: "Forget this bucket",
    sk: "Zabudnúť tento bucket",
    de: "Diesen Bucket vergessen",
  },
  "panes.backup.target.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "panes.backup.target.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "panes.backup.target.caption": {
    en: "Any S3-compatible bucket works: Amazon S3 (including Glacier), Backblaze B2, Wasabi, or a MinIO on your own network. On Amazon S3 the address is https://s3.<region>.amazonaws.com. The box encrypts every backup before it leaves, so the bucket holds only data nobody can read without your recovery code. The secret key is never shown again once saved.",
    sk: "Funguje akýkoľvek bucket kompatibilný s S3: Amazon S3 (aj Glacier), Backblaze B2, Wasabi alebo MinIO vo vašej vlastnej sieti. Na Amazon S3 je adresa https://s3.<región>.amazonaws.com. Zariadenie každú zálohu pred odoslaním zašifruje, takže v buckete sú len dáta, ktoré bez vášho kódu obnovy nikto neprečíta. Tajný kľúč sa po uložení už nikdy nezobrazí.",
    de: "Jeder S3-kompatible Bucket geht: Amazon S3 (auch Glacier), Backblaze B2, Wasabi oder ein MinIO im eigenen Netz. Bei Amazon S3 lautet die Adresse https://s3.<region>.amazonaws.com. Die Box verschlüsselt jede Sicherung, bevor sie hinausgeht, also liegen im Bucket nur Daten, die ohne deinen Wiederherstellungscode niemand lesen kann. Der geheime Schlüssel wird nach dem Speichern nie wieder angezeigt.",
  },
  "panes.backup.target.orBucket": {
    en: "Or a bucket you rent",
    sk: "Alebo bucket, ktorý si prenajímate",
    de: "Oder ein Bucket, den du mietest",
  },
  "panes.backup.bought.title": {
    en: "Storage you bought on the mesh",
    sk: "Úložisko, ktoré ste kúpili v sieti",
    de: "Speicher, den du im Mesh gekauft hast",
  },
  "panes.backup.bought.size": {
    en: "{size} bought on the mesh",
    sk: "{size} kúpených v sieti",
    de: "{size} im Mesh gekauft",
  },
  "panes.backup.bought.use": {
    en: "Keep backups here",
    sk: "Zálohovať sem",
    de: "Hier sichern",
  },
  "panes.backup.bought.current": {
    en: "Storage you bought",
    sk: "Kúpené úložisko",
    de: "Gekaufter Speicher",
  },
  "panes.backup.bought.edge": { en: "Through", sk: "Cez", de: "Über" },
  "panes.backup.bought.forget": {
    en: "Stop backing up here",
    sk: "Prestať sem zálohovať",
    de: "Hier nicht mehr sichern",
  },
  "panes.backup.bought.toBucket": {
    en: "Use a bucket instead…",
    sk: "Použiť radšej bucket…",
    de: "Lieber einen Bucket nutzen…",
  },
  "panes.backup.bought.caption": {
    en: "The box encrypts every backup with your recovery code before it leaves, so the box that keeps the volume and the edge that sold it hold only data nobody can read without the code. The edge opens the volume only to a key your recovery code makes, so the same code reaches it from a new box.",
    sk: "Zariadenie každú zálohu pred odoslaním zašifruje vaším kódom obnovy, takže zariadenie, ktoré zväzok drží, aj edge, ktorý ho predal, majú len dáta, ktoré bez kódu nikto neprečíta. Edge otvorí zväzok len kľúču, ktorý vytvorí váš kód obnovy, takže ten istý kód sa k nemu dostane aj z nového zariadenia.",
    de: "Die Box verschlüsselt jede Sicherung mit deinem Wiederherstellungscode, bevor sie hinausgeht. Die Box, die das Volume hält, und die Edge, die es verkauft hat, haben also nur Daten, die ohne den Code niemand lesen kann. Die Edge öffnet das Volume nur für einen Schlüssel, den dein Wiederherstellungscode ergibt, also erreicht derselbe Code es auch von einer neuen Box aus.",
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
    en: "The box encrypts every backup with this code, and nobody can open one without it, LosOS included. Keep it on paper or in a password manager. After a restore the box keeps the code of the backup it came from.",
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
  "panes.backup.waiting.title": {
    en: "A change is being applied",
    sk: "Uplatňuje sa zmena",
    de: "Eine Änderung wird übernommen",
  },
  "panes.backup.waiting.detail": {
    en: "Backups, restores and the erase wait until it is done.",
    sk: "Zálohy, obnova a vymazanie počkajú, kým sa dokončí.",
    de: "Sicherungen, Wiederherstellung und das Löschen warten, bis sie fertig ist.",
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
    en: "A backup holds LosOS Cloud's files and database, LosOS Git's repositories, the folder you share with the mesh, your settings and the look of the home page. The bucket keeps the last seven.",
    sk: "Záloha obsahuje súbory a databázu LosOS Cloud, repozitáre LosOS Git, priečinok, ktorý zdieľate s mesh sieťou, vaše nastavenia a vzhľad domovskej stránky. Bucket uchováva posledných sedem.",
    de: "Eine Sicherung enthält die Dateien und die Datenbank von LosOS Cloud, die Repositorys von LosOS Git, den Ordner, den du mit dem Mesh teilst, deine Einstellungen und das Aussehen der Startseite. Der Bucket behält die letzten sieben.",
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
    en: "LosOS Cloud and LosOS Git are stopped until it is done.",
    sk: "LosOS Cloud a LosOS Git sú dovtedy zastavené.",
    de: "LosOS Cloud und LosOS Git sind bis dahin angehalten.",
  },
  "panes.backup.restore.runningThaw": {
    en: "AWS is thawing the backup first, which takes hours. LosOS Cloud and LosOS Git keep running until the files come back.",
    sk: "AWS najprv zálohu sprístupňuje, čo trvá hodiny. LosOS Cloud a LosOS Git bežia, kým sa súbory nevrátia.",
    de: "AWS stellt die Sicherung zuerst bereit, das dauert Stunden. LosOS Cloud und LosOS Git laufen weiter, bis die Dateien zurück sind.",
  },
  "panes.backup.restore.done": {
    en: "Restored",
    sk: "Obnovené",
    de: "Wiederhergestellt",
  },
  "panes.backup.restore.doneDetail": {
    en: "The box now keeps the recovery code you typed. The box applies the restored settings by itself.",
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
  "panes.backup.restore.thaw": {
    en: "The backup is in Glacier. AWS needs hours to thaw it and charges for the retrieval.",
    sk: "Záloha je v Glacieri. AWS ju sprístupní až o niekoľko hodín a za vyžiadanie si účtuje poplatok.",
    de: "Die Sicherung liegt in Glacier. AWS braucht Stunden, um sie bereitzustellen, und berechnet den Abruf.",
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
  "panes.backup.toast.boughtSaved": {
    en: "Backups go to the storage you bought",
    sk: "Zálohy pôjdu do úložiska, ktoré ste kúpili",
    de: "Sicherungen gehen auf den Speicher, den du gekauft hast",
  },
  "panes.backup.toast.targetForgotten": {
    en: "Bucket forgotten. The backups in it are still there.",
    sk: "Bucket zabudnutý. Zálohy v ňom zostali.",
    de: "Bucket vergessen. Die Sicherungen darin sind noch da.",
  },
  "panes.backup.toast.notKeyed": {
    en: "The edge did not take the key",
    sk: "Edge kľúč neprijal",
    de: "Die Edge hat den Schlüssel nicht angenommen",
  },
  "panes.backup.toast.notKeyedBody": {
    en: "The storage is kept as the place for backups. Backing up will fail until the edge that sold it opens it to this box's recovery code.",
    sk: "Úložisko zostáva miestom pre zálohy. Zálohovanie zlyhá, kým ho edge, ktorý ho predal, neotvorí kódu obnovy tohto zariadenia.",
    de: "Der Speicher bleibt als Ort für Sicherungen eingetragen. Sichern schlägt fehl, bis die Edge, die ihn verkauft hat, ihn für den Wiederherstellungscode dieser Box öffnet.",
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
  "panes.erase.waitBackup": {
    en: "A backup or restore is running. Erase when it is done.",
    sk: "Beží záloha alebo obnova. Vymažte, keď skončí.",
    de: "Eine Sicherung oder Wiederherstellung läuft. Lösche, wenn sie fertig ist.",
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
    en: "The box erases your files, repositories, the shared folder, every setting, the password, the spare key and the recovery code.",
    sk: "Vymažú sa vaše súbory, repozitáre, zdieľaný priečinok, všetky nastavenia, heslo, náhradný kľúč a kód obnovy.",
    de: "Gelöscht werden deine Dateien, Repositorys, der geteilte Ordner, jede Einstellung, das Passwort, der Ersatzschlüssel und der Wiederherstellungscode.",
  },
  "panes.erase.outside": {
    en: "The box gives up your custom domains, market listings and its place on the edge.",
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
  "panes.storage.held.spent": {
    en: "Already added to this box's disk. More room needs another drive.",
    sk: "Už je pridané k disku zariadenia. Viac miesta dá len ďalší disk.",
    de: "Schon dem Datenträger dieser Box hinzugefügt. Mehr Platz bringt nur ein weiteres Laufwerk.",
  },
  "panes.storage.reserveUsed": {
    en: "Reserve in use",
    sk: "Rezerva využitá",
    de: "Reserve genutzt",
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
  "panes.market.share.untilJoined": {
    en: "nothing is copied until this box joins a mesh",
    sk: "nič sa nekopíruje, kým sa zariadenie nepripojí k sieti mesh",
    de: "nichts wird kopiert, bis die Box einem Mesh beitritt",
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
    en: "The edge proxy this box found did not pass the gateway checks. Sharing storage through it works; trading needs a checked LosOS gateway in reach.",
    sk: "Okrajový proxy server, ktorý toto zariadenie našlo, neprešiel overením brány. Zdieľanie úložiska cez neho funguje; obchodovanie vyžaduje dostupnú overenú bránu LosOS.",
    de: "Der Edge-Proxy, den diese Box gefunden hat, hat die Gateway-Prüfung nicht bestanden. Speicher darüber zu teilen funktioniert; Handel braucht ein erreichbares geprüftes LosOS-Gateway.",
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
    en: "You pay on Stripe's own page, which opens in a new tab. This box never sees your card. Come back here and refresh once you have paid. The seller is paid when your period ends; if their box is offline for too long, you get back the part you did not get.",
    sk: "Platíte na stránke Stripe, ktorá sa otvorí na novej karte. Toto zariadenie vašu kartu nikdy nevidí. Po zaplatení sa vráťte sem a obnovte stránku. Predajca dostane peniaze po skončení vášho obdobia; ak je jeho zariadenie príliš dlho offline, dostanete späť časť, ktorú ste nedostali.",
    de: "Du bezahlst auf der Seite von Stripe, die sich in einem neuen Tab öffnet. Diese Box sieht deine Karte nie. Komm nach der Zahlung hierher zurück und aktualisiere. Der Verkäufer wird bezahlt, wenn dein Zeitraum endet; ist seine Box zu lange offline, bekommst du den Teil zurück, den du nicht bekommen hast.",
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
  "panes.market.kind.vm": {
    en: "Virtual machine",
    sk: "Virtuálny stroj",
    de: "Virtuelle Maschine",
  },
  "panes.market.unit.vm": {
    en: "replica-month",
    sk: "replika-mesiac",
    de: "Replikat-Monat",
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
  /* The same units after a count: "4 vCPU-hours", "1 GiB-month". */
  "panes.market.units.vm": {
    en: { one: "replica-month", other: "replica-months" },
    sk: {
      one: "replika-mesiac",
      few: "replika-mesiace",
      many: "replika-mesiaca",
      other: "replika-mesiacov",
    },
    de: { one: "Replikat-Monat", other: "Replikat-Monate" },
  },
  "panes.market.units.storage": {
    en: { one: "GiB-month", other: "GiB-months" },
    sk: {
      one: "GiB-mesiac",
      few: "GiB-mesiace",
      many: "GiB-mesiaca",
      other: "GiB-mesiacov",
    },
    de: { one: "GiB-Monat", other: "GiB-Monate" },
  },
  "panes.market.units.compute": {
    en: { one: "vCPU-hour", other: "vCPU-hours" },
    sk: {
      one: "vCPU-hodina",
      few: "vCPU-hodiny",
      many: "vCPU-hodiny",
      other: "vCPU-hodín",
    },
    de: { one: "vCPU-Stunde", other: "vCPU-Stunden" },
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
  "panes.market.paidOn": {
    en: "Paid on {network}",
    sk: "Zaplatené na {network}",
    de: "Bezahlt auf {network}",
  },
  "panes.market.viewTx": {
    en: "View the transaction",
    sk: "Pozrieť transakciu",
    de: "Transaktion ansehen",
  },
  "panes.market.escrow.held": {
    en: "Held until the paid period ends",
    sk: "Držané do konca zaplateného obdobia",
    de: "Gehalten, bis der bezahlte Zeitraum endet",
  },
  "panes.market.escrow.over": {
    en: "The paid period is over; waiting to be paid out",
    sk: "Zaplatené obdobie skončilo; čaká na výplatu",
    de: "Der bezahlte Zeitraum ist vorbei; wartet auf die Auszahlung",
  },
  "panes.market.escrow.release": {
    en: "On its way to the seller",
    sk: "Na ceste k predajcovi",
    de: "Unterwegs zum Verkäufer",
  },
  "panes.market.escrow.released": {
    en: "Released to the seller",
    sk: "Uvoľnené predajcovi",
    de: "An den Verkäufer freigegeben",
  },
  "panes.market.escrow.refund": {
    en: "Going back to the buyer",
    sk: "Vracia sa kupujúcemu",
    de: "Geht an den Käufer zurück",
  },
  "panes.market.escrow.refunded": {
    en: "Refunded to the buyer",
    sk: "Vrátené kupujúcemu",
    de: "An den Käufer zurückerstattet",
  },
  "panes.market.escrow.split": {
    en: "{refund} going back to the buyer, the rest to the seller",
    sk: "{refund} sa vracia kupujúcemu, zvyšok ide predajcovi",
    de: "{refund} gehen an den Käufer zurück, der Rest an den Verkäufer",
  },
  "panes.market.escrow.settled": {
    en: "{refund} refunded to the buyer, the rest paid to the seller",
    sk: "{refund} vrátené kupujúcemu, zvyšok vyplatený predajcovi",
    de: "{refund} an den Käufer erstattet, der Rest an den Verkäufer gezahlt",
  },
  "panes.market.escrow.sellerOffline": {
    en: "the seller's box was offline for too long",
    sk: "zariadenie predajcu bolo príliš dlho offline",
    de: "die Box des Verkäufers war zu lange offline",
  },
  "panes.market.escrow.lapsed": {
    en: "the payment came after the order had run out",
    sk: "platba prišla, keď už objednávka vypršala",
    de: "die Zahlung kam, nachdem die Bestellung abgelaufen war",
  },
  "panes.market.escrow.alreadyPaid": {
    en: "the order had already been paid",
    sk: "objednávka už bola zaplatená",
    de: "die Bestellung war schon bezahlt",
  },
  "panes.market.escrow.sellerCannotReceive": {
    en: "the seller's address cannot receive USDC on this network",
    sk: "adresa predajcu na tejto sieti nemôže prijať USDC",
    de: "die Adresse des Verkäufers kann in diesem Netz kein USDC empfangen",
  },
  "panes.market.escrow.notDelivered": {
    en: "what was bought could not be set up in time",
    sk: "kúpené sa nepodarilo včas pripraviť",
    de: "das Gekaufte ließ sich nicht rechtzeitig einrichten",
  },
  "panes.market.escrow.buyerReclaimed": {
    en: "the buyer took it back after the deadline",
    sk: "kupujúci si ju po lehote vzal späť",
    de: "der Käufer hat sie nach der Frist zurückgeholt",
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
  "panes.market.lockedUntil": {
    en: "paid for until {date}",
    sk: "zaplatené do {date}",
    de: "bezahlt bis {date}",
  },
  "panes.market.lockedHint": {
    en: "Someone has paid for this until {date}. You can stop selling it once that ends.",
    sk: "Niekto za toto zaplatil do {date}. Predávať to môžete prestať, až potom.",
    de: "Jemand hat bis {date} dafür bezahlt. Du kannst den Verkauf beenden, wenn das vorbei ist.",
  },
  "panes.market.earnings.title": {
    en: "Earnings",
    sk: "Zárobky",
    de: "Einnahmen",
  },
  "panes.market.earnings.held": {
    en: "Held while buyers use it",
    sk: "Držané, kým to kupujúci používajú",
    de: "Gehalten, solange Käufer es nutzen",
  },
  "panes.market.earnings.heldDetail": {
    en: "If this box is offline for {hours} hours in a row while someone's period runs, they get back the part they did not get.",
    sk: "Ak je toto zariadenie počas niečieho obdobia {hours} hodín v kuse offline, kupujúci dostane späť časť, ktorú nedostal.",
    de: "Ist diese Box während des Zeitraums eines Käufers {hours} Stunden am Stück offline, bekommt er den Teil zurück, den er nicht bekommen hat.",
  },
  "panes.market.earnings.ready": {
    en: "Ready to pay out",
    sk: "Pripravené na výplatu",
    de: "Bereit zur Auszahlung",
  },
  "panes.market.earnings.readyDetail": {
    en: "From periods that have ended. Otherwise the edge pays it out by itself a day after each one ends.",
    sk: "Z období, ktoré skončili. Inak ich edge vyplatí sám deň po konci každého z nich.",
    de: "Aus Zeiträumen, die vorbei sind. Sonst zahlt der Edge sie einen Tag nach dem Ende jedes Zeitraums selbst aus.",
  },
  "panes.market.earnings.waitDetail": {
    en: "You can pay out once a period ends, the next on {date}.",
    sk: "Vyplatiť si môžete, keď obdobie skončí, najbližšie {date}.",
    de: "Auszahlen kannst du, sobald ein Zeitraum endet, der nächste am {date}.",
  },
  "panes.market.earnings.paying": {
    en: "On its way to you",
    sk: "Na ceste k vám",
    de: "Unterwegs zu dir",
  },
  "panes.market.earnings.payOut": {
    en: "Pay out",
    sk: "Vyplatiť",
    de: "Auszahlen",
  },
  "panes.market.earnings.sentTitle": {
    en: "Payout started",
    sk: "Výplata sa začala",
    de: "Auszahlung gestartet",
  },
  "panes.market.earnings.sentBody": {
    en: "{amount} is on its way to you.",
    sk: "{amount} je na ceste k vám.",
    de: "{amount} ist unterwegs zu dir.",
  },
  "panes.market.sellCaption": {
    en: "The platform keeps {percent}% of each sale to cover running costs. The rest is held until the buyer's period ends, then goes to your Stripe account.",
    sk: "Platforma si z každého predaja necháva {percent} % na pokrytie nákladov. Zvyšok sa drží do konca obdobia kupujúceho a potom ide na váš účet Stripe.",
    de: "Die Plattform behält {percent} % jedes Verkaufs für die Betriebskosten. Der Rest wird bis zum Ende des Zeitraums des Käufers gehalten und geht dann an dein Stripe-Konto.",
  },
  "panes.market.youReceive": {
    en: "you receive {amount}",
    sk: "dostanete {amount}",
    de: "du erhältst {amount}",
  },
  "panes.market.youReceiveOffset": {
    en: "you receive {amount}, after {offset} toward platform fees owed on coin sales",
    sk: "dostanete {amount}, po odpočítaní {offset} na poplatky platformy dlžné z predajov v minciach",
    de: "du erhältst {amount}, nach {offset} für offene Plattformgebühren aus Coin-Verkäufen",
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
  "panes.market.usdc.title": {
    en: "Stablecoins",
    sk: "Stablecoiny",
    de: "Stablecoins",
  },
  "panes.market.usdc.howTitle": {
    en: "Pay and get paid in USDC",
    sk: "Platiť a dostávať platby v USDC",
    de: "In USDC zahlen und bezahlt werden",
  },
  "panes.market.usdc.how": {
    en: "Orders on this edge are paid in USDC on {chain}; you pick the network when you pay. Create a wallet here, import the 12 or 24 words from MetaMask, Rabby, Rainbow, Trust Wallet or Phantom, or connect the wallet already in this browser. One phrase gives you an address on every network listed. The platform fee goes to the edge's treasury and the rest to the seller once the edge has confirmed the payment and what was bought is ready. The box never sees your phrase or your key.",
    sk: "Objednávky na tomto edge sa platia v USDC na sieti {chain}; sieť si vyberiete pri platbe. Vytvorte peňaženku tu, importujte 12 alebo 24 slov z MetaMask, Rabby, Rainbow, Trust Wallet alebo Phantom, alebo pripojte peňaženku, ktorá už v tomto prehliadači je. Jedna fráza vám dá adresu na každej uvedenej sieti. Poplatok platformy ide do pokladnice edge a zvyšok predajcovi, keď edge platbu potvrdí a kúpené je pripravené. Zariadenie vašu frázu ani kľúč nikdy nevidí.",
    de: "Bestellungen auf diesem Edge werden in USDC auf {chain} bezahlt; das Netzwerk wählst du beim Bezahlen. Lege hier eine Wallet an, importiere die 12 oder 24 Wörter aus MetaMask, Rabby, Rainbow, Trust Wallet oder Phantom, oder verbinde die Wallet, die schon in diesem Browser steckt. Eine Phrase gibt dir eine Adresse auf jedem genannten Netzwerk. Die Plattformgebühr geht an die Kasse des Edge, der Rest an den Verkäufer, sobald der Edge die Zahlung bestätigt hat und das Gekaufte bereitsteht. Die Box sieht weder deine Wörter noch deinen Schlüssel.",
  },
  "panes.market.usdc.writeDown": {
    en: "Write these words down. They are the wallet. This page cannot show them again.",
    sk: "Zapíšte si tieto slová. Sú to peňaženka. Táto stránka ich už nezobrazí.",
    de: "Schreib diese Wörter auf. Sie sind die Wallet. Diese Seite kann sie nicht noch einmal zeigen.",
  },
  "panes.market.usdc.wroteItDown": {
    en: "I wrote them down",
    sk: "Zapísal som si ich",
    de: "Ich habe sie aufgeschrieben",
  },
  "panes.market.usdc.password": {
    en: "A password for this browser (8 or more characters)",
    sk: "Heslo pre tento prehliadač (aspoň 8 znakov)",
    de: "Ein Passwort für diesen Browser (mindestens 8 Zeichen)",
  },
  "panes.market.usdc.saveWallet": {
    en: "Save in this browser",
    sk: "Uložiť v tomto prehliadači",
    de: "In diesem Browser sichern",
  },
  "panes.market.usdc.create": {
    en: "Create a wallet",
    sk: "Vytvoriť peňaženku",
    de: "Wallet anlegen",
  },
  "panes.market.usdc.connect": {
    en: "Connect this browser's wallet",
    sk: "Pripojiť peňaženku tohto prehliadača",
    de: "Wallet dieses Browsers verbinden",
  },
  "panes.market.usdc.import": {
    en: "Import",
    sk: "Importovať",
    de: "Importieren",
  },
  "panes.market.usdc.importPlaceholder": {
    en: "12 or 24 words from MetaMask, Rabby, Rainbow, Trust Wallet or Phantom",
    sk: "12 alebo 24 slov z MetaMask, Rabby, Rainbow, Trust Wallet alebo Phantom",
    de: "12 oder 24 Wörter aus MetaMask, Rabby, Rainbow, Trust Wallet oder Phantom",
  },
  "panes.market.usdc.forget": {
    en: "Forget this browser's copy",
    sk: "Zabudnúť kópiu v tomto prehliadači",
    de: "Kopie in diesem Browser vergessen",
  },
  "panes.market.usdc.receiveTitle": {
    en: "Where you get paid",
    sk: "Kam vám chodia platby",
    de: "Wohin du bezahlt wirst",
  },
  "panes.market.usdc.payCaption": {
    en: "You pay from the wallet below, in USDC on {chain}. The escrow holds it until your period ends, then the platform fee goes to the edge's treasury and the rest to the seller. If the seller's box is offline for too long, you get back the part you did not get.",
    sk: "Platíte z peňaženky nižšie, v USDC na sieti {chain}. Úschova platbu drží do konca vášho obdobia, potom ide poplatok platformy do pokladnice edge a zvyšok predajcovi. Ak je zariadenie predajcu príliš dlho offline, dostanete späť časť, ktorú ste nedostali.",
    de: "Du bezahlst mit der Wallet unten, in USDC auf {chain}. Die Treuhand hält die Zahlung bis zum Ende deines Zeitraums, dann geht die Plattformgebühr an die Kasse des Edge und der Rest an den Verkäufer. Ist die Box des Verkäufers zu lange offline, bekommst du den Teil zurück, den du nicht bekommen hast.",
  },
  "panes.market.usdc.sellCaption": {
    en: "The platform keeps {percent}% of each sale to cover running costs. The escrow holds the rest until the buyer's period ends, then it reaches your payout address.",
    sk: "Platforma si z každého predaja necháva {percent} % na pokrytie nákladov. Zvyšok drží úschova do konca obdobia kupujúceho a potom príde na vašu adresu pre výplaty.",
    de: "Die Plattform behält {percent} % jedes Verkaufs für die Betriebskosten. Den Rest hält die Treuhand bis zum Ende des Zeitraums des Käufers, dann kommt er auf deiner Auszahlungsadresse an.",
  },
  "panes.market.usdc.payoutFirst": {
    en: "Set where you get paid in the wallet above, then list what this box shares.",
    sk: "V peňaženke vyššie nastavte, kam vám prídu peniaze, a potom ponúknite, čo toto zariadenie zdieľa.",
    de: "Lege in der Wallet oben fest, wohin du bezahlt wirst, und biete dann an, was diese Box teilt.",
  },
  "panes.market.usdc.receive": {
    en: "Sign with the wallet that should receive USDC on {chain}. The edge checks the signature and stores only the address.",
    sk: "Podpíšte peňaženkou, ktorá má dostávať USDC na sieti {chain}. Edge podpis skontroluje a uloží len adresu.",
    de: "Signiere mit der Wallet, die USDC auf {chain} empfangen soll. Der Edge prüft die Signatur und speichert nur die Adresse.",
  },
  "panes.market.usdc.payoutOn": {
    en: "USDC for your sales on {chains} goes to {address}.",
    sk: "USDC za vaše predaje na sieti {chains} ide na {address}.",
    de: "USDC für deine Verkäufe auf {chains} geht an {address}.",
  },
  "panes.market.usdc.or": {
    en: "or",
    sk: "alebo",
    de: "oder",
  },
  "panes.market.usdc.and": {
    en: "and",
    sk: "a",
    de: "und",
  },
  "panes.market.usdc.connectSolana": {
    en: "Connect a Solana wallet",
    sk: "Pripojiť peňaženku Solana",
    de: "Solana-Wallet verbinden",
  },
  "panes.market.usdc.noSolanaExtension": {
    en: "This browser has no Solana wallet extension. Create a wallet here, or install Phantom, Solflare or Backpack and try again.",
    sk: "Tento prehliadač nemá rozšírenie peňaženky Solana. Vytvorte peňaženku tu, alebo nainštalujte Phantom, Solflare či Backpack a skúste to znova.",
    de: "Dieser Browser hat keine Solana-Wallet-Erweiterung. Lege hier eine Wallet an, oder installiere Phantom, Solflare oder Backpack und versuche es erneut.",
  },
  "panes.market.usdc.balancesTitle": {
    en: "Your wallet",
    sk: "Vaša peňaženka",
    de: "Deine Wallet",
  },
  "panes.market.usdc.evmAddress": {
    en: "Ethereum networks: {address}",
    sk: "Siete Ethereum: {address}",
    de: "Ethereum-Netzwerke: {address}",
  },
  "panes.market.usdc.solanaAddress": {
    en: "Solana: {address}",
    sk: "Solana: {address}",
    de: "Solana: {address}",
  },
  "panes.market.usdc.refresh": {
    en: "Refresh",
    sk: "Obnoviť",
    de: "Aktualisieren",
  },
  "panes.market.onramp.title": {
    en: "Buy USDC with euros",
    sk: "Kúpiť USDC za eurá",
    de: "USDC mit Euro kaufen",
  },
  "panes.market.onramp.detail": {
    en: "At Binance, with your Binance account. The USDC arrives on Base at {address}.",
    sk: "Na Binance, s vaším účtom Binance. USDC príde na sieti Base na {address}.",
    de: "Bei Binance, mit deinem Binance-Konto. Die USDC kommen auf Base bei {address} an.",
  },
  "panes.market.onramp.buy": {
    en: "Buy",
    sk: "Kúpiť",
    de: "Kaufen",
  },
  "panes.market.onramp.notice": {
    en: "Binance sells the USDC, not LosOS. You sign in to Binance, Binance checks who you are, and it shows its rate and fee before you pay. LosOS sends Binance only this wallet's address and the amount, and never sees your identity or payment details.",
    sk: "USDC predáva Binance, nie LosOS. Prihlásite sa do Binance, Binance overí vašu totožnosť a pred platbou ukáže kurz a poplatok. LosOS pošle Binance len adresu tejto peňaženky a sumu a vašu totožnosť ani platobné údaje nikdy nevidí.",
    de: "Die USDC verkauft Binance, nicht LosOS. Du meldest dich bei Binance an, Binance prüft deine Identität und zeigt Kurs und Gebühr, bevor du zahlst. LosOS schickt Binance nur die Adresse dieser Wallet und den Betrag und sieht deine Identität und Zahlungsdaten nie.",
  },
  "panes.market.onramp.region": {
    en: "Binance decides which countries it sells to and may refuse a purchase from yours.",
    sk: "Binance rozhoduje, v ktorých krajinách predáva, a nákup z vašej krajiny môže odmietnuť.",
    de: "Binance entscheidet, in welche Länder es verkauft, und kann einen Kauf aus deinem Land ablehnen.",
  },
  "panes.market.onramp.readFirst": {
    en: "Read Binance's documents first. Each one opens in a new tab.",
    sk: "Najprv si prečítajte dokumenty Binance. Každý sa otvorí na novej karte.",
    de: "Lies zuerst die Dokumente von Binance. Jedes öffnet sich in einem neuen Tab.",
  },
  "panes.market.onramp.documents": {
    en: "Binance's documents",
    sk: "Dokumenty Binance",
    de: "Dokumente von Binance",
  },
  "panes.market.onramp.doc.terms": {
    en: "Terms of Use",
    sk: "Podmienky používania",
    de: "Nutzungsbedingungen",
  },
  "panes.market.onramp.doc.product": {
    en: "Product Terms",
    sk: "Podmienky produktov",
    de: "Produktbedingungen",
  },
  "panes.market.onramp.doc.privacy": {
    en: "Privacy Notice",
    sk: "Oznámenie o ochrane súkromia",
    de: "Datenschutzhinweis",
  },
  "panes.market.onramp.doc.risk": {
    en: "Risk Warning",
    sk: "Upozornenie na riziká",
    de: "Risikowarnung",
  },
  "panes.market.onramp.docOpened": {
    en: "Opened",
    sk: "Otvorené",
    de: "Geöffnet",
  },
  "panes.market.onramp.docNotOpened": {
    en: "Not opened",
    sk: "Neotvorené",
    de: "Nicht geöffnet",
  },
  "panes.market.onramp.accept": {
    en: "I have read Binance's Terms of Use, Product Terms, Privacy Notice and Risk Warning, and accept them for this purchase.",
    sk: "Poznám Podmienky používania, Podmienky produktov, Oznámenie o ochrane súkromia a Upozornenie na riziká Binance a pre tento nákup ich prijímam.",
    de: "Ich habe die Nutzungsbedingungen, Produktbedingungen, den Datenschutzhinweis und die Risikowarnung von Binance gelesen und akzeptiere sie für diesen Kauf.",
  },
  "panes.market.onramp.openEach": {
    en: "Open each document above to accept them.",
    sk: "Ak ich chcete prijať, otvorte každý dokument vyššie.",
    de: "Öffne jedes Dokument oben, um sie zu akzeptieren.",
  },
  "panes.market.onramp.amount": {
    en: "Amount in euros",
    sk: "Suma v eurách",
    de: "Betrag in Euro",
  },
  "panes.market.onramp.amountPlaceholder": {
    en: "EUR",
    sk: "EUR",
    de: "EUR",
  },
  "panes.market.onramp.continue": {
    en: "Create the order",
    sk: "Vytvoriť objednávku",
    de: "Bestellung anlegen",
  },
  "panes.market.onramp.cancel": {
    en: "Cancel",
    sk: "Zrušiť",
    de: "Abbrechen",
  },
  "panes.market.onramp.range": {
    en: "Enter {min} to {max} euros. Binance shows its own limits for your payment method.",
    sk: "Zadajte {min} až {max} eur. Binance ukáže vlastné limity pre váš spôsob platby.",
    de: "Gib {min} bis {max} Euro ein. Binance zeigt die eigenen Grenzen für deine Zahlungsart.",
  },
  "panes.market.onramp.checking": {
    en: "Checking the order at Binance…",
    sk: "Kontroluje sa objednávka na Binance…",
    de: "Bestellung bei Binance wird geprüft…",
  },
  "panes.market.onramp.stage.open": {
    en: "Your order for {euros} euros is ready. Open Binance to sign in and pay.",
    sk: "Objednávka za {euros} eur je pripravená. Otvorte Binance, prihláste sa a zaplaťte.",
    de: "Deine Bestellung über {euros} Euro ist bereit. Öffne Binance, melde dich an und bezahle.",
  },
  "panes.market.onramp.stage.paying": {
    en: "Binance is taking your payment of {euros} euros.",
    sk: "Binance prijíma vašu platbu {euros} eur.",
    de: "Binance nimmt deine Zahlung über {euros} Euro entgegen.",
  },
  "panes.market.onramp.stage.sending": {
    en: "Bought. Binance is sending {usdc} USDC to this wallet on Base.",
    sk: "Kúpené. Binance posiela {usdc} USDC do tejto peňaženky na sieti Base.",
    de: "Gekauft. Binance schickt {usdc} USDC auf Base an diese Wallet.",
  },
  "panes.market.onramp.stage.done": {
    en: "Binance sent {usdc} USDC to this wallet on Base.",
    sk: "Binance poslal {usdc} USDC do tejto peňaženky na sieti Base.",
    de: "Binance hat {usdc} USDC auf Base an diese Wallet geschickt.",
  },
  "panes.market.onramp.stage.kept": {
    en: "Binance bought the USDC but did not send it here. It is in your Binance account.",
    sk: "Binance USDC kúpil, ale neposlal ich sem. Sú na vašom účte Binance.",
    de: "Binance hat die USDC gekauft, aber nicht hierher geschickt. Sie liegen in deinem Binance-Konto.",
  },
  "panes.market.onramp.stage.failed": {
    en: "Binance did not complete the purchase. Nothing was bought.",
    sk: "Binance nákup nedokončil. Nič sa nekúpilo.",
    de: "Binance hat den Kauf nicht abgeschlossen. Es wurde nichts gekauft.",
  },
  "panes.market.onramp.openBinance": {
    en: "Open Binance",
    sk: "Otvoriť Binance",
    de: "Binance öffnen",
  },
  "panes.market.onramp.viewTransaction": {
    en: "View the transaction",
    sk: "Zobraziť transakciu",
    de: "Transaktion ansehen",
  },
  "panes.market.onramp.stop": {
    en: "Stop following",
    sk: "Prestať sledovať",
    de: "Nicht mehr verfolgen",
  },
  "panes.market.onramp.dismiss": {
    en: "Done",
    sk: "Hotovo",
    de: "Fertig",
  },
  "panes.market.usdc.sendTitle": {
    en: "Send USDC",
    sk: "Poslať USDC",
    de: "USDC senden",
  },
  "panes.market.usdc.sendTo": {
    en: "Address",
    sk: "Adresa",
    de: "Adresse",
  },
  "panes.market.usdc.sendToName": {
    en: "Address or ENS name",
    sk: "Adresa alebo meno ENS",
    de: "Adresse oder ENS-Name",
  },
  "panes.market.usdc.sendAmount": {
    en: "Amount in USDC",
    sk: "Suma v USDC",
    de: "Betrag in USDC",
  },
  "panes.market.usdc.network": {
    en: "Network",
    sk: "Sieť",
    de: "Netzwerk",
  },
  "panes.market.usdc.send": {
    en: "Send",
    sk: "Poslať",
    de: "Senden",
  },
  "panes.market.usdc.resolving": {
    en: "Looking the name up…",
    sk: "Hľadám meno…",
    de: "Name wird nachgeschlagen…",
  },
  "panes.market.usdc.noName": {
    en: "{name} does not point at an address.",
    sk: "{name} neukazuje na žiadnu adresu.",
    de: "{name} zeigt auf keine Adresse.",
  },
  "panes.market.usdc.resolved": {
    en: "{name} is {address}.",
    sk: "{name} je {address}.",
    de: "{name} ist {address}.",
  },
  "panes.market.usdc.badAmount": {
    en: "Enter an amount in USDC, with at most six decimals.",
    sk: "Zadajte sumu v USDC, najviac so šiestimi desatinnými miestami.",
    de: "Gib einen Betrag in USDC ein, mit höchstens sechs Nachkommastellen.",
  },
  "panes.market.usdc.badRecipient": {
    en: "That is not an address on this network, or a name that points at one.",
    sk: "To nie je adresa na tejto sieti ani meno, ktoré na ňu ukazuje.",
    de: "Das ist keine Adresse auf diesem Netzwerk und kein Name, der auf eine zeigt.",
  },
  "panes.market.usdc.sent": {
    en: "Sent {amount} to {to}",
    sk: "Odoslané {amount} na {to}",
    de: "{amount} an {to} gesendet",
  },
  "panes.market.usdc.payOn": {
    en: "Pay on",
    sk: "Zaplatiť na sieti",
    de: "Zahlen auf",
  },
  "panes.market.usdc.withFee": {
    en: "{chain}, fee about {fee}",
    sk: "{chain}, poplatok asi {fee}",
    de: "{chain}, Gebühr etwa {fee}",
  },
  "panes.market.coins.payWith": {
    en: "Pay with",
    sk: "Zaplatiť cez",
    de: "Zahlen mit",
  },
  "panes.market.coins.payDetail": {
    en: "{gross}, paid in {coin} straight to the seller's own wallet at the price the edge quotes. The platform's fee is billed to the seller.",
    sk: "{gross}, zaplatené cez {coin} priamo do vlastnej peňaženky predajcu za cenu, ktorú uvedie edge. Poplatok platformy sa účtuje predajcovi.",
    de: "{gross}, in {coin} direkt an die eigene Wallet des Verkäufers bezahlt, zum Preis, den der Edge nennt. Die Gebühr der Plattform wird dem Verkäufer berechnet.",
  },
  "panes.market.coins.also": {
    en: "Sellers can also take {coins}, paid straight to their own wallet.",
    sk: "Predajcovia môžu prijímať aj platby cez {coins}, priamo do vlastnej peňaženky.",
    de: "Verkäufer können auch {coins} annehmen, direkt in ihre eigene Wallet.",
  },
  "panes.market.coins.loading": {
    en: "Getting an address for this order…",
    sk: "Získavam adresu pre túto objednávku…",
    de: "Adresse für diese Bestellung wird geholt…",
  },
  "panes.market.coins.unreachable": {
    en: "The edge did not answer. The page tries again on its own.",
    sk: "Edge neodpovedal. Stránka to skúsi znova sama.",
    de: "Der Edge hat nicht geantwortet. Die Seite versucht es von selbst erneut.",
  },
  "panes.market.coins.send": {
    en: "Send exactly this much {ticker} to this address, from any wallet: scan the code or open the link. The price holds until {time}.",
    sk: "Pošlite presne túto sumu {ticker} na túto adresu z akejkoľvek peňaženky: naskenujte kód alebo otvorte odkaz. Cena platí do {time}.",
    de: "Sende genau diesen Betrag in {ticker} an diese Adresse, aus einer beliebigen Wallet: Code scannen oder Link öffnen. Der Preis gilt bis {time}.",
  },
  "panes.market.coins.copyAmount": {
    en: "Copy amount",
    sk: "Kopírovať sumu",
    de: "Betrag kopieren",
  },
  "panes.market.coins.copyAddress": {
    en: "Copy address",
    sk: "Kopírovať adresu",
    de: "Adresse kopieren",
  },
  "panes.market.coins.copied": {
    en: "Copied",
    sk: "Skopírované",
    de: "Kopiert",
  },
  "panes.market.coins.openWallet": {
    en: "Open in your wallet",
    sk: "Otvoriť v peňaženke",
    de: "In der Wallet öffnen",
  },
  "panes.market.coins.qr": {
    en: "QR code of the payment",
    sk: "QR kód platby",
    de: "QR-Code der Zahlung",
  },
  "panes.market.coins.state.waiting": {
    en: "Waiting for the payment. This page checks on its own.",
    sk: "Čaká sa na platbu. Stránka to kontroluje sama.",
    de: "Warte auf die Zahlung. Die Seite prüft von selbst.",
  },
  "panes.market.coins.state.seen": {
    en: "Payment seen, waiting for its first block.",
    sk: "Platba je viditeľná, čaká sa na jej prvý blok.",
    de: "Zahlung gesehen, warte auf ihren ersten Block.",
  },
  "panes.market.coins.state.underpaid": {
    en: "{paid} {ticker} arrived of {due}. Send the rest to the same address.",
    sk: "Prišlo {paid} {ticker} z {due}. Zvyšok pošlite na tú istú adresu.",
    de: "{paid} {ticker} von {due} sind angekommen. Sende den Rest an dieselbe Adresse.",
  },
  "panes.market.coins.state.confirming": {
    en: "Confirming: {n} of {required} blocks.",
    sk: "Potvrdzuje sa: {n} z {required} blokov.",
    de: "Wird bestätigt: {n} von {required} Blöcken.",
  },
  "panes.market.coins.state.paid": {
    en: "Paid.",
    sk: "Zaplatené.",
    de: "Bezahlt.",
  },
  "panes.market.coins.state.late": {
    en: "The payment arrived after the price lapsed and is short of the price now, {due} {ticker}. Send the rest to the same address.",
    sk: "Platba prišla po vypršaní ceny a chýba jej do dnešnej ceny {due} {ticker}. Zvyšok pošlite na tú istú adresu.",
    de: "Die Zahlung kam nach Ablauf des Preises an und reicht für den aktuellen Preis von {due} {ticker} nicht. Sende den Rest an dieselbe Adresse.",
  },
  "panes.market.coins.state.repricing": {
    en: "The payment arrived after the price lapsed. The edge checks it against the current price as soon as it has one.",
    sk: "Platba prišla po vypršaní ceny. Porovná sa s dnešnou cenou, hneď ako ju edge bude mať.",
    de: "Die Zahlung kam nach Ablauf des Preises an. Sie wird mit dem aktuellen Preis verglichen, sobald der Edge einen hat.",
  },
  "panes.market.coins.keyTitle": {
    en: "Get paid in {coin}",
    sk: "Platby cez {coin}",
    de: "Zahlungen in {coin}",
  },
  "panes.market.coins.keyDetail": {
    en: "Paste your wallet's extended public key (xpub or zpub) or a descriptor. Each order gets a fresh address from it, so the coins go straight to your wallet. The edge can see them arrive but can never spend them. Use an account you keep for these sales, since anything paid to its addresses counts toward orders. The platform's fee on these sales comes out of your next USDC sales, or you pay it in USDC.",
    sk: "Vložte rozšírený verejný kľúč vašej peňaženky (xpub alebo zpub) alebo deskriptor. Každá objednávka z neho dostane novú adresu, takže mince idú priamo do vašej peňaženky: edge vidí, že prišli, ale nikdy ich nemôže minúť. Použite účet, ktorý držíte len na tieto predaje, lebo všetko zaplatené na jeho adresy sa počíta k objednávkam. Poplatok platformy z týchto predajov sa strhne z vašich ďalších predajov v USDC, alebo ho zaplatíte v USDC.",
    de: "Füge den erweiterten öffentlichen Schlüssel deiner Wallet (xpub oder zpub) oder einen Deskriptor ein. Jede Bestellung bekommt daraus eine neue Adresse, die Coins gehen also direkt in deine Wallet: Der Edge sieht sie ankommen, kann sie aber nie ausgeben. Nimm ein Konto nur für diese Verkäufe, denn alles, was an seine Adressen geht, zählt für Bestellungen. Die Gebühr der Plattform auf diese Verkäufe geht von deinen nächsten USDC-Verkäufen ab, oder du zahlst sie in USDC.",
  },
  "panes.market.coins.keyDetailXmr": {
    en: "Paste your wallet's primary address (it starts with 4) and its private view key, which wallets show under their keys or seed settings. Each order gets a fresh subaddress of your wallet, so the coins go straight to you. The view key lets the edge see them arrive but never spend them. Never paste the spend key or the seed. The platform's fee on these sales comes out of your next USDC sales, or you pay it in USDC.",
    sk: "Vložte hlavnú adresu vašej peňaženky (začína sa na 4) a jej súkromný kľúč na prezeranie (view key), ktorý peňaženky ukazujú v nastaveniach kľúčov alebo seedu. Každá objednávka dostane novú podadresu vašej peňaženky, takže mince idú priamo k vám: kľúč na prezeranie dovolí edgu vidieť, že prišli, ale nikdy ich nemôže minúť. Nikdy nevkladajte kľúč na míňanie (spend key) ani seed. Poplatok platformy z týchto predajov sa strhne z vašich ďalších predajov v USDC, alebo ho zaplatíte v USDC.",
    de: "Füge die Hauptadresse deiner Wallet (sie beginnt mit 4) und ihren privaten View-Key ein, den Wallets in den Einstellungen zu Schlüsseln oder Seed zeigen. Jede Bestellung bekommt eine neue Subadresse deiner Wallet, die Coins gehen also direkt an dich: Mit dem View-Key sieht der Edge sie ankommen, kann sie aber nie ausgeben. Füge nie den Spend-Key oder den Seed ein. Die Gebühr der Plattform auf diese Verkäufe geht von deinen nächsten USDC-Verkäufen ab, oder du zahlst sie in USDC.",
  },
  "panes.market.coins.keyDetailZec": {
    en: "Paste your wallet's unified viewing key (uview…), or better its incoming viewing key (uivk…), which wallets show under their export or backup settings. Each order gets a fresh shielded address of your wallet, so the coins go straight to you. The edge keeps only the part that sees payments arrive, never spends them and never sees what you send. Never paste the spending key or the seed phrase. The platform's fee on these sales comes out of your next USDC sales, or you pay it in USDC.",
    sk: "Vložte zjednotený kľúč na prezeranie vašej peňaženky (uview…), alebo lepšie jeho časť pre prichádzajúce platby (uivk…), ktorú peňaženky ukazujú v nastaveniach exportu alebo zálohy. Každá objednávka dostane novú tienenú adresu vašej peňaženky, takže mince idú priamo k vám: edge si nechá len časť, ktorá vidí prichádzajúce platby, nikdy ich nemôže minúť a nikdy nevidí, čo posielate vy. Nikdy nevkladajte kľúč na míňanie ani seed. Poplatok platformy z týchto predajov sa strhne z vašich ďalších predajov v USDC, alebo ho zaplatíte v USDC.",
    de: "Füge den Unified Viewing Key deiner Wallet ein (uview…), besser noch den Incoming Viewing Key (uivk…), den Wallets in den Einstellungen zu Export oder Sicherung zeigen. Jede Bestellung bekommt eine neue abgeschirmte Adresse deiner Wallet, die Coins gehen also direkt an dich: Der Edge behält nur den Teil, der Zahlungen ankommen sieht, kann sie nie ausgeben und sieht nie, was du sendest. Füge nie den Spending Key oder den Seed ein. Die Gebühr der Plattform auf diese Verkäufe geht von deinen nächsten USDC-Verkäufen ab, oder du zahlst sie in USDC.",
  },
  "panes.market.coins.viewingKeyPlaceholder": {
    en: "Unified viewing key (uview… or uivk…)",
    sk: "Zjednotený kľúč na prezeranie (uview… alebo uivk…)",
    de: "Unified Viewing Key (uview… oder uivk…)",
  },
  "panes.market.coins.addressPlaceholder": {
    en: "Primary address (4…)",
    sk: "Hlavná adresa (4…)",
    de: "Hauptadresse (4…)",
  },
  "panes.market.coins.viewKeyPlaceholder": {
    en: "Private view key",
    sk: "Súkromný kľúč na prezeranie",
    de: "Privater View-Key",
  },
  "panes.market.coins.keyCurrent": {
    en: "Paid to",
    sk: "Platí sa na",
    de: "Bezahlt an",
  },
  "panes.market.coins.keyPlaceholder": {
    en: "xpub, zpub or descriptor",
    sk: "xpub, zpub alebo deskriptor",
    de: "xpub, zpub oder Deskriptor",
  },
  "panes.market.coins.keySave": {
    en: "Save key",
    sk: "Uložiť kľúč",
    de: "Schlüssel speichern",
  },
  "panes.market.coins.keyReplace": {
    en: "Replace key",
    sk: "Nahradiť kľúč",
    de: "Schlüssel ersetzen",
  },
  "panes.market.coins.keySaved": {
    en: "{coin} key saved",
    sk: "Kľúč pre {coin} je uložený",
    de: "Schlüssel für {coin} gespeichert",
  },
  "panes.market.coins.keyFailed": {
    en: "The key was not saved",
    sk: "Kľúč sa neuložil",
    de: "Der Schlüssel wurde nicht gespeichert",
  },
  "panes.market.coinFees.title": {
    en: "Platform fees on coin sales",
    sk: "Poplatky platformy za predaje v minciach",
    de: "Plattformgebühren aus Coin-Verkäufen",
  },
  "panes.market.coinFees.owed": {
    en: "You owe {amount}: coins go straight to your wallet, so the platform's share of those sales is settled apart. Each USDC sale pays toward it, up to half its price, or you can pay it now.",
    sk: "Dlžíte {amount}: mince idú priamo do vašej peňaženky, preto sa podiel platformy z týchto predajov vyrovnáva zvlášť. Každý predaj v USDC na to prispeje, najviac polovicou svojej ceny, alebo to môžete zaplatiť hneď.",
    de: "Du schuldest {amount}: Coins gehen direkt an deine Wallet, deshalb wird der Anteil der Plattform an diesen Verkäufen getrennt beglichen. Jeder USDC-Verkauf zahlt darauf ein, bis zur Hälfte seines Preises, oder du zahlst es jetzt.",
  },
  "panes.market.coinFees.limits": {
    en: "Buyers can keep paying you in coins while you owe no more than {limit} and it is settled by {date}.",
    sk: "Kupujúci vám môžu platiť v minciach, kým dlžíte najviac {limit} a vyrovnáte to do {date}.",
    de: "Käufer können dich weiter in Coins bezahlen, solange du höchstens {limit} schuldest und es bis {date} beglichen ist.",
  },
  "panes.market.coinFees.paused": {
    en: "Buyers cannot pay you in coins until this is settled. USDC still works, and each USDC sale pays toward it.",
    sk: "Kupujúci vám nemôžu platiť v minciach, kým to nevyrovnáte. USDC funguje ďalej a každý predaj v USDC na to prispeje.",
    de: "Käufer können dich nicht in Coins bezahlen, bis das beglichen ist. USDC funktioniert weiter, und jeder USDC-Verkauf zahlt darauf ein.",
  },
  "panes.market.coinFees.credit": {
    en: "{amount} from a USDC sale went beyond what you owed and pays the fees of your next coin sales first.",
    sk: "{amount} z predaja v USDC prevýšilo váš dlh a najprv sa použije na poplatky z vašich ďalších predajov v minciach.",
    de: "{amount} aus einem USDC-Verkauf ging über deine Schuld hinaus und zahlt zuerst die Gebühren deiner nächsten Coin-Verkäufe.",
  },
  "panes.market.coinFees.payFrom": {
    en: "The transfer goes from your payout address to the platform's treasury. A transfer from any other address is not counted, and anything above what you owe is not refunded.",
    sk: "Prevod ide z vašej adresy pre výplaty do pokladnice platformy. Prevod z inej adresy sa nezapočíta a čo zaplatíte navyše, sa nevracia.",
    de: "Die Überweisung geht von deiner Auszahlungsadresse an die Kasse der Plattform. Eine Überweisung von einer anderen Adresse zählt nicht, und was über deine Schuld hinausgeht, wird nicht erstattet.",
  },
  "panes.market.coinFees.notPayout": {
    en: "Pay from your payout address, {address}: open that wallet here first. A transfer from any other address is not counted, and anything above what you owe is not refunded.",
    sk: "Zaplaťte z vašej adresy pre výplaty, {address}: najprv tu otvorte tú peňaženku. Prevod z inej adresy sa nezapočíta a čo zaplatíte navyše, sa nevracia.",
    de: "Zahle von deiner Auszahlungsadresse, {address}: öffne zuerst diese Wallet hier. Eine Überweisung von einer anderen Adresse zählt nicht, und was über deine Schuld hinausgeht, wird nicht erstattet.",
  },
  "panes.market.coinFees.noPayout": {
    en: "Set a USDC payout address on the Ethereum networks first. The fees are paid from it.",
    sk: "Najprv nastavte adresu pre výplaty v USDC na sieťach Etherea. Poplatky sa platia z nej.",
    de: "Lege zuerst eine USDC-Auszahlungsadresse auf den Ethereum-Netzen fest. Die Gebühren werden von ihr bezahlt.",
  },
  "panes.market.coinFees.pay": {
    en: "Pay {amount}",
    sk: "Zaplatiť {amount}",
    de: "{amount} bezahlen",
  },
  "panes.market.coinFees.counting": {
    en: "The transfer is sent. The edge counts it once it has enough blocks.",
    sk: "Prevod je odoslaný. Edge ho započíta, keď bude mať dosť blokov.",
    de: "Die Überweisung ist gesendet. Der Edge zählt sie, sobald sie genug Blöcke hat.",
  },
  "panes.market.coinFees.paidTitle": {
    en: "Platform fees paid",
    sk: "Poplatky platformy sú zaplatené",
    de: "Plattformgebühren bezahlt",
  },
  "panes.market.coinFees.paidDetail": {
    en: "{credited} counted. Still owed: {owed}.",
    sk: "Započítané: {credited}. Ešte zostáva: {owed}.",
    de: "{credited} angerechnet. Noch offen: {owed}.",
  },
  "panes.market.usdc.madeHere": {
    en: "This wallet was made on this box and lives only in this browser. Choose where your sales go before you list: sign with this wallet, or use the address of a wallet you keep elsewhere.",
    sk: "Táto peňaženka vznikla na tomto zariadení a žije len v tomto prehliadači. Pred ponukou zvoľte, kam pôjdu vaše predaje: podpíšte touto peňaženkou, alebo použite adresu peňaženky, ktorú máte inde.",
    de: "Diese Wallet wurde auf diesem Gerät erzeugt und lebt nur in diesem Browser. Lege vor dem Anbieten fest, wohin deine Verkäufe gehen: mit dieser Wallet signieren oder die Adresse einer Wallet nehmen, die du anderswo hast.",
  },
  "panes.market.usdc.otherWallet": {
    en: "Use another wallet's address",
    sk: "Použiť adresu inej peňaženky",
    de: "Adresse einer anderen Wallet nehmen",
  },
  "panes.market.usdc.otherAddress": {
    en: "Payout address on the Ethereum networks (0x…)",
    sk: "Adresa na výplatu na sieťach Etherea (0x…)",
    de: "Auszahlungsadresse auf den Ethereum-Netzen (0x…)",
  },
  "panes.market.usdc.otherSign": {
    en: "Sign this exact message with that wallet's message signing, then paste the signature. The edge checks it and stores only the address.",
    sk: "Podpíšte presne túto správu podpisovaním správ v tej peňaženke a vložte podpis. Edge ho overí a uloží len adresu.",
    de: "Signiere genau diese Nachricht mit der Nachrichtensignatur jener Wallet und füge die Signatur ein. Der Edge prüft sie und speichert nur die Adresse.",
  },
  "panes.market.usdc.otherSignature": {
    en: "Signature (0x…)",
    sk: "Podpis (0x…)",
    de: "Signatur (0x…)",
  },
  "panes.market.usdc.otherSave": {
    en: "Save this payout address",
    sk: "Uložiť túto adresu na výplatu",
    de: "Diese Auszahlungsadresse speichern",
  },
  "panes.market.usdc.otherBad": {
    en: "That is not an address with its signature. Paste the 0x address and the 0x signature of the message shown.",
    sk: "To nie je adresa s jej podpisom. Vložte adresu 0x a podpis 0x zobrazenej správy.",
    de: "Das ist keine Adresse mit ihrer Signatur. Füge die 0x-Adresse und die 0x-Signatur der angezeigten Nachricht ein.",
  },
  "panes.market.usdc.setPayout": {
    en: "Set this payout address",
    sk: "Nastaviť túto adresu pre výplaty",
    de: "Diese Auszahlungsadresse setzen",
  },
  "panes.market.usdc.payoutSet": {
    en: "Payout address saved",
    sk: "Adresa pre výplaty je uložená",
    de: "Auszahlungsadresse gespeichert",
  },
  "panes.market.usdc.payTitle": {
    en: "Pay this order",
    sk: "Zaplatiť túto objednávku",
    de: "Diese Bestellung bezahlen",
  },
  "panes.market.usdc.payDetail": {
    en: "{gross} on {chain}. The platform keeps {fee}; the seller gets the rest once the edge has confirmed the payment and what was bought is ready.",
    sk: "{gross} na sieti {chain}. Platforma si nechá {fee}; zvyšok dostane predajca, keď edge platbu potvrdí a kúpené je pripravené.",
    de: "{gross} auf {chain}. Die Plattform behält {fee}; den Rest bekommt der Verkäufer, sobald der Edge die Zahlung bestätigt hat und das Gekaufte bereitsteht.",
  },
  "panes.market.usdc.payDetailOffset": {
    en: "{gross} on {chain}. The platform keeps {fee}, of which {offset} settles fees the seller owes on earlier coin sales; the seller gets the rest once the edge has confirmed the payment and what was bought is ready.",
    sk: "{gross} na sieti {chain}. Platforma si nechá {fee}, z toho {offset} vyrovnáva poplatky, ktoré predajca dlží z predchádzajúcich predajov v minciach; zvyšok dostane predajca, keď edge platbu potvrdí a kúpené je pripravené.",
    de: "{gross} auf {chain}. Die Plattform behält {fee}, davon begleichen {offset} Gebühren, die der Verkäufer aus früheren Coin-Verkäufen schuldet; den Rest bekommt der Verkäufer, sobald der Edge die Zahlung bestätigt hat und das Gekaufte bereitsteht.",
  },
  "panes.market.usdc.pay": {
    en: "Pay with USDC",
    sk: "Zaplatiť v USDC",
    de: "Mit USDC bezahlen",
  },
  "panes.market.usdc.payOnMarket": {
    en: "The order is waiting on the Market pane. Pay it there with USDC.",
    sk: "Objednávka čaká na paneli Trh. Zaplaťte ju tam v USDC.",
    de: "Die Bestellung wartet im Markt. Bezahle sie dort mit USDC.",
  },
  "panes.market.usdc.paid": {
    en: "Payment sent",
    sk: "Platba odoslaná",
    de: "Zahlung gesendet",
  },
  "panes.market.usdc.refundedTitle": {
    en: "Payment going back",
    sk: "Platba sa vracia",
    de: "Zahlung geht zurück",
  },
  "panes.market.usdc.gas": {
    en: "USDC does not pay the network fee. The wallet also needs a little ETH on the network it pays on, or SOL on Solana. Through the escrow on Solana, a payment also sets aside about 0.0043 SOL for its deposit, which comes back when the deposit closes. The wallet offers Solana's escrow first, then the network with the lowest fee, and pays the lowest fee that still gets into a block.",
    sk: "USDC neplatí poplatok siete. Peňaženka na to potrebuje aj trochu ETH na sieti, na ktorej platí, alebo SOL na Solane. Cez escrow na Solane platba navyše odloží asi 0,0043 SOL na svoj vklad, ktoré sa vrátia, keď sa vklad uzavrie. Peňaženka ponúkne najprv escrow na Solane, potom sieť s najnižším poplatkom, a platí najnižší poplatok, s ktorým sa platba ešte dostane do bloku.",
    de: "USDC zahlt die Netzwerkgebühr nicht. Die Wallet braucht dafür auch ein wenig ETH auf dem Netzwerk, auf dem sie zahlt, oder SOL auf Solana. Über den Escrow auf Solana legt eine Zahlung zusätzlich etwa 0,0043 SOL für ihre Einlage zurück, die zurückkommen, sobald die Einlage geschlossen wird. Die Wallet bietet zuerst den Escrow auf Solana an, dann das Netzwerk mit der niedrigsten Gebühr, und zahlt die niedrigste Gebühr, mit der die Zahlung noch in einen Block kommt.",
  },
  "panes.market.usdc.saved": {
    en: "Wallet saved in this browser",
    sk: "Peňaženka je uložená v tomto prehliadači",
    de: "Wallet in diesem Browser gesichert",
  },
  "panes.market.usdc.badPhrase": {
    en: "Those words are not a wallet phrase.",
    sk: "Tieto slová nie sú frázou peňaženky.",
    de: "Diese Wörter sind keine Wallet-Phrase.",
  },
  "panes.market.usdc.noExtension": {
    en: "This browser has no wallet extension. Create a wallet here, or install MetaMask, Rabby or Rainbow and try again.",
    sk: "Tento prehliadač nemá rozšírenie peňaženky. Vytvorte peňaženku tu, alebo nainštalujte MetaMask, Rabby či Rainbow a skúste to znova.",
    de: "Dieser Browser hat keine Wallet-Erweiterung. Lege hier eine Wallet an, oder installiere MetaMask, Rabby oder Rainbow und versuche es erneut.",
  },
  "panes.market.usdc.unlock": {
    en: "This browser's wallet password",
    sk: "Heslo k peňaženke v tomto prehliadači",
    de: "Passwort der Wallet in diesem Browser",
  },
  "panes.market.usdc.wrongPassword": {
    en: "That password does not open the wallet saved in this browser.",
    sk: "Toto heslo neotvorí peňaženku uloženú v tomto prehliadači.",
    de: "Dieses Passwort öffnet die in diesem Browser gesicherte Wallet nicht.",
  },
  "panes.market.usdc.needWallet": {
    en: "Create, import or connect a wallet first.",
    sk: "Najprv vytvorte, importujte alebo pripojte peňaženku.",
    de: "Lege zuerst eine Wallet an, importiere oder verbinde eine.",
  },
  "panes.market.usdc.lapsed": {
    en: "This order no longer holds its units, so it was not paid. Order again from the shelf.",
    sk: "Táto objednávka už nedrží svoje jednotky, preto nebola zaplatená. Objednajte znova z ponuky.",
    de: "Diese Bestellung hält ihre Einheiten nicht mehr und wurde deshalb nicht bezahlt. Bestelle erneut aus dem Angebot.",
  },
  "panes.market.usdc.counting": {
    en: "The payment is on chain. The edge has not counted it yet; check again in a moment. Paying again would pay twice.",
    sk: "Platba je na blockchaine. Edge ju zatiaľ nezapočítal; o chvíľu to skúste znova. Opätovná platba by zaplatila dvakrát.",
    de: "Die Zahlung ist auf der Chain. Der Edge hat sie noch nicht gezählt; prüfe gleich noch einmal. Erneut zu zahlen würde doppelt zahlen.",
  },
  "panes.market.usdc.checkAgain": {
    en: "Check the payment again",
    sk: "Znova skontrolovať platbu",
    de: "Zahlung erneut prüfen",
  },
  "panes.market.usdc.failedTitle": {
    en: "The wallet could not do that",
    sk: "Peňaženka to nedokázala",
    de: "Die Wallet konnte das nicht",
  },
  "panes.market.usdc.failed": {
    en: "Try again.",
    sk: "Skúste to znova.",
    de: "Versuch es erneut.",
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
