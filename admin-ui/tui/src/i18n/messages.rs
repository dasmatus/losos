//! This client's own strings, the ones the web UI has no words for. Every
//! key is in English, Slovak and German; a plural entry lists the CLDR forms
//! its language uses and always an `other`.

use super::{Entry, Text};

macro_rules! text {
    ({ $($form:ident: $s:literal),+ $(,)? }) => {
        Text::Plural(&[$((stringify!($form), $s)),+])
    };
    ($s:literal) => {
        Text::One($s)
    };
}

macro_rules! messages {
    ($($key:literal => { en: $en:tt, sk: $sk:tt, de: $de:tt $(,)? }),* $(,)?) => {
        pub(crate) static TUI: &[(&str, Entry)] = &[
            $(($key, Entry { en: text!($en), sk: text!($sk), de: text!($de) })),*
        ];
    };
}

messages! {
    // ── Kinds and shared words ──────────────────────────────────────────────
    "tui.kind.box" => {
        en: "box",
        sk: "zariadenie",
        de: "Box",
    },
    "tui.kind.edge" => {
        en: "edge",
        sk: "edge",
        de: "Edge",
    },
    "tui.loading" => {
        en: "Asking the box",
        sk: "Pýtam sa zariadenia",
        de: "Die Box wird gefragt",
    },
    "tui.couldNotRead" => {
        en: "Could not read this",
        sk: "Nepodarilo sa to načítať",
        de: "Konnte nicht gelesen werden",
    },
    "tui.failed" => {
        en: "That did not work",
        sk: "Nepodarilo sa",
        de: "Das hat nicht geklappt",
    },
    "tui.cancel" => {
        en: "Cancel",
        sk: "Zrušiť",
        de: "Abbrechen",
    },
    "tui.refreshed" => {
        en: "Asked again",
        sk: "Načítané znova",
        de: "Neu abgefragt",
    },
    "tui.searchPanes" => {
        en: "Search",
        sk: "Hľadať",
        de: "Suchen",
    },
    "tui.noPaneMatches" => {
        en: "Nothing matches",
        sk: "Nič sa nezhoduje",
        de: "Kein Treffer",
    },
    "tui.ago.seconds" => {
        en: { one: "{count} second", other: "{count} seconds" },
        sk: { one: "{count} sekunda", few: "{count} sekundy", many: "{count} sekúnd", other: "{count} sekúnd" },
        de: { one: "{count} Sekunde", other: "{count} Sekunden" },
    },
    "tui.ago.minutes" => {
        en: { one: "{count} minute", other: "{count} minutes" },
        sk: { one: "{count} minúta", few: "{count} minúty", many: "{count} minút", other: "{count} minút" },
        de: { one: "{count} Minute", other: "{count} Minuten" },
    },
    "tui.ago.hours" => {
        en: { one: "{count} hour", other: "{count} hours" },
        sk: { one: "{count} hodina", few: "{count} hodiny", many: "{count} hodín", other: "{count} hodín" },
        de: { one: "{count} Stunde", other: "{count} Stunden" },
    },
    "tui.ago.days" => {
        en: { one: "{count} day", other: "{count} days" },
        sk: { one: "{count} deň", few: "{count} dni", many: "{count} dní", other: "{count} dní" },
        de: { one: "{count} Tag", other: "{count} Tage" },
    },
    // ── The keychain ────────────────────────────────────────────────────────
    "tui.keychain.noTool" => {
        en: "losos keeps keys and tokens in your Secret Service through secret-tool, and secret-tool is not installed. Install libsecret-tools on Debian and Ubuntu, or libsecret on Fedora and Arch. Nothing was stored.",
        sk: "losos uchováva kľúče a tokeny v službe Secret Service cez secret-tool, a secret-tool nie je nainštalovaný. Nainštalujte libsecret-tools na Debiane a Ubuntu, alebo libsecret na Fedore a Archu. Nič sa neuložilo.",
        de: "losos bewahrt Schlüssel und Tokens über secret-tool im Secret Service auf, und secret-tool ist nicht installiert. Installiere libsecret-tools auf Debian und Ubuntu oder libsecret auf Fedora und Arch. Es wurde nichts gespeichert.",
    },
    "tui.keychain.noService" => {
        en: "No Secret Service answered, so losos cannot keep anything. Start GNOME Keyring, KWallet or KeePassXC with its Secret Service on. secret-tool said: {detail}",
        sk: "Neodpovedala žiadna služba Secret Service, takže losos nemôže nič uložiť. Spustite GNOME Keyring, KWallet alebo KeePassXC so zapnutou službou Secret Service. secret-tool hlási: {detail}",
        de: "Kein Secret Service hat geantwortet, daher kann losos nichts aufbewahren. Starte GNOME Keyring, KWallet oder KeePassXC mit eingeschaltetem Secret Service. secret-tool meldet: {detail}",
    },
    "tui.keychain.failed" => {
        en: "secret-tool failed without saying why. Nothing was stored.",
        sk: "secret-tool zlyhal bez udania dôvodu. Nič sa neuložilo.",
        de: "secret-tool ist ohne Begründung gescheitert. Es wurde nichts gespeichert.",
    },
    // ── Keys ────────────────────────────────────────────────────────────────
    "tui.key.notArmoured" => {
        en: "That is not an ASCII-armoured OpenPGP key.",
        sk: "Toto nie je kľúč OpenPGP v textovom (armored) tvare.",
        de: "Das ist kein ASCII-armierter OpenPGP-Schlüssel.",
    },
    "tui.key.notSecret" => {
        en: "That is a public key. losos needs the secret key, which signs your requests and reads the answers.",
        sk: "Toto je verejný kľúč. losos potrebuje tajný kľúč, ktorý podpisuje požiadavky a číta odpovede.",
        de: "Das ist ein öffentlicher Schlüssel. losos braucht den geheimen Schlüssel, der deine Anfragen signiert und die Antworten liest.",
    },
    "tui.key.cannotSign" => {
        en: "This key cannot sign. The box accepts only requests signed by a key that can.",
        sk: "Tento kľúč nevie podpisovať. Zariadenie prijíma iba požiadavky podpísané kľúčom, ktorý to vie.",
        de: "Dieser Schlüssel kann nicht signieren. Die Box nimmt nur Anfragen an, die mit einem signierfähigen Schlüssel signiert sind.",
    },
    "tui.key.noEncryption" => {
        en: "This key has no encryption subkey, so the box cannot encrypt its answers to it. Keys made by gpg --full-generate-key have one.",
        sk: "Tento kľúč nemá šifrovací podkľúč, takže zariadenie mu nevie zašifrovať odpovede. Kľúče z gpg --full-generate-key ho majú.",
        de: "Dieser Schlüssel hat keinen Verschlüsselungs-Unterschlüssel, die Box kann ihm also nicht antworten. Schlüssel aus gpg --full-generate-key haben einen.",
    },
    "tui.key.wrongPassphrase" => {
        en: "That passphrase does not unlock the key.",
        sk: "Toto heslo kľúč neodomkne.",
        de: "Diese Passphrase entsperrt den Schlüssel nicht.",
    },
    "tui.key.passphrase" => {
        en: "Passphrase for key {fingerprint}: ",
        sk: "Heslo ku kľúču {fingerprint}: ",
        de: "Passphrase für Schlüssel {fingerprint}: ",
    },
    "tui.key.needsPassphrase" => {
        en: "The key is under a passphrase and there is no terminal to ask on. Set LOSOS_PASSPHRASE for this run.",
        sk: "Kľúč je chránený heslom a nie je terminál, kde sa naň opýtať. Pre tento beh nastavte LOSOS_PASSPHRASE.",
        de: "Der Schlüssel hat eine Passphrase, und es gibt kein Terminal zum Fragen. Setze LOSOS_PASSPHRASE für diesen Aufruf.",
    },
    "tui.key.imported" => {
        en: "Stored key {fingerprint} in your keychain.",
        sk: "Kľúč {fingerprint} je uložený v kľúčenke.",
        de: "Schlüssel {fingerprint} ist im Schlüsselbund gespeichert.",
    },
    "tui.key.protected" => {
        en: "It is under a passphrase. losos asks for it once per run and never stores it.",
        sk: "Je chránený heslom. losos sa naň opýta raz pri každom spustení a nikdy ho neukladá.",
        de: "Er hat eine Passphrase. losos fragt einmal pro Aufruf danach und speichert sie nie.",
    },
    "tui.key.noGpg" => {
        en: "gpg is not installed, so there is nothing to export from. Import the key from a file instead.",
        sk: "gpg nie je nainštalované, takže nie je odkiaľ exportovať. Importujte kľúč zo súboru.",
        de: "gpg ist nicht installiert, es gibt also nichts zu exportieren. Importiere den Schlüssel aus einer Datei.",
    },
    "tui.key.gpgFailed" => {
        en: "gpg exported no secret key for {id}: {detail}",
        sk: "gpg neexportovalo žiadny tajný kľúč pre {id}: {detail}",
        de: "gpg hat für {id} keinen geheimen Schlüssel exportiert: {detail}",
    },
    "tui.key.unreadable" => {
        en: "Cannot read {file}.",
        sk: "Súbor {file} sa nedá prečítať.",
        de: "{file} kann nicht gelesen werden.",
    },
    "tui.key.none" => {
        en: "No keys yet. Import one with losos key import.",
        sk: "Zatiaľ žiadne kľúče. Importujte jeden príkazom losos key import.",
        de: "Noch keine Schlüssel. Importiere einen mit losos key import.",
    },
    "tui.key.removed" => {
        en: "Removed key {fingerprint} from your keychain.",
        sk: "Kľúč {fingerprint} je odstránený z kľúčenky.",
        de: "Schlüssel {fingerprint} wurde aus dem Schlüsselbund entfernt.",
    },
    "tui.key.orphans" => {
        en: "These hosts used it and need a new login: {hosts}",
        sk: "Tieto hostiteľa ho používali a potrebujú nové prihlásenie: {hosts}",
        de: "Diese Hosts haben ihn benutzt und brauchen eine neue Anmeldung: {hosts}",
    },
    // ── Hosts ───────────────────────────────────────────────────────────────
    "tui.host.none" => {
        en: "No box or edge yet. Run losos login <url> first.",
        sk: "Zatiaľ žiadne zariadenie ani edge. Najprv spustite losos login <url>.",
        de: "Noch keine Box und keine Edge. Führe zuerst losos login <url> aus.",
    },
    "tui.host.noEdge" => {
        en: "No edge yet. Run losos login https://register.<domain> first.",
        sk: "Zatiaľ žiadny edge. Najprv spustite losos login https://register.<doména>.",
        de: "Noch keine Edge. Führe zuerst losos login https://register.<Domain> aus.",
    },
    "tui.host.unknown" => {
        en: "You are not logged in to {url}. Run losos login {url}.",
        sk: "K {url} nie ste prihlásení. Spustite losos login {url}.",
        de: "Du bist bei {url} nicht angemeldet. Führe losos login {url} aus.",
    },
    "tui.host.several" => {
        en: "You are logged in to more than one. Pick one with --host: {hosts}",
        sk: "Ste prihlásení k viacerým. Vyberte jeden cez --host: {hosts}",
        de: "Du bist bei mehreren angemeldet. Wähle einen mit --host: {hosts}",
    },
    "tui.host.keyGone" => {
        en: "The key this host uses ({fingerprint}) is no longer in your keychain. Import it again, or log in again with another key.",
        sk: "Kľúč, ktorý tento hostiteľ používa ({fingerprint}), už v kľúčenke nie je. Importujte ho znova, alebo sa prihláste znova s iným kľúčom.",
        de: "Der Schlüssel dieses Hosts ({fingerprint}) ist nicht mehr im Schlüsselbund. Importiere ihn erneut oder melde dich mit einem anderen Schlüssel neu an.",
    },
    "tui.host.pinBroken" => {
        en: "The stored host key does not match its own fingerprint. Log out and log in again.",
        sk: "Uložený kľúč hostiteľa nezodpovedá vlastnému odtlačku. Odhláste sa a prihláste znova.",
        de: "Der gespeicherte Host-Schlüssel passt nicht zu seinem Fingerabdruck. Melde dich ab und neu an.",
    },
    "tui.host.certificateChanged" => {
        en: "{url} shows a different TLS certificate from the one pinned at login. losos sent nothing.",
        sk: "{url} ukazuje iný certifikát TLS, než aký sa pripol pri prihlásení. losos nič neposlal.",
        de: "{url} zeigt ein anderes TLS-Zertifikat als das bei der Anmeldung festgehaltene. losos hat nichts gesendet.",
    },
    "tui.host.certificateHelp" => {
        en: "If the box was reinstalled or its certificate renewed, compare the host key in the web UI and run losos login {url} again.",
        sk: "Ak bolo zariadenie preinštalované alebo certifikát obnovený, porovnajte kľúč hostiteľa vo webovom rozhraní a spustite znova losos login {url}.",
        de: "Wenn die Box neu installiert oder ihr Zertifikat erneuert wurde, vergleiche den Host-Schlüssel in der Weboberfläche und führe losos login {url} erneut aus.",
    },
    // ── Login ───────────────────────────────────────────────────────────────
    "tui.login.badUrl" => {
        en: "{url} is not an address losos can use.",
        sk: "{url} nie je adresa, ktorú losos vie použiť.",
        de: "{url} ist keine Adresse, die losos verwenden kann.",
    },
    "tui.login.unreachable" => {
        en: "Cannot reach {url}: {detail}",
        sk: "{url} nie je dostupné: {detail}",
        de: "{url} ist nicht erreichbar: {detail}",
    },
    "tui.login.noHostKey" => {
        en: "{url} serves no host key, so it is not a LosOS box or edge that takes terminal logins. Last answer: {detail}",
        sk: "{url} neposkytuje kľúč hostiteľa, takže to nie je zariadenie ani edge LosOS s prihlásením z terminálu. Posledná odpoveď: {detail}",
        de: "{url} liefert keinen Host-Schlüssel, ist also keine LosOS-Box oder -Edge mit Terminal-Anmeldung. Letzte Antwort: {detail}",
    },
    "tui.login.inconsistent" => {
        en: "The host key served does not match the fingerprint served beside it. Nothing was stored.",
        sk: "Poskytnutý kľúč hostiteľa nezodpovedá odtlačku poskytnutému vedľa neho. Nič sa neuložilo.",
        de: "Der gelieferte Host-Schlüssel passt nicht zum mitgelieferten Fingerabdruck. Es wurde nichts gespeichert.",
    },
    "tui.login.changed1" => {
        en: "THE HOST KEY HAS CHANGED.",
        sk: "KĽÚČ HOSTITEĽA SA ZMENIL.",
        de: "DER HOST-SCHLÜSSEL HAT SICH GEÄNDERT.",
    },
    "tui.login.changed2" => {
        en: "{url} now shows a different key from the one you pinned at your first login.",
        sk: "{url} teraz ukazuje iný kľúč, než aký ste si pripli pri prvom prihlásení.",
        de: "{url} zeigt jetzt einen anderen Schlüssel als den, den du bei der ersten Anmeldung festgehalten hast.",
    },
    "tui.login.changed3" => {
        en: "Someone may be in the middle, or the box was installed again.",
        sk: "Niekto môže byť uprostred, alebo bolo zariadenie nainštalované znova.",
        de: "Jemand könnte dazwischen sitzen, oder die Box wurde neu installiert.",
    },
    "tui.login.changed4" => {
        en: "If you know it was installed again, compare the new fingerprint with the web UI, then run losos logout {url} and log in again.",
        sk: "Ak viete, že bolo nainštalované znova, porovnajte nový odtlačok s webovým rozhraním, potom spustite losos logout {url} a prihláste sa znova.",
        de: "Wenn du weißt, dass sie neu installiert wurde, vergleiche den neuen Fingerabdruck mit der Weboberfläche, führe dann losos logout {url} aus und melde dich neu an.",
    },
    "tui.login.pinned" => {
        en: "pinned",
        sk: "pripnutý",
        de: "festgehalten",
    },
    "tui.login.served" => {
        en: "served",
        sk: "teraz",
        de: "jetzt",
    },
    "tui.login.refused" => {
        en: "Refused. Nothing was sent and nothing was stored.",
        sk: "Odmietnuté. Nič sa neodoslalo a nič sa neuložilo.",
        de: "Abgelehnt. Es wurde nichts gesendet und nichts gespeichert.",
    },
    "tui.login.noKeys" => {
        en: "There is no key in your keychain to sign with. Import the key that was seeded on the box: losos key import <file.asc> or --gpg <id>.",
        sk: "V kľúčenke nie je kľúč na podpisovanie. Importujte kľúč zadaný do zariadenia pri inštalácii: losos key import <súbor.asc> alebo --gpg <id>.",
        de: "Im Schlüsselbund ist kein Schlüssel zum Signieren. Importiere den Schlüssel, der bei der Installation auf der Box hinterlegt wurde: losos key import <datei.asc> oder --gpg <id>.",
    },
    "tui.login.keyNotStored" => {
        en: "Key {fingerprint} is not in your keychain.",
        sk: "Kľúč {fingerprint} nie je v kľúčenke.",
        de: "Schlüssel {fingerprint} ist nicht im Schlüsselbund.",
    },
    "tui.login.flagMismatch" => {
        en: "The host key is not the one you named with --fingerprint. It is {served}. Nothing was stored.",
        sk: "Kľúč hostiteľa nie je ten, ktorý ste uviedli cez --fingerprint. Je to {served}. Nič sa neuložilo.",
        de: "Der Host-Schlüssel ist nicht der mit --fingerprint genannte. Er ist {served}. Es wurde nichts gespeichert.",
    },
    "tui.login.needsFingerprint" => {
        en: "Without a terminal, confirm the host key with --fingerprint. It shows {served}; compare it with the web UI first.",
        sk: "Bez terminálu potvrďte kľúč hostiteľa cez --fingerprint. Ukazuje {served}; najprv ho porovnajte s webovým rozhraním.",
        de: "Ohne Terminal bestätige den Host-Schlüssel mit --fingerprint. Er zeigt {served}; vergleiche ihn zuerst mit der Weboberfläche.",
    },
    "tui.login.whichKeyFlag" => {
        en: "There is more than one key in your keychain. Name one with --key.",
        sk: "V kľúčenke je viac kľúčov. Jeden uveďte cez --key.",
        de: "Im Schlüsselbund ist mehr als ein Schlüssel. Nenne einen mit --key.",
    },
    "tui.login.badToken" => {
        en: "That is not an access token. A token starts with losos_pat_ and the web UI shows it once, when you make it.",
        sk: "Toto nie je prístupový token. Token začína na losos_pat_ a webové rozhranie ho ukáže raz, keď ho vytvoríte.",
        de: "Das ist kein Zugangstoken. Ein Token beginnt mit losos_pat_, und die Weboberfläche zeigt es einmal, beim Erstellen.",
    },
    "tui.login.cancelled" => {
        en: "Cancelled. Nothing was stored.",
        sk: "Zrušené. Nič sa neuložilo.",
        de: "Abgebrochen. Es wurde nichts gespeichert.",
    },
    "tui.login.done" => {
        en: "Logged in to the {kind} at {url}. The host key is pinned, and losos will refuse a different one later.",
        sk: "Prihlásené k {url}, typ {kind}. Kľúč hostiteľa je pripnutý a iný kľúč losos neskôr odmietne.",
        de: "Bei {url} angemeldet, Typ {kind}. Der Host-Schlüssel ist festgehalten, und losos lehnt später einen anderen ab.",
    },
    "tui.login.certificatePinned" => {
        en: "Its own TLS certificate is pinned too: {pin}",
        sk: "Pripnutý je aj jeho vlastný certifikát TLS: {pin}",
        de: "Sein eigenes TLS-Zertifikat ist ebenfalls festgehalten: {pin}",
    },
    "tui.login.hostKey" => {
        en: "Host key",
        sk: "Kľúč hostiteľa",
        de: "Host-Schlüssel",
    },
    "tui.login.compareBox" => {
        en: "Does this match the fingerprint on the Access tokens pane of the box's web UI?",
        sk: "Zhoduje sa s odtlačkom na paneli Prístupové tokeny vo webovom rozhraní zariadenia?",
        de: "Stimmt das mit dem Fingerabdruck im Bereich Zugangstokens der Weboberfläche der Box überein?",
    },
    "tui.login.compareEdge" => {
        en: "Does this match the edge's host key fingerprint from losos-registrar on the operator's machine?",
        sk: "Zhoduje sa s odtlačkom kľúča hostiteľa edge z losos-registrar na počítači operátora?",
        de: "Stimmt das mit dem Fingerabdruck des Host-Schlüssels der Edge aus losos-registrar auf dem Rechner des Betreibers überein?",
    },
    "tui.login.matches" => {
        en: "It matches",
        sk: "Zhoduje sa",
        de: "Stimmt überein",
    },
    "tui.login.differs" => {
        en: "It does not",
        sk: "Nezhoduje sa",
        de: "Stimmt nicht",
    },
    "tui.login.token" => {
        en: "Access token",
        sk: "Prístupový token",
        de: "Zugangstoken",
    },
    "tui.login.tokenHint" => {
        en: "Paste it and press Enter. It is not shown as you type.",
        sk: "Vložte ho a stlačte Enter. Pri písaní sa nezobrazuje.",
        de: "Einfügen und Enter drücken. Es wird beim Tippen nicht angezeigt.",
    },
    "tui.login.whichKey" => {
        en: "Which key signs for this host?",
        sk: "Ktorý kľúč podpisuje pre tohto hostiteľa?",
        de: "Welcher Schlüssel signiert für diesen Host?",
    },
    "tui.logout.done" => {
        en: "Logged out of {url}. Its token and pinned key are gone from your keychain.",
        sk: "Odhlásené z {url}. Jeho token a pripnutý kľúč sú z kľúčenky preč.",
        de: "Von {url} abgemeldet. Token und festgehaltener Schlüssel sind aus dem Schlüsselbund entfernt.",
    },
    "tui.pick.title" => {
        en: "Which one?",
        sk: "Ktoré?",
        de: "Welche?",
    },
    "tui.open.needsTerminal" => {
        en: "The full view needs a terminal. For scripts use losos call.",
        sk: "Celé zobrazenie potrebuje terminál. Pre skripty použite losos call.",
        de: "Die volle Ansicht braucht ein Terminal. Für Skripte gibt es losos call.",
    },
    // ── The envelope ────────────────────────────────────────────────────────
    "tui.envelope.badSignature" => {
        en: "The answer is not signed by the host key you pinned. losos threw it away unread.",
        sk: "Odpoveď nie je podpísaná pripnutým kľúčom hostiteľa. losos ju zahodil neprečítanú.",
        de: "Die Antwort ist nicht mit dem festgehaltenen Host-Schlüssel signiert. losos hat sie ungelesen verworfen.",
    },
    "tui.envelope.wrongNonce" => {
        en: "The answer belongs to another request. losos threw it away unread.",
        sk: "Odpoveď patrí k inej požiadavke. losos ju zahodil neprečítanú.",
        de: "Die Antwort gehört zu einer anderen Anfrage. losos hat sie ungelesen verworfen.",
    },
    "tui.envelope.notAnswer" => {
        en: "The host sent something that is not an answer.",
        sk: "Hostiteľ poslal niečo, čo nie je odpoveď.",
        de: "Der Host hat etwas geschickt, das keine Antwort ist.",
    },
    "tui.envelope.unreachable" => {
        en: "Cannot reach {url}: {detail}",
        sk: "{url} nie je dostupné: {detail}",
        de: "{url} ist nicht erreichbar: {detail}",
    },
    // ── losos call ──────────────────────────────────────────────────────────
    "tui.call.badJson" => {
        en: "The body is not JSON.",
        sk: "Telo nie je JSON.",
        de: "Der Inhalt ist kein JSON.",
    },
    "tui.call.notSealed" => {
        en: "refused before sealing",
        sk: "odmietnuté pred zapečatením",
        de: "vor dem Versiegeln abgelehnt",
    },
    // ── The full view ───────────────────────────────────────────────────────
    "tui.overview.summary" => {
        en: "How the box is doing, its apps and its disk.",
        sk: "Ako sa zariadeniu darí, jeho aplikácie a disk.",
        de: "Wie es der Box geht, ihre Apps und ihr Speicher.",
    },
    "tui.overview.keywords" => {
        en: "home status",
        sk: "domov stav",
        de: "start status",
    },
    "tui.apply.title" => {
        en: "Apply and rebuild?",
        sk: "Použiť a znova zostaviť?",
        de: "Anwenden und neu bauen?",
    },
    "tui.apply.body" => {
        en: { one: "{count} change goes out and the box rebuilds itself. It stays reachable, and the outcome shows here in a few minutes.", other: "{count} changes go out and the box rebuilds itself. It stays reachable, and the outcome shows here in a few minutes." },
        sk: { one: "Odošle sa {count} zmena a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.", few: "Odošlú sa {count} zmeny a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.", many: "Odošle sa {count} zmien a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút.", other: "Odošle sa {count} zmien a zariadenie sa znova zostaví. Zostane dostupné a výsledok sa tu ukáže o pár minút." },
        de: { one: "{count} Änderung wird übertragen und die Box baut sich neu. Sie bleibt erreichbar, das Ergebnis erscheint hier in ein paar Minuten.", other: "{count} Änderungen werden übertragen und die Box baut sich neu. Sie bleibt erreichbar, das Ergebnis erscheint hier in ein paar Minuten." },
    },
    "tui.edit.hint" => {
        en: "Enter saves to the draft. Esc leaves it as it was.",
        sk: "Enter uloží do návrhu. Esc nechá pôvodné.",
        de: "Enter übernimmt in den Entwurf. Esc lässt es, wie es war.",
    },
    "tui.edit.hintMasked" => {
        en: "Not shown as you type. Enter sends it. Esc cancels.",
        sk: "Pri písaní sa nezobrazuje. Enter odošle. Esc zruší.",
        de: "Wird beim Tippen nicht angezeigt. Enter sendet. Esc bricht ab.",
    },
    "tui.storage.grew" => {
        en: "The disk now holds {size}.",
        sk: "Disk má teraz {size}.",
        de: "Der Speicher fasst jetzt {size}.",
    },
    "tui.storage.notGrown" => {
        en: "The box measured no change in size.",
        sk: "Zariadenie nenameralo zmenu veľkosti.",
        de: "Die Box hat keine Größenänderung gemessen.",
    },
    "tui.apps.removing" => {
        en: "Removing {name}",
        sk: "Odstraňuje sa {name}",
        de: "{name} wird entfernt",
    },
    "tui.apps.sharedInWeb" => {
        en: "It runs as the private user. To run it as the shared user or change its properties, install it from the web UI.",
        sk: "Spustí sa ako súkromný používateľ. Ak ho chcete spustiť ako zdieľaného používateľa alebo zmeniť vlastnosti, nainštalujte ho z webového rozhrania.",
        de: "Sie läuft als privater Benutzer. Um sie als geteilter Benutzer laufen zu lassen oder Eigenschaften zu ändern, installiere sie über die Weboberfläche.",
    },
    "tui.advanced.summary" => {
        en: "Every option this box declares and the value it has now, including the ones the other panes do not show. Change them in the web UI.",
        sk: "Každá možnosť, ktorú toto zariadenie deklaruje, a jej súčasná hodnota, aj tie, ktoré ostatné panely nezobrazujú. Meníte ich vo webovom rozhraní.",
        de: "Jede Option, die diese Box deklariert, mit ihrem aktuellen Wert, auch die, die die anderen Bereiche nicht zeigen. Geändert werden sie in der Weboberfläche.",
    },
    "tui.history.synced" => {
        en: "Synced with LosOS Git",
        sk: "Synchronizované s LosOS Git",
        de: "Mit LosOS Git abgeglichen",
    },
    "tui.backup.noTarget" => {
        en: "No bucket set",
        sk: "Nie je nastavený bucket",
        de: "Kein Bucket eingestellt",
    },
    "tui.backup.inWeb" => {
        en: "You set the bucket, the recovery code and restores in the web UI.",
        sk: "Bucket, kód na obnovenie a obnovu nastavujete vo webovom rozhraní.",
        de: "Bucket, Wiederherstellungscode und Wiederherstellen stellst du in der Weboberfläche ein.",
    },
    "tui.about.title" => {
        en: "This terminal",
        sk: "Tento terminál",
        de: "Dieses Terminal",
    },
    "tui.about.client" => {
        en: "Client",
        sk: "Klient",
        de: "Client",
    },
    "tui.about.hostKey" => {
        en: "Host key",
        sk: "Kľúč hostiteľa",
        de: "Host-Schlüssel",
    },
    "tui.about.userKey" => {
        en: "Your key",
        sk: "Váš kľúč",
        de: "Dein Schlüssel",
    },
    "tui.about.webOnly" => {
        en: "Factory reset and erasing the box are in the web UI only, as are access tokens.",
        sk: "Obnovenie továrenských nastavení a vymazanie zariadenia sú iba vo webovom rozhraní, rovnako ako prístupové tokeny.",
        de: "Zurücksetzen und Löschen der Box gibt es nur in der Weboberfläche, ebenso die Zugangstokens.",
    },
    // ── Keys in the full view ───────────────────────────────────────────────
    "tui.keys.move" => {
        en: "move",
        sk: "pohyb",
        de: "bewegen",
    },
    "tui.keys.open" => {
        en: "open",
        sk: "otvoriť",
        de: "öffnen",
    },
    "tui.keys.change" => {
        en: "change",
        sk: "zmeniť",
        de: "ändern",
    },
    "tui.keys.switch" => {
        en: "panes",
        sk: "panely",
        de: "Bereiche",
    },
    "tui.keys.search" => {
        en: "search",
        sk: "hľadať",
        de: "suchen",
    },
    "tui.keys.clear" => {
        en: "clear",
        sk: "vymazať",
        de: "leeren",
    },
    "tui.keys.refresh" => {
        en: "refresh",
        sk: "obnoviť",
        de: "neu laden",
    },
    "tui.keys.help" => {
        en: "help",
        sk: "pomoc",
        de: "Hilfe",
    },
    "tui.keys.quit" => {
        en: "quit",
        sk: "koniec",
        de: "beenden",
    },
    "tui.help.title" => {
        en: "Keys",
        sk: "Klávesy",
        de: "Tasten",
    },
    "tui.help.move" => {
        en: "Move up and down",
        sk: "Pohyb hore a dole",
        de: "Nach oben und unten",
    },
    "tui.help.enter" => {
        en: "Open a pane, flip a switch, edit a value, run an action",
        sk: "Otvoriť panel, prepnúť vypínač, upraviť hodnotu, spustiť akciu",
        de: "Bereich öffnen, Schalter umlegen, Wert bearbeiten, Aktion starten",
    },
    "tui.help.tab" => {
        en: "Between the sidebar and the pane",
        sk: "Medzi bočným panelom a obsahom",
        de: "Zwischen Seitenleiste und Bereich",
    },
    "tui.help.back" => {
        en: "Back to the sidebar",
        sk: "Späť na bočný panel",
        de: "Zurück zur Seitenleiste",
    },
    "tui.help.search" => {
        en: "Search the panes",
        sk: "Hľadať v paneloch",
        de: "Bereiche durchsuchen",
    },
    "tui.help.apply" => {
        en: "Apply or discard the changes waiting",
        sk: "Použiť alebo zahodiť čakajúce zmeny",
        de: "Wartende Änderungen anwenden oder verwerfen",
    },
    "tui.help.refresh" => {
        en: "Ask the box again",
        sk: "Opýtať sa zariadenia znova",
        de: "Die Box erneut fragen",
    },
    "tui.help.page" => {
        en: "Move a page",
        sk: "Posun o stranu",
        de: "Seitenweise bewegen",
    },
    "tui.help.quit" => {
        en: "Quit",
        sk: "Koniec",
        de: "Beenden",
    },
    "tui.help.close" => {
        en: "Any key closes this.",
        sk: "Ľubovoľný kláves toto zavrie.",
        de: "Eine beliebige Taste schließt das.",
    },
    // ── --help ──────────────────────────────────────────────────────────────
    "tui.help.what" => {
        en: "Manage LosOS boxes and edges from a terminal, without SSH. Every request is signed with your key and encrypted to the host.",
        sk: "Správa zariadení a edge LosOS z terminálu, bez SSH. Každá požiadavka je podpísaná vaším kľúčom a zašifrovaná pre hostiteľa.",
        de: "LosOS-Boxen und -Edges vom Terminal aus verwalten, ohne SSH. Jede Anfrage ist mit deinem Schlüssel signiert und für den Host verschlüsselt.",
    },
    "tui.help.commands" => {
        en: "Commands",
        sk: "Príkazy",
        de: "Befehle",
    },
    "tui.help.options" => {
        en: "Options",
        sk: "Voľby",
        de: "Optionen",
    },
    "tui.help.arguments" => {
        en: "Arguments",
        sk: "Argumenty",
        de: "Argumente",
    },
    "tui.help.urlBeforeCommand" => {
        en: "{url} goes after the command, as in losos open {url} or losos login {url}; use --host {url} for the others",
        sk: "{url} patrí za príkaz, napríklad losos open {url} alebo losos login {url}; pri ostatných použite --host {url}",
        de: "{url} gehört hinter den Befehl, etwa losos open {url} oder losos login {url}; bei den anderen --host {url}",
    },
    "tui.help.usage" => {
        en: "Usage",
        sk: "Použitie",
        de: "Aufruf",
    },
    "tui.help.cmd.open" => {
        en: "The full view of the one box or edge, or the one named",
        sk: "Celé zobrazenie jediného zariadenia alebo edge, alebo uvedeného",
        de: "Die volle Ansicht der einzigen Box oder Edge, oder der genannten",
    },
    "tui.help.cmd.login" => {
        en: "Pin the host key, store a token and pick the key that signs",
        sk: "Pripnúť kľúč hostiteľa, uložiť token a vybrať podpisový kľúč",
        de: "Host-Schlüssel festhalten, Token speichern, signierenden Schlüssel wählen",
    },
    "tui.help.cmd.logout" => {
        en: "Forget a host's token and pinned key",
        sk: "Zabudnúť token a pripnutý kľúč hostiteľa",
        de: "Token und festgehaltenen Schlüssel eines Hosts vergessen",
    },
    "tui.help.cmd.hosts" => {
        en: "List the hosts you are logged in to",
        sk: "Vypísať hostiteľov, ku ktorým ste prihlásení",
        de: "Hosts auflisten, bei denen du angemeldet bist",
    },
    "tui.help.cmd.keyImport" => {
        en: "Store a secret key from a file",
        sk: "Uložiť tajný kľúč zo súboru",
        de: "Geheimen Schlüssel aus einer Datei speichern",
    },
    "tui.help.cmd.keyImportGpg" => {
        en: "Export the key from gpg instead, by its ID or email",
        sk: "Namiesto toho exportovať kľúč z gpg, podľa ID alebo e-mailu",
        de: "Den Schlüssel stattdessen aus gpg exportieren, per ID oder E-Mail",
    },
    "tui.help.cmd.keyList" => {
        en: "List the stored keys",
        sk: "Vypísať uložené kľúče",
        de: "Gespeicherte Schlüssel auflisten",
    },
    "tui.help.cmd.keyRemove" => {
        en: "Remove a stored key",
        sk: "Odstrániť uložený kľúč",
        de: "Gespeicherten Schlüssel entfernen",
    },
    "tui.help.cmd.call" => {
        en: "One request; prints the status and the JSON answer",
        sk: "Jedna požiadavka; vypíše stav a odpoveď v JSON",
        de: "Eine Anfrage; gibt Status und JSON-Antwort aus",
    },
    "tui.help.cmd.releaser" => {
        en: "Show the escrow key's address and ETH, or make it (--new)",
        sk: "Zobraziť adresu a ETH kľúča pre escrow, alebo ho vytvoriť (--new)",
        de: "Adresse und ETH des Escrow-Schlüssels zeigen oder ihn erzeugen (--new)",
    },
    "tui.help.opt.newReleaser" => {
        en: "Make the key",
        sk: "Vytvoriť kľúč",
        de: "Den Schlüssel erzeugen",
    },
    "tui.help.cmd.secretList" => {
        en: "List an edge's secrets",
        sk: "Vypísať tajomstvá edge",
        de: "Geheimnisse einer Edge auflisten",
    },
    "tui.help.cmd.secretSet" => {
        en: "Seal one secret on an edge, typed without echo or piped in",
        sk: "Zapečatiť jedno tajomstvo na edge, písané bez zobrazenia alebo cez rúru",
        de: "Ein Geheimnis auf einer Edge versiegeln, verdeckt getippt oder per Pipe",
    },
    "tui.help.cmd.reconcile" => {
        en: "Regenerate an edge's Traefik and rathole config now",
        sk: "Hneď znova vytvoriť konfiguráciu Traefik a rathole na edge",
        de: "Traefik- und rathole-Konfiguration einer Edge jetzt neu erzeugen",
    },
    "tui.help.cmd.ciSetup" => {
        en: "Make a key for CI and put it in GitHub Actions with gh",
        sk: "Vytvoriť kľúč pre CI a vložiť ho do GitHub Actions cez gh",
        de: "Einen Schlüssel für CI erzeugen und mit gh in GitHub Actions ablegen",
    },
    "tui.help.opt.host" => {
        en: "Which host, when you have more than one",
        sk: "Ktorý hostiteľ, keď ich máte viac",
        de: "Welcher Host, wenn es mehrere gibt",
    },
    "tui.help.opt.lang" => {
        en: "Language, else from LANG",
        sk: "Jazyk, inak podľa LANG",
        de: "Sprache, sonst aus LANG",
    },
    "tui.help.opt.theme" => {
        en: "Colours, else from COLORFGBG, else dark",
        sk: "Farby, inak podľa COLORFGBG, inak tmavé",
        de: "Farben, sonst aus COLORFGBG, sonst dunkel",
    },
    "tui.help.opt.key" => {
        en: "The key that signs, when you have more than one",
        sk: "Podpisový kľúč, keď ich máte viac",
        de: "Der signierende Schlüssel, wenn es mehrere gibt",
    },
    "tui.help.opt.fingerprint" => {
        en: "Confirm the host key without a terminal",
        sk: "Potvrdiť kľúč hostiteľa bez terminálu",
        de: "Host-Schlüssel ohne Terminal bestätigen",
    },
    "tui.help.opt.repo" => {
        en: "The GitHub repository",
        sk: "Repozitár na GitHube",
        de: "Das GitHub-Repository",
    },
    "tui.help.opt.print" => {
        en: "Print the values when gh cannot set them",
        sk: "Vypísať hodnoty, keď ich gh nevie nastaviť",
        de: "Werte ausgeben, wenn gh sie nicht setzen kann",
    },
    "tui.help.opt.version" => {
        en: "Print the version",
        sk: "Vypísať verziu",
        de: "Version ausgeben",
    },
    "tui.help.keychain" => {
        en: "Keys and tokens live in your Secret Service, such as GNOME Keyring, KWallet or KeePassXC, through secret-tool. losos writes no plain file.",
        sk: "Kľúče a tokeny sú v službe Secret Service, napríklad GNOME Keyring, KWallet alebo KeePassXC, cez secret-tool. losos nezapisuje žiadny obyčajný súbor.",
        de: "Schlüssel und Tokens liegen über secret-tool im Secret Service, etwa GNOME Keyring, KWallet oder KeePassXC. losos schreibt keine einfache Datei.",
    },
    "tui.help.ci" => {
        en: "With LOSOS_CI=1, call and edge read the key, token, URL and host key from LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL and LOSOS_EDGE_HOST_KEY and never touch the keychain.",
        sk: "S LOSOS_CI=1 čítajú call a edge kľúč, token, URL a kľúč hostiteľa z LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL a LOSOS_EDGE_HOST_KEY a kľúčenky sa nedotknú.",
        de: "Mit LOSOS_CI=1 lesen call und edge Schlüssel, Token, URL und Host-Schlüssel aus LOSOS_CI_GPG_KEY, LOSOS_EDGE_TOKEN, LOSOS_EDGE_URL und LOSOS_EDGE_HOST_KEY und rühren den Schlüsselbund nicht an.",
    },
    "tui.help.opt.replaceReleaser" => {
        en: "Replace the key that is sealed",
        sk: "Nahradiť zapečatený kľúč",
        de: "Den versiegelten Schlüssel ersetzen",
    },
    "tui.help.opt.verbose" => {
        en: "Log what losos does on stderr; give it twice or three times for more",
        sk: "Zapisovať do stderr, čo losos robí; dvakrát alebo trikrát pre viac",
        de: "Auf stderr protokollieren, was losos tut; zwei- oder dreimal für mehr",
    },
    "tui.help.opt.help" => {
        en: "Print this help",
        sk: "Vypísať túto pomoc",
        de: "Diese Hilfe ausgeben",
    },
    "tui.help.cmd.key" => {
        en: "Store, list and remove the keys that sign",
        sk: "Uložiť, vypísať a odstrániť podpisové kľúče",
        de: "Signierende Schlüssel speichern, auflisten und entfernen",
    },
    "tui.help.cmd.edge" => {
        en: "An edge's secrets, its config and the escrow key",
        sk: "Tajomstvá edge, jej konfigurácia a kľúč pre escrow",
        de: "Geheimnisse, Konfiguration und Escrow-Schlüssel einer Edge",
    },
    "tui.help.cmd.ci" => {
        en: "Let GitHub Actions call an edge",
        sk: "Umožniť GitHub Actions volať edge",
        de: "GitHub Actions eine Edge aufrufen lassen",
    },
    "tui.help.cmd.secret" => {
        en: "The secrets sealed on an edge",
        sk: "Tajomstvá zapečatené na edge",
        de: "Die auf einer Edge versiegelten Geheimnisse",
    },
    "tui.help.cmd.escrow" => {
        en: "The escrow's releaser key on an edge",
        sk: "Kľúč na uvoľňovanie escrow na edge",
        de: "Der Freigabeschlüssel des Escrow auf einer Edge",
    },
    "tui.help.arg.url" => {
        en: "The box or edge, as https://name or a bare name",
        sk: "Zariadenie alebo edge, ako https://meno alebo samotné meno",
        de: "Die Box oder Edge, als https://name oder nur der Name",
    },
    "tui.help.arg.file" => {
        en: "An armoured secret key, such as key.asc",
        sk: "Tajný kľúč v ASCII armor, napríklad key.asc",
        de: "Ein geheimer Schlüssel in ASCII-Armor, etwa key.asc",
    },
    "tui.help.arg.keyFingerprint" => {
        en: "The key's fingerprint, spaces allowed",
        sk: "Odtlačok kľúča, môže obsahovať medzery",
        de: "Der Fingerabdruck des Schlüssels, Leerzeichen erlaubt",
    },
    "tui.help.arg.method" => {
        en: "GET, POST, PUT or DELETE",
        sk: "GET, POST, PUT alebo DELETE",
        de: "GET, POST, PUT oder DELETE",
    },
    "tui.help.arg.path" => {
        en: "The API path, such as /api/state",
        sk: "Cesta v API, napríklad /api/state",
        de: "Der API-Pfad, etwa /api/state",
    },
    "tui.help.arg.json" => {
        en: "The request body as JSON",
        sk: "Telo požiadavky v JSON",
        de: "Der Anfragetext als JSON",
    },
    "tui.help.arg.name" => {
        en: "The secret's name",
        sk: "Názov tajomstva",
        de: "Der Name des Geheimnisses",
    },
    "tui.help.arg.edge" => {
        en: "The edge CI calls, where you are logged in",
        sk: "Edge, ktorú volá CI a ku ktorej ste prihlásení",
        de: "Die Edge, die CI aufruft und bei der du angemeldet bist",
    },
    // ── The edge ────────────────────────────────────────────────────────────
    "tui.edge.releaserNone" => {
        en: "This edge has no escrow key yet. Make one with losos edge escrow releaser --new.",
        sk: "Tento edge ešte nemá kľúč pre escrow. Vytvorte ho príkazom losos edge escrow releaser --new.",
        de: "Diese Edge hat noch keinen Escrow-Schlüssel. Legen Sie ihn mit losos edge escrow releaser --new an.",
    },
    "tui.edge.releaserAddress" => {
        en: "The escrow takes release and refund calls from {address}.",
        sk: "Escrow prijíma uvoľnenia a vrátenia z adresy {address}.",
        de: "Der Escrow nimmt Freigaben und Erstattungen von {address} an.",
    },
    "tui.edge.releaserAgrees" => {
        en: "the escrow takes orders from it",
        sk: "escrow od neho prijíma príkazy",
        de: "der Escrow nimmt Aufträge von ihm an",
    },
    "tui.edge.releaserDisagrees" => {
        en: "the escrow {escrow} names another key; its treasury calls setReleaser with this address",
        sk: "escrow {escrow} uvádza iný kľúč; jeho pokladnica zavolá setReleaser s touto adresou",
        de: "der Escrow {escrow} nennt einen anderen Schlüssel; seine Treasury ruft setReleaser mit dieser Adresse auf",
    },
    "tui.edge.releaserSolanaAddress" => {
        en: "On Solana it signs as {address}.",
        sk: "Na Solane podpisuje ako {address}.",
        de: "Auf Solana unterschreibt er als {address}.",
    },
    "tui.edge.releaserNoConfig" => {
        en: "the escrow program {escrow} has no config for this treasury yet; the treasury runs initialize with this key's Solana address",
        sk: "program escrow {escrow} ešte nemá konfiguráciu pre túto pokladnicu; pokladnica spustí initialize so Solana adresou tohto kľúča",
        de: "das Escrow-Programm {escrow} hat noch keine Konfiguration für diese Treasury; die Treasury ruft initialize mit der Solana-Adresse dieses Schlüssels auf",
    },
    "tui.edge.releaserSolanaDisagrees" => {
        en: "the escrow program {escrow} names another key; its treasury calls set_releaser with this key's Solana address",
        sk: "program escrow {escrow} uvádza iný kľúč; jeho pokladnica zavolá set_releaser so Solana adresou tohto kľúča",
        de: "das Escrow-Programm {escrow} nennt einen anderen Schlüssel; seine Treasury ruft set_releaser mit der Solana-Adresse dieses Schlüssels auf",
    },
    "tui.edge.releaserSolanaNext" => {
        en: "On Solana the same key signs as {address}. Send it a little SOL for fees and new token accounts, and have the treasury run initialize (or set_releaser) on the escrow program with that address.",
        sk: "Na Solane ten istý kľúč podpisuje ako {address}. Pošlite naň trochu SOL na poplatky a nové tokenové účty a nech pokladnica spustí initialize (alebo set_releaser) v programe escrow s touto adresou.",
        de: "Auf Solana unterschreibt derselbe Schlüssel als {address}. Senden Sie ihm etwas SOL für Gebühren und neue Token-Konten, und lassen Sie die Treasury im Escrow-Programm initialize (oder set_releaser) mit dieser Adresse aufrufen.",
    },
    "tui.edge.releaserMade" => {
        en: "The edge made a new escrow key and sealed it. Its address is {address}. The key itself never leaves the edge.",
        sk: "Edge vytvoril nový kľúč pre escrow a zapečatil ho. Jeho adresa je {address}. Samotný kľúč edge nikdy neopustí.",
        de: "Die Edge hat einen neuen Escrow-Schlüssel erzeugt und versiegelt. Seine Adresse ist {address}. Der Schlüssel selbst verlässt die Edge nie.",
    },
    "tui.edge.releaserNext" => {
        en: "Send {address} a little ETH on every EVM network for gas. Deploy LososEscrow with it as the releaser, or have the treasury call setReleaser({address}) on each escrow already deployed.",
        sk: "Pošlite na {address} trochu ETH na každej sieti EVM na poplatky. Nasaďte LososEscrow s touto adresou ako releaser, alebo nech pokladnica zavolá setReleaser({address}) na každom už nasadenom escrow.",
        de: "Senden Sie {address} auf jedem EVM-Netz etwas ETH für Gas. Setzen Sie LososEscrow mit dieser Adresse als Releaser auf, oder lassen Sie die Treasury auf jedem bestehenden Escrow setReleaser({address}) aufrufen.",
    },
    "tui.edge.notEdge" => {
        en: "{url} is a box, not an edge.",
        sk: "{url} je zariadenie, nie edge.",
        de: "{url} ist eine Box, keine Edge.",
    },
    "tui.edge.overviewSummary" => {
        en: "The edge's health, its tenants and their last heartbeat, and what is switched on.",
        sk: "Stav edge, jeho nájomníci s posledným signálom a čo je zapnuté.",
        de: "Zustand der Edge, ihre Mandanten mit letztem Lebenszeichen und was eingeschaltet ist.",
    },
    "tui.edge.overviewKeywords" => {
        en: "tenants heartbeat",
        sk: "nájomníci signál",
        de: "mandanten lebenszeichen",
    },
    "tui.edge.thisEdge" => {
        en: "This edge",
        sk: "Tento edge",
        de: "Diese Edge",
    },
    "tui.edge.official" => {
        en: "Holds a LosOS certificate",
        sk: "Má certifikát LosOS",
        de: "Hat ein LosOS-Zertifikat",
    },
    "tui.edge.certificate" => {
        en: "Certificate valid until",
        sk: "Certifikát platí do",
        de: "Zertifikat gültig bis",
    },
    "tui.edge.enrolled" => {
        en: "Enrolled boxes",
        sk: "Zapísané zariadenia",
        de: "Eingeschriebene Boxen",
    },
    "tui.edge.routes" => {
        en: "Relay routes",
        sk: "Trasy relé",
        de: "Relay-Routen",
    },
    "tui.edge.domains" => {
        en: "Domains",
        sk: "Domény",
        de: "Domains",
    },
    "tui.edge.reconcile" => {
        en: "Reconcile now",
        sk: "Zosúladiť teraz",
        de: "Jetzt abgleichen",
    },
    "tui.edge.reconcileDetail" => {
        en: "Writes the Traefik and rathole config again from the registry, without waiting for the timer.",
        sk: "Znova zapíše konfiguráciu Traefik a rathole z registra, bez čakania na časovač.",
        de: "Schreibt die Traefik- und rathole-Konfiguration aus dem Register neu, ohne auf den Timer zu warten.",
    },
    "tui.edge.reconcileButton" => {
        en: "Reconcile",
        sk: "Zosúladiť",
        de: "Abgleichen",
    },
    "tui.edge.reconciled" => {
        en: "The edge wrote its Traefik and rathole config again.",
        sk: "Edge znova zapísal konfiguráciu Traefik a rathole.",
        de: "Die Edge hat ihre Traefik- und rathole-Konfiguration neu geschrieben.",
    },
    "tui.edge.optIns" => {
        en: "Opt-ins",
        sk: "Zapnuté funkcie",
        de: "Zuschaltbares",
    },
    "tui.edge.optInsCaption" => {
        en: "The edge's NixOS configuration sets these. This view cannot change them.",
        sk: "Nastavuje ich konfigurácia NixOS na edge. Toto zobrazenie ich nezmení.",
        de: "Die NixOS-Konfiguration der Edge legt sie fest. Diese Ansicht ändert sie nicht.",
    },
    "tui.edge.tenants" => {
        en: { one: "{count} tenant", other: "{count} tenants" },
        sk: { one: "{count} nájomník", few: "{count} nájomníci", many: "{count} nájomníkov", other: "{count} nájomníkov" },
        de: { one: "{count} Mandant", other: "{count} Mandanten" },
    },
    "tui.edge.tenant" => {
        en: "Tenant",
        sk: "Nájomník",
        de: "Mandant",
    },
    "tui.edge.hostname" => {
        en: "Host name",
        sk: "Názov hostiteľa",
        de: "Hostname",
    },
    "tui.edge.heartbeat" => {
        en: "Last heartbeat",
        sk: "Posledný signál",
        de: "Letztes Lebenszeichen",
    },
    "tui.edge.ago" => {
        en: "{ago} ago",
        sk: "pred {ago}",
        de: "vor {ago}",
    },
    "tui.edge.noTenants" => {
        en: "No boxes have registered yet.",
        sk: "Zatiaľ sa nezaregistrovalo žiadne zariadenie.",
        de: "Noch hat sich keine Box registriert.",
    },
    "tui.edge.tenantsCaption" => {
        en: "A box sends a heartbeat every minute. This view marks one that has been quiet for two minutes.",
        sk: "Zariadenie posiela signál každú minútu. Toto zobrazenie označí také, ktoré dve minúty mlčí.",
        de: "Eine Box sendet jede Minute ein Lebenszeichen. Diese Ansicht markiert eine, die zwei Minuten still ist.",
    },
    "tui.edge.secrets" => {
        en: "Secrets",
        sk: "Tajomstvá",
        de: "Geheimnisse",
    },
    "tui.edge.secretsSummary" => {
        en: "The edge's sealed credentials. The edge seals each value and never shows it again.",
        sk: "Zapečatené prístupové údaje edge. Edge každú hodnotu zapečatí a už ju nikdy neukáže.",
        de: "Die versiegelten Zugangsdaten der Edge. Die Edge versiegelt jeden Wert und zeigt ihn nie wieder.",
    },
    "tui.edge.secretsKeywords" => {
        en: "credentials stripe",
        sk: "prístupové údaje stripe",
        de: "zugangsdaten stripe",
    },
    "tui.edge.noSecrets" => {
        en: "This edge declares no secrets.",
        sk: "Tento edge nedeklaruje žiadne tajomstvá.",
        de: "Diese Edge deklariert keine Geheimnisse.",
    },
    "tui.edge.sealed" => {
        en: "Sealed",
        sk: "Zapečatené",
        de: "Versiegelt",
    },
    "tui.edge.sealedAt" => {
        en: "Sealed {when}",
        sk: "Zapečatené {when}",
        de: "Versiegelt {when}",
    },
    "tui.edge.notSealed" => {
        en: "Not set",
        sk: "Nenastavené",
        de: "Nicht gesetzt",
    },
    "tui.edge.newValue" => {
        en: "New value for {name}",
        sk: "Nová hodnota pre {name}",
        de: "Neuer Wert für {name}",
    },
    "tui.edge.valuePrompt" => {
        en: "Value for {name}: ",
        sk: "Hodnota pre {name}: ",
        de: "Wert für {name}: ",
    },
    "tui.edge.emptyValue" => {
        en: "The value is empty. Nothing was sent.",
        sk: "Hodnota je prázdna. Nič sa neodoslalo.",
        de: "Der Wert ist leer. Es wurde nichts gesendet.",
    },
    "tui.edge.secretSet" => {
        en: "Sealed {name} on the edge.",
        sk: "{name} je zapečatené na edge.",
        de: "{name} ist auf der Edge versiegelt.",
    },
    "tui.edge.secretsCaption" => {
        en: "The edge checks the value's shape before it seals it. Enter on a row sets a new value.",
        sk: "Edge pred zapečatením skontroluje tvar hodnoty. Enter na riadku nastaví novú hodnotu.",
        de: "Die Edge prüft die Form des Werts, bevor sie ihn versiegelt. Enter auf einer Zeile setzt einen neuen Wert.",
    },
    // ── ci setup ────────────────────────────────────────────────────────────
    "tui.ci.notLoggedIn" => {
        en: "Log in to the edge first, so its host key is pinned: losos login {url}",
        sk: "Najprv sa prihláste k edge, aby bol jeho kľúč pripnutý: losos login {url}",
        de: "Melde dich zuerst bei der Edge an, damit ihr Host-Schlüssel festgehalten ist: losos login {url}",
    },
    "tui.ci.generated" => {
        en: "Made a new key for CI in {repo}, not your own key. Its fingerprint:",
        sk: "Vytvorený nový kľúč pre CI v {repo}, nie váš vlastný. Jeho odtlačok:",
        de: "Neuer Schlüssel für CI in {repo} erzeugt, nicht dein eigener. Sein Fingerabdruck:",
    },
    "tui.ci.seed" => {
        en: "Seed the public key above in the edge's NixOS configuration, under",
        sk: "Verejný kľúč vyššie zadajte do konfigurácie NixOS na edge, do",
        de: "Hinterlege den öffentlichen Schlüssel oben in der NixOS-Konfiguration der Edge, unter",
    },
    "tui.ci.token" => {
        en: "and rebuild it. Then make an edge token bound to this key, on your own machine:",
        sk: "a edge znova zostavte. Potom na svojom počítači vytvorte token edge viazaný na tento kľúč:",
        de: "und baue sie neu. Erzeuge dann auf deinem eigenen Rechner ein an diesen Schlüssel gebundenes Edge-Token:",
    },
    "tui.ci.tokenPrompt" => {
        en: "Edge token for CI: ",
        sk: "Token edge pre CI: ",
        de: "Edge-Token für CI: ",
    },
    "tui.ci.setSecret" => {
        en: "Set secret {name} in {repo}.",
        sk: "Nastavené tajomstvo {name} v {repo}.",
        de: "Secret {name} in {repo} gesetzt.",
    },
    "tui.ci.setVariable" => {
        en: "Set variable {name} in {repo}.",
        sk: "Nastavená premenná {name} v {repo}.",
        de: "Variable {name} in {repo} gesetzt.",
    },
    "tui.ci.ghFailed" => {
        en: "gh could not set {name} and exited with {code}. Check gh auth status and your rights on the repository.",
        sk: "gh nevedel nastaviť {name} a skončil s kódom {code}. Skontrolujte gh auth status a svoje práva v repozitári.",
        de: "gh konnte {name} nicht setzen und endete mit Code {code}. Prüfe gh auth status und deine Rechte am Repository.",
    },
    "tui.ci.noGh" => {
        en: "gh is not installed or not signed in, so add these four to {repo} by hand, under Settings, Secrets and variables, Actions:",
        sk: "gh nie je nainštalovaný alebo prihlásený, takže tieto štyri pridajte do {repo} ručne, v Settings, Secrets and variables, Actions:",
        de: "gh ist nicht installiert oder nicht angemeldet, also trage diese vier in {repo} von Hand ein, unter Settings, Secrets and variables, Actions:",
    },
    "tui.ci.kindSecret" => {
        en: "secret",
        sk: "tajomstvo",
        de: "Secret",
    },
    "tui.ci.kindVariable" => {
        en: "variable",
        sk: "premenná",
        de: "Variable",
    },
    "tui.ci.printHint" => {
        en: "Run again with --print to see the values. The key and the token are secret.",
        sk: "Hodnoty uvidíte po spustení s --print. Kľúč a token sú tajné.",
        de: "Mit --print siehst du die Werte. Schlüssel und Token sind geheim.",
    },
    "tui.ci.rotate" => {
        en: "The CI key {fingerprint} is also in your keychain, so you can rotate it later.",
        sk: "Kľúč CI {fingerprint} je aj v kľúčenke, aby ste ho neskôr mohli vymeniť.",
        de: "Der CI-Schlüssel {fingerprint} liegt auch im Schlüsselbund, damit du ihn später tauschen kannst.",
    },
    "tui.ci.missingVar" => {
        en: "LOSOS_CI=1 is set but {name} is empty. losos ci setup puts it in the repository.",
        sk: "LOSOS_CI=1 je nastavené, ale {name} je prázdne. losos ci setup ho vloží do repozitára.",
        de: "LOSOS_CI=1 ist gesetzt, aber {name} ist leer. losos ci setup legt es im Repository ab.",
    },
    "tui.ci.keyProtected" => {
        en: "LOSOS_CI_GPG_KEY is under a passphrase. CI needs a key without one, as losos ci setup makes.",
        sk: "LOSOS_CI_GPG_KEY je chránený heslom. CI potrebuje kľúč bez hesla, aký vytvorí losos ci setup.",
        de: "LOSOS_CI_GPG_KEY hat eine Passphrase. CI braucht einen Schlüssel ohne, wie ihn losos ci setup erzeugt.",
    },

    // ── This computer: LosOS cloud and LosOS Git ────────────────────────────
    "tui.help.cmd.cloud" => {
        en: "Set up this computer for LosOS cloud",
        sk: "Nastaviť tento počítač pre LosOS cloud",
        de: "Diesen Computer für LosOS cloud einrichten",
    },
    "tui.help.cmd.cloudSetup" => {
        en: "Put LosOS cloud in the file manager, signed in with an app password of this computer's own",
        sk: "Pridať LosOS cloud do správcu súborov, prihlásený vlastným heslom aplikácie tohto počítača",
        de: "LosOS cloud in die Dateiverwaltung legen, angemeldet mit einem eigenen App-Passwort dieses Computers",
    },
    "tui.help.cmd.git" => {
        en: "Set up this computer for LosOS Git",
        sk: "Nastaviť tento počítač pre LosOS Git",
        de: "Diesen Computer für LosOS Git einrichten",
    },
    "tui.help.cmd.gitSetup" => {
        en: "Let git clone and push to LosOS Git with a token of this computer's own",
        sk: "Umožniť git klonovať a posielať do LosOS Git vlastným tokenom tohto počítača",
        de: "git mit einem eigenen Token dieses Computers aus LosOS Git klonen und dorthin pushen lassen",
    },
    "tui.help.opt.device" => {
        en: "What this computer is called in the box's lists (default: its hostname)",
        sk: "Ako sa tento počítač volá v zoznamoch zariadenia (predvolene názov hostiteľa)",
        de: "Wie dieser Computer in den Listen der Box heißt (Standard: sein Hostname)",
    },
    "tui.computer.pane" => {
        en: "This computer",
        sk: "Tento počítač",
        de: "Dieser Computer",
    },
    "tui.computer.summary" => {
        en: "LosOS cloud and LosOS Git on the computer you are using now",
        sk: "LosOS cloud a LosOS Git na počítači, ktorý práve používate",
        de: "LosOS cloud und LosOS Git auf dem Computer, den du gerade benutzt",
    },
    "tui.computer.keywords" => {
        en: "computer laptop device files webdav git clone setup",
        sk: "počítač notebook zariadenie súbory webdav git klonovať nastavenie",
        de: "Computer Laptop Gerät Dateien WebDAV git klonen einrichten",
    },
    "tui.computer.cloud" => {
        en: "LosOS cloud",
        sk: "LosOS cloud",
        de: "LosOS cloud",
    },
    "tui.computer.git" => {
        en: "LosOS Git",
        sk: "LosOS Git",
        de: "LosOS Git",
    },
    "tui.computer.cloudDetail" => {
        en: "Puts your files in the file manager's sidebar, signed in with a password of this computer's own",
        sk: "Pridá vaše súbory do bočného panela správcu súborov, prihlásené vlastným heslom tohto počítača",
        de: "Legt deine Dateien in die Seitenleiste der Dateiverwaltung, angemeldet mit einem eigenen Passwort dieses Computers",
    },
    "tui.computer.gitDetail" => {
        en: "Lets git clone and push with a token of this computer's own, kept in the keychain",
        sk: "Umožní git klonovať a posielať vlastným tokenom tohto počítača, uloženým v kľúčenke",
        de: "Lässt git mit einem eigenen Token dieses Computers klonen und pushen, abgelegt im Schlüsselbund",
    },
    "tui.computer.setUp" => {
        en: "Set up",
        sk: "Nastaviť",
        de: "Einrichten",
    },
    "tui.computer.setUpAgain" => {
        en: "Set up again",
        sk: "Nastaviť znova",
        de: "Neu einrichten",
    },
    "tui.computer.setUpAt" => {
        en: "Set up: {location}",
        sk: "Nastavené: {location}",
        de: "Eingerichtet: {location}",
    },
    "tui.computer.notSetUp" => {
        en: "Not set up.",
        sk: "Nenastavené.",
        de: "Nicht eingerichtet.",
    },
    "tui.computer.passwordFor" => {
        en: "Your LosOS password",
        sk: "Vaše heslo k LosOS",
        de: "Dein LosOS-Passwort",
    },
    "tui.computer.caption" => {
        en: "This computer is called {device} in the box's lists. Setting it up again replaces its password or token.",
        sk: "Tento počítač sa v zoznamoch zariadenia volá {device}. Nové nastavenie nahradí jeho heslo alebo token.",
        de: "Dieser Computer heißt in den Listen der Box {device}. Erneutes Einrichten ersetzt sein Passwort oder Token.",
    },
    "tui.computer.noKeychain" => {
        en: "Run losos on the computer you want to set up; this screen was opened without its keychain.",
        sk: "Spustite losos na počítači, ktorý chcete nastaviť; táto obrazovka bola otvorená bez jeho kľúčenky.",
        de: "Starte losos auf dem Computer, den du einrichten willst; dieser Bildschirm wurde ohne seinen Schlüsselbund geöffnet.",
    },
    "tui.computer.passwordPrompt" => {
        en: "Your LosOS password (the one for LosOS cloud): ",
        sk: "Vaše heslo k LosOS (to isté ako do LosOS cloud): ",
        de: "Dein LosOS-Passwort (das von LosOS cloud): ",
    },
    "tui.computer.emptyPassword" => {
        en: "No password was given. Nothing was set up.",
        sk: "Nebolo zadané heslo. Nič sa nenastavilo.",
        de: "Es wurde kein Passwort angegeben. Nichts wurde eingerichtet.",
    },
    "tui.computer.wrongPassword" => {
        en: "That is not the password of LosOS cloud. Nothing was set up.",
        sk: "To nie je heslo k LosOS cloud. Nič sa nenastavilo.",
        de: "Das ist nicht das Passwort von LosOS cloud. Nichts wurde eingerichtet.",
    },
    "tui.computer.gitOff" => {
        en: "LosOS Git is off on this box, so there is nothing to set up.",
        sk: "LosOS Git je na tomto zariadení vypnutý, takže nie je čo nastaviť.",
        de: "LosOS Git ist auf dieser Box aus, es gibt also nichts einzurichten.",
    },
    "tui.computer.tooOld" => {
        en: "This box does not make passwords for other computers yet. It will after an update.",
        sk: "Toto zariadenie zatiaľ nevytvára heslá pre iné počítače. Bude po aktualizácii.",
        de: "Diese Box erzeugt noch keine Passwörter für andere Computer. Nach einem Update schon.",
    },
    "tui.computer.oddAnswer" => {
        en: "The box answered without a password or token. Nothing was set up.",
        sk: "Zariadenie odpovedalo bez hesla aj tokenu. Nič sa nenastavilo.",
        de: "Die Box hat ohne Passwort oder Token geantwortet. Nichts wurde eingerichtet.",
    },
    "tui.computer.noCertificate" => {
        en: "The box's certificate could not be read for git. Nothing was set up.",
        sk: "Certifikát zariadenia sa pre git nepodarilo načítať. Nič sa nenastavilo.",
        de: "Das Zertifikat der Box konnte für git nicht gelesen werden. Nichts wurde eingerichtet.",
    },
    "tui.computer.noGit" => {
        en: "git is not installed. The token is in your keychain; run losos git setup again once git is there.",
        sk: "git nie je nainštalovaný. Token je v kľúčenke; po inštalácii gitu spustite losos git setup znova.",
        de: "git ist nicht installiert. Das Token liegt im Schlüsselbund; führe losos git setup erneut aus, sobald git da ist.",
    },
    "tui.computer.gitFailed" => {
        en: "{command} failed. The token is in your keychain; run losos git setup again once git works.",
        sk: "{command} zlyhal. Token je v kľúčenke; keď git funguje, spustite losos git setup znova.",
        de: "{command} ist fehlgeschlagen. Das Token liegt im Schlüsselbund; führe losos git setup erneut aus, sobald git funktioniert.",
    },
    "tui.computer.cloudLabel" => {
        en: "LosOS cloud on {host}",
        sk: "LosOS cloud na {host}",
        de: "LosOS cloud auf {host}",
    },
    "tui.computer.gitLabel" => {
        en: "LosOS Git on {host}",
        sk: "LosOS Git na {host}",
        de: "LosOS Git auf {host}",
    },
    "tui.computer.notBox" => {
        en: "{url} is an edge. LosOS cloud and LosOS Git live on a box.",
        sk: "{url} je edge. LosOS cloud a LosOS Git bežia na zariadení.",
        de: "{url} ist eine Edge. LosOS cloud und LosOS Git laufen auf einer Box.",
    },
    "tui.computer.cloudDone" => {
        en: "LosOS cloud is set up on this computer as {device}.",
        sk: "LosOS cloud je na tomto počítači nastavený ako {device}.",
        de: "LosOS cloud ist auf diesem Computer als {device} eingerichtet.",
    },
    "tui.computer.cloudWhere" => {
        en: "Open LosOS cloud in the file manager's sidebar, or at {location}",
        sk: "LosOS cloud otvoríte v bočnom paneli správcu súborov alebo na {location}",
        de: "Öffne LosOS cloud in der Seitenleiste der Dateiverwaltung oder unter {location}",
    },
    "tui.computer.cloudCertificate" => {
        en: "The file manager asks you to accept the box's own certificate when it first connects.",
        sk: "Správca súborov sa pri prvom pripojení spýta, či prijať vlastný certifikát zariadenia.",
        de: "Die Dateiverwaltung fragt beim ersten Verbinden, ob sie das eigene Zertifikat der Box annehmen soll.",
    },
    "tui.computer.cloudRevoke" => {
        en: "To take this computer's access away, delete \"LosOS on {device}\" under Security in LosOS cloud's settings.",
        sk: "Prístup tohto počítača zrušíte vymazaním „LosOS on {device}“ v časti Zabezpečenie v nastaveniach LosOS cloud.",
        de: "Um diesem Computer den Zugang zu nehmen, lösche „LosOS on {device}“ unter Sicherheit in den Einstellungen von LosOS cloud.",
    },
    "tui.computer.gitDone" => {
        en: "LosOS Git is set up on this computer as {device}.",
        sk: "LosOS Git je na tomto počítači nastavený ako {device}.",
        de: "LosOS Git ist auf diesem Computer als {device} eingerichtet.",
    },
    "tui.computer.gitWhere" => {
        en: "Clone with: git clone {location}",
        sk: "Klonujte príkazom: git clone {location}",
        de: "Klonen mit: git clone {location}",
    },
    "tui.computer.gitRevoke" => {
        en: "To take this computer's access away, delete \"LosOS on {device}\" under Applications in LosOS Git's settings.",
        sk: "Prístup tohto počítača zrušíte vymazaním „LosOS on {device}“ v časti Aplikácie v nastaveniach LosOS Git.",
        de: "Um diesem Computer den Zugang zu nehmen, lösche „LosOS on {device}“ unter Anwendungen in den Einstellungen von LosOS Git.",
    },
    "tui.help.cmd.credentials" => {
        en: "The secrets this box keeps, sealed to it",
        sk: "Tajomstvá, ktoré toto zariadenie uchováva, zapečatené preň",
        de: "Die Geheimnisse, die diese Box aufbewahrt, an sie gebunden",
    },
    "tui.help.cmd.credentialsList" => {
        en: "List each one, what reads it and when it changed, never its value",
        sk: "Vypísať každé z nich, čo ho číta a kedy sa zmenilo, nikdy nie jeho hodnotu",
        de: "Jedes auflisten, was es liest und wann es sich geändert hat, nie seinen Wert",
    },
    "tui.help.cmd.credentialsRotate" => {
        en: "Have the box make a new value for one, seal it and restart what reads it",
        sk: "Nechať zariadenie vytvoriť preň novú hodnotu, zapečatiť ju a reštartovať, čo ho číta",
        de: "Die Box einen neuen Wert dafür erzeugen, versiegeln und neu starten lassen, was ihn liest",
    },
    "tui.help.arg.credentialName" => {
        en: "The credential's name, as the list shows it",
        sk: "Názov prihlasovacieho údaja, ako ho ukazuje zoznam",
        de: "Der Name des Zugangsdatums, wie die Liste ihn zeigt",
    },
    "tui.credentials.notBox" => {
        en: "{url} is an edge. The credential store is on a box.",
        sk: "{url} je edge. Úložisko prihlasovacích údajov je na zariadení.",
        de: "{url} ist eine Edge. Der Speicher für Zugangsdaten ist auf einer Box.",
    },
    "tui.credentials.badName" => {
        en: "\"{name}\" is not a credential's name: lowercase letters, digits and dashes.",
        sk: "„{name}“ nie je názov prihlasovacieho údaja: malé písmená, číslice a pomlčky.",
        de: "„{name}“ ist kein Name eines Zugangsdatums: Kleinbuchstaben, Ziffern und Bindestriche.",
    },
    "tui.credentials.sealedTpm" => {
        en: "Sealed to this box's TPM.",
        sk: "Zapečatené pre TPM tohto zariadenia.",
        de: "An das TPM dieser Box gebunden.",
    },
    "tui.credentials.sealedHost" => {
        en: "Sealed to this box's own key (it has no TPM).",
        sk: "Zapečatené vlastným kľúčom tohto zariadenia (nemá TPM).",
        de: "Mit dem eigenen Schlüssel dieser Box versiegelt (sie hat kein TPM).",
    },
    "tui.credentials.none" => {
        en: "This box keeps no credentials.",
        sk: "Toto zariadenie neuchováva žiadne prihlasovacie údaje.",
        de: "Diese Box bewahrt keine Zugangsdaten auf.",
    },
    "tui.credentials.byBox" => {
        en: "made by the box",
        sk: "vytvorené zariadením",
        de: "von der Box erzeugt",
    },
    "tui.credentials.byOwner" => {
        en: "set by you",
        sk: "nastavené vami",
        de: "von dir gesetzt",
    },
    "tui.credentials.notSet" => {
        en: "not set",
        sk: "nenastavené",
        de: "nicht gesetzt",
    },
    "tui.credentials.readBy" => {
        en: "read by {units}",
        sk: "číta {units}",
        de: "gelesen von {units}",
    },
    "tui.credentials.adopted" => {
        en: "moved from a plain file",
        sk: "presunuté z obyčajného súboru",
        de: "aus einer einfachen Datei übernommen",
    },
    "tui.credentials.remade" => {
        en: "made again at boot",
        sk: "pri štarte vytvorené znova",
        de: "beim Start neu erzeugt",
    },
    "tui.credentials.canRotate" => {
        en: "`losos credentials rotate <name>` makes a new value for: {names}",
        sk: "`losos credentials rotate <name>` vytvorí novú hodnotu pre: {names}",
        de: "`losos credentials rotate <name>` erzeugt einen neuen Wert für: {names}",
    },
    "tui.credentials.rotated" => {
        en: "{name} has a new value, sealed to this box.",
        sk: "{name} má novú hodnotu, zapečatenú pre toto zariadenie.",
        de: "{name} hat einen neuen Wert, an diese Box gebunden.",
    },
    "tui.credentials.rotatedRestarted" => {
        en: "{name} has a new value, sealed to this box; restarted {units}.",
        sk: "{name} má novú hodnotu, zapečatenú pre toto zariadenie; reštartované: {units}.",
        de: "{name} hat einen neuen Wert, an diese Box gebunden; neu gestartet: {units}.",
    },
    // ── AI agent tools ──────────────────────────────────────────────────────
    "tui.help.cmd.agents" => {
        en: "Find the AI tools on this computer and give them your LosOS files",
        sk: "Nájsť nástroje AI na tomto počítači a sprístupniť im vaše súbory v LosOS",
        de: "Die KI-Werkzeuge auf diesem Computer finden und ihnen deine LosOS-Dateien geben",
    },
    "tui.help.cmd.agentsList" => {
        en: "List the AI tools found and whether the LosOS companion is in them",
        sk: "Vypísať nájdené nástroje AI a či je v nich pomocník LosOS",
        de: "Die gefundenen KI-Werkzeuge auflisten und ob der LosOS-Begleiter in ihnen ist",
    },
    "tui.help.cmd.agentsInstall" => {
        en: "Put the LosOS companion in the AI tools, asking about each one",
        sk: "Pridať pomocníka LosOS do nástrojov AI, pri každom sa spýta",
        de: "Den LosOS-Begleiter in die KI-Werkzeuge einbauen, mit einer Frage zu jedem",
    },
    "tui.help.cmd.agentsRemove" => {
        en: "Take the LosOS companion out of the AI tools again",
        sk: "Odstrániť pomocníka LosOS z nástrojov AI",
        de: "Den LosOS-Begleiter wieder aus den KI-Werkzeugen entfernen",
    },
    "tui.help.cmd.mcp" => {
        en: "Serve the companion's read-only tools over MCP on stdin and stdout, as an AI tool starts it",
        sk: "Poskytovať nástroje pomocníka len na čítanie cez MCP na stdin a stdout, ako ho spúšťa nástroj AI",
        de: "Die lesenden Werkzeuge des Begleiters per MCP auf stdin und stdout anbieten, so wie ein KI-Werkzeug ihn startet",
    },
    "tui.help.arg.tools" => {
        en: "Which tools, by id (losos agents lists them); none means every one found",
        sk: "Ktoré nástroje, podľa id (vypíše ich losos agents); bez neho všetky nájdené",
        de: "Welche Werkzeuge, nach ID (losos agents listet sie); ohne alle gefundenen",
    },
    "tui.help.opt.yes" => {
        en: "Do it for every tool without asking",
        sk: "Urobiť to pre každý nástroj bez pýtania",
        de: "Es für jedes Werkzeug ohne Nachfrage tun",
    },
    "tui.agents.yesNo" => {
        en: "y/N",
        sk: "a/N",
        de: "j/N",
    },
    "tui.agents.yesWords" => {
        en: "y,yes",
        sk: "a,áno",
        de: "j,ja",
    },
    "tui.agents.kindCli" => {
        en: "terminal",
        sk: "terminál",
        de: "Terminal",
    },
    "tui.agents.kindApp" => {
        en: "app",
        sk: "aplikácia",
        de: "App",
    },
    "tui.agents.found" => {
        en: "AI tools on this computer:",
        sk: "Nástroje AI na tomto počítači:",
        de: "KI-Werkzeuge auf diesem Computer:",
    },
    "tui.agents.noneFound" => {
        en: "No AI tool LosOS knows was found on this computer.",
        sk: "Na tomto počítači sa nenašiel žiadny nástroj AI, ktorý LosOS pozná.",
        de: "Auf diesem Computer wurde kein KI-Werkzeug gefunden, das LosOS kennt.",
    },
    "tui.agents.looksFor" => {
        en: "LosOS looks for {tools}.",
        sk: "LosOS hľadá {tools}.",
        de: "LosOS sucht nach {tools}.",
    },
    "tui.agents.stateIn" => {
        en: "companion in",
        sk: "pomocník pridaný",
        de: "Begleiter drin",
    },
    "tui.agents.stateOut" => {
        en: "not yet",
        sk: "zatiaľ nie",
        de: "noch nicht",
    },
    "tui.agents.stateNoProgram" => {
        en: "needs the {program} program on PATH",
        sk: "potrebuje program {program} v PATH",
        de: "braucht das Programm {program} im PATH",
    },
    "tui.agents.companionFor" => {
        en: "The companion reads {url}.",
        sk: "Pomocník číta {url}.",
        de: "Der Begleiter liest {url}.",
    },
    "tui.agents.installHint" => {
        en: "Run losos agents install to give them your files and repositories on your box.",
        sk: "Spustite losos agents install a sprístupnite im súbory a repozitáre na vašom zariadení.",
        de: "Führe losos agents install aus, um ihnen deine Dateien und Repositories auf deiner Box zu geben.",
    },
    "tui.agents.hint" => {
        en: "AI tools found on this computer: {tools}. To let them read your LosOS files, run: losos agents install",
        sk: "Na tomto počítači sú nástroje AI: {tools}. Aby mohli čítať vaše súbory v LosOS, spustite: losos agents install",
        de: "KI-Werkzeuge auf diesem Computer: {tools}. Damit sie deine LosOS-Dateien lesen können, führe aus: losos agents install",
    },
    "tui.agents.unknownTool" => {
        en: "LosOS does not know a tool called {tool}.",
        sk: "LosOS nepozná nástroj s názvom {tool}.",
        de: "LosOS kennt kein Werkzeug namens {tool}.",
    },
    "tui.agents.knownTools" => {
        en: "The tools it knows: {tools}",
        sk: "Nástroje, ktoré pozná: {tools}",
        de: "Die Werkzeuge, die es kennt: {tools}",
    },
    "tui.agents.notBox" => {
        en: "{url} is an edge. The companion reads a box.",
        sk: "{url} je edge. Pomocník číta zariadenie.",
        de: "{url} ist eine Edge. Der Begleiter liest eine Box.",
    },
    "tui.agents.skipNoProgram" => {
        en: "{tool} is skipped: its {program} program is not on PATH.",
        sk: "{tool} sa preskakuje: jeho program {program} nie je v PATH.",
        de: "{tool} wird übersprungen: sein Programm {program} ist nicht im PATH.",
    },
    "tui.agents.ask" => {
        en: "Put the LosOS companion in {tool}?",
        sk: "Pridať pomocníka LosOS do {tool}?",
        de: "Den LosOS-Begleiter in {tool} einbauen?",
    },
    "tui.agents.askAgain" => {
        en: "The LosOS companion is in {tool} already. Put it in again?",
        sk: "Pomocník LosOS už je v {tool}. Pridať ho znova?",
        de: "Der LosOS-Begleiter ist schon in {tool}. Erneut einbauen?",
    },
    "tui.agents.askRemove" => {
        en: "Take the LosOS companion out of {tool}?",
        sk: "Odstrániť pomocníka LosOS z {tool}?",
        de: "Den LosOS-Begleiter aus {tool} entfernen?",
    },
    "tui.agents.nothingChanged" => {
        en: "Nothing was changed.",
        sk: "Nič sa nezmenilo.",
        de: "Nichts wurde geändert.",
    },
    "tui.agents.certificate" => {
        en: "{url} shows a certificate no public authority vouches for. Its SHA-256 fingerprint is {pin}. Compare it with the one your browser shows for the box.",
        sk: "{url} ukazuje certifikát, za ktorý neručí žiadna verejná autorita. Jeho odtlačok SHA-256 je {pin}. Porovnajte ho s tým, ktorý pre zariadenie ukazuje váš prehliadač.",
        de: "{url} zeigt ein Zertifikat, für das keine öffentliche Stelle bürgt. Sein SHA-256-Fingerabdruck ist {pin}. Vergleiche ihn mit dem, den dein Browser für die Box zeigt.",
    },
    "tui.agents.askCertificate" => {
        en: "Do they match, and should the companion keep this certificate?",
        sk: "Zhodujú sa a má si pomocník tento certifikát ponechať?",
        de: "Stimmen sie überein, und soll der Begleiter dieses Zertifikat behalten?",
    },
    "tui.agents.certificateRefused" => {
        en: "Nothing was changed: the certificate was not taken.",
        sk: "Nič sa nezmenilo: certifikát nebol prijatý.",
        de: "Nichts wurde geändert: Das Zertifikat wurde nicht angenommen.",
    },
    "tui.agents.reusing" => {
        en: "This computer is signed in to {url} for the companion already.",
        sk: "Tento počítač je už pre pomocníka prihlásený do {url}.",
        de: "Dieser Computer ist für den Begleiter schon bei {url} angemeldet.",
    },
    "tui.agents.signIn" => {
        en: "The companion signs in to {url} with LosOS Login. Open this link in a browser, sign in and approve the LosOS companion:",
        sk: "Pomocník sa do {url} prihlasuje cez LosOS Login. Otvorte tento odkaz v prehliadači, prihláste sa a schváľte pomocníka LosOS:",
        de: "Der Begleiter meldet sich bei {url} mit LosOS Login an. Öffne diesen Link im Browser, melde dich an und erlaube den LosOS-Begleiter:",
    },
    "tui.agents.signInCode" => {
        en: "The page shows the code {code}. Waiting for the approval…",
        sk: "Stránka ukazuje kód {code}. Čaká sa na schválenie…",
        de: "Die Seite zeigt den Code {code}. Warte auf die Freigabe…",
    },
    "tui.agents.signedIn" => {
        en: "Signed in. The companion reads {url} as you, and stays signed in while it is used at least once in 30 days.",
        sk: "Prihlásené. Pomocník číta {url} ako vy a zostane prihlásený, kým sa použije aspoň raz za 30 dní.",
        de: "Angemeldet. Der Begleiter liest {url} als du und bleibt angemeldet, solange er mindestens einmal in 30 Tagen benutzt wird.",
    },
    "tui.agents.loginUnreachable" => {
        en: "LosOS Login on {url} did not answer: {detail}",
        sk: "LosOS Login na {url} neodpovedal: {detail}",
        de: "LosOS Login auf {url} hat nicht geantwortet: {detail}",
    },
    "tui.agents.loginRefused" => {
        en: "LosOS Login would not start the companion's sign-in (HTTP {status}).",
        sk: "LosOS Login nezačal prihlásenie pomocníka (HTTP {status}).",
        de: "LosOS Login hat die Anmeldung des Begleiters nicht begonnen (HTTP {status}).",
    },
    "tui.agents.loginRefusedHelp" => {
        en: "A box that has not updated yet has no sign-in for the companion. Let it update tonight, then try again.",
        sk: "Zariadenie, ktoré sa ešte neaktualizovalo, prihlásenie pre pomocníka nemá. Nechajte ho dnes v noci aktualizovať a skúste to znova.",
        de: "Eine Box, die sich noch nicht aktualisiert hat, hat keine Anmeldung für den Begleiter. Lass sie heute Nacht aktualisieren und versuche es dann erneut.",
    },
    "tui.agents.loginNoRefresh" => {
        en: "LosOS Login gave the companion no refresh token, so it would stop working after an hour.",
        sk: "LosOS Login nedal pomocníkovi obnovovací token, takže by po hodine prestal fungovať.",
        de: "LosOS Login hat dem Begleiter kein Aktualisierungstoken gegeben, er würde also nach einer Stunde aufhören.",
    },
    "tui.agents.loginDenied" => {
        en: "The sign-in was denied in the browser. Nothing was changed.",
        sk: "Prihlásenie bolo v prehliadači zamietnuté. Nič sa nezmenilo.",
        de: "Die Anmeldung wurde im Browser abgelehnt. Nichts wurde geändert.",
    },
    "tui.agents.loginExpired" => {
        en: "The code ran out before the sign-in was approved.",
        sk: "Kód vypršal skôr, než bolo prihlásenie schválené.",
        de: "Der Code ist abgelaufen, bevor die Anmeldung freigegeben wurde.",
    },
    "tui.agents.loginAgain" => {
        en: "Run losos agents install again.",
        sk: "Spustite znova losos agents install.",
        de: "Führe losos agents install erneut aus.",
    },
    "tui.agents.loginFailed" => {
        en: "LosOS Login refused the sign-in (HTTP {status}): {detail}",
        sk: "LosOS Login prihlásenie odmietol (HTTP {status}): {detail}",
        de: "LosOS Login hat die Anmeldung abgelehnt (HTTP {status}): {detail}",
    },
    "tui.agents.signedOut" => {
        en: "The companion is no longer signed in to {url}.",
        sk: "Pomocník už nie je prihlásený do {url}.",
        de: "Der Begleiter ist nicht mehr bei {url} angemeldet.",
    },
    "tui.agents.readOnly" => {
        en: "The companion only reads, so it sends no {method}.",
        sk: "Pomocník iba číta, preto neposiela {method}.",
        de: "Der Begleiter liest nur, also sendet er kein {method}.",
    },
    "tui.agents.keychainLabel" => {
        en: "LosOS companion for {url}",
        sk: "Pomocník LosOS pre {url}",
        de: "LosOS-Begleiter für {url}",
    },
    "tui.agents.installed" => {
        en: "The LosOS companion is in {tool}.",
        sk: "Pomocník LosOS je v {tool}.",
        de: "Der LosOS-Begleiter ist in {tool}.",
    },
    "tui.agents.removed" => {
        en: "The LosOS companion is out of {tool}.",
        sk: "Pomocník LosOS je odstránený z {tool}.",
        de: "Der LosOS-Begleiter ist aus {tool} entfernt.",
    },
    "tui.agents.backedUp" => {
        en: "  The file as it was is kept at {path}",
        sk: "  Pôvodný súbor je uložený v {path}",
        de: "  Die Datei, wie sie war, liegt unter {path}",
    },
    "tui.agents.restart" => {
        en: "Restart a tool that was open so it starts the companion.",
        sk: "Nástroj, ktorý bol otvorený, reštartujte, aby spustil pomocníka.",
        de: "Starte ein Werkzeug, das offen war, neu, damit es den Begleiter startet.",
    },
    "tui.agents.removeHint" => {
        en: "losos agents remove takes it out again.",
        sk: "losos agents remove ho znova odstráni.",
        de: "losos agents remove entfernt ihn wieder.",
    },
    "tui.agents.forgot" => {
        en: "The companion is signed out of {url}, and this computer no longer keeps its sign-in.",
        sk: "Pomocník je odhlásený z {url} a tento počítač už jeho prihlásenie nemá.",
        de: "Der Begleiter ist von {url} abgemeldet, und dieser Computer hat seine Anmeldung nicht mehr.",
    },
    "tui.agents.forgotOffline" => {
        en: "{url} did not answer, so its sign-in lasts until it runs out, at most 30 days. This computer no longer keeps it.",
        sk: "{url} neodpovedal, takže prihlásenie platí, kým nevyprší, najviac 30 dní. Tento počítač ho už nemá.",
        de: "{url} hat nicht geantwortet, die Anmeldung gilt also, bis sie abläuft, höchstens 30 Tage. Dieser Computer hat sie nicht mehr.",
    },
    "tui.agents.noCompanion" => {
        en: "This computer keeps no LosOS companion for {url}.",
        sk: "Tento počítač nemá pomocníka LosOS pre {url}.",
        de: "Dieser Computer hat keinen LosOS-Begleiter für {url}.",
    },
    "tui.agents.noCompanionHelp" => {
        en: "Run losos agents install in a terminal to set it up.",
        sk: "Nastavíte ho príkazom losos agents install v termináli.",
        de: "Richte ihn mit losos agents install in einem Terminal ein.",
    },
    "tui.agents.noHome" => {
        en: "HOME is not set, so the AI tools' settings cannot be found.",
        sk: "HOME nie je nastavené, takže nastavenia nástrojov AI sa nedajú nájsť.",
        de: "HOME ist nicht gesetzt, also sind die Einstellungen der KI-Werkzeuge nicht zu finden.",
    },
    "tui.agents.noProgram" => {
        en: "{tool} takes the companion through its own {program} program, which is not on PATH.",
        sk: "{tool} pridáva pomocníka vlastným programom {program}, ktorý nie je v PATH.",
        de: "{tool} baut den Begleiter mit seinem eigenen Programm {program} ein, das nicht im PATH ist.",
    },
    "tui.agents.couldNotRun" => {
        en: "{program} could not be started: {detail}",
        sk: "{program} sa nepodarilo spustiť: {detail}",
        de: "{program} konnte nicht gestartet werden: {detail}",
    },
    "tui.agents.toolFailed" => {
        en: "{command} ended with exit status {code}: {detail}",
        sk: "{command} skončil s návratovým kódom {code}: {detail}",
        de: "{command} endete mit dem Status {code}: {detail}",
    },
    "tui.agents.toolTimedOut" => {
        en: "{command} did not finish in {seconds} seconds and was stopped.",
        sk: "{command} neskončil do {seconds} sekúnd a bol zastavený.",
        de: "{command} war nach {seconds} Sekunden nicht fertig und wurde beendet.",
    },
    "tui.agents.couldNotWrite" => {
        en: "{path} could not be written: {detail}",
        sk: "{path} sa nepodarilo zapísať: {detail}",
        de: "{path} konnte nicht geschrieben werden: {detail}",
    },
    "tui.agents.couldNotRead" => {
        en: "{path} could not be read, so it was left as it is: {detail}",
        sk: "{path} sa nepodarilo prečítať, preto zostal bez zmeny: {detail}",
        de: "{path} konnte nicht gelesen werden und bleibt deshalb unverändert: {detail}",
    },
    "tui.agents.notOurs" => {
        en: "{tool} already has an MCP server called losos that LosOS did not write, in {path}. It was left as it is.",
        sk: "{tool} už má MCP server s názvom losos, ktorý nezapísal LosOS, v {path}. Zostal bez zmeny.",
        de: "{tool} hat in {path} schon einen MCP-Server namens losos, den nicht LosOS geschrieben hat. Er bleibt unverändert.",
    },
}
