import { defineMessages } from "./define";

/* The Credentials pane (screens/settings/pane-credentials.tsx): every secret
 * the box keeps, sealed to this box, listed without its value. The pane's
 * name, summary and search words live under settings.panes.credentials.*
 * with the other panes'. */

export default defineMessages({
  "credentials.loading": {
    en: "Loading the credentials…",
    sk: "Načítavam prihlasovacie údaje…",
    de: "Lade die Zugangsdaten…",
  },
  "credentials.unavailable": {
    en: "This box does not keep its credentials in a credential store yet.",
    sk: "Toto zariadenie zatiaľ neuchováva prihlasovacie údaje v úložisku údajov.",
    de: "Diese Box bewahrt ihre Zugangsdaten noch nicht in einem Zugangsdatenspeicher auf.",
  },
  "credentials.failed": {
    en: "Could not load the credentials.",
    sk: "Prihlasovacie údaje sa nepodarilo načítať.",
    de: "Die Zugangsdaten konnten nicht geladen werden.",
  },
  "credentials.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── What they are sealed to ─────────────────────────────────────────────
  "credentials.seal.title": { en: "Sealed to this box", sk: "Zapečatené pre toto zariadenie", de: "An diese Box gebunden" },
  "credentials.seal.tpm": {
    en: "Sealed to this box's TPM",
    sk: "Zapečatené čipom TPM tohto zariadenia",
    de: "Mit dem TPM dieser Box versiegelt",
  },
  "credentials.seal.tpmDetail": {
    en: "{count} secrets, each encrypted with a key that never leaves the chip. A copy of the disk opens none of them anywhere else.",
    sk: "Tajomstvá: {count}. Každé je zašifrované kľúčom, ktorý nikdy neopustí čip. Kópia disku žiadne z nich inde neotvorí.",
    de: "{count} Geheimnisse, jedes mit einem Schlüssel verschlüsselt, der den Chip nie verlässt. Eine Kopie der Festplatte öffnet woanders keines davon.",
  },
  "credentials.seal.host": {
    en: "Sealed with this box's own key",
    sk: "Zapečatené vlastným kľúčom zariadenia",
    de: "Mit dem eigenen Schlüssel dieser Box versiegelt",
  },
  "credentials.seal.hostDetail": {
    en: "{count} secrets, each encrypted with a key only the system can read. This box has no TPM, so that key is on the same disk.",
    sk: "Tajomstvá: {count}. Každé je zašifrované kľúčom, ktorý prečíta iba systém. Toto zariadenie nemá TPM, takže kľúč je na tom istom disku.",
    de: "{count} Geheimnisse, jedes mit einem Schlüssel verschlüsselt, den nur das System lesen kann. Diese Box hat kein TPM, daher liegt der Schlüssel auf derselben Festplatte.",
  },
  "credentials.seal.caption": {
    en: "Each service gets only its own secrets, opened in memory as it starts. No value is ever shown here.",
    sk: "Každá služba dostane iba svoje tajomstvá, otvorené v pamäti pri spustení. Žiadna hodnota sa tu nikdy nezobrazí.",
    de: "Jeder Dienst bekommt nur seine eigenen Geheimnisse, beim Start im Arbeitsspeicher geöffnet. Hier wird nie ein Wert angezeigt.",
  },
  "credentials.seal.more": { en: "How it works", sk: "Ako to funguje", de: "So funktioniert es" },

  // ── Badges and facts ────────────────────────────────────────────────────
  "credentials.badge.tpm": { en: "TPM", sk: "TPM", de: "TPM" },
  "credentials.badge.host": { en: "Box key", sk: "Kľúč zariadenia", de: "Box-Schlüssel" },
  "credentials.badge.box": { en: "Made by the box", sk: "Vytvorilo zariadenie", de: "Von der Box erzeugt" },
  "credentials.badge.owner": { en: "Set by you", sk: "Nastavili ste vy", de: "Von dir gesetzt" },
  "credentials.badge.notSet": { en: "Not set", sk: "Nenastavené", de: "Nicht gesetzt" },
  "credentials.badge.adopted": {
    en: "Moved from a plain file",
    sk: "Presunuté z otvoreného súboru",
    de: "Aus einer Klartextdatei übernommen",
  },
  "credentials.badge.remade": {
    en: "Made again at boot",
    sk: "Pri štarte vytvorené znova",
    de: "Beim Start neu erzeugt",
  },
  "credentials.changed": { en: "Changed {date}", sk: "Zmenené {date}", de: "Geändert am {date}" },
  "credentials.readBy": { en: "Read by {units}", sk: "Číta {units}", de: "Gelesen von {units}" },
  "credentials.change": { en: "Change", sk: "Zmeniť", de: "Ändern" },
  "credentials.setUp": { en: "Set up", sk: "Nastaviť", de: "Einrichten" },

  // ── A new value ─────────────────────────────────────────────────────────
  "credentials.rotate.button": { en: "Make new", sk: "Vytvoriť nový", de: "Neu erzeugen" },
  "credentials.rotate.title": {
    en: "Make a new value for {name}?",
    sk: "Vytvoriť nový údaj: {name}?",
    de: "Neu erzeugen: {name}?",
  },
  "credentials.rotate.effect": {
    en: "The box makes a new value, seals it and restarts what reads it.",
    sk: "Zariadenie vytvorí novú hodnotu, zapečatí ju a reštartuje to, čo ju číta.",
    de: "Die Box erzeugt einen neuen Wert, versiegelt ihn und startet neu, was ihn liest.",
  },
  "credentials.rotate.keep": { en: "Keep it", sk: "Ponechať", de: "Behalten" },
  "credentials.rotate.confirm": { en: "Make a new one", sk: "Vytvoriť nový", de: "Neu erzeugen" },
  "credentials.rotated": {
    en: "Made a new value for {name}.",
    sk: "Vytvorený nový údaj: {name}.",
    de: "Neu erzeugt: {name}.",
  },
  "credentials.rotateFailed": {
    en: "The new value could not be made.",
    sk: "Novú hodnotu sa nepodarilo vytvoriť.",
    de: "Der neue Wert konnte nicht erzeugt werden.",
  },

  // ── The groups ──────────────────────────────────────────────────────────
  "credentials.app.admin": { en: "This box", sk: "Toto zariadenie", de: "Diese Box" },
  "credentials.app.login": { en: "LosOS Login", sk: "LosOS Login", de: "LosOS Login" },
  "credentials.app.mail": { en: "LosOS Mail", sk: "LosOS Mail", de: "LosOS Mail" },
  "credentials.app.backup": { en: "Backups", sk: "Zálohy", de: "Sicherungen" },
  "credentials.app.market": { en: "Market", sk: "Trh", de: "Markt" },

  // ── Each credential ─────────────────────────────────────────────────────
  "credentials.name.adminToken.title": { en: "Spare key", sk: "Náhradný kľúč", de: "Ersatzschlüssel" },
  "credentials.name.adminToken.detail": {
    en: "Opens the admin pages when LosOS Login cannot. You saw it once, when you set up the box.",
    sk: "Otvorí stránky správy, keď LosOS Login nemôže. Videli ste ho raz, pri nastavení zariadenia.",
    de: "Öffnet die Verwaltung, wenn LosOS Login es nicht kann. Du hast ihn einmal gesehen, beim Einrichten der Box.",
  },
  "credentials.name.adminToken.rotate": {
    en: "The spare key you wrote down stops working.",
    sk: "Náhradný kľúč, ktorý ste si zapísali, prestane fungovať.",
    de: "Der Ersatzschlüssel, den du dir notiert hast, funktioniert dann nicht mehr.",
  },
  "credentials.name.autheliaJwt.title": {
    en: "Confirmation links",
    sk: "Potvrdzovacie odkazy",
    de: "Bestätigungslinks",
  },
  "credentials.name.autheliaJwt.detail": {
    en: "Signs the links LosOS Login uses to confirm it is you, for example when you add a passkey.",
    sk: "Podpisuje odkazy, ktorými LosOS Login overuje, že ste to vy, napríklad pri pridaní prístupového kľúča.",
    de: "Signiert die Links, mit denen LosOS Login bestätigt, dass du es bist, etwa wenn du einen Passkey hinzufügst.",
  },
  "credentials.name.autheliaJwt.rotate": {
    en: "A confirmation link that is still open stops working. LosOS Login restarts, which takes a few seconds.",
    sk: "Potvrdzovací odkaz, ktorý je ešte otvorený, prestane fungovať. LosOS Login sa reštartuje, čo trvá pár sekúnd.",
    de: "Ein Bestätigungslink, der noch offen ist, funktioniert dann nicht mehr. LosOS Login startet neu, das dauert ein paar Sekunden.",
  },
  "credentials.name.autheliaSession.title": { en: "Sessions", sk: "Relácie", de: "Sitzungen" },
  "credentials.name.autheliaSession.detail": {
    en: "Protects everyone's sign-in to this box.",
    sk: "Chráni prihlásenie každého do tohto zariadenia.",
    de: "Schützt die Anmeldung aller an dieser Box.",
  },
  "credentials.name.autheliaSession.rotate": {
    en: "Everyone is signed out, you too, and signs in again.",
    sk: "Všetci budú odhlásení, aj vy, a prihlásia sa znova.",
    de: "Alle werden abgemeldet, du auch, und melden sich neu an.",
  },
  "credentials.name.autheliaStorage.title": {
    en: "Second factors",
    sk: "Druhé faktory",
    de: "Zweite Faktoren",
  },
  "credentials.name.autheliaStorage.detail": {
    en: "Encrypts the passkeys and authenticator codes people set up. It cannot change without losing them.",
    sk: "Šifruje prístupové kľúče a overovacie kódy, ktoré si ľudia nastavili. Nedá sa zmeniť bez ich straty.",
    de: "Verschlüsselt die Passkeys und Authenticator-Codes, die Personen eingerichtet haben. Ändern geht nicht, ohne sie zu verlieren.",
  },
  "credentials.name.autheliaStorage.rotate": {
    en: "Everyone's second factors would be lost.",
    sk: "Druhé faktory všetkých by sa stratili.",
    de: "Die zweiten Faktoren aller gingen verloren.",
  },
  "credentials.name.mailMaster.title": { en: "Mail access", sk: "Prístup k pošte", de: "Mail-Zugang" },
  "credentials.name.mailMaster.detail": {
    en: "Lets the box open each person's mailbox for them, once they are signed in.",
    sk: "Umožňuje zariadeniu otvoriť každému jeho schránku, keď je prihlásený.",
    de: "Damit öffnet die Box jeder Person ihr Postfach, sobald sie angemeldet ist.",
  },
  "credentials.name.mailMaster.rotate": {
    en: "LosOS Mail and the admin pages restart.",
    sk: "LosOS Mail a stránky správy sa reštartujú.",
    de: "LosOS Mail und die Verwaltung starten neu.",
  },
  "credentials.name.marketEscrowKey.title": {
    en: "Escrow key",
    sk: "Kľúč úschovy",
    de: "Treuhandschlüssel",
  },
  "credentials.name.marketEscrowKey.detail": {
    en: "Agrees, with the edge, to pay out what this box sold and to give back what it bought. Without it a held payment waits for its deadline.",
    sk: "Spolu s edge súhlasí s vyplatením toho, čo zariadenie predalo, a s vrátením toho, čo kúpilo. Bez neho držaná platba čaká na svoj termín.",
    de: "Stimmt zusammen mit der Edge zu, dass Verkauftes ausgezahlt und Gekauftes zurückgegeben wird. Ohne ihn wartet eine gehaltene Zahlung auf ihre Frist.",
  },
  "credentials.name.marketEscrowKey.rotate": {
    en: "Payments still held keep waiting on the old key.",
    sk: "Platby, ktoré sú ešte držané, ďalej čakajú na starý kľúč.",
    de: "Noch gehaltene Zahlungen warten weiter auf den alten Schlüssel.",
  },
  "credentials.name.backupBucket.title": {
    en: "Bucket keys",
    sk: "Kľúče úložiska",
    de: "Bucket-Schlüssel",
  },
  "credentials.name.backupBucket.detail": {
    en: "The access key and secret of the bucket backups go to. You set them on the Backup pane.",
    sk: "Prístupový kľúč a tajomstvo úložiska, kam idú zálohy. Nastavujete ich v časti Záloha.",
    de: "Zugriffsschlüssel und Geheimnis des Buckets, in den die Sicherungen gehen. Du setzt sie im Bereich Sicherung.",
  },
  "credentials.name.backupBucket.rotate": {
    en: "Change them on the Backup pane.",
    sk: "Zmeňte ich v časti Záloha.",
    de: "Ändere sie im Bereich Sicherung.",
  },
  "credentials.name.autheliaOidcHmac.title": {
    en: "Companion tokens",
    sk: "Tokeny pomocníka",
    de: "Tokens des Begleiters",
  },
  "credentials.name.autheliaOidcHmac.detail": {
    en: "Signs the tokens LosOS Login gives the AI agent companion. A new one would sign every companion out.",
    sk: "Podpisuje tokeny, ktoré LosOS Login dáva pomocníkovi pre nástroje AI. Nový by odhlásil každého pomocníka.",
    de: "Signiert die Tokens, die LosOS Login dem Begleiter für KI-Werkzeuge gibt. Ein neues würde jeden Begleiter abmelden.",
  },
  "credentials.name.autheliaOidcHmac.rotate": {
    en: "Every AI agent companion is signed out.",
    sk: "Každý pomocník pre nástroje AI sa odhlási.",
    de: "Jeder Begleiter für KI-Werkzeuge wird abgemeldet.",
  },
  "credentials.name.autheliaOidcJwks.title": {
    en: "Signing key",
    sk: "Podpisový kľúč",
    de: "Signaturschlüssel",
  },
  "credentials.name.autheliaOidcJwks.detail": {
    en: "The private key LosOS Login signs who someone is with, for the companion.",
    sk: "Súkromný kľúč, ktorým LosOS Login pre pomocníka podpisuje, kto kto je.",
    de: "Der private Schlüssel, mit dem LosOS Login für den Begleiter signiert, wer jemand ist.",
  },
  "credentials.name.autheliaOidcJwks.rotate": {
    en: "Every AI agent companion is signed out.",
    sk: "Každý pomocník pre nástroje AI sa odhlási.",
    de: "Jeder Begleiter für KI-Werkzeuge wird abgemeldet.",
  },
  "credentials.name.oidcLososd.title": {
    en: "Admin pages' own key",
    sk: "Vlastný kľúč stránok správy",
    de: "Eigener Schlüssel der Verwaltung",
  },
  "credentials.name.oidcLososd.detail": {
    en: "How the admin pages ask LosOS Login whether a companion's token is still good. Made again on its own when needed.",
    sk: "Ním sa stránky správy pýtajú LosOS Login, či je token pomocníka ešte platný. V prípade potreby sa vytvorí znova sám.",
    de: "Damit fragt die Verwaltung LosOS Login, ob das Token eines Begleiters noch gilt. Wird bei Bedarf von selbst neu erzeugt.",
  },
  "credentials.name.oidcLososd.rotate": {
    en: "The admin pages restart.",
    sk: "Stránky správy sa reštartujú.",
    de: "Die Verwaltung startet neu.",
  },
});
