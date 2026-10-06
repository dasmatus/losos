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
  "wizard.nav.finish": { en: "Finish", sk: "Dokončiť", de: "Abschließen" },
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
  // The two recovery keys stay for the hidden step (see steps.ts).
  "wizard.steps.recovery.title": {
    en: "Write down your recovery code",
    sk: "Zapíšte si kód na obnovenie",
    de: "Schreib deinen Wiederherstellungscode auf",
  },
  "wizard.steps.recovery.rail": {
    en: "Recovery code",
    sk: "Kód na obnovenie",
    de: "Wiederherstellungscode",
  },
  "wizard.steps.finish": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
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
    en: "{error} Your device may have saved a passkey this box did not keep; remove it from your device's passkey list before trying again.",
    sk: "{error} Váš prístroj si možno uložil prístupový kľúč, ktorý toto zariadenie neuchovalo; pred ďalším pokusom ho odstráňte zo zoznamu prístupových kľúčov vo svojom prístroji.",
    de: "{error} Dein Gerät hat vielleicht einen Passkey gespeichert, den diese Box nicht behalten hat; entferne ihn aus der Passkey-Liste deines Geräts, bevor du es noch einmal versuchst.",
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
    en: "This box is not serving an encrypted address, so there is no certificate to trust. You can continue; the passkey option on the next step will not be offered.",
    sk: "Toto zariadenie neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať, ale v ďalšom kroku nebude ponúknutá možnosť prístupového kľúča.",
    de: "Diese Box stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen; die Passkey-Option im nächsten Schritt wird dann nicht angeboten.",
  },
  "wizard.trust.noTls": {
    en: "{name} is not serving an encrypted address, so there is no certificate to trust. You can continue.",
    sk: "Zariadenie {name} neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať.",
    de: "{name} stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen.",
  },
  "wizard.trust.noTlsNoPasskey": {
    en: "{name} is not serving an encrypted address, so there is no certificate to trust. You can continue; the passkey option on the next step will not be offered.",
    sk: "Zariadenie {name} neposkytuje šifrovanú adresu, takže tu nie je žiadny certifikát, ktorému treba dôverovať. Môžete pokračovať, ale v ďalšom kroku nebude ponúknutá možnosť prístupového kľúča.",
    de: "{name} stellt keine verschlüsselte Adresse bereit, also gibt es kein Zertifikat, dem du vertrauen müsstest. Du kannst weitermachen; die Passkey-Option im nächsten Schritt wird dann nicht angeboten.",
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
    en: "Your browser shows the same fingerprint when it asks whether to trust the certificate. If the two do not match character for character, stop: something between you and {name} is not this box.",
    sk: "Keď sa váš prehliadač opýta, či certifikátu dôverovať, zobrazí ten istý odtlačok. Ak sa tieto dva nezhodujú znak po znaku, zastavte sa: niečo medzi vami a zariadením {name} nie je toto zariadenie.",
    de: "Dein Browser zeigt denselben Fingerabdruck, wenn er fragt, ob er dem Zertifikat vertrauen soll. Wenn die beiden nicht Zeichen für Zeichen übereinstimmen, hör auf: Dann ist etwas zwischen dir und {name} nicht diese Box.",
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
    en: "You can continue either way. On an unencrypted address no browser will offer to make a passkey, so step 2 will ask you for a password only.",
    sk: "Pokračovať môžete tak či tak. Na nešifrovanej adrese žiadny prehliadač neponúkne vytvorenie prístupového kľúča, takže krok 2 si vypýta iba heslo.",
    de: "Weitermachen kannst du so oder so. Auf einer unverschlüsselten Adresse bietet kein Browser an, einen Passkey zu erstellen, also fragt Schritt 2 nur nach einem Passwort.",
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
    en: "This box made itself a random password when it was installed and showed it to nobody, which is why nothing can sign in yet. Choose one now. It is the one password for everything here: LosOS cloud, the apps on your phone and computer, and these admin pages.",
    sk: "Toto zariadenie si pri inštalácii vytvorilo náhodné heslo a nikomu ho neukázalo, preto sa zatiaľ nikto nemôže prihlásiť. Zvoľte si teraz vlastné. Je to jediné heslo na všetko: LosOS cloud, aplikácie v telefóne a počítači aj tieto správcovské stránky.",
    de: "Diese Box hat sich bei der Installation ein zufälliges Passwort gegeben und es niemandem gezeigt, deshalb kann sich noch niemand anmelden. Wähl jetzt eins. Es ist das eine Passwort für alles hier: LosOS cloud, die Apps auf Handy und Computer und diese Admin-Seiten.",
  },
  "wizard.signin.waiting.title": {
    en: "Waiting for this box to finish starting",
    sk: "Čaká sa, kým sa zariadenie dokončí spúšťať",
    de: "Warten, bis diese Box fertig gestartet ist",
  },
  "wizard.signin.waiting.body": {
    en: "The password can be set as soon as the file service is ready. This page checks every few seconds and lets you continue on its own; there is nothing to click.",
    sk: "Heslo bude možné nastaviť, hneď ako bude súborová služba pripravená. Táto stránka to kontroluje každých pár sekúnd a sama vás pustí ďalej; nie je potrebné nič stláčať.",
    de: "Das Passwort kann gesetzt werden, sobald der Dateidienst bereit ist. Diese Seite prüft das alle paar Sekunden und lässt dich von selbst weiter; es gibt nichts zu klicken.",
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
  "wizard.signin.ready": {
    en: "Ready. You can set the password now.",
    sk: "Pripravené. Heslo môžete nastaviť teraz.",
    de: "Bereit. Du kannst das Passwort jetzt setzen.",
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
  "wizard.signin.done.name": {
    en: "Your sign-in name",
    sk: "Vaše prihlasovacie meno",
    de: "Dein Anmeldename",
  },
  "wizard.signin.done.body": {
    en: "This is the name to type on the sign-in page at the end of the wizard, and in the desktop and phone apps.",
    sk: "Toto meno zadáte na prihlasovacej stránke na konci sprievodcu a v aplikáciách pre počítač a telefón.",
    de: "Diesen Namen gibst du auf der Anmeldeseite am Ende der Einrichtung ein und in den Apps für Computer und Handy.",
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
    en: "From now on your password unlocks these admin pages. This key is the spare: it opens them even while LosOS cloud is not running to check the password. The box shows it only now and keeps no copy a browser can ask for again, so copy it into a password manager, or print it and keep the sheet.",
    sk: "Odteraz tieto správcovské stránky odomyká vaše heslo. Tento kľúč je náhradný: otvorí ich, aj keď LosOS cloud práve nebeží a heslo sa nedá overiť. Zariadenie ho zobrazí iba teraz a nenecháva si kópiu, o ktorú by prehliadač mohol znova požiadať, preto si ho skopírujte do správcu hesiel alebo si ho vytlačte a hárok odložte.",
    de: "Ab jetzt entsperrt dein Passwort diese Admin-Seiten. Dieser Schlüssel ist der Ersatz: Er öffnet sie auch, während LosOS cloud nicht läuft und das Passwort nicht prüfen kann. Die Box zeigt ihn nur jetzt und behält keine Kopie, die ein Browser noch einmal abfragen könnte, also kopiere ihn in einen Passwortmanager, oder druck ihn aus und heb das Blatt auf.",
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
    en: "The admin pages normally unlock with your password. If LosOS cloud is not running and the password cannot be checked, choose \"Use the spare admin key instead\" and paste this. It was shown once, during setup, and the box will not show it again.",
    sk: "Správcovské stránky sa bežne odomykajú vaším heslom. Ak LosOS cloud nebeží a heslo sa nedá overiť, zvoľte „Použiť namiesto toho náhradný správcovský kľúč“ a vložte tento. Zobrazil sa raz, počas nastavenia, a zariadenie ho znova neukáže.",
    de: "Die Admin-Seiten entsperrst du normalerweise mit deinem Passwort. Wenn LosOS cloud nicht läuft und das Passwort nicht geprüft werden kann, wähl „Stattdessen den Ersatz-Admin-Schlüssel verwenden“ und gib diesen ein. Er wurde einmal gezeigt, bei der Einrichtung, und die Box zeigt ihn nicht noch einmal.",
  },
  "wizard.signin.key.sheetKey": {
    en: "Treat it like a key to the box, because that is what it is.",
    sk: "Zaobchádzajte s ním ako s kľúčom od zariadenia, pretože presne tým je.",
    de: "Behandle ihn wie einen Schlüssel zur Box, denn genau das ist er.",
  },
  "wizard.signin.key.copied": {
    en: "Copied.",
    sk: "Skopírované.",
    de: "Kopiert.",
  },
  "wizard.signin.key.copyFailed": {
    en: "This browser would not copy it. The key is selected; press Ctrl+C, or Cmd+C on a Mac.",
    sk: "Tento prehliadač ho neskopíroval. Kľúč je označený; stlačte Ctrl+C, na Macu Cmd+C.",
    de: "Dieser Browser wollte ihn nicht kopieren. Der Schlüssel ist markiert; drück Strg+C, auf dem Mac Cmd+C.",
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
    en: "A passkey signs you in with your face, your fingerprint or your screen lock, from this browser on this device. It is an addition, not a replacement: the desktop and phone apps still sign in with the password you just set.",
    sk: "Prístupový kľúč vás prihlási tvárou, odtlačkom prsta alebo zámkou obrazovky, a to v tomto prehliadači na tomto prístroji. Je to doplnok, nie náhrada: aplikácie pre počítač a telefón sa naďalej prihlasujú heslom, ktoré ste práve nastavili.",
    de: "Ein Passkey meldet dich mit deinem Gesicht, deinem Fingerabdruck oder deiner Bildschirmsperre an, in diesem Browser auf diesem Gerät. Er kommt dazu, er ersetzt nichts: Die Apps für Computer und Handy melden sich weiter mit dem Passwort an, das du gerade festgelegt hast.",
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
    en: "The software on this box cannot register a passkey yet. The password you set above is enough to sign in; you can add a passkey later, from the security settings inside your files.",
    sk: "Softvér na tomto zariadení zatiaľ nevie zaregistrovať prístupový kľúč. Na prihlásenie stačí heslo, ktoré ste nastavili vyššie; prístupový kľúč môžete pridať neskôr v nastaveniach zabezpečenia vo svojich súboroch.",
    de: "Die Software auf dieser Box kann noch keinen Passkey registrieren. Das Passwort, das du oben festgelegt hast, reicht zum Anmelden; einen Passkey kannst du später in den Sicherheitseinstellungen in deinen Dateien hinzufügen.",
  },
  "wizard.passkey.problem.title": {
    en: "That did not work",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },

  // ── StepRecovery.tsx (hidden step; kept so it can come back) ────────────
  "wizard.recovery.intro": {
    en: "A factory reset or a reinstall erases this box's disk, and it comes back with a brand new identity. To every other box it works with, that looks like a stranger claiming a name they already know, and they refuse it. This code is the only proof that the new box is the old one, so it cannot live on the disk it is meant to recover. Put it on paper, or in a password manager.",
    sk: "Obnovenie továrenských nastavení alebo preinštalovanie vymaže disk tohto zariadenia a to sa vráti s úplne novou identitou. Pre všetky ostatné zariadenia, s ktorými spolupracuje, to vyzerá ako cudzinec, ktorý si nárokuje meno, ktoré už poznajú, a tak ho odmietnu. Tento kód je jediným dôkazom, že nové zariadenie je to staré, preto nemôže byť uložený na disku, ktorý má obnoviť. Zapíšte si ho na papier alebo do správcu hesiel.",
    de: "Ein Zurücksetzen auf Werkseinstellungen oder eine Neuinstallation löscht die Festplatte dieser Box, und sie kommt mit einer ganz neuen Identität zurück. Für jede andere Box, mit der sie zusammenarbeitet, sieht das aus wie ein Fremder, der einen Namen beansprucht, den sie schon kennen, und sie lehnen ihn ab. Dieser Code ist der einzige Beweis, dass die neue Box die alte ist, deshalb kann er nicht auf der Festplatte liegen, die er wiederherstellen soll. Schreib ihn auf Papier oder leg ihn in einem Passwortmanager ab.",
  },
  "wizard.recovery.codeFor": {
    en: "Recovery code for {name}",
    sk: "Kód na obnovenie pre {name}",
    de: "Wiederherstellungscode für {name}",
  },
  "wizard.recovery.notMinted": {
    en: "This box minted it earlier. It is the same code as before, and it will not change.",
    sk: "Toto zariadenie ho vytvorilo už skôr. Je to ten istý kód ako predtým a nezmení sa.",
    de: "Diese Box hat ihn schon früher erstellt. Es ist derselbe Code wie vorher, und er ändert sich nicht.",
  },
  "wizard.recovery.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "wizard.recovery.print": { en: "Print", sk: "Tlačiť", de: "Drucken" },
  "wizard.recovery.saved": {
    en: "Taken off the screen.",
    sk: "Kód máte uložený mimo obrazovky.",
    de: "Vom Bildschirm übernommen.",
  },
  "wizard.recovery.copyFailed.title": {
    en: "This browser would not copy it",
    sk: "Tento prehliadač ho neskopíroval",
    de: "Dieser Browser wollte ihn nicht kopieren",
  },
  "wizard.recovery.copyFailed.body": {
    en: "The code above is selected. Press Ctrl+C, or Cmd+C on a Mac. Printing works either way and counts as saved.",
    sk: "Kód vyššie je označený. Stlačte Ctrl+C, na Macu Cmd+C. Tlač funguje tak či tak a počíta sa ako uloženie.",
    de: "Der Code oben ist markiert. Drück Strg+C, auf dem Mac Cmd+C. Drucken klappt so oder so und zählt als gesichert.",
  },
  "wizard.recovery.notYet.title": {
    en: "No code on this box yet",
    sk: "Na tomto zariadení zatiaľ nie je kód",
    de: "Auf dieser Box gibt es noch keinen Code",
  },
  "wizard.recovery.notYet.body": {
    en: "The software on this box cannot produce a recovery code yet. You can continue without one; come back to this page after the box has updated itself and write the code down then.",
    sk: "Softvér na tomto zariadení zatiaľ nevie vytvoriť kód na obnovenie. Môžete pokračovať bez neho; vráťte sa na túto stránku, keď sa zariadenie samo aktualizuje, a kód si zapíšte potom.",
    de: "Die Software auf dieser Box kann noch keinen Wiederherstellungscode erzeugen. Du kannst ohne weitermachen; komm auf diese Seite zurück, wenn die Box sich aktualisiert hat, und schreib den Code dann auf.",
  },
  "wizard.recovery.failed.title": {
    en: "Could not read the code",
    sk: "Kód sa nepodarilo načítať",
    de: "Der Code konnte nicht gelesen werden",
  },
  "wizard.recovery.retry": {
    en: "Try again",
    sk: "Skúsiť znova",
    de: "Erneut versuchen",
  },
  "wizard.recovery.printed": {
    en: "Printed {date}.",
    sk: "Vytlačené {date}.",
    de: "Gedruckt am {date}.",
  },
  "wizard.recovery.sheetBody": {
    en: "Keep this. If {name} is ever reset or reinstalled, this code is the only proof that the rebuilt box is the same one. Without it the boxes it works with will treat it as a stranger and refuse it. There is no copy anywhere else that survives a reset.",
    sk: "Tento papier si odložte. Ak sa {name} niekedy resetuje alebo preinštaluje, tento kód je jediným dôkazom, že obnovené zariadenie je to isté. Bez neho ho zariadenia, s ktorými spolupracuje, budú považovať za cudzinca a odmietnu ho. Nikde inde neexistuje kópia, ktorá by reset prežila.",
    de: "Heb das auf. Falls {name} jemals zurückgesetzt oder neu installiert wird, ist dieser Code der einzige Beweis, dass die neu aufgesetzte Box dieselbe ist. Ohne ihn behandeln die Boxen, mit denen sie zusammenarbeitet, sie als Fremde und lehnen sie ab. Es gibt nirgendwo sonst eine Kopie, die ein Zurücksetzen übersteht.",
  },
  "wizard.recovery.sheetAdminKey": {
    en: "Spare admin key for {name}",
    sk: "Náhradný správcovský kľúč pre {name}",
    de: "Ersatz-Admin-Schlüssel für {name}",
  },
  "wizard.recovery.sheetAdminKeyBody": {
    en: "The admin pages normally unlock with your password. If LosOS cloud is not running and the password cannot be checked, choose \"Use the spare admin key instead\" and paste this. It was shown once, during setup, and the box will not show it again.",
    sk: "Správcovské stránky sa bežne odomykajú vaším heslom. Ak LosOS cloud nebeží a heslo sa nedá overiť, zvoľte „Použiť namiesto toho náhradný správcovský kľúč“ a vložte tento. Zobrazil sa raz, počas nastavenia, a zariadenie ho znova neukáže.",
    de: "Die Admin-Seiten entsperrst du normalerweise mit deinem Passwort. Wenn LosOS cloud nicht läuft und das Passwort nicht geprüft werden kann, wähl „Stattdessen den Ersatz-Admin-Schlüssel verwenden“ und gib diesen ein. Er wurde einmal gezeigt, bei der Einrichtung, und die Box zeigt ihn nicht noch einmal.",
  },
  "wizard.recovery.sheetKey": {
    en: "Treat it like a key to the box, because that is what it is.",
    sk: "Zaobchádzajte s ním ako s kľúčom od zariadenia, pretože presne tým je.",
    de: "Behandle ihn wie einen Schlüssel zur Box, denn genau das ist er.",
  },

  // ── StepFirstSignIn.tsx ─────────────────────────────────────────────────
  "wizard.first.signedIn.title": {
    en: "You are signed in",
    sk: "Ste prihlásení",
    de: "Du bist angemeldet",
  },
  "wizard.first.signedIn.body": {
    en: "That was the last step. Finish below, or carry on in the window here. Nothing is waiting on you.",
    sk: "To bol posledný krok. Dokončite nastavenie nižšie alebo pokračujte v okne tu. Nič na vás nečaká.",
    de: "Das war der letzte Schritt. Schließ unten ab oder mach im Fenster hier weiter. Nichts wartet auf dich.",
  },
  "wizard.first.intro": {
    en: "Sign in below with the name and password you set. This page notices when you are through and finishes on its own.",
    sk: "Nižšie sa prihláste menom a heslom, ktoré ste nastavili. Táto stránka spozná, keď budete prihlásení, a dokončí sa sama.",
    de: "Melde dich unten mit dem Namen und dem Passwort an, die du festgelegt hast. Diese Seite merkt, wenn du durch bist, und schließt von selbst ab.",
  },
  "wizard.first.introNamed": {
    en: "Sign in below with the name and password you set in step 2. The name is {name}. This page notices when you are through and finishes on its own.",
    sk: "Nižšie sa prihláste menom a heslom, ktoré ste nastavili v kroku 2. Meno je {name}. Táto stránka spozná, keď budete prihlásení, a dokončí sa sama.",
    de: "Melde dich unten mit dem Namen und dem Passwort an, die du in Schritt 2 festgelegt hast. Der Name ist {name}. Diese Seite merkt, wenn du durch bist, und schließt von selbst ab.",
  },
  "wizard.first.starting.title": {
    en: "Your files are still starting",
    sk: "Vaše súbory sa ešte spúšťajú",
    de: "Deine Dateien starten noch",
  },
  "wizard.first.starting.body": {
    en: "The files app has not answered yet. This page keeps asking in the background and the sign-in form appears here on its own. On a new box this takes a few minutes.",
    sk: "Aplikácia so súbormi ešte neodpovedá. Táto stránka sa pýta ďalej na pozadí a prihlasovací formulár sa tu objaví sám. Na novom zariadení to trvá pár minút.",
    de: "Die Dateien-App antwortet noch nicht. Diese Seite fragt im Hintergrund weiter nach, und das Anmeldeformular erscheint hier von selbst. Auf einer neuen Box dauert das ein paar Minuten.",
  },
  "wizard.first.opening": {
    en: "Opening the sign-in page…",
    sk: "Otvára sa prihlasovacia stránka…",
    de: "Die Anmeldeseite wird geöffnet …",
  },
  "wizard.first.lost.title": {
    en: "Cannot follow along",
    sk: "Nedá sa sledovať",
    de: "Kann nicht mitverfolgen",
  },
  "wizard.first.lost.body": {
    en: "The window below went somewhere this page cannot see. Sign in there, then say so with the button underneath.",
    sk: "Okno nižšie prešlo niekam, kam táto stránka nevidí. Prihláste sa tam a potom to potvrďte tlačidlom pod ním.",
    de: "Das Fenster unten ist irgendwohin gewechselt, wo diese Seite nicht hinsieht. Melde dich dort an und sag es dann mit dem Button darunter.",
  },
  "wizard.first.frameTitle": {
    en: "Sign in to your files",
    sk: "Prihlásenie k vašim súborom",
    de: "Bei deinen Dateien anmelden",
  },
  "wizard.first.newTab": {
    en: "Open in a new tab instead",
    sk: "Otvoriť radšej na novej karte",
    de: "Stattdessen in neuem Tab öffnen",
  },
  "wizard.first.manual": {
    en: "I have signed in",
    sk: "Prihlásenie je hotové",
    de: "Ich habe mich angemeldet",
  },
});
