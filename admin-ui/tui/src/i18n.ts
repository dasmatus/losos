/* The strings this client adds to the web UI's catalogue: the command line,
 * the keychain, the login screen, the edge panes and the keys. Every key has
 * English, Slovak and German, as the web UI's catalogue requires. */

import { defineMessages } from "@app/i18n/define";

export const TUI_MESSAGES = defineMessages({
  // ── Kinds and shared words ──────────────────────────────────────────────
  "tui.kind.box": { en: "box", sk: "zariadenie", de: "Box" },
  "tui.kind.edge": { en: "edge", sk: "edge", de: "Edge" },
  "tui.loading": { en: "Asking the box", sk: "Pýtam sa zariadenia", de: "Die Box wird gefragt" },
  "tui.couldNotRead": { en: "Could not read this", sk: "Nepodarilo sa to načítať", de: "Konnte nicht gelesen werden" },
  "tui.failed": { en: "That did not work", sk: "Nepodarilo sa", de: "Das hat nicht geklappt" },
  "tui.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "tui.refreshed": { en: "Asked again", sk: "Načítané znova", de: "Neu abgefragt" },
  "tui.searchPanes": { en: "Search", sk: "Hľadať", de: "Suchen" },
  "tui.noPaneMatches": { en: "Nothing matches", sk: "Nič sa nezhoduje", de: "Kein Treffer" },
  "tui.unknownCommand": {
    en: "There is no command called {command}. Run losos --help for the list.",
    sk: "Príkaz {command} neexistuje. Zoznam vypíše losos --help.",
    de: "Einen Befehl {command} gibt es nicht. losos --help zeigt die Liste.",
  },
  "tui.ago.seconds": {
    en: { one: "{count} second", other: "{count} seconds" },
    sk: { one: "{count} sekunda", few: "{count} sekundy", many: "{count} sekúnd", other: "{count} sekúnd" },
    de: { one: "{count} Sekunde", other: "{count} Sekunden" },
  },
  "tui.ago.minutes": {
    en: { one: "{count} minute", other: "{count} minutes" },
    sk: { one: "{count} minúta", few: "{count} minúty", many: "{count} minút", other: "{count} minút" },
    de: { one: "{count} Minute", other: "{count} Minuten" },
  },
  "tui.ago.hours": {
    en: { one: "{count} hour", other: "{count} hours" },
    sk: { one: "{count} hodina", few: "{count} hodiny", many: "{count} hodín", other: "{count} hodín" },
    de: { one: "{count} Stunde", other: "{count} Stunden" },
  },
  "tui.ago.days": {
    en: { one: "{count} day", other: "{count} days" },
    sk: { one: "{count} deň", few: "{count} dni", many: "{count} dní", other: "{count} dní" },
    de: { one: "{count} Tag", other: "{count} Tage" },
  },

  // ── The keychain ────────────────────────────────────────────────────────
  "tui.keychain.noTool": {
    en: "losos keeps keys and tokens in your Secret Service through secret-tool, and secret-tool is not installed. Install libsecret-tools on Debian and Ubuntu, or libsecret on Fedora and Arch. Nothing was stored.",
    sk: "losos uchováva kľúče a tokeny v službe Secret Service cez secret-tool, a secret-tool nie je nainštalovaný. Nainštalujte libsecret-tools na Debiane a Ubuntu, alebo libsecret na Fedore a Archu. Nič sa neuložilo.",
    de: "losos bewahrt Schlüssel und Tokens über secret-tool im Secret Service auf, und secret-tool ist nicht installiert. Installiere libsecret-tools auf Debian und Ubuntu oder libsecret auf Fedora und Arch. Es wurde nichts gespeichert.",
  },
  "tui.keychain.noService": {
    en: "No Secret Service answered, so losos cannot keep anything. Start GNOME Keyring, KWallet or KeePassXC with its Secret Service on. secret-tool said: {detail}",
    sk: "Neodpovedala žiadna služba Secret Service, takže losos nemôže nič uložiť. Spustite GNOME Keyring, KWallet alebo KeePassXC so zapnutou službou Secret Service. secret-tool hlási: {detail}",
    de: "Kein Secret Service hat geantwortet, daher kann losos nichts aufbewahren. Starte GNOME Keyring, KWallet oder KeePassXC mit eingeschaltetem Secret Service. secret-tool meldet: {detail}",
  },
  "tui.keychain.failed": {
    en: "secret-tool failed without saying why. Nothing was stored.",
    sk: "secret-tool zlyhal bez udania dôvodu. Nič sa neuložilo.",
    de: "secret-tool ist ohne Begründung gescheitert. Es wurde nichts gespeichert.",
  },

  // ── Keys ────────────────────────────────────────────────────────────────
  "tui.key.notArmoured": {
    en: "That is not an ASCII-armoured OpenPGP key.",
    sk: "Toto nie je kľúč OpenPGP v textovom (armored) tvare.",
    de: "Das ist kein ASCII-armierter OpenPGP-Schlüssel.",
  },
  "tui.key.notSecret": {
    en: "That is a public key. losos needs the secret key, which signs your requests and reads the answers.",
    sk: "Toto je verejný kľúč. losos potrebuje tajný kľúč, ktorý podpisuje požiadavky a číta odpovede.",
    de: "Das ist ein öffentlicher Schlüssel. losos braucht den geheimen Schlüssel, der deine Anfragen signiert und die Antworten liest.",
  },
  "tui.key.cannotSign": {
    en: "This key cannot sign. The box accepts only requests signed by a key that can.",
    sk: "Tento kľúč nevie podpisovať. Zariadenie prijíma iba požiadavky podpísané kľúčom, ktorý to vie.",
    de: "Dieser Schlüssel kann nicht signieren. Die Box nimmt nur Anfragen an, die mit einem signierfähigen Schlüssel signiert sind.",
  },
  "tui.key.noEncryption": {
    en: "This key has no encryption subkey, so the box cannot encrypt its answers to it. Keys made by gpg --full-generate-key have one.",
    sk: "Tento kľúč nemá šifrovací podkľúč, takže zariadenie mu nevie zašifrovať odpovede. Kľúče z gpg --full-generate-key ho majú.",
    de: "Dieser Schlüssel hat keinen Verschlüsselungs-Unterschlüssel, die Box kann ihm also nicht antworten. Schlüssel aus gpg --full-generate-key haben einen.",
  },
  "tui.key.wrongPassphrase": {
    en: "That passphrase does not unlock the key.",
    sk: "Toto heslo kľúč neodomkne.",
    de: "Diese Passphrase entsperrt den Schlüssel nicht.",
  },
  "tui.key.passphrase": {
    en: "Passphrase for key {fingerprint}: ",
    sk: "Heslo ku kľúču {fingerprint}: ",
    de: "Passphrase für Schlüssel {fingerprint}: ",
  },
  "tui.key.needsPassphrase": {
    en: "The key is under a passphrase and there is no terminal to ask on. Set LOSOS_PASSPHRASE for this run.",
    sk: "Kľúč je chránený heslom a nie je terminál, kde sa naň opýtať. Pre tento beh nastavte LOSOS_PASSPHRASE.",
    de: "Der Schlüssel hat eine Passphrase, und es gibt kein Terminal zum Fragen. Setze LOSOS_PASSPHRASE für diesen Aufruf.",
  },
  "tui.key.imported": {
    en: "Stored key {fingerprint} in your keychain.",
    sk: "Kľúč {fingerprint} je uložený v kľúčenke.",
    de: "Schlüssel {fingerprint} ist im Schlüsselbund gespeichert.",
  },
  "tui.key.protected": {
    en: "It is under a passphrase. losos asks for it once per run and never stores it.",
    sk: "Je chránený heslom. losos sa naň opýta raz pri každom spustení a nikdy ho neukladá.",
    de: "Er hat eine Passphrase. losos fragt einmal pro Aufruf danach und speichert sie nie.",
  },
  "tui.key.noGpg": {
    en: "gpg is not installed, so there is nothing to export from. Import the key from a file instead.",
    sk: "gpg nie je nainštalované, takže nie je odkiaľ exportovať. Importujte kľúč zo súboru.",
    de: "gpg ist nicht installiert, es gibt also nichts zu exportieren. Importiere den Schlüssel aus einer Datei.",
  },
  "tui.key.gpgFailed": {
    en: "gpg exported no secret key for {id}: {detail}",
    sk: "gpg neexportovalo žiadny tajný kľúč pre {id}: {detail}",
    de: "gpg hat für {id} keinen geheimen Schlüssel exportiert: {detail}",
  },
  "tui.key.unreadable": { en: "Cannot read {file}.", sk: "Súbor {file} sa nedá prečítať.", de: "{file} kann nicht gelesen werden." },
  "tui.key.none": {
    en: "No keys yet. Import one with losos key import.",
    sk: "Zatiaľ žiadne kľúče. Importujte jeden príkazom losos key import.",
    de: "Noch keine Schlüssel. Importiere einen mit losos key import.",
  },
  "tui.key.removed": {
    en: "Removed key {fingerprint} from your keychain.",
    sk: "Kľúč {fingerprint} je odstránený z kľúčenky.",
    de: "Schlüssel {fingerprint} wurde aus dem Schlüsselbund entfernt.",
  },
  "tui.key.orphans": {
    en: "These hosts used it and need a new login: {hosts}",
    sk: "Tieto hostiteľa ho používali a potrebujú nové prihlásenie: {hosts}",
    de: "Diese Hosts haben ihn benutzt und brauchen eine neue Anmeldung: {hosts}",
  },
  "tui.key.importUsage": {
    en: "Name a file: losos key import <file.asc>, or a key in gpg: losos key import --gpg <fingerprint or email>.",
    sk: "Uveďte súbor: losos key import <súbor.asc>, alebo kľúč v gpg: losos key import --gpg <odtlačok alebo e-mail>.",
    de: "Nenne eine Datei: losos key import <datei.asc>, oder einen Schlüssel in gpg: losos key import --gpg <Fingerabdruck oder E-Mail>.",
  },
  "tui.key.removeUsage": {
    en: "Name the key: losos key remove <fingerprint>.",
    sk: "Uveďte kľúč: losos key remove <odtlačok>.",
    de: "Nenne den Schlüssel: losos key remove <Fingerabdruck>.",
  },
  "tui.key.usage": {
    en: "losos key import, losos key list or losos key remove.",
    sk: "losos key import, losos key list alebo losos key remove.",
    de: "losos key import, losos key list oder losos key remove.",
  },

  // ── Hosts ───────────────────────────────────────────────────────────────
  "tui.host.none": {
    en: "No box or edge yet. Run losos login <url> first.",
    sk: "Zatiaľ žiadne zariadenie ani edge. Najprv spustite losos login <url>.",
    de: "Noch keine Box und keine Edge. Führe zuerst losos login <url> aus.",
  },
  "tui.host.noEdge": {
    en: "No edge yet. Run losos login https://register.<domain> first.",
    sk: "Zatiaľ žiadny edge. Najprv spustite losos login https://register.<doména>.",
    de: "Noch keine Edge. Führe zuerst losos login https://register.<Domain> aus.",
  },
  "tui.host.unknown": {
    en: "You are not logged in to {url}. Run losos login {url}.",
    sk: "K {url} nie ste prihlásení. Spustite losos login {url}.",
    de: "Du bist bei {url} nicht angemeldet. Führe losos login {url} aus.",
  },
  "tui.host.several": {
    en: "You are logged in to more than one. Pick one with --host: {hosts}",
    sk: "Ste prihlásení k viacerým. Vyberte jeden cez --host: {hosts}",
    de: "Du bist bei mehreren angemeldet. Wähle einen mit --host: {hosts}",
  },
  "tui.host.keyGone": {
    en: "The key this host uses ({fingerprint}) is no longer in your keychain. Import it again, or log in again with another key.",
    sk: "Kľúč, ktorý tento hostiteľ používa ({fingerprint}), už v kľúčenke nie je. Importujte ho znova, alebo sa prihláste znova s iným kľúčom.",
    de: "Der Schlüssel dieses Hosts ({fingerprint}) ist nicht mehr im Schlüsselbund. Importiere ihn erneut oder melde dich mit einem anderen Schlüssel neu an.",
  },
  "tui.host.pinBroken": {
    en: "The stored host key does not match its own fingerprint. Log out and log in again.",
    sk: "Uložený kľúč hostiteľa nezodpovedá vlastnému odtlačku. Odhláste sa a prihláste znova.",
    de: "Der gespeicherte Host-Schlüssel passt nicht zu seinem Fingerabdruck. Melde dich ab und neu an.",
  },

  // ── Login ───────────────────────────────────────────────────────────────
  "tui.login.usage": {
    en: "Name the box or edge: losos login <url>.",
    sk: "Uveďte zariadenie alebo edge: losos login <url>.",
    de: "Nenne die Box oder Edge: losos login <url>.",
  },
  "tui.login.badUrl": { en: "{url} is not an address losos can use.", sk: "{url} nie je adresa, ktorú losos vie použiť.", de: "{url} ist keine Adresse, die losos verwenden kann." },
  "tui.login.unreachable": {
    en: "Cannot reach {url}: {detail}",
    sk: "{url} nie je dostupné: {detail}",
    de: "{url} ist nicht erreichbar: {detail}",
  },
  "tui.login.noHostKey": {
    en: "{url} serves no host key, so it is not a LosOS box or edge that takes terminal logins. Last answer: {detail}",
    sk: "{url} neposkytuje kľúč hostiteľa, takže to nie je zariadenie ani edge LosOS s prihlásením z terminálu. Posledná odpoveď: {detail}",
    de: "{url} liefert keinen Host-Schlüssel, ist also keine LosOS-Box oder -Edge mit Terminal-Anmeldung. Letzte Antwort: {detail}",
  },
  "tui.login.inconsistent": {
    en: "The host key served does not match the fingerprint served beside it. Nothing was stored.",
    sk: "Poskytnutý kľúč hostiteľa nezodpovedá odtlačku poskytnutému vedľa neho. Nič sa neuložilo.",
    de: "Der gelieferte Host-Schlüssel passt nicht zum mitgelieferten Fingerabdruck. Es wurde nichts gespeichert.",
  },
  "tui.login.changed1": {
    en: "THE HOST KEY HAS CHANGED.",
    sk: "KĽÚČ HOSTITEĽA SA ZMENIL.",
    de: "DER HOST-SCHLÜSSEL HAT SICH GEÄNDERT.",
  },
  "tui.login.changed2": {
    en: "{url} now shows a different key from the one you pinned at your first login.",
    sk: "{url} teraz ukazuje iný kľúč, než aký ste si pripli pri prvom prihlásení.",
    de: "{url} zeigt jetzt einen anderen Schlüssel als den, den du bei der ersten Anmeldung festgehalten hast.",
  },
  "tui.login.changed3": {
    en: "Someone may be in the middle, or the box was installed again.",
    sk: "Niekto môže byť uprostred, alebo bolo zariadenie nainštalované znova.",
    de: "Jemand könnte dazwischen sitzen, oder die Box wurde neu installiert.",
  },
  "tui.login.changed4": {
    en: "If you know it was installed again, compare the new fingerprint with the web UI, then run losos logout {url} and log in again.",
    sk: "Ak viete, že bolo nainštalované znova, porovnajte nový odtlačok s webovým rozhraním, potom spustite losos logout {url} a prihláste sa znova.",
    de: "Wenn du weißt, dass sie neu installiert wurde, vergleiche den neuen Fingerabdruck mit der Weboberfläche, führe dann losos logout {url} aus und melde dich neu an.",
  },
  "tui.login.pinned": { en: "pinned", sk: "pripnutý", de: "festgehalten" },
  "tui.login.served": { en: "served", sk: "teraz", de: "jetzt" },
  "tui.login.refused": {
    en: "Refused. Nothing was sent and nothing was stored.",
    sk: "Odmietnuté. Nič sa neodoslalo a nič sa neuložilo.",
    de: "Abgelehnt. Es wurde nichts gesendet und nichts gespeichert.",
  },
  "tui.login.noKeys": {
    en: "There is no key in your keychain to sign with. Import the key that was seeded on the box: losos key import <file.asc> or --gpg <id>.",
    sk: "V kľúčenke nie je kľúč na podpisovanie. Importujte kľúč zadaný do zariadenia pri inštalácii: losos key import <súbor.asc> alebo --gpg <id>.",
    de: "Im Schlüsselbund ist kein Schlüssel zum Signieren. Importiere den Schlüssel, der bei der Installation auf der Box hinterlegt wurde: losos key import <datei.asc> oder --gpg <id>.",
  },
  "tui.login.keyNotStored": {
    en: "Key {fingerprint} is not in your keychain.",
    sk: "Kľúč {fingerprint} nie je v kľúčenke.",
    de: "Schlüssel {fingerprint} ist nicht im Schlüsselbund.",
  },
  "tui.login.flagMismatch": {
    en: "The host key is not the one you named with --fingerprint. It is {served}. Nothing was stored.",
    sk: "Kľúč hostiteľa nie je ten, ktorý ste uviedli cez --fingerprint. Je to {served}. Nič sa neuložilo.",
    de: "Der Host-Schlüssel ist nicht der mit --fingerprint genannte. Er ist {served}. Es wurde nichts gespeichert.",
  },
  "tui.login.needsFingerprint": {
    en: "Without a terminal, confirm the host key with --fingerprint. It shows {served}; compare it with the web UI first.",
    sk: "Bez terminálu potvrďte kľúč hostiteľa cez --fingerprint. Ukazuje {served}; najprv ho porovnajte s webovým rozhraním.",
    de: "Ohne Terminal bestätige den Host-Schlüssel mit --fingerprint. Er zeigt {served}; vergleiche ihn zuerst mit der Weboberfläche.",
  },
  "tui.login.whichKeyFlag": {
    en: "There is more than one key in your keychain. Name one with --key.",
    sk: "V kľúčenke je viac kľúčov. Jeden uveďte cez --key.",
    de: "Im Schlüsselbund ist mehr als ein Schlüssel. Nenne einen mit --key.",
  },
  "tui.login.badToken": {
    en: "That is not an access token. A token starts with losos_pat_ and the web UI shows it once, when you make it.",
    sk: "Toto nie je prístupový token. Token začína na losos_pat_ a webové rozhranie ho ukáže raz, keď ho vytvoríte.",
    de: "Das ist kein Zugangstoken. Ein Token beginnt mit losos_pat_, und die Weboberfläche zeigt es einmal, beim Erstellen.",
  },
  "tui.login.cancelled": { en: "Cancelled. Nothing was stored.", sk: "Zrušené. Nič sa neuložilo.", de: "Abgebrochen. Es wurde nichts gespeichert." },
  "tui.login.done": {
    en: "Logged in to the {kind} at {url}. The host key is pinned, and losos will refuse a different one later.",
    sk: "Prihlásené k {url}, typ {kind}. Kľúč hostiteľa je pripnutý a iný kľúč losos neskôr odmietne.",
    de: "Bei {url} angemeldet, Typ {kind}. Der Host-Schlüssel ist festgehalten, und losos lehnt später einen anderen ab.",
  },
  "tui.login.hostKey": { en: "Host key", sk: "Kľúč hostiteľa", de: "Host-Schlüssel" },
  "tui.login.compareBox": {
    en: "Does this match the fingerprint on the Access tokens pane of the box's web UI?",
    sk: "Zhoduje sa s odtlačkom na paneli Prístupové tokeny vo webovom rozhraní zariadenia?",
    de: "Stimmt das mit dem Fingerabdruck im Bereich Zugangstokens der Weboberfläche der Box überein?",
  },
  "tui.login.compareEdge": {
    en: "Does this match the edge's host key fingerprint from losos-registrar on the operator's machine?",
    sk: "Zhoduje sa s odtlačkom kľúča hostiteľa edge z losos-registrar na počítači operátora?",
    de: "Stimmt das mit dem Fingerabdruck des Host-Schlüssels der Edge aus losos-registrar auf dem Rechner des Betreibers überein?",
  },
  "tui.login.matches": { en: "It matches", sk: "Zhoduje sa", de: "Stimmt überein" },
  "tui.login.differs": { en: "It does not", sk: "Nezhoduje sa", de: "Stimmt nicht" },
  "tui.login.token": { en: "Access token", sk: "Prístupový token", de: "Zugangstoken" },
  "tui.login.tokenHint": {
    en: "Paste it and press Enter. It is not shown as you type.",
    sk: "Vložte ho a stlačte Enter. Pri písaní sa nezobrazuje.",
    de: "Einfügen und Enter drücken. Es wird beim Tippen nicht angezeigt.",
  },
  "tui.login.whichKey": {
    en: "Which key signs for this host?",
    sk: "Ktorý kľúč podpisuje pre tohto hostiteľa?",
    de: "Welcher Schlüssel signiert für diesen Host?",
  },
  "tui.logout.usage": { en: "Name the host: losos logout <url>.", sk: "Uveďte hostiteľa: losos logout <url>.", de: "Nenne den Host: losos logout <url>." },
  "tui.logout.done": {
    en: "Logged out of {url}. Its token and pinned key are gone from your keychain.",
    sk: "Odhlásené z {url}. Jeho token a pripnutý kľúč sú z kľúčenky preč.",
    de: "Von {url} abgemeldet. Token und festgehaltener Schlüssel sind aus dem Schlüsselbund entfernt.",
  },
  "tui.pick.title": { en: "Which one?", sk: "Ktoré?", de: "Welche?" },
  "tui.open.needsTerminal": {
    en: "The full view needs a terminal. For scripts use losos call.",
    sk: "Celé zobrazenie potrebuje terminál. Pre skripty použite losos call.",
    de: "Die volle Ansicht braucht ein Terminal. Für Skripte gibt es losos call.",
  },

  // ── The envelope ────────────────────────────────────────────────────────
  "tui.envelope.badSignature": {
    en: "The answer is not signed by the host key you pinned. losos threw it away unread.",
    sk: "Odpoveď nie je podpísaná pripnutým kľúčom hostiteľa. losos ju zahodil neprečítanú.",
    de: "Die Antwort ist nicht mit dem festgehaltenen Host-Schlüssel signiert. losos hat sie ungelesen verworfen.",
  },
  "tui.envelope.wrongNonce": {
    en: "The answer belongs to another request. losos threw it away unread.",
    sk: "Odpoveď patrí k inej požiadavke. losos ju zahodil neprečítanú.",
    de: "Die Antwort gehört zu einer anderen Anfrage. losos hat sie ungelesen verworfen.",
  },
  "tui.envelope.notAnswer": {
    en: "The host sent something that is not an answer.",
    sk: "Hostiteľ poslal niečo, čo nie je odpoveď.",
    de: "Der Host hat etwas geschickt, das keine Antwort ist.",
  },
  "tui.envelope.unreachable": {
    en: "Cannot reach {url}: {detail}",
    sk: "{url} nie je dostupné: {detail}",
    de: "{url} ist nicht erreichbar: {detail}",
  },

  // ── losos call ──────────────────────────────────────────────────────────
  "tui.call.usage": {
    en: "losos call <METHOD> <path> [json-body], for example: losos call GET /api/state",
    sk: "losos call <METÓDA> <cesta> [json], napríklad: losos call GET /api/state",
    de: "losos call <METHODE> <Pfad> [json], zum Beispiel: losos call GET /api/state",
  },
  "tui.call.badJson": { en: "The body is not JSON.", sk: "Telo nie je JSON.", de: "Der Inhalt ist kein JSON." },
  "tui.call.notSealed": { en: "refused before sealing", sk: "odmietnuté pred zapečatením", de: "vor dem Versiegeln abgelehnt" },

  // ── The full view ───────────────────────────────────────────────────────
  "tui.overview.summary": {
    en: "How the box is doing, its apps and its disk.",
    sk: "Ako sa zariadeniu darí, jeho aplikácie a disk.",
    de: "Wie es der Box geht, ihre Apps und ihr Speicher.",
  },
  "tui.overview.keywords": { en: "home status", sk: "domov stav", de: "start status" },
  "tui.apply.title": { en: "Apply and rebuild?", sk: "Použiť a znova zostaviť?", de: "Anwenden und neu bauen?" },
  "tui.apply.body": {
    en: {
      one: "{count} change goes out and the box rebuilds itself. It stays reachable, and the outcome shows here in a few minutes.",
      other: "{count} changes go out and the box rebuilds itself. It stays reachable, and the outcome shows here in a few minutes.",
    },
    sk: {
      one: "Odošle sa {count} zmena a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.",
      few: "Odošlú sa {count} zmeny a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.",
      many: "Odošle sa {count} zmien a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.",
      other: "Odošle sa {count} zmien a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.",
    },
    de: {
      one: "{count} Änderung wird übertragen und die Box baut sich neu. Sie bleibt erreichbar, das Ergebnis erscheint hier in ein paar Minuten.",
      other: "{count} Änderungen werden übertragen und die Box baut sich neu. Sie bleibt erreichbar, das Ergebnis erscheint hier in ein paar Minuten.",
    },
  },
  "tui.edit.hint": { en: "Enter saves to the draft. Esc leaves it as it was.", sk: "Enter uloží do návrhu. Esc nechá pôvodné.", de: "Enter übernimmt in den Entwurf. Esc lässt es, wie es war." },
  "tui.edit.hintMasked": {
    en: "Not shown as you type. Enter sends it. Esc cancels.",
    sk: "Pri písaní sa nezobrazuje. Enter odošle. Esc zruší.",
    de: "Wird beim Tippen nicht angezeigt. Enter sendet. Esc bricht ab.",
  },
  "tui.storage.grew": {
    en: "The disk now holds {size}.",
    sk: "Disk má teraz {size}.",
    de: "Der Speicher fasst jetzt {size}.",
  },
  "tui.storage.notGrown": {
    en: "The box measured no change in size.",
    sk: "Zariadenie nenameralo zmenu veľkosti.",
    de: "Die Box hat keine Größenänderung gemessen.",
  },
  "tui.apps.removing": { en: "Removing {name}", sk: "Odstraňuje sa {name}", de: "{name} wird entfernt" },
  "tui.apps.sharedInWeb": {
    en: "It runs as the private user. To run it as the shared user or change its properties, install it from the web UI.",
    sk: "Spustí sa ako súkromný používateľ. Ak ho chcete spustiť ako zdieľaného používateľa alebo zmeniť vlastnosti, nainštalujte ho z webového rozhrania.",
    de: "Sie läuft als privater Benutzer. Um sie als geteilter Benutzer laufen zu lassen oder Eigenschaften zu ändern, installiere sie über die Weboberfläche.",
  },
  "tui.advanced.summary": {
    en: "Every option this box declares and the value it has now, including the ones the other panes do not show. Change them in the web UI.",
    sk: "Každá možnosť, ktorú toto zariadenie deklaruje, a jej súčasná hodnota, aj tie, ktoré ostatné panely nezobrazujú. Meníte ich vo webovom rozhraní.",
    de: "Jede Option, die diese Box deklariert, mit ihrem aktuellen Wert, auch die, die die anderen Bereiche nicht zeigen. Geändert werden sie in der Weboberfläche.",
  },
  "tui.history.synced": { en: "Synced with LosOS Git", sk: "Synchronizované s LosOS Git", de: "Mit LosOS Git abgeglichen" },
  "tui.backup.noTarget": { en: "No bucket set", sk: "Nie je nastavený bucket", de: "Kein Bucket eingestellt" },
  "tui.backup.inWeb": {
    en: "You set the bucket, the recovery code and restores in the web UI.",
    sk: "Bucket, kód na obnovenie a obnovu nastavujete vo webovom rozhraní.",
    de: "Bucket, Wiederherstellungscode und Wiederherstellen stellst du in der Weboberfläche ein.",
  },
  "tui.about.title": { en: "This terminal", sk: "Tento terminál", de: "Dieses Terminal" },
  "tui.about.client": { en: "Client", sk: "Klient", de: "Client" },
  "tui.about.hostKey": { en: "Host key", sk: "Kľúč hostiteľa", de: "Host-Schlüssel" },
  "tui.about.userKey": { en: "Your key", sk: "Váš kľúč", de: "Dein Schlüssel" },
  "tui.about.webOnly": {
    en: "Factory reset and erasing the box are in the web UI only, as are access tokens.",
    sk: "Obnovenie továrenských nastavení a vymazanie zariadenia sú iba vo webovom rozhraní, rovnako ako prístupové tokeny.",
    de: "Zurücksetzen und Löschen der Box gibt es nur in der Weboberfläche, ebenso die Zugangstokens.",
  },

  // ── Keys in the full view ───────────────────────────────────────────────
  "tui.keys.move": { en: "move", sk: "pohyb", de: "bewegen" },
  "tui.keys.open": { en: "open", sk: "otvoriť", de: "öffnen" },
  "tui.keys.change": { en: "change", sk: "zmeniť", de: "ändern" },
  "tui.keys.switch": { en: "panes", sk: "panely", de: "Bereiche" },
  "tui.keys.search": { en: "search", sk: "hľadať", de: "suchen" },
  "tui.keys.clear": { en: "clear", sk: "vymazať", de: "leeren" },
  "tui.keys.refresh": { en: "refresh", sk: "obnoviť", de: "neu laden" },
  "tui.keys.help": { en: "help", sk: "pomoc", de: "Hilfe" },
  "tui.keys.quit": { en: "quit", sk: "koniec", de: "beenden" },
  "tui.help.title": { en: "Keys", sk: "Klávesy", de: "Tasten" },
  "tui.help.move": { en: "Move up and down", sk: "Pohyb hore a dole", de: "Nach oben und unten" },
  "tui.help.enter": {
    en: "Open a pane, flip a switch, edit a value, run an action",
    sk: "Otvoriť panel, prepnúť vypínač, upraviť hodnotu, spustiť akciu",
    de: "Bereich öffnen, Schalter umlegen, Wert bearbeiten, Aktion starten",
  },
  "tui.help.tab": { en: "Between the sidebar and the pane", sk: "Medzi bočným panelom a obsahom", de: "Zwischen Seitenleiste und Bereich" },
  "tui.help.back": { en: "Back to the sidebar", sk: "Späť na bočný panel", de: "Zurück zur Seitenleiste" },
  "tui.help.search": { en: "Search the panes", sk: "Hľadať v paneloch", de: "Bereiche durchsuchen" },
  "tui.help.apply": {
    en: "Apply or discard the changes waiting",
    sk: "Použiť alebo zahodiť čakajúce zmeny",
    de: "Wartende Änderungen anwenden oder verwerfen",
  },
  "tui.help.refresh": { en: "Ask the box again", sk: "Opýtať sa zariadenia znova", de: "Die Box erneut fragen" },
  "tui.help.page": { en: "Move a page", sk: "Posun o stranu", de: "Seitenweise bewegen" },
  "tui.help.quit": { en: "Quit", sk: "Koniec", de: "Beenden" },
  "tui.help.close": { en: "Any key closes this.", sk: "Ľubovoľný kláves toto zavrie.", de: "Eine beliebige Taste schließt das." },

  // ── --help ──────────────────────────────────────────────────────────────
  "tui.help.what": {
    en: "Manage LosOS boxes and edges from a terminal, without SSH. Every request is signed with your key and encrypted to the host.",
    sk: "Správa zariadení a edge LosOS z terminálu, bez SSH. Každá požiadavka je podpísaná vaším kľúčom a zašifrovaná pre hostiteľa.",
    de: "LosOS-Boxen und -Edges vom Terminal aus verwalten, ohne SSH. Jede Anfrage ist mit deinem Schlüssel signiert und für den Host verschlüsselt.",
  },
  "tui.help.commands": { en: "Commands:", sk: "Príkazy:", de: "Befehle:" },
  "tui.help.options": { en: "Options:", sk: "Voľby:", de: "Optionen:" },
  "tui.help.cmd.open": {
    en: "The full view of the one box or edge, or the one named",
    sk: "Celé zobrazenie jediného zariadenia alebo edge, alebo uvedeného",
    de: "Die volle Ansicht der einzigen Box oder Edge, oder der genannten",
  },
  "tui.help.cmd.login": {
    en: "Pin the host key, store a token and pick the key that signs",
    sk: "Pripnúť kľúč hostiteľa, uložiť token a vybrať podpisový kľúč",
    de: "Host-Schlüssel festhalten, Token speichern, signierenden Schlüssel wählen",
  },
  "tui.help.cmd.logout": { en: "Forget a host's token and pinned key", sk: "Zabudnúť token a pripnutý kľúč hostiteľa", de: "Token und festgehaltenen Schlüssel eines Hosts vergessen" },
  "tui.help.cmd.hosts": { en: "List the hosts you are logged in to", sk: "Vypísať hostiteľov, ku ktorým ste prihlásení", de: "Hosts auflisten, bei denen du angemeldet bist" },
  "tui.help.cmd.keyImport": { en: "Store a secret key from a file", sk: "Uložiť tajný kľúč zo súboru", de: "Geheimen Schlüssel aus einer Datei speichern" },
  "tui.help.cmd.keyImportGpg": {
    en: "Store a secret key exported from gpg",
    sk: "Uložiť tajný kľúč exportovaný z gpg",
    de: "Aus gpg exportierten geheimen Schlüssel speichern",
  },
  "tui.help.cmd.keyList": { en: "List the stored keys", sk: "Vypísať uložené kľúče", de: "Gespeicherte Schlüssel auflisten" },
  "tui.help.cmd.keyRemove": { en: "Remove a stored key", sk: "Odstrániť uložený kľúč", de: "Gespeicherten Schlüssel entfernen" },
  "tui.help.cmd.call": {
    en: "One request; prints the status and the JSON answer",
    sk: "Jedna požiadavka; vypíše stav a odpoveď v JSON",
    de: "Eine Anfrage; gibt Status und JSON-Antwort aus",
  },
  "tui.help.cmd.releaser": {
    en: "Show the escrow key's address and ETH, or make it (--new)",
    sk: "Zobraziť adresu a ETH kľúča pre escrow, alebo ho vytvoriť (--new)",
    de: "Adresse und ETH des Escrow-Schlüssels zeigen oder ihn erzeugen (--new)",
  },
  "tui.help.opt.newReleaser": {
    en: "With escrow releaser: make the key, or replace the one sealed",
    sk: "Pri escrow releaser: vytvoriť kľúč, alebo nahradiť zapečatený",
    de: "Mit escrow releaser: den Schlüssel erzeugen oder den versiegelten ersetzen",
  },
  "tui.help.cmd.secretList": { en: "List an edge's secrets", sk: "Vypísať tajomstvá edge", de: "Geheimnisse einer Edge auflisten" },
  "tui.help.cmd.secretSet": {
    en: "Seal one secret on an edge, typed without echo or piped in",
    sk: "Zapečatiť jedno tajomstvo na edge, písané bez zobrazenia alebo cez rúru",
    de: "Ein Geheimnis auf einer Edge versiegeln, verdeckt getippt oder per Pipe",
  },
  "tui.help.cmd.reconcile": {
    en: "Regenerate an edge's Traefik and rathole config now",
    sk: "Hneď znova vytvoriť konfiguráciu Traefik a rathole na edge",
    de: "Traefik- und rathole-Konfiguration einer Edge jetzt neu erzeugen",
  },
  "tui.help.cmd.ciSetup": {
    en: "Make a key for CI and put it in GitHub Actions with gh",
    sk: "Vytvoriť kľúč pre CI a vložiť ho do GitHub Actions cez gh",
    de: "Einen Schlüssel für CI erzeugen und mit gh in GitHub Actions ablegen",
  },
  "tui.help.opt.host": { en: "Which host, when you have more than one", sk: "Ktorý hostiteľ, keď ich máte viac", de: "Welcher Host, wenn es mehrere gibt" },
  "tui.help.opt.lang": { en: "Language, else from LANG", sk: "Jazyk, inak podľa LANG", de: "Sprache, sonst aus LANG" },
  "tui.help.opt.theme": { en: "Colours, else from COLORFGBG, else dark", sk: "Farby, inak podľa COLORFGBG, inak tmavé", de: "Farben, sonst aus COLORFGBG, sonst dunkel" },
  "tui.help.opt.key": { en: "login: the key that signs", sk: "login: podpisový kľúč", de: "login: der signierende Schlüssel" },
  "tui.help.opt.fingerprint": {
    en: "login: confirm the host key without a terminal",
    sk: "login: potvrdiť kľúč hostiteľa bez terminálu",
    de: "login: Host-Schlüssel ohne Terminal bestätigen",
  },
  "tui.help.opt.repo": { en: "ci setup: the GitHub repository", sk: "ci setup: repozitár na GitHube", de: "ci setup: das GitHub-Repository" },
  "tui.help.opt.print": {
    en: "ci setup: print the values when gh cannot set them",
    sk: "ci setup: vypísať hodnoty, keď ich gh nevie nastaviť",
    de: "ci setup: Werte ausgeben, wenn gh sie nicht setzen kann",
  },
  "tui.help.opt.version": { en: "Print the version", sk: "Vypísať verziu", de: "Version ausgeben" },
  "tui.help.keychain": {
    en: "Keys and tokens live in your Secret Service, such as GNOME Keyring, KWallet or KeePassXC, through secret-tool. losos writes no plain file.",
    sk: "Kľúče a tokeny sú v službe Secret Service, napríklad GNOME Keyring, KWallet alebo KeePassXC, cez secret-tool. losos nezapisuje žiadny obyčajný súbor.",
    de: "Schlüssel und Tokens liegen über secret-tool im Secret Service, etwa GNOME Keyring, KWallet oder KeePassXC. losos schreibt keine einfache Datei.",
  },
  "tui.help.ci": {
    en: "With LOSOS_CI=1, call and edge read the key, token, URL and host key from LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL and LOSOS_EDGE_HOST_KEY and never touch the keychain.",
    sk: "S LOSOS_CI=1 čítajú call a edge kľúč, token, URL a kľúč hostiteľa z LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL a LOSOS_EDGE_HOST_KEY a kľúčenky sa nedotknú.",
    de: "Mit LOSOS_CI=1 lesen call und edge Schlüssel, Token, URL und Host-Schlüssel aus LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL und LOSOS_EDGE_HOST_KEY und rühren den Schlüsselbund nicht an.",
  },

  // ── The edge ────────────────────────────────────────────────────────────
  "tui.edge.usage": {
    en: "losos edge secret list, losos edge secret set <name>, losos edge reconcile or losos edge escrow releaser.",
    sk: "losos edge secret list, losos edge secret set <názov>, losos edge reconcile alebo losos edge escrow releaser.",
    de: "losos edge secret list, losos edge secret set <Name>, losos edge reconcile oder losos edge escrow releaser.",
  },
  "tui.edge.releaserNone": {
    en: "This edge has no escrow key yet. Make one with losos edge escrow releaser --new.",
    sk: "Tento edge ešte nemá kľúč pre escrow. Vytvorte ho príkazom losos edge escrow releaser --new.",
    de: "Diese Edge hat noch keinen Escrow-Schlüssel. Legen Sie ihn mit losos edge escrow releaser --new an.",
  },
  "tui.edge.releaserAddress": {
    en: "The escrow takes release and refund calls from {address}.",
    sk: "Escrow prijíma uvoľnenia a vrátenia z adresy {address}.",
    de: "Der Escrow nimmt Freigaben und Erstattungen von {address} an.",
  },
  "tui.edge.releaserAgrees": {
    en: "the escrow takes orders from it",
    sk: "escrow od neho prijíma príkazy",
    de: "der Escrow nimmt Aufträge von ihm an",
  },
  "tui.edge.releaserDisagrees": {
    en: "the escrow {escrow} names another key; its treasury calls setReleaser with this address",
    sk: "escrow {escrow} uvádza iný kľúč; jeho pokladnica zavolá setReleaser s touto adresou",
    de: "der Escrow {escrow} nennt einen anderen Schlüssel; seine Treasury ruft setReleaser mit dieser Adresse auf",
  },
  "tui.edge.releaserMade": {
    en: "The edge made a new escrow key and sealed it. Its address is {address}. The key itself never leaves the edge.",
    sk: "Edge vytvoril nový kľúč pre escrow a zapečatil ho. Jeho adresa je {address}. Samotný kľúč edge nikdy neopustí.",
    de: "Die Edge hat einen neuen Escrow-Schlüssel erzeugt und versiegelt. Seine Adresse ist {address}. Der Schlüssel selbst verlässt die Edge nie.",
  },
  "tui.edge.releaserNext": {
    en: "Send {address} a little ETH on every EVM network for gas. Deploy LososEscrow with it as the releaser, or have the treasury call setReleaser({address}) on each escrow already deployed.",
    sk: "Pošlite na {address} trochu ETH na každej sieti EVM na poplatky. Nasaďte LososEscrow s touto adresou ako releaser, alebo nech pokladnica zavolá setReleaser({address}) na každom už nasadenom escrow.",
    de: "Senden Sie {address} auf jedem EVM-Netz etwas ETH für Gas. Setzen Sie LososEscrow mit dieser Adresse als Releaser auf, oder lassen Sie die Treasury auf jedem bestehenden Escrow setReleaser({address}) aufrufen.",
  },
  "tui.edge.notEdge": { en: "{url} is a box, not an edge.", sk: "{url} je zariadenie, nie edge.", de: "{url} ist eine Box, keine Edge." },
  "tui.edge.overviewSummary": {
    en: "The edge's health, its tenants and their last heartbeat, and what is switched on.",
    sk: "Stav edge, jeho nájomníci s posledným signálom a čo je zapnuté.",
    de: "Zustand der Edge, ihre Mandanten mit letztem Lebenszeichen und was eingeschaltet ist.",
  },
  "tui.edge.overviewKeywords": { en: "tenants heartbeat", sk: "nájomníci signál", de: "mandanten lebenszeichen" },
  "tui.edge.thisEdge": { en: "This edge", sk: "Tento edge", de: "Diese Edge" },
  "tui.edge.official": { en: "Holds a LosOS certificate", sk: "Má certifikát LosOS", de: "Hat ein LosOS-Zertifikat" },
  "tui.edge.certificate": { en: "Certificate valid until", sk: "Certifikát platí do", de: "Zertifikat gültig bis" },
  "tui.edge.enrolled": { en: "Enrolled boxes", sk: "Zapísané zariadenia", de: "Eingeschriebene Boxen" },
  "tui.edge.routes": { en: "Relay routes", sk: "Trasy relé", de: "Relay-Routen" },
  "tui.edge.domains": { en: "Domains", sk: "Domény", de: "Domains" },
  "tui.edge.reconcile": { en: "Reconcile now", sk: "Zosúladiť teraz", de: "Jetzt abgleichen" },
  "tui.edge.reconcileDetail": {
    en: "Writes the Traefik and rathole config again from the registry, without waiting for the timer.",
    sk: "Znova zapíše konfiguráciu Traefik a rathole z registra, bez čakania na časovač.",
    de: "Schreibt die Traefik- und rathole-Konfiguration aus dem Register neu, ohne auf den Timer zu warten.",
  },
  "tui.edge.reconcileButton": { en: "Reconcile", sk: "Zosúladiť", de: "Abgleichen" },
  "tui.edge.reconciled": {
    en: "The edge wrote its Traefik and rathole config again.",
    sk: "Edge znova zapísal konfiguráciu Traefik a rathole.",
    de: "Die Edge hat ihre Traefik- und rathole-Konfiguration neu geschrieben.",
  },
  "tui.edge.optIns": { en: "Opt-ins", sk: "Zapnuté funkcie", de: "Zuschaltbares" },
  "tui.edge.optInsCaption": {
    en: "The edge's NixOS configuration sets these. This view cannot change them.",
    sk: "Nastavuje ich konfigurácia NixOS na edge. Toto zobrazenie ich nezmení.",
    de: "Die NixOS-Konfiguration der Edge legt sie fest. Diese Ansicht ändert sie nicht.",
  },
  "tui.edge.tenants": {
    en: { one: "{count} tenant", other: "{count} tenants" },
    sk: { one: "{count} nájomník", few: "{count} nájomníci", many: "{count} nájomníkov", other: "{count} nájomníkov" },
    de: { one: "{count} Mandant", other: "{count} Mandanten" },
  },
  "tui.edge.tenant": { en: "Tenant", sk: "Nájomník", de: "Mandant" },
  "tui.edge.hostname": { en: "Host name", sk: "Názov hostiteľa", de: "Hostname" },
  "tui.edge.heartbeat": { en: "Last heartbeat", sk: "Posledný signál", de: "Letztes Lebenszeichen" },
  "tui.edge.ago": { en: "{ago} ago", sk: "pred {ago}", de: "vor {ago}" },
  "tui.edge.noTenants": { en: "No boxes have registered yet.", sk: "Zatiaľ sa nezaregistrovalo žiadne zariadenie.", de: "Noch hat sich keine Box registriert." },
  "tui.edge.tenantsCaption": {
    en: "A box sends a heartbeat every minute. This view marks one that has been quiet for two minutes.",
    sk: "Zariadenie posiela signál každú minútu. Toto zobrazenie označí také, ktoré dve minúty mlčí.",
    de: "Eine Box sendet jede Minute ein Lebenszeichen. Diese Ansicht markiert eine, die zwei Minuten still ist.",
  },
  "tui.edge.secrets": { en: "Secrets", sk: "Tajomstvá", de: "Geheimnisse" },
  "tui.edge.secretsSummary": {
    en: "The edge's sealed credentials. The edge seals each value and never shows it again.",
    sk: "Zapečatené prístupové údaje edge. Edge každú hodnotu zapečatí a už ju nikdy neukáže.",
    de: "Die versiegelten Zugangsdaten der Edge. Die Edge versiegelt jeden Wert und zeigt ihn nie wieder.",
  },
  "tui.edge.secretsKeywords": { en: "credentials stripe", sk: "prístupové údaje stripe", de: "zugangsdaten stripe" },
  "tui.edge.noSecrets": { en: "This edge declares no secrets.", sk: "Tento edge nedeklaruje žiadne tajomstvá.", de: "Diese Edge deklariert keine Geheimnisse." },
  "tui.edge.sealed": { en: "Sealed", sk: "Zapečatené", de: "Versiegelt" },
  "tui.edge.sealedAt": { en: "Sealed {when}", sk: "Zapečatené {when}", de: "Versiegelt {when}" },
  "tui.edge.notSealed": { en: "Not set", sk: "Nenastavené", de: "Nicht gesetzt" },
  "tui.edge.newValue": { en: "New value for {name}", sk: "Nová hodnota pre {name}", de: "Neuer Wert für {name}" },
  "tui.edge.valuePrompt": { en: "Value for {name}: ", sk: "Hodnota pre {name}: ", de: "Wert für {name}: " },
  "tui.edge.emptyValue": { en: "The value is empty. Nothing was sent.", sk: "Hodnota je prázdna. Nič sa neodoslalo.", de: "Der Wert ist leer. Es wurde nichts gesendet." },
  "tui.edge.secretSet": { en: "Sealed {name} on the edge.", sk: "{name} je zapečatené na edge.", de: "{name} ist auf der Edge versiegelt." },
  "tui.edge.secretsCaption": {
    en: "The edge checks the value's shape before it seals it. Enter on a row sets a new value.",
    sk: "Edge pred zapečatením skontroluje tvar hodnoty. Enter na riadku nastaví novú hodnotu.",
    de: "Die Edge prüft die Form des Werts, bevor sie ihn versiegelt. Enter auf einer Zeile setzt einen neuen Wert.",
  },

  // ── ci setup ────────────────────────────────────────────────────────────
  "tui.ci.usage": {
    en: "losos ci setup --edge <url> [--repo owner/name] [--print]",
    sk: "losos ci setup --edge <url> [--repo vlastník/názov] [--print]",
    de: "losos ci setup --edge <url> [--repo besitzer/name] [--print]",
  },
  "tui.ci.notLoggedIn": {
    en: "Log in to the edge first, so its host key is pinned: losos login {url}",
    sk: "Najprv sa prihláste k edge, aby bol jeho kľúč pripnutý: losos login {url}",
    de: "Melde dich zuerst bei der Edge an, damit ihr Host-Schlüssel festgehalten ist: losos login {url}",
  },
  "tui.ci.generated": {
    en: "Made a new key for CI in {repo}, not your own key. Its fingerprint:",
    sk: "Vytvorený nový kľúč pre CI v {repo}, nie váš vlastný. Jeho odtlačok:",
    de: "Neuer Schlüssel für CI in {repo} erzeugt, nicht dein eigener. Sein Fingerabdruck:",
  },
  "tui.ci.seed": {
    en: "Seed the public key above in the edge's NixOS configuration, under",
    sk: "Verejný kľúč vyššie zadajte do konfigurácie NixOS na edge, do",
    de: "Hinterlege den öffentlichen Schlüssel oben in der NixOS-Konfiguration der Edge, unter",
  },
  "tui.ci.token": {
    en: "and rebuild it. Then make an edge token bound to this key, on your own machine:",
    sk: "a edge znova zostavte. Potom na svojom počítači vytvorte token edge viazaný na tento kľúč:",
    de: "und baue sie neu. Erzeuge dann auf deinem eigenen Rechner ein an diesen Schlüssel gebundenes Edge-Token:",
  },
  "tui.ci.tokenPrompt": { en: "Edge token for CI: ", sk: "Token edge pre CI: ", de: "Edge-Token für CI: " },
  "tui.ci.setSecret": { en: "Set secret {name} in {repo}.", sk: "Nastavené tajomstvo {name} v {repo}.", de: "Secret {name} in {repo} gesetzt." },
  "tui.ci.setVariable": { en: "Set variable {name} in {repo}.", sk: "Nastavená premenná {name} v {repo}.", de: "Variable {name} in {repo} gesetzt." },
  "tui.ci.ghFailed": {
    en: "gh could not set {name} and exited with {code}. Check gh auth status and your rights on the repository.",
    sk: "gh nevedel nastaviť {name} a skončil s kódom {code}. Skontrolujte gh auth status a svoje práva v repozitári.",
    de: "gh konnte {name} nicht setzen und endete mit Code {code}. Prüfe gh auth status und deine Rechte am Repository.",
  },
  "tui.ci.noGh": {
    en: "gh is not installed or not signed in, so add these four to {repo} by hand, under Settings, Secrets and variables, Actions:",
    sk: "gh nie je nainštalovaný alebo prihlásený, takže tieto štyri pridajte do {repo} ručne, v Settings, Secrets and variables, Actions:",
    de: "gh ist nicht installiert oder nicht angemeldet, also trage diese vier in {repo} von Hand ein, unter Settings, Secrets and variables, Actions:",
  },
  "tui.ci.kindSecret": { en: "secret", sk: "tajomstvo", de: "Secret" },
  "tui.ci.kindVariable": { en: "variable", sk: "premenná", de: "Variable" },
  "tui.ci.printHint": {
    en: "Run again with --print to see the values. The key and the token are secret.",
    sk: "Hodnoty uvidíte po spustení s --print. Kľúč a token sú tajné.",
    de: "Mit --print siehst du die Werte. Schlüssel und Token sind geheim.",
  },
  "tui.ci.rotate": {
    en: "The CI key {fingerprint} is also in your keychain, so you can rotate it later.",
    sk: "Kľúč CI {fingerprint} je aj v kľúčenke, aby ste ho neskôr mohli vymeniť.",
    de: "Der CI-Schlüssel {fingerprint} liegt auch im Schlüsselbund, damit du ihn später tauschen kannst.",
  },
  "tui.ci.missingVar": {
    en: "LOSOS_CI=1 is set but {name} is empty. losos ci setup puts it in the repository.",
    sk: "LOSOS_CI=1 je nastavené, ale {name} je prázdne. losos ci setup ho vloží do repozitára.",
    de: "LOSOS_CI=1 ist gesetzt, aber {name} ist leer. losos ci setup legt es im Repository ab.",
  },
  "tui.ci.keyProtected": {
    en: "LOSOS_CI_GPG_KEY is under a passphrase. CI needs a key without one, as losos ci setup makes.",
    sk: "LOSOS_CI_GPG_KEY je chránený heslom. CI potrebuje kľúč bez hesla, aký vytvorí losos ci setup.",
    de: "LOSOS_CI_GPG_KEY hat eine Passphrase. CI braucht einen Schlüssel ohne, wie ihn losos ci setup erzeugt.",
  },
});
