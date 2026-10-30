import { defineMessages } from "./define";

/* The shell (App.tsx) and the shared ui/ primitives. */
export default defineMessages({
  "shell.nav.overview": { en: "Overview", sk: "Prehľad", de: "Übersicht" },
  "shell.nav.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "shell.nav.storage": { en: "Storage", sk: "Úložisko", de: "Speicher" },
  "shell.nav.mesh": { en: "Mesh", sk: "Mesh", de: "Mesh" },
  /* LosOS Diagrams at /diagrams/ (admin-ui/app/diagrams/, admin-ui/lab/). */
  "shell.nav.lab": { en: "Diagrams", sk: "Diagramy", de: "Diagramme" },
  "shell.nav.cloud": { en: "Cloud", sk: "Cloud", de: "Cloud" },
  "shell.nav.chat": { en: "Chat", sk: "Chat", de: "Chat" },
  "shell.nav.mail": { en: "Mail", sk: "Pošta", de: "Mail" },
  "shell.nav.wiki": { en: "Wiki", sk: "Wiki", de: "Wiki" },
  "shell.nav.contacts": { en: "Contacts", sk: "Kontakty", de: "Kontakte" },
  "shell.nav.calendar": { en: "Calendar", sk: "Kalendár", de: "Kalender" },
  "shell.nav.ci": { en: "Mesh CI", sk: "Mesh CI", de: "Mesh CI" },
  "shell.nav.settings": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },
  "shell.nav.label": { en: "Sections", sk: "Sekcie", de: "Bereiche" },
  "shell.nav.collapse": { en: "Collapse the sidebar", sk: "Zbaliť bočný panel", de: "Seitenleiste einklappen" },
  "shell.nav.expand": { en: "Expand the sidebar", sk: "Rozbaliť bočný panel", de: "Seitenleiste ausklappen" },
  /* The fold toggle on an entry with sub-entries; aria-expanded says which way. */
  "shell.nav.subEntries": { en: "Entries under {name}", sk: "Položky pod {name}", de: "Einträge unter {name}" },
  "shell.asking": {
    en: "Asking this box whether it has been set up.",
    sk: "Zisťujeme, či je toto zariadenie už nastavené.",
    de: "Frage die Box, ob sie schon eingerichtet ist.",
  },
  "shell.thisBox": { en: "this box", sk: "toto zariadenie", de: "diese Box" },
  "shell.logoTip": {
    en: "Losos is Slovak for salmon. The logo is a live coho salmon, photographed underwater.",
    sk: "LosOS je pomenovaný po lososovi. Logo je živý losos kisuč, odfotený pod vodou.",
    de: "Losos ist Slowakisch für Lachs. Das Logo ist ein lebender Silberlachs, unter Wasser fotografiert.",
  },
  "shell.logoTipHalloween": {
    en: "Losos is Slovak for salmon. For Halloween, the logo is a plate of it.",
    sk: "LosOS je pomenovaný po lososovi. Na Halloween je logo tanier lososa.",
    de: "Losos ist Slowakisch für Lachs. An Halloween ist das Logo ein Teller davon.",
  },
  "shell.signOut": { en: "Sign out", sk: "Odhlásiť sa", de: "Abmelden" },
  "shell.me.spareKey": { en: "Spare admin key", sk: "Náhradný kľúč", de: "Ersatzschlüssel" },
  "shell.me.signedIn": {
    en: "Signed in on LosOS Login as {name}.",
    sk: "Prihlásený v LosOS Login ako {name}.",
    de: "Bei LosOS Login angemeldet als {name}.",
  },
  "shell.me.admin": {
    en: "You are an administrator, so every pane is yours.",
    sk: "Ste správca, takže môžete používať všetky časti.",
    de: "Du bist Administrator, also gehören dir alle Bereiche.",
  },
  "shell.me.limited": {
    en: "You see the panes your account was given.",
    sk: "Vidíte časti, ktoré dostal váš účet.",
    de: "Du siehst die Bereiche, die dein Konto bekommen hat.",
  },
  "shell.me.spareKeyTip": {
    en: "This tab is unlocked with the spare admin key, which opens every pane.",
    sk: "Táto karta je odomknutá náhradným správcovským kľúčom, ktorý otvorí všetky časti.",
    de: "Dieser Tab ist mit dem Ersatz-Admin-Schlüssel entsperrt, der alle Bereiche öffnet.",
  },
  "shell.notFound.title": { en: "Nothing here", sk: "Nič tu nie je", de: "Hier ist nichts" },
  "shell.notFound.body": {
    en: "That address does not match any section of this box's admin page.",
    sk: "Táto adresa nezodpovedá žiadnej časti správy tohto zariadenia.",
    de: "Diese Adresse passt zu keinem Bereich der Verwaltungsseite dieser Box.",
  },
  "shell.signIn.badShape": {
    en: "An admin key is 64 characters, digits and the letters a to f.",
    sk: "Správcovský kľúč má 64 znakov: číslice a písmená a až f.",
    de: "Ein Admin-Schlüssel hat 64 Zeichen: Ziffern und die Buchstaben a bis f.",
  },
  "shell.signIn.rejected": {
    en: "That key was not accepted.",
    sk: "Tento kľúč nebol prijatý.",
    de: "Dieser Schlüssel wurde nicht akzeptiert.",
  },
  "shell.signIn.noAnswer": {
    en: "This box did not answer. Check that it is switched on.",
    sk: "Zariadenie neodpovedá. Skontrolujte, či je zapnuté.",
    de: "Die Box antwortet nicht. Prüfe, ob sie eingeschaltet ist.",
  },
  "shell.signIn.title": { en: "Sign in to this box", sk: "Prihláste sa do tohto zariadenia", de: "An dieser Box anmelden" },
  "shell.signIn.authHint": {
    en: "Sign in on LosOS Login, this box's sign-in page, with your name and password. Each person has their own, and what you can change here depends on yours.",
    sk: "Prihláste sa v LosOS Login, prihlasovacej stránke tohto zariadenia, svojím menom a heslom. Každý má vlastné a to, čo tu môžete meniť, závisí od vášho účtu.",
    de: "Melde dich bei LosOS Login, der Anmeldeseite dieser Box, mit deinem Namen und Passwort an. Jede Person hat ein eigenes, und was du hier ändern kannst, hängt von deinem ab.",
  },
  "shell.signIn.go": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "shell.signIn.starting": {
    en: "LosOS Login is starting. This page goes on by itself once it answers.",
    sk: "LosOS Login sa spúšťa. Keď odpovie, táto stránka pokračuje sama.",
    de: "LosOS Login startet. Sobald es antwortet, geht es hier von selbst weiter.",
  },
  "shell.facts.title": { en: "While you wait", sk: "Kým čakáte", de: "Während du wartest" },
  "shell.facts.1": {
    en: "The LosOS logo is a live coho salmon, cut out of a NOAA Fisheries photo.",
    sk: "Logo LosOS je živý losos kisuč, vystrihnutý z fotografie NOAA Fisheries.",
    de: "Das LosOS-Logo ist ein lebender Silberlachs, ausgeschnitten aus einem Foto von NOAA Fisheries.",
  },
  "shell.facts.2": {
    en: "On 31 October the salmon in the logo is served on a plate.",
    sk: "31. októbra sa losos z loga podáva na tanieri.",
    de: "Am 31. Oktober wird der Lachs im Logo auf einem Teller serviert.",
  },
  "shell.facts.3": {
    en: "The startup sound has no recording behind it. It is written as code and played from the end of the LosOS launch film's score.",
    sk: "Za zvukom pri štarte nie je žiadna nahrávka. Je napísaný ako kód a hrá koniec hudby z úvodného filmu LosOS.",
    de: "Hinter dem Startton steckt keine Aufnahme. Er ist als Code geschrieben und spielt das Ende der Musik aus dem LosOS-Startfilm.",
  },
  "shell.facts.4": {
    en: "This box has no SSH and no shell. Everything you change on it goes through pages like this one.",
    sk: "Toto zariadenie nemá SSH ani shell. Všetko, čo na ňom meníte, ide cez stránky ako táto.",
    de: "Diese Box hat kein SSH und keine Shell. Alles, was du an ihr änderst, läuft über Seiten wie diese.",
  },
  "shell.facts.5": {
    en: "The box starts from an empty root file system on every boot. Only what is listed for /persist survives.",
    sk: "Zariadenie pri každom štarte začína s prázdnym koreňovým súborovým systémom. Prežije len to, čo je uvedené pre /persist.",
    de: "Die Box startet bei jedem Boot mit einem leeren Root-Dateisystem. Nur was für /persist eingetragen ist, bleibt erhalten.",
  },
  "shell.facts.6": {
    en: "The whole system is described in Nix. Every setting you apply is a commit in the box's own Git repository.",
    sk: "Celý systém je opísaný v Nixe. Každé použité nastavenie je commit vo vlastnom Git repozitári zariadenia.",
    de: "Das ganze System ist in Nix beschrieben. Jede übernommene Einstellung ist ein Commit im eigenen Git-Repository der Box.",
  },
  "shell.facts.7": {
    en: "These pages, LosOS Cloud, LosOS Git and LosOS Login all take their colours from one file.",
    sk: "Tieto stránky, LosOS Cloud, LosOS Git aj LosOS Login berú farby z jedného súboru.",
    de: "Diese Seiten, LosOS Cloud, LosOS Git und LosOS Login nehmen ihre Farben aus einer einzigen Datei.",
  },
  "shell.facts.8": {
    en: "LosOS Diagrams draws your network with Bevy, a game engine, compiled to WebAssembly.",
    sk: "LosOS Diagrams kreslí vašu sieť pomocou Bevy, herného enginu skompilovaného do WebAssembly.",
    de: "LosOS Diagrams zeichnet dein Netzwerk mit Bevy, einer Spiele-Engine, kompiliert zu WebAssembly.",
  },
  "shell.facts.9": {
    en: "When the box boots, the salmon swims in beside the name, and that screen stays on as the box's face.",
    sk: "Pri štarte zariadenia priplává losos k názvu a táto obrazovka potom zostane ako tvár zariadenia.",
    de: "Beim Start der Box schwimmt der Lachs neben den Namen, und dieser Bildschirm bleibt danach das Gesicht der Box.",
  },
  "shell.facts.10": {
    en: "The box restarts itself at 00:07 every night and installs its updates at 03:00.",
    sk: "Zariadenie sa každú noc o 00:07 samo reštartuje a o 03:00 si nainštaluje aktualizácie.",
    de: "Die Box startet jede Nacht um 00:07 von selbst neu und installiert um 03:00 ihre Updates.",
  },
  "shell.facts.11": {
    en: "Build with AI runs its language model on this box. No key, no account and no charge.",
    sk: "Karta Vytvoriť s AI spúšťa jazykový model priamo na tomto zariadení. Bez kľúča, bez účtu a zadarmo.",
    de: "Der Tab Mit KI bauen lässt sein Sprachmodell auf dieser Box laufen. Kein Schlüssel, kein Konto, keine Kosten.",
  },
  "shell.facts.12": {
    en: "Every call from the losos terminal client is one OpenPGP message, signed by your key and encrypted to the box's own.",
    sk: "Každé volanie z terminálového klienta losos je jedna správa OpenPGP, podpísaná vaším kľúčom a zašifrovaná pre kľúč zariadenia.",
    de: "Jeder Aufruf aus dem Terminal-Client losos ist eine OpenPGP-Nachricht, mit deinem Schlüssel signiert und für den Schlüssel der Box verschlüsselt.",
  },
  "shell.facts.13": {
    en: "The first-run tour has no video in it. Each scene is drawn by code, frame by frame, in the language on screen.",
    sk: "V úvodnej prehliadke nie je žiadne video. Každú scénu kreslí kód, snímku po snímke, v jazyku na obrazovke.",
    de: "In der Einführungstour steckt kein Video. Jede Szene zeichnet Code, Bild für Bild, in der Sprache auf dem Bildschirm.",
  },
  "shell.facts.14": {
    en: "The hosted copy of LosOS Diagrams runs real QEMU guests inside your browser, compiled to WebAssembly.",
    sk: "Verejná verzia LosOS Diagrams spúšťa skutočné hosťovské systémy v QEMU priamo v prehliadači, skompilované do WebAssembly.",
    de: "Die gehostete Fassung von LosOS Diagrams startet echte QEMU-Gäste in deinem Browser, kompiliert zu WebAssembly.",
  },
  "shell.facts.15": {
    en: "Storage on the mesh can be paid for in USDC, Bitcoin, Monero or shielded Zcash. The edge counts money in whole cents, never in floating point.",
    sk: "Úložisko v sieti sa dá zaplatiť v USDC, bitcoine, monere alebo tienenom Zcashi. Edge počíta peniaze v celých centoch, nikdy v pohyblivej čiarke.",
    de: "Speicher im Mesh lässt sich mit USDC, Bitcoin, Monero oder abgeschirmtem Zcash bezahlen. Der Edge rechnet Geld in ganzen Cent, nie in Gleitkommazahlen.",
  },
  "shell.facts.16": {
    en: "Before the installer starts, it checks its own system image against a SHA-256 written on its kernel command line.",
    sk: "Inštalátor pred spustením overí vlastný obraz systému podľa SHA-256 zapísaného v príkazovom riadku jadra.",
    de: "Bevor der Installer startet, prüft er sein eigenes Systemabbild gegen eine SHA-256-Summe auf seiner Kernel-Kommandozeile.",
  },
  "shell.facts.17": {
    en: "LosOS Cloud runs on this box itself and LosOS Git in a Kubernetes cluster of its own here, so both stay up when the mesh is away.",
    sk: "LosOS Cloud beží priamo na tomto zariadení a LosOS Git vo vlastnom klastri Kubernetes tu, takže oba fungujú, aj keď sieť nie je dostupná.",
    de: "LosOS Cloud läuft direkt auf dieser Box und LosOS Git in einem eigenen Kubernetes-Cluster hier, deshalb bleiben beide erreichbar, wenn das Mesh weg ist.",
  },
  "shell.signIn.useSignIn": {
    en: "Sign in with your name instead",
    sk: "Prihlásiť sa namiesto toho menom",
    de: "Stattdessen mit deinem Namen anmelden",
  },
  "shell.signIn.firstTitle": {
    en: "Make your sign-in",
    sk: "Vytvorte si prihlásenie",
    de: "Lege deine Anmeldung an",
  },
  "shell.signIn.firstHint": {
    en: "This box was set up before people signed in with their own name. Choose yours and a password; you become its first administrator. The spare admin key proves the box is yours.",
    sk: "Toto zariadenie bolo nastavené skôr, než sa ľudia prihlasovali vlastným menom. Zvoľte si meno a heslo; stanete sa jeho prvým správcom. Náhradný správcovský kľúč dokazuje, že zariadenie je vaše.",
    de: "Diese Box wurde eingerichtet, bevor sich Personen mit eigenem Namen anmeldeten. Wähle deinen Namen und ein Passwort; du wirst ihr erster Administrator. Der Ersatz-Admin-Schlüssel beweist, dass die Box dir gehört.",
  },
  "shell.signIn.nameLabel": { en: "Sign-in name", sk: "Prihlasovacie meno", de: "Anmeldename" },
  "shell.signIn.newPassword": { en: "Password", sk: "Heslo", de: "Passwort" },
  "shell.signIn.make": { en: "Make it", sk: "Vytvoriť", de: "Anlegen" },
  "shell.signIn.made": { en: "Your sign-in is ready", sk: "Prihlásenie je pripravené", de: "Deine Anmeldung ist bereit" },
  "shell.signIn.madeBody": {
    en: "From now on, sign in with {name} and that password.",
    sk: "Odteraz sa prihlasujte menom {name} a týmto heslom.",
    de: "Ab jetzt meldest du dich mit {name} und diesem Passwort an.",
  },
  "shell.signIn.notMade": {
    en: "The box did not make that sign-in.",
    sk: "Zariadenie toto prihlásenie nevytvorilo.",
    de: "Die Box hat diese Anmeldung nicht angelegt.",
  },
  "shell.signIn.useKey": {
    en: "Use the spare admin key instead",
    sk: "Použiť namiesto toho náhradný správcovský kľúč",
    de: "Stattdessen den Ersatz-Admin-Schlüssel verwenden",
  },
  "shell.signIn.hint": {
    en: "Paste the spare admin key. It was shown once, during setup, and is on the sheet if you printed it.",
    sk: "Vložte náhradný správcovský kľúč. Zobrazil sa raz, počas nastavenia, a je na hárku, ak ste si ho vytlačili.",
    de: "Füge den Ersatz-Admin-Schlüssel ein. Er wurde einmal gezeigt, bei der Einrichtung, und steht auf dem Blatt, falls du es gedruckt hast.",
  },
  "shell.signIn.label": { en: "Spare admin key", sk: "Náhradný správcovský kľúč", de: "Ersatz-Admin-Schlüssel" },
  "shell.signIn.remembered": {
    en: "This tab stays unlocked until it is closed.",
    sk: "Táto karta zostane odomknutá, kým ju nezatvoríte.",
    de: "Dieser Tab bleibt entsperrt, bis er geschlossen wird.",
  },
  "shell.signIn.checking": { en: "Checking", sk: "Overujeme", de: "Wird geprüft" },
  "shell.signIn.unlock": { en: "Unlock", sk: "Odomknúť", de: "Entsperren" },

  "ui.working": { en: "Working", sk: "Pracujeme", de: "Wird ausgeführt" },
  "ui.dismiss": { en: "Dismiss", sk: "Zavrieť", de: "Schließen" },
  /* The phone's bottom bar (components/ui/bottom-nav.tsx): its name, and
   * the last tab, which raises every place as a sheet. */
  "ui.bottomNav.label": { en: "Main places", sk: "Hlavné miesta", de: "Wichtigste Orte" },
  "ui.bottomNav.more": { en: "More", sk: "Viac", de: "Mehr" },
  "ui.whatToDo": { en: "What to do", sk: "Čo robiť", de: "Was tun" },
  // The warning a box installed without a TPM shows (components/no-tpm-notice.tsx).
  "ui.noTpm.title": {
    en: "This box has no TPM chip",
    sk: "Toto zariadenie nemá čip TPM",
    de: "Diese Box hat keinen TPM-Chip",
  },
  "ui.noTpm.body": {
    en: "Its disk key sits on the unencrypted boot partition, because there is no chip to keep it in. Anyone who gets hold of the disk can read your files with it. To move the key into a chip, switch the TPM on in the firmware and reinstall.",
    sk: "Kľúč k disku leží na nešifrovanom zavádzacom oddiele, lebo nie je čip, ktorý by ho uchoval. Ktokoľvek, kto získa disk, si ním prečíta vaše súbory. Ak chcete kľúč presunúť do čipu, zapnite TPM vo firmvéri a nainštalujte zariadenie znova.",
    de: "Der Schlüssel zur Festplatte liegt auf der unverschlüsselten Boot-Partition, weil kein Chip da ist, der ihn verwahrt. Wer die Festplatte in die Hände bekommt, kann damit deine Dateien lesen. Um den Schlüssel in einen Chip zu verlegen, schalte das TPM in der Firmware ein und installiere die Box neu.",
  },
  "ui.noTpm.link": { en: "What a TPM does", sk: "Čo robí TPM", de: "Was ein TPM tut" },
  // The banner while the box is not on a mesh (components/mesh-notice.tsx).
  "ui.mesh.title": {
    en: "Some features may not be available",
    sk: "Niektoré funkcie nemusia byť dostupné",
    de: "Manche Funktionen sind vielleicht nicht verfügbar",
  },
  "ui.mesh.notJoined": {
    en: "This box has not joined a mesh yet. Some features need one and stay off until it joins.",
    sk: "Toto zariadenie sa ešte nepripojilo k sieti mesh. Niektoré funkcie ju potrebujú a zostanú vypnuté, kým sa nepripojí.",
    de: "Diese Box ist noch keinem Mesh beigetreten. Manche Funktionen brauchen eines und bleiben aus, bis sie beitritt.",
  },
  "ui.mesh.notSharing": {
    en: "This box is on a mesh but does not share its storage. Some features need that and stay off until it does.",
    sk: "Toto zariadenie je v sieti mesh, ale nezdieľa svoje úložisko. Niektoré funkcie to potrebujú a zostanú vypnuté, kým ho nezačne zdieľať.",
    de: "Diese Box ist in einem Mesh, teilt aber ihren Speicher nicht. Manche Funktionen brauchen das und bleiben aus, bis sie ihn teilt.",
  },
  "ui.mesh.askAdmin": {
    en: "An administrator of this box can change that.",
    sk: "Zmeniť to môže správca tohto zariadenia.",
    de: "Eine Person, die diese Box verwaltet, kann das ändern.",
  },
  "ui.mesh.join": { en: "Join a mesh", sk: "Pripojiť sa k sieti mesh", de: "Einem Mesh beitreten" },
  "ui.mesh.share": { en: "Share storage", sk: "Zdieľať úložisko", de: "Speicher teilen" },
  "ui.notifications": { en: "Notifications", sk: "Upozornenia", de: "Benachrichtigungen" },
  "ui.confirmations": { en: "Confirmations", sk: "Potvrdenia", de: "Bestätigungen" },
  "ui.theme.label": { en: "Appearance", sk: "Vzhľad", de: "Erscheinungsbild" },
  "ui.theme.auto": { en: "Match the browser", sk: "Podľa prehliadača", de: "Wie im Browser" },
  "ui.theme.light": { en: "Light", sk: "Svetlý", de: "Hell" },
  "ui.theme.dark": { en: "Dark", sk: "Tmavý", de: "Dunkel" },
  "ui.language.label": { en: "Language", sk: "Jazyk", de: "Sprache" },
  "ui.language.auto": {
    en: "Browser language",
    sk: "Jazyk prehliadača",
    de: "Browsersprache",
  },
  "ui.display.label": {
    en: "Appearance and language",
    sk: "Vzhľad a jazyk",
    de: "Erscheinungsbild und Sprache",
  },

  // ── Toasts the shell raises ───────────────────────────────────────────────
  "shell.signIn.unlocked": { en: "Unlocked", sk: "Odomknuté", de: "Entsperrt" },
  "shell.signIn.unlockedBody": {
    en: "The admin pages are open in this tab until you close it.",
    sk: "Správcovské stránky sú v tejto karte otvorené, kým ju nezavriete.",
    de: "Die Admin-Seiten sind in diesem Tab geöffnet, bis du ihn schließt.",
  },
});
