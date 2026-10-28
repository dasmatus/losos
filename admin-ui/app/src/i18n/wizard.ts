/* The first-run wizard: src/screens/Wizard.tsx and src/screens/wizard/*.
 *
 * "recovery code" is kept as a code, not a key, in all three languages: the
 * screen calls it a code throughout and the owner copies it as one.
 *
 * Slovak uses "zariadenie" for the box, so where the owner's own phone or
 * laptop appears in the same sentence it is "prístroj" — two "zariadenie"s
 * side by side would leave the reader guessing which one is meant. */

import { defineMessages } from "./define";

export default defineMessages({
  // ── Shared ──────────────────────────────────────────────────────────────
  "wizard.noAnswer": {
    en: "This box did not answer.",
    sk: "Zariadenie neodpovedalo.",
    de: "Die Box hat nicht geantwortet.",
  },
  "wizard.boxName.fallback": {
    en: "this box",
    sk: "toto zariadenie",
    de: "diese Box",
  },

  // ── Wizard.tsx ──────────────────────────────────────────────────────────
  "wizard.title.named": {
    en: "Set up {name}",
    sk: "Nastavenie zariadenia {name}",
    de: "{name} einrichten",
  },
  "wizard.title.unnamed": {
    en: "Set up this box",
    sk: "Nastavenie tohto zariadenia",
    de: "Diese Box einrichten",
  },
  "wizard.nav.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "wizard.nav.continue": { en: "Continue", sk: "Pokračovať", de: "Weiter" },
  "wizard.nav.continueWithoutCode": {
    en: "Continue without a code",
    sk: "Pokračovať bez kódu",
    de: "Ohne Code weiter",
  },
  "wizard.nav.letsGo": { en: "Let's go", sk: "Poďme na to", de: "Los geht's" },
  "wizard.closing.titleNamed": {
    en: "{name} is ready",
    sk: "Zariadenie {name} je pripravené",
    de: "{name} ist bereit",
  },
  "wizard.closing.titleUnnamed": {
    en: "This box is ready",
    sk: "Zariadenie je pripravené",
    de: "Die Box ist bereit",
  },
  "wizard.closing.body": {
    en: "You can sign in from any device on this network. Keep the spare admin key somewhere that is not this box.",
    sk: "Prihlásiť sa môžete z akéhokoľvek prístroja v tejto sieti. Náhradný správcovský kľúč uchovávajte niekde mimo tohto zariadenia.",
    de: "Du kannst dich von jedem Gerät in diesem Netzwerk anmelden. Bewahre den Ersatz-Admin-Schlüssel irgendwo auf, nur nicht auf dieser Box.",
  },

  // ── steps.ts / StepRail.tsx ─────────────────────────────────────────────
  "wizard.steps.trust": {
    en: "Trust this box",
    sk: "Dôverujte tomuto zariadeniu",
    de: "Dieser Box vertrauen",
  },
  "wizard.steps.signin": {
    en: "Choose how you sign in",
    sk: "Vyberte si spôsob prihlásenia",
    de: "Wähle, wie du dich anmeldest",
  },
  "wizard.steps.finish": { en: "Your desktop", sk: "Vaša plocha", de: "Dein Desktop" },
  "wizard.rail.label": {
    en: "Setup progress",
    sk: "Priebeh nastavenia",
    de: "Fortschritt der Einrichtung",
  },
  "wizard.rail.done": {
    en: "{step} (done)",
    sk: "{step} (hotovo)",
    de: "{step} (erledigt)",
  },
  "wizard.rail.current": {
    en: "{step} (current)",
    sk: "{step} (aktuálny krok)",
    de: "{step} (aktuell)",
  },
  "wizard.rail.position": {
    en: "Step {current} of {total}",
    sk: "Krok {current} z {total}",
    de: "Schritt {current} von {total}",
  },

  // ── api.ts ──────────────────────────────────────────────────────────────
  "wizard.api.unauthorized": {
    en: "unauthorized",
    sk: "neautorizované",
    de: "nicht autorisiert",
  },
  "wizard.api.notObject": {
    en: "the setup document is not an object",
    sk: "dokument s nastavením nie je objekt",
    de: "das Einrichtungsdokument ist kein Objekt",
  },
  "wizard.api.noName": {
    en: "the setup document does not name this box",
    sk: "dokument s nastavením neuvádza názov tohto zariadenia",
    de: "das Einrichtungsdokument nennt den Namen dieser Box nicht",
  },
  "wizard.password.tooShort": {
    en: {
      one: "Use at least {min} characters. This one has {count}.",
      other: "Use at least {min} characters. This one has {count}.",
    },
    sk: {
      one: "Použite aspoň {min} znakov. Toto heslo má {count} znak.",
      few: "Použite aspoň {min} znakov. Toto heslo má {count} znaky.",
      many: "Použite aspoň {min} znakov. Toto heslo má {count} znakov.",
      other: "Použite aspoň {min} znakov. Toto heslo má {count} znakov.",
    },
    de: {
      one: "Nimm mindestens {min} Zeichen. Dieses hat {count}.",
      other: "Nimm mindestens {min} Zeichen. Dieses hat {count}.",
    },
  },
  "wizard.password.tooLong": {
    en: "That is longer than this box accepts. Shorten it.",
    sk: "Heslo je dlhšie, než toto zariadenie prijme. Skráťte ho.",
    de: "Das ist länger, als diese Box annimmt. Kürze es.",
  },
  "wizard.password.control": {
    en: "Remove the line breaks and control characters.",
    sk: "Odstráňte zalomenia riadkov a riadiace znaky.",
    de: "Entferne die Zeilenumbrüche und Steuerzeichen.",
  },
  "wizard.password.noLower": {
    en: "Add a lower-case letter.",
    sk: "Pridajte malé písmeno.",
    de: "Füge einen Kleinbuchstaben hinzu.",
  },
  "wizard.password.noUpper": {
    en: "Add an upper-case letter.",
    sk: "Pridajte veľké písmeno.",
    de: "Füge einen Großbuchstaben hinzu.",
  },
  "wizard.password.noDigit": {
    en: "Add a digit.",
    sk: "Pridajte číslicu.",
    de: "Füge eine Ziffer hinzu.",
  },
  "wizard.password.noSymbol": {
    en: "Add a symbol, such as - or !.",
    sk: "Pridajte symbol, napríklad - alebo !.",
    de: "Füge ein Sonderzeichen hinzu, etwa - oder !.",
  },

  // ── passkey.ts ──────────────────────────────────────────────────────────
  "wizard.passkey.needsHttps": {
    en: "Passkeys need an encrypted connection. Go back a step, install this box's certificate, and reopen this page at its https address.",
    sk: "Prístupové kľúče potrebujú šifrované pripojenie. Vráťte sa o krok späť, nainštalujte certifikát tohto zariadenia a otvorte túto stránku znova na jeho adrese https.",
    de: "Passkeys brauchen eine verschlüsselte Verbindung. Geh einen Schritt zurück, installiere das Zertifikat dieser Box und öffne diese Seite erneut unter ihrer https-Adresse.",
  },
  "wizard.passkey.noBrowserSupport": {
    en: "This browser cannot create passkeys.",
    sk: "Tento prehliadač nevie vytvárať prístupové kľúče.",
    de: "Dieser Browser kann keine Passkeys erstellen.",
  },
  "wizard.passkey.unreadable": {
    en: "This browser returned a passkey this box cannot read.",
    sk: "Prehliadač vrátil prístupový kľúč, ktorý toto zariadenie nevie prečítať.",
    de: "Dieser Browser hat einen Passkey geliefert, den diese Box nicht lesen kann.",
  },
  "wizard.passkey.orphan": {
    en: "{error} Your device may have saved a passkey this box did not keep. Remove it from your device's passkey list before trying again.",
    sk: "{error} Váš prístroj si možno uložil prístupový kľúč, ktorý toto zariadenie neuchovalo. Pred ďalším pokusom ho odstráňte zo zoznamu prístupových kľúčov vo svojom prístroji.",
    de: "{error} Dein Gerät hat vielleicht einen Passkey gespeichert, den diese Box nicht behalten hat. Entferne ihn aus der Passkey-Liste deines Geräts, bevor du es noch einmal versuchst.",
  },
  "wizard.passkey.duplicate": {
    en: "This device already has a passkey for this box.",
    sk: "Tento prístroj už má prístupový kľúč pre toto zariadenie.",
    de: "Dieses Gerät hat schon einen Passkey für diese Box.",
  },
  "wizard.passkey.securityError": {
    en: "This browser would not accept this box's local name for a passkey. Use the password instead.",
    sk: "Tento prehliadač neprijal lokálny názov tohto zariadenia pre prístupový kľúč. Použite namiesto neho heslo.",
    de: "Dieser Browser akzeptiert den lokalen Namen dieser Box nicht für einen Passkey. Nimm stattdessen das Passwort.",
  },

  // ── StepTrust.tsx ───────────────────────────────────────────────────────
  "wizard.trust.intro": {
    en: "This box signs its own certificate, so your browser has never seen it before and will not trust it yet. Install the certificate, then reopen this page at the address below. Everything after this step travels over that connection. That includes the password you are about to set and the passkey you may want, and neither should cross the network in the clear.",
    sk: "Toto zariadenie si podpisuje vlastný certifikát, takže ho váš prehliadač ešte nikdy nevidel a zatiaľ mu nebude dôverovať. Nainštalujte certifikát a potom túto stránku otvorte znova na adrese nižšie. Všetko po tomto kroku pôjde cez toto pripojenie. Patrí sem aj heslo, ktoré sa chystáte nastaviť, a prístupový kľúč, ktorý možno budete chcieť, a ani jedno by nemalo ísť sieťou nezašifrované.",
    de: "Diese Box signiert ihr Zertifikat selbst, also hat dein Browser es noch nie gesehen und vertraut ihm noch nicht. Installiere das Zertifikat und öffne diese Seite dann unter der Adresse unten erneut. Alles nach diesem Schritt läuft über diese Verbindung. Dazu gehören das Passwort, das du gleich festlegst, und der Passkey, den du vielleicht möchtest, und beides sollte nicht unverschlüsselt durchs Netzwerk gehen.",
  },
  "wizard.trust.blocked.title": {
    en: "Not available from here",
    sk: "Odtiaľto nie je k dispozícii",
    de: "Von hier aus nicht verfügbar",
  },
  "wizard.trust.blocked.body": {
    en: "The certificate is only handed out to devices on the same network as this box. Open this page from that network to install it.",
    sk: "Certifikát sa poskytuje iba prístrojom v tej istej sieti ako toto zariadenie. Ak ho chcete nainštalovať, otvorte túto stránku z tejto siete.",
    de: "Das Zertifikat gibt es nur für Geräte im selben Netzwerk wie diese Box. Öffne diese Seite aus diesem Netzwerk, um es zu installieren.",
  },
  "wizard.trust.nothing.title": {
    en: "Nothing to install",
    sk: "Nie je čo inštalovať",
    de: "Nichts zu installieren",
  },
  "wizard.trust.absent": {
    en: "This box is not serving an encrypted address, so there is no certificate to trust. You can continue.",
    sk: "Toto zariadenie neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať.",
    de: "Diese Box stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen.",
  },
  "wizard.trust.absentNoPasskey": {
    en: "This box is not serving an encrypted address, so there is no certificate to trust. You can continue. The next step will not offer a passkey.",
    sk: "Toto zariadenie neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať, ale v ďalšom kroku nebude ponúknutá možnosť prístupového kľúča.",
    de: "Diese Box stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen. Der nächste Schritt bietet dann keinen Passkey an.",
  },
  "wizard.trust.noTls": {
    en: "{name} is not serving an encrypted address, so there is no certificate to trust. You can continue.",
    sk: "Zariadenie {name} neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať.",
    de: "{name} stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen.",
  },
  "wizard.trust.noTlsNoPasskey": {
    en: "{name} is not serving an encrypted address, so there is no certificate to trust. You can continue. The next step will not offer a passkey.",
    sk: "Zariadenie {name} neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať, ale v ďalšom kroku nebude ponúknutá možnosť prístupového kľúča.",
    de: "{name} stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen. Der nächste Schritt bietet dann keinen Passkey an.",
  },
  "wizard.trust.failed.title": {
    en: "Could not read the setup details",
    sk: "Údaje o nastavení sa nepodarilo načítať",
    de: "Die Einrichtungsdaten konnten nicht gelesen werden",
  },
  "wizard.trust.failed.body": {
    en: "You can continue without installing the certificate, or reload this page to try again.",
    sk: "Môžete pokračovať bez inštalácie certifikátu alebo stránku načítať znova a skúsiť to ešte raz.",
    de: "Du kannst ohne das Zertifikat weitermachen oder die Seite neu laden und es noch einmal versuchen.",
  },
  // TrustCommand.tsx — the one-line installer.
  "wizard.trust.quick.title": {
    en: "The quick way: one line in a terminal",
    sk: "Rýchla cesta: jeden riadok v termináli",
    de: "Der schnelle Weg: eine Zeile im Terminal",
  },
  "wizard.trust.quick.osLabel": {
    en: "Operating system",
    sk: "Operačný systém",
    de: "Betriebssystem",
  },
  "wizard.trust.quick.unix": {
    en: "macOS / Linux",
    sk: "macOS / Linux",
    de: "macOS / Linux",
  },
  "wizard.trust.quick.windows": { en: "Windows", sk: "Windows", de: "Windows" },
  "wizard.trust.quick.bodyUnix": {
    en: "On this computer, open Terminal, paste this line and press Enter. macOS asks for your account password once, to change what your keychain trusts.",
    sk: "Na tomto počítači otvorte Terminál, vložte tento riadok a stlačte Enter. macOS sa raz opýta na heslo k účtu, aby mohol zmeniť, čomu dôveruje vaša kľúčenka.",
    de: "Öffne auf diesem Computer das Terminal, füge diese Zeile ein und drücke Enter. macOS fragt einmal nach deinem Kontopasswort, um zu ändern, wem dein Schlüsselbund vertraut.",
  },
  "wizard.trust.quick.bodyWindows": {
    en: "On this computer, press the Windows key, type PowerShell and press Enter. Paste this line into the window that opens and press Enter. Windows then shows its own dialog asking whether to install the certificate.",
    sk: "Na tomto počítači stlačte kláves Windows, napíšte PowerShell a stlačte Enter. Do okna, ktoré sa otvorí, vložte tento riadok a stlačte Enter. Windows potom zobrazí vlastný dialóg s otázkou, či certifikát nainštalovať.",
    de: "Drück auf diesem Computer die Windows-Taste, tipp PowerShell und drück Enter. Füge diese Zeile in das Fenster ein, das sich öffnet, und drück Enter. Windows zeigt dann einen eigenen Dialog, ob das Zertifikat installiert werden soll.",
  },
  "wizard.trust.quick.bodyPhone": {
    en: "Phones have no terminal. On this device, use the certificate download below instead. On a computer, this line does the whole step.",
    sk: "Telefóny nemajú terminál. Na tomto prístroji použite namiesto toho stiahnutie certifikátu nižšie. Na počítači tento riadok urobí celý krok.",
    de: "Telefone haben kein Terminal. Nimm auf diesem Gerät stattdessen den Zertifikat-Download unten. Auf einem Computer erledigt diese Zeile den ganzen Schritt.",
  },
  "wizard.trust.quick.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "wizard.trust.quick.what": {
    en: "It adds one certificate to the stores your browsers read, for your user only. It installs no software, changes nothing else, and is undone by deleting that one entry.",
    sk: "Pridá jeden certifikát do úložísk, ktoré čítajú vaše prehliadače, len pre vášho používateľa. Neinštaluje žiadny softvér, nič iné nemení a vráti sa späť zmazaním tej jednej položky.",
    de: "Sie fügt den Speichern, die deine Browser lesen, ein Zertifikat hinzu, nur für deinen Benutzer. Sie installiert keine Software, ändert sonst nichts und lässt sich durch Löschen dieses einen Eintrags rückgängig machen.",
  },
  "wizard.trust.quick.where": {
    en: "It comes from this box, on your own network, not from the internet. The box's private key never leaves the box.",
    sk: "Pochádza z tohto zariadenia, z vašej vlastnej siete, nie z internetu. Súkromný kľúč zariadenia nikdy zariadenie neopustí.",
    de: "Sie kommt von dieser Box, aus deinem eigenen Netzwerk, nicht aus dem Internet. Der private Schlüssel der Box verlässt die Box nie.",
  },
  "wizard.trust.quick.read": {
    en: "Read the script first",
    sk: "Najprv si skript prečítajte",
    de: "Lies das Skript zuerst",
  },
  "wizard.trust.quick.readTail": {
    en: "if you like. It is short, plain text, and says at the top what it does.",
    sk: "ak chcete. Je krátky, v čistom texte a hneď na začiatku hovorí, čo robí.",
    de: "wenn du magst. Es ist kurz, reiner Text, und sagt ganz oben, was es tut.",
  },
  "wizard.trust.quick.then": {
    en: "It prints the fingerprint shown below so you can compare the two. Then restart your browser and reopen this page at https://{fqdn}.",
    sk: "Vypíše odtlačok zobrazený nižšie, aby ste si oba mohli porovnať. Potom reštartujte prehliadač a otvorte túto stránku znova na https://{fqdn}.",
    de: "Sie gibt den unten gezeigten Fingerabdruck aus, damit du beide vergleichen kannst. Starte danach deinen Browser neu und öffne diese Seite unter https://{fqdn} erneut.",
  },
  "wizard.trust.byHand": {
    en: "Or by hand. Download the certificate and add it to your browser or system as a trusted authority.",
    sk: "Alebo ručne. Stiahnite certifikát a pridajte ho do prehliadača alebo systému ako dôveryhodnú autoritu.",
    de: "Oder von Hand. Lade das Zertifikat herunter und füge es deinem Browser oder System als vertrauenswürdige Stelle hinzu.",
  },
  "wizard.trust.getCert": {
    en: "Get the certificate",
    sk: "Stiahnuť certifikát",
    de: "Zertifikat herunterladen",
  },
  "wizard.trust.alreadyEncrypted": {
    en: "You are already on the encrypted address.",
    sk: "Už ste na šifrovanej adrese.",
    de: "Du bist schon auf der verschlüsselten Adresse.",
  },
  "wizard.trust.reopen": {
    en: "Reopen at https://{fqdn}",
    sk: "Otvoriť znova na https://{fqdn}",
    de: "Unter https://{fqdn} neu öffnen",
  },
  "wizard.trust.checkFingerprint": {
    en: "Check this fingerprint",
    sk: "Skontrolujte tento odtlačok",
    de: "Prüfe diesen Fingerabdruck",
  },
  "wizard.trust.fingerprintBody": {
    en: "Your browser shows the same fingerprint when it asks whether to trust the certificate. If the two do not match character for character, stop. Something between you and {name} is not this box.",
    sk: "Keď sa váš prehliadač opýta, či certifikátu dôverovať, zobrazí ten istý odtlačok. Ak sa tieto dva nezhodujú znak po znaku, zastavte sa. Niečo medzi vami a zariadením {name} nie je toto zariadenie.",
    de: "Dein Browser zeigt denselben Fingerabdruck, wenn er fragt, ob er dem Zertifikat vertrauen soll. Wenn die beiden nicht Zeichen für Zeichen übereinstimmen, hör auf. Dann ist etwas zwischen dir und {name} nicht diese Box.",
  },
  "wizard.trust.validUntil": {
    en: "Valid until {date}. This box mints a new one before then on its own.",
    sk: "Platný do {date}. Toto zariadenie si dovtedy samo vytvorí nový.",
    de: "Gültig bis {date}. Diese Box erstellt vorher von selbst ein neues.",
  },
  "wizard.trust.unencrypted.title": {
    en: "This page is not encrypted yet",
    sk: "Táto stránka zatiaľ nie je šifrovaná",
    de: "Diese Seite ist noch nicht verschlüsselt",
  },
  "wizard.trust.unencrypted.body": {
    en: "You can continue either way. On an unencrypted address no browser will offer to make a passkey, so step 2 will ask you for a password only. If {fqdn} does not open on this computer, stay on the address you are on. The box answers the same there.",
    sk: "Pokračovať môžete tak či tak. Na nešifrovanej adrese žiadny prehliadač neponúkne vytvorenie prístupového kľúča, takže krok 2 si vypýta iba heslo. Ak sa {fqdn} na tomto počítači neotvorí, zostaňte na adrese, na ktorej ste. Zariadenie tam odpovedá rovnako.",
    de: "Weitermachen kannst du so oder so. Auf einer unverschlüsselten Adresse bietet kein Browser an, einen Passkey zu erstellen, also fragt Schritt 2 nur nach einem Passwort. Öffnet sich {fqdn} auf diesem Computer nicht, bleib auf der Adresse, auf der du bist. Die Box antwortet dort genauso.",
  },
  "wizard.trust.otherName.title": {
    en: "Encrypted, but under another name",
    sk: "Šifrované, ale pod iným názvom",
    de: "Verschlüsselt, aber unter einem anderen Namen",
  },
  "wizard.trust.otherName.body": {
    en: "The certificate is issued for {fqdn}. Reopen this page there so your browser accepts it.",
    sk: "Certifikát je vydaný pre {fqdn}. Otvorte túto stránku znova tam, aby ho prehliadač prijal.",
    de: "Das Zertifikat ist für {fqdn} ausgestellt. Öffne diese Seite dort neu, damit dein Browser es akzeptiert.",
  },

  // ── StepSignIn.tsx ──────────────────────────────────────────────────────
  "wizard.signin.intro": {
    en: "This box made itself a random password when it was installed and showed it to nobody, which is why nothing can sign in yet. Choose one now. It is the one password for everything here: LosOS Cloud, the apps on your phone and computer, and these admin pages.",
    sk: "Toto zariadenie si pri inštalácii vytvorilo náhodné heslo a nikomu ho neukázalo, preto sa zatiaľ nikto nemôže prihlásiť. Zvoľte si teraz vlastné. Je to jediné heslo na všetko: LosOS Cloud, aplikácie v telefóne a počítači aj tieto správcovské stránky.",
    de: "Diese Box hat sich bei der Installation ein zufälliges Passwort gegeben und es niemandem gezeigt, deshalb kann sich noch niemand anmelden. Wähl jetzt eins. Es ist das eine Passwort für alles hier: LosOS Cloud, die Apps auf Handy und Computer und diese Admin-Seiten.",
  },
  "wizard.signin.waiting.title": {
    en: "Waiting for this box to finish starting",
    sk: "Čaká sa, kým sa zariadenie dokončí spúšťať",
    de: "Warten, bis diese Box fertig gestartet ist",
  },
  "wizard.signin.waiting.body": {
    en: "The password can be set as soon as the file service is ready. This page checks every few seconds and lets you continue on its own. There is nothing to click.",
    sk: "Heslo bude možné nastaviť, hneď ako bude súborová služba pripravená. Táto stránka to kontroluje každých pár sekúnd a sama vás pustí ďalej. Nie je potrebné nič stláčať.",
    de: "Das Passwort kann gesetzt werden, sobald der Dateidienst bereit ist. Diese Seite prüft das alle paar Sekunden und lässt dich von selbst weiter. Es gibt nichts zu klicken.",
  },
  "wizard.signin.waiting.since": {
    en: {
      one: "Waiting for {count} second so far.",
      other: "Waiting for {count} seconds so far.",
    },
    sk: {
      one: "Zatiaľ sa čaká {count} sekundu.",
      few: "Zatiaľ sa čakajú {count} sekundy.",
      many: "Zatiaľ sa čaká {count} sekundy.",
      other: "Zatiaľ sa čaká {count} sekúnd.",
    },
    de: {
      one: "Bisher {count} Sekunde gewartet.",
      other: "Bisher {count} Sekunden gewartet.",
    },
  },
  "wizard.signin.waiting.unreachable": {
    en: "This box did not answer the last check. It keeps trying.",
    sk: "Zariadenie neodpovedalo na poslednú kontrolu. Skúša sa to ďalej.",
    de: "Diese Box hat auf die letzte Prüfung nicht geantwortet. Es wird weiter versucht.",
  },
  "wizard.signin.err.notReady": {
    en: "This box is not ready for the password yet. The page keeps checking and will let you try again.",
    sk: "Zariadenie ešte nie je pripravené na heslo. Stránka to ďalej kontroluje a pustí vás skúsiť to znova.",
    de: "Diese Box ist noch nicht bereit für das Passwort. Die Seite prüft weiter und lässt dich es erneut versuchen.",
  },
  "wizard.signin.mismatch": {
    en: "The two passwords are not the same.",
    sk: "Heslá sa nezhodujú.",
    de: "Die beiden Passwörter sind nicht gleich.",
  },
  "wizard.signin.name": {
    en: "Sign-in name",
    sk: "Prihlasovacie meno",
    de: "Anmeldename",
  },
  "wizard.signin.nameHint": {
    en: "What you type to sign in to this box, for example your first name. Lower-case letters and digits. You are its first administrator.",
    sk: "To, čo zadáte pri prihlásení do tohto zariadenia, napríklad vaše krstné meno. Malé písmená a číslice. Stanete sa jeho prvým správcom.",
    de: "Das tippst du bei der Anmeldung an dieser Box ein, zum Beispiel deinen Vornamen. Kleinbuchstaben und Ziffern. Du bist ihr erster Administrator.",
  },
  "wizard.signin.badName": {
    en: "A sign-in name starts with a letter and has up to 32 lower-case letters, digits, dots, dashes or underscores.",
    sk: "Prihlasovacie meno začína písmenom a má najviac 32 malých písmen, číslic, bodiek, pomlčiek alebo podčiarkovníkov.",
    de: "Ein Anmeldename beginnt mit einem Buchstaben und hat bis zu 32 Kleinbuchstaben, Ziffern, Punkte, Bindestriche oder Unterstriche.",
  },
  "wizard.signin.newPassword": {
    en: "New password",
    sk: "Nové heslo",
    de: "Neues Passwort",
  },
  "wizard.signin.hide": {
    en: "Hide the password",
    sk: "Skryť heslo",
    de: "Passwort verbergen",
  },
  "wizard.signin.show": {
    en: "Show the password",
    sk: "Zobraziť heslo",
    de: "Passwort anzeigen",
  },
  "wizard.signin.hint": {
    en: "Four rules, and these are all of them. A few unrelated words joined with a dash, with a capital letter and a number somewhere, is the easiest way to meet them.",
    sk: "Štyri pravidlá, a sú to všetky. Najľahšie ich splníte niekoľkými nesúvisiacimi slovami spojenými pomlčkou, s veľkým písmenom a číslom niekde vnútri.",
    de: "Vier Regeln, und das sind alle. Am einfachsten erfüllst du sie mit ein paar Wörtern, die nichts miteinander zu tun haben, durch Bindestriche verbunden, mit einem Großbuchstaben und einer Zahl irgendwo.",
  },
  "wizard.signin.rules": { en: "Password rules", sk: "Pravidlá hesla", de: "Passwortregeln" },
  "wizard.signin.rule.length": {
    en: { one: "At least {count} character", other: "At least {count} characters" },
    sk: {
      one: "Aspoň {count} znak",
      few: "Aspoň {count} znaky",
      many: "Aspoň {count} znakov",
      other: "Aspoň {count} znakov",
    },
    de: { one: "Mindestens {count} Zeichen", other: "Mindestens {count} Zeichen" },
  },
  "wizard.signin.rule.cases": {
    en: "A capital letter and a small letter",
    sk: "Veľké aj malé písmeno",
    de: "Ein Großbuchstabe und ein Kleinbuchstabe",
  },
  "wizard.signin.rule.digit": { en: "A digit", sk: "Číslica", de: "Eine Ziffer" },
  "wizard.signin.rule.symbol": {
    en: "A symbol, such as - or !",
    sk: "Symbol, napríklad - alebo !",
    de: "Ein Sonderzeichen, etwa - oder !",
  },
  "wizard.signin.rule.met": { en: "(done)", sk: "(splnené)", de: "(erfüllt)" },
  "wizard.signin.rule.unmet": { en: "(not yet)", sk: "(ešte nie)", de: "(noch nicht)" },
  "wizard.signin.again": {
    en: "Type it again",
    sk: "Zadajte ho znova",
    de: "Gib es noch einmal ein",
  },
  "wizard.signin.againHint": {
    en: "Nothing on this box can tell you what you typed, so a slip here means starting the box over.",
    sk: "Nič na tomto zariadení vám nedokáže povedať, čo ste zadali, takže preklep tu znamená nastavovať zariadenie odznova.",
    de: "Nichts auf dieser Box kann dir sagen, was du eingegeben hast, also heißt ein Tippfehler hier, die Box neu aufzusetzen.",
  },
  "wizard.signin.set": {
    en: "Set the password",
    sk: "Nastaviť heslo",
    de: "Passwort festlegen",
  },
  "wizard.signin.setDifferent": {
    en: "Set a different password",
    sk: "Nastaviť iné heslo",
    de: "Anderes Passwort festlegen",
  },
  "wizard.signin.setting": {
    en: "Setting the password",
    sk: "Nastavuje sa heslo",
    de: "Passwort wird festgelegt",
  },
  "wizard.signin.done.title": {
    en: "Password set",
    sk: "Heslo je nastavené",
    de: "Passwort festgelegt",
  },
  "wizard.signin.done.login": {
    en: "Your name on LosOS Login",
    sk: "Vaše meno v LosOS Login",
    de: "Dein Name bei LosOS Login",
  },
  "wizard.signin.done.name": {
    en: "Your LosOS Cloud account",
    sk: "Váš účet v LosOS Cloud",
    de: "Dein Konto in LosOS Cloud",
  },
  "wizard.signin.done.body": {
    en: "LosOS Login opens these admin pages. The LosOS Cloud account is the one to type on the sign-in page at the end of the wizard, and in the desktop and phone apps. Both take the password you just set.",
    sk: "LosOS Login otvára tieto správcovské stránky. Účet LosOS Cloud zadáte na prihlasovacej stránke na konci sprievodcu a v aplikáciách pre počítač a telefón. Oba berú heslo, ktoré ste práve nastavili.",
    de: "LosOS Login öffnet diese Admin-Seiten. Das Konto in LosOS Cloud gibst du auf der Anmeldeseite am Ende der Einrichtung ein und in den Apps für Computer und Handy. Beide nehmen das Passwort, das du gerade festgelegt hast.",
  },
  "wizard.signin.err.unauthorized": {
    en: "This box stopped accepting the admin key. Unlock the page again, then set the password.",
    sk: "Toto zariadenie prestalo prijímať správcovský kľúč. Stránku znova odomknite a potom nastavte heslo.",
    de: "Diese Box akzeptiert den Admin-Schlüssel nicht mehr. Entsperre die Seite erneut und leg dann das Passwort fest.",
  },
  "wizard.signin.err.missing": {
    en: "The software on this box cannot set the password yet. Update it and come back.",
    sk: "Softvér na tomto zariadení zatiaľ nevie nastaviť heslo. Aktualizujte ho a vráťte sa.",
    de: "Die Software auf dieser Box kann das Passwort noch nicht festlegen. Aktualisiere sie und komm zurück.",
  },
  "wizard.signin.key.title": {
    en: "Your spare admin key, shown only now",
    sk: "Váš náhradný správcovský kľúč, zobrazí sa iba teraz",
    de: "Dein Ersatz-Admin-Schlüssel, nur jetzt zu sehen",
  },
  "wizard.signin.key.body": {
    en: "From now on you sign in to these admin pages on LosOS Login with your name and password. This key is the spare. It opens every pane even on a day LosOS Login does not answer. The box shows it only now and keeps no copy a browser can ask for again, so copy it into a password manager, or print it and keep the sheet.",
    sk: "Odteraz sa do týchto správcovských stránok prihlasujete v LosOS Login svojím menom a heslom. Tento kľúč je náhradný. Otvorí každú časť aj v deň, keď LosOS Login neodpovedá. Zariadenie ho zobrazí iba teraz a nenecháva si kópiu, o ktorú by prehliadač mohol znova požiadať, preto si ho skopírujte do správcu hesiel alebo si ho vytlačte a hárok odložte.",
    de: "Ab jetzt meldest du dich bei LosOS Login mit deinem Namen und Passwort an diesen Admin-Seiten an. Dieser Schlüssel ist der Ersatz. Er öffnet jeden Bereich auch an einem Tag, an dem LosOS Login nicht antwortet. Die Box zeigt ihn nur jetzt und behält keine Kopie, die ein Browser noch einmal abfragen könnte, also kopiere ihn in einen Passwortmanager, oder druck ihn aus und heb das Blatt auf.",
  },
  "wizard.signin.key.label": {
    en: "Spare admin key",
    sk: "Náhradný správcovský kľúč",
    de: "Ersatz-Admin-Schlüssel",
  },
  "wizard.signin.key.copy": {
    en: "Copy the key",
    sk: "Kopírovať kľúč",
    de: "Schlüssel kopieren",
  },
  "wizard.signin.key.print": { en: "Print", sk: "Tlačiť", de: "Drucken" },
  // The printed sheet. Its own markup in StepSignIn.tsx (KeySheet).
  "wizard.signin.key.sheetTitle": {
    en: "Spare admin key for {name}",
    sk: "Náhradný správcovský kľúč pre {name}",
    de: "Ersatz-Admin-Schlüssel für {name}",
  },
  "wizard.signin.key.sheetPrinted": {
    en: "Printed {date}.",
    sk: "Vytlačené {date}.",
    de: "Gedruckt am {date}.",
  },
  "wizard.signin.key.sheetBody": {
    en: "You normally sign in to the admin pages on LosOS Login with your name and password. If LosOS Login does not answer, choose \"Use the spare admin key instead\" and paste this. It was shown once, during setup, and the box will not show it again.",
    sk: "Do správcovských stránok sa bežne prihlasujete v LosOS Login svojím menom a heslom. Ak LosOS Login neodpovedá, zvoľte „Použiť namiesto toho náhradný správcovský kľúč“ a vložte tento. Zobrazil sa raz, počas nastavenia, a zariadenie ho znova neukáže.",
    de: "An den Admin-Seiten meldest du dich normalerweise bei LosOS Login mit deinem Namen und Passwort an. Wenn LosOS Login nicht antwortet, wähl „Stattdessen den Ersatz-Admin-Schlüssel verwenden“ und gib diesen ein. Er wurde einmal gezeigt, bei der Einrichtung, und die Box zeigt ihn nicht noch einmal.",
  },
  "wizard.signin.key.sheetKey": {
    en: "Treat it like a key to the box, because that is what it is.",
    sk: "Zaobchádzajte s ním ako s kľúčom od zariadenia, pretože presne tým je.",
    de: "Behandle ihn wie einen Schlüssel zur Box, denn genau das ist er.",
  },
  "wizard.signin.key.copyFailed": {
    en: "This browser would not copy it. The key is selected. Press Ctrl+C, or Cmd+C on a Mac.",
    sk: "Tento prehliadač ho neskopíroval. Kľúč je označený. Stlačte Ctrl+C, na Macu Cmd+C.",
    de: "Dieser Browser wollte ihn nicht kopieren. Der Schlüssel ist markiert. Drück Strg+C, auf dem Mac Cmd+C.",
  },
  "wizard.passkey.label": {
    en: "{name} admin",
    sk: "{name} (správca)",
    de: "{name} Admin",
  },
  "wizard.passkey.heading": {
    en: "Add a passkey as well",
    sk: "Pridajte aj prístupový kľúč",
    de: "Zusätzlich einen Passkey hinzufügen",
  },
  "wizard.passkey.body": {
    en: "A passkey signs you in with your face, your fingerprint or your screen lock, from this browser on this device. It adds to the password rather than replacing it. The desktop and phone apps still sign in with the password you just set.",
    sk: "Prístupový kľúč vás prihlási tvárou, odtlačkom prsta alebo zámkou obrazovky, a to v tomto prehliadači na tomto prístroji. Heslo dopĺňa, nenahrádza ho. Aplikácie pre počítač a telefón sa naďalej prihlasujú heslom, ktoré ste práve nastavili.",
    de: "Ein Passkey meldet dich mit deinem Gesicht, deinem Fingerabdruck oder deiner Bildschirmsperre an, in diesem Browser auf diesem Gerät. Er kommt zum Passwort dazu und ersetzt es nicht. Die Apps für Computer und Handy melden sich weiter mit dem Passwort an, das du gerade festgelegt hast.",
  },
  "wizard.passkey.checking": {
    en: "Checking what this browser can do.",
    sk: "Zisťuje sa, čo tento prehliadač dokáže.",
    de: "Prüfe, was dieser Browser kann.",
  },
  "wizard.passkey.unavailable.title": {
    en: "Not available here",
    sk: "Tu nie je k dispozícii",
    de: "Hier nicht verfügbar",
  },
  "wizard.passkey.create": {
    en: "Create passkey",
    sk: "Vytvoriť prístupový kľúč",
    de: "Passkey erstellen",
  },
  "wizard.passkey.waiting": {
    en: "Waiting for your device",
    sk: "Čaká sa na váš prístroj",
    de: "Warte auf dein Gerät",
  },
  "wizard.passkey.securityKey": {
    en: "This device has no built-in fingerprint or face unlock, so your browser will ask for a security key.",
    sk: "Tento prístroj nemá vstavané odomykanie odtlačkom prsta ani tvárou, takže prehliadač si vypýta bezpečnostný kľúč.",
    de: "Dieses Gerät hat keine eingebaute Entsperrung per Fingerabdruck oder Gesicht, daher fragt dein Browser nach einem Sicherheitsschlüssel.",
  },
  "wizard.passkey.created.title": {
    en: "Passkey created",
    sk: "Prístupový kľúč je vytvorený",
    de: "Passkey erstellt",
  },
  "wizard.passkey.created.body": {
    en: "Saved as {label}. It works in this browser on this device only.",
    sk: "Uložený ako {label}. Funguje iba v tomto prehliadači na tomto prístroji.",
    de: "Gespeichert als {label}. Er funktioniert nur in diesem Browser auf diesem Gerät.",
  },
  "wizard.passkey.notYet.title": {
    en: "Not on this box yet",
    sk: "Na tomto zariadení zatiaľ nie",
    de: "Auf dieser Box noch nicht",
  },
  "wizard.passkey.notYet.body": {
    en: "The software on this box cannot register a passkey yet. The password you set above is enough to sign in. You can add a passkey later, from the security settings inside your files.",
    sk: "Softvér na tomto zariadení zatiaľ nevie zaregistrovať prístupový kľúč. Na prihlásenie stačí heslo, ktoré ste nastavili vyššie. Prístupový kľúč môžete pridať neskôr v nastaveniach zabezpečenia vo svojich súboroch.",
    de: "Die Software auf dieser Box kann noch keinen Passkey registrieren. Das Passwort, das du oben festgelegt hast, reicht zum Anmelden. Einen Passkey kannst du später in den Sicherheitseinstellungen in deinen Dateien hinzufügen.",
  },
  "wizard.passkey.problem.title": {
    en: "That did not work",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },

  // ── StepDesktop.tsx ─────────────────────────────────────────────────────
  "wizard.desktop.signingIn": {
    en: "Signing you in",
    sk: "Prihlasujeme vás",
    de: "Du wirst angemeldet",
  },
  "wizard.desktop.signingInNamed": {
    en: "Signing you in as {name}",
    sk: "Prihlasujeme vás ako {name}",
    de: "Du wirst als {name} angemeldet",
  },
  "wizard.desktop.starting.title": {
    en: "LosOS Login is still starting",
    sk: "LosOS Login sa ešte spúšťa",
    de: "LosOS Login startet noch",
  },
  "wizard.desktop.starting.body": {
    en: "It starts once the box has its first password. This page keeps asking and signs you in when it answers. On a new box this takes a minute.",
    sk: "Spustí sa, keď má zariadenie prvé heslo. Táto stránka sa pýta ďalej a prihlási vás, hneď ako odpovie. Na novom zariadení to trvá minútu.",
    de: "Es startet, sobald die Box ihr erstes Passwort hat. Diese Seite fragt weiter nach und meldet dich an, sobald es antwortet. Auf einer neuen Box dauert das eine Minute.",
  },
  "wizard.desktop.ready": {
    en: "This is your desktop, and you are signed in. Your apps, your files and every setting are here.",
    sk: "Toto je vaša plocha a ste prihlásení. Sú tu vaše aplikácie, súbory a všetky nastavenia.",
    de: "Das ist dein Desktop, und du bist angemeldet. Hier sind deine Apps, deine Dateien und alle Einstellungen.",
  },
  "wizard.desktop.readyNamed": {
    en: "This is your desktop, and you are signed in as {name}. Your apps, your files and every setting are here.",
    sk: "Toto je vaša plocha a ste prihlásení ako {name}. Sú tu vaše aplikácie, súbory a všetky nastavenia.",
    de: "Das ist dein Desktop, und du bist als {name} angemeldet. Hier sind deine Apps, deine Dateien und alle Einstellungen.",
  },
  "wizard.desktop.spare.title": {
    en: "Signed in with the spare key",
    sk: "Prihlásené náhradným kľúčom",
    de: "Mit dem Ersatzschlüssel angemeldet",
  },
  "wizard.desktop.spare.plainHttp": {
    en: "This page is not encrypted, and LosOS Login signs people in over https only. This tab stays signed in with the spare key. Next time, open the box at its https address and sign in with your password.",
    sk: "Táto stránka nie je šifrovaná a LosOS Login prihlasuje iba cez https. Táto karta zostane prihlásená náhradným kľúčom. Nabudúce otvorte zariadenie na jeho adrese https a prihláste sa heslom.",
    de: "Diese Seite ist nicht verschlüsselt, und LosOS Login meldet nur über https an. Dieser Tab bleibt mit dem Ersatzschlüssel angemeldet. Öffne die Box nächstes Mal unter ihrer https-Adresse und melde dich mit deinem Passwort an.",
  },
  "wizard.desktop.spare.noLogin": {
    en: "This box has no LosOS Login, so the spare key is how you sign in to it. This tab stays signed in with it.",
    sk: "Toto zariadenie nemá LosOS Login, takže sa doň prihlasujete náhradným kľúčom. Táto karta ním zostane prihlásená.",
    de: "Diese Box hat kein LosOS Login, also meldest du dich mit dem Ersatzschlüssel an. Dieser Tab bleibt damit angemeldet.",
  },
  "wizard.desktop.spare.address": {
    en: "LosOS Login took your password but does not sign people in at this address. This tab stays signed in with the spare key. Open the box by its name to sign in with your password.",
    sk: "LosOS Login vaše heslo prijal, ale na tejto adrese neprihlasuje. Táto karta zostane prihlásená náhradným kľúčom. Ak sa chcete prihlásiť heslom, otvorte zariadenie podľa jeho mena.",
    de: "LosOS Login hat dein Passwort angenommen, meldet unter dieser Adresse aber nicht an. Dieser Tab bleibt mit dem Ersatzschlüssel angemeldet. Öffne die Box unter ihrem Namen, um dich mit deinem Passwort anzumelden.",
  },
  "wizard.desktop.password.again": {
    en: "This tab no longer has the password you set in step 2. Type it once more to sign in.",
    sk: "Táto karta si už nepamätá heslo, ktoré ste nastavili v kroku 2. Zadajte ho ešte raz a prihláste sa.",
    de: "Dieser Tab kennt das Passwort aus Schritt 2 nicht mehr. Gib es noch einmal ein, um dich anzumelden.",
  },
  "wizard.desktop.password.refused": {
    en: "LosOS Login did not take that password. Type the one you set in step 2.",
    sk: "LosOS Login toto heslo neprijal. Zadajte to, ktoré ste nastavili v kroku 2.",
    de: "LosOS Login hat dieses Passwort nicht angenommen. Gib das aus Schritt 2 ein.",
  },
  "wizard.desktop.password.name": {
    en: "Sign-in name",
    sk: "Prihlasovacie meno",
    de: "Anmeldename",
  },
  "wizard.desktop.password.label": { en: "Password", sk: "Heslo", de: "Passwort" },
  "wizard.desktop.password.wrong": {
    en: "Wrong name or password.",
    sk: "Nesprávne meno alebo heslo.",
    de: "Falscher Name oder falsches Passwort.",
  },
  "wizard.desktop.password.submit": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },

  // ── Toasts the wizard raises ──────────────────────────────────────────────
  "wizard.trust.quick.copiedTitle": {
    en: "Command copied",
    sk: "Príkaz je skopírovaný",
    de: "Befehl kopiert",
  },
  "wizard.trust.quick.copiedBody": {
    en: "Paste it into a terminal on this computer.",
    sk: "Vložte ho do terminálu na tomto počítači.",
    de: "Füg ihn in ein Terminal auf diesem Computer ein.",
  },
  "wizard.trust.quick.copyFailed": {
    en: "This browser would not copy it. The line is selected. Press Ctrl+C, or Cmd+C on a Mac.",
    sk: "Tento prehliadač ho neskopíroval. Riadok je označený. Stlačte Ctrl+C, na Macu Cmd+C.",
    de: "Dieser Browser wollte ihn nicht kopieren. Die Zeile ist markiert. Drück Strg+C, auf dem Mac Cmd+C.",
  },
  "wizard.copyFailedTitle": {
    en: "Not copied",
    sk: "Neskopírované",
    de: "Nicht kopiert",
  },
  "wizard.signin.readyTitle": { en: "Ready", sk: "Pripravené", de: "Bereit" },
  "wizard.signin.readyBody": {
    en: "You can set the password now.",
    sk: "Heslo môžete nastaviť teraz.",
    de: "Du kannst das Passwort jetzt setzen.",
  },
  "wizard.signin.done.toast": {
    en: "Sign in as {name} from now on.",
    sk: "Odteraz sa prihlasujte ako {name}.",
    de: "Melde dich ab jetzt als {name} an.",
  },
  "wizard.signin.notSet": {
    en: "The password was not set",
    sk: "Heslo sa nenastavilo",
    de: "Das Passwort wurde nicht gesetzt",
  },
  "wizard.signin.key.copiedTitle": {
    en: "Spare key copied",
    sk: "Náhradný kľúč je skopírovaný",
    de: "Ersatzschlüssel kopiert",
  },
  "wizard.signin.key.copiedBody": {
    en: "Paste it somewhere that is not this box.",
    sk: "Vložte ho niekam mimo tohto zariadenia.",
    de: "Füg ihn irgendwo ein, nur nicht auf dieser Box.",
  },
});
