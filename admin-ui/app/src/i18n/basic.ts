import { defineMessages } from "./define";

/* The pages for a browser with JavaScript off (/basic/, drawn on the box by
 * backend/plugins/basic/). They use the rest of the catalogue as it is and
 * need these few words of their own: the sign-in steps a script page leaves
 * to LosOS Login's own page, and what a page says while it is still only in
 * the script version. */
export default defineMessages({
  "basic.signIn.hint": {
    en: "Sign in with your name and password. Each person has their own, and what you can change here depends on yours.",
    sk: "Prihláste sa svojím menom a heslom. Každý má vlastné a to, čo tu môžete meniť, závisí od vášho účtu.",
    de: "Melde dich mit deinem Namen und Passwort an. Jede Person hat ein eigenes, und was du hier ändern kannst, hängt von deinem ab.",
  },
  "basic.signIn.wrong": {
    en: "That name and password do not match.",
    sk: "Toto meno a heslo k sebe nepatria.",
    de: "Name und Passwort passen nicht zusammen.",
  },
  "basic.code.title": { en: "Enter your code", sk: "Zadajte kód", de: "Gib deinen Code ein" },
  "basic.code.hint": {
    en: "Your account asks for a second step. Enter the six digits your authenticator app shows now.",
    sk: "Váš účet žiada druhý krok. Zadajte šesť číslic, ktoré teraz ukazuje vaša overovacia aplikácia.",
    de: "Dein Konto verlangt einen zweiten Schritt. Gib die sechs Ziffern ein, die deine Authenticator-App gerade zeigt.",
  },
  "basic.code.label": { en: "Six-digit code", sk: "Šesťmiestny kód", de: "Sechsstelliger Code" },
  "basic.code.go": { en: "Continue", sk: "Pokračovať", de: "Weiter" },
  "basic.code.wrong": {
    en: "That code did not work. Codes change every 30 seconds; try the one shown now.",
    sk: "Tento kód nefungoval. Kódy sa menia každých 30 sekúnd; skúste ten, ktorý sa zobrazuje teraz.",
    de: "Dieser Code hat nicht funktioniert. Codes wechseln alle 30 Sekunden; versuche den, der jetzt angezeigt wird.",
  },
  "basic.passkey.title": { en: "This account uses a passkey", sk: "Tento účet používa prístupový kľúč", de: "Dieses Konto nutzt einen Passkey" },
  "basic.passkey.body": {
    en: "A passkey needs JavaScript in the browser. Turn it on for this box, or sign in with the spare admin key.",
    sk: "Prístupový kľúč potrebuje v prehliadači JavaScript. Zapnite ho pre toto zariadenie alebo sa prihláste náhradným správcovským kľúčom.",
    de: "Ein Passkey braucht JavaScript im Browser. Schalte es für diese Box ein oder melde dich mit dem Ersatz-Admin-Schlüssel an.",
  },
  "basic.passkey.back": { en: "Back to sign-in", sk: "Späť na prihlásenie", de: "Zurück zur Anmeldung" },
  "basic.key.remembered": {
    en: "This browser keeps the key until it is closed.",
    sk: "Tento prehliadač si kľúč pamätá, kým ho nezatvoríte.",
    de: "Dieser Browser behält den Schlüssel, bis er geschlossen wird.",
  },
  "basic.later.summary": {
    en: "This page needs JavaScript for now.",
    sk: "Táto stránka zatiaľ potrebuje JavaScript.",
    de: "Diese Seite braucht vorerst JavaScript.",
  },
  "basic.later.title": {
    en: "Not in the version without JavaScript yet",
    sk: "Vo verzii bez JavaScriptu zatiaľ nie je",
    de: "In der Version ohne JavaScript noch nicht da",
  },
  "basic.later.body": {
    en: "Turn JavaScript on for this box to use this page. Everything else in the sidebar that opens here works without it.",
    sk: "Ak chcete použiť túto stránku, zapnite pre toto zariadenie JavaScript. Všetko ostatné v bočnom paneli, čo sa otvorí tu, funguje aj bez neho.",
    de: "Schalte JavaScript für diese Box ein, um diese Seite zu nutzen. Alles andere in der Seitenleiste, das sich hier öffnet, geht auch ohne.",
  },
  "basic.notFound.title": { en: "There is no such page", sk: "Taká stránka neexistuje", de: "Diese Seite gibt es nicht" },
  "basic.notFound.body": {
    en: "The address may be mistyped, or the page is not open to your account.",
    sk: "Adresa je možno preklepnutá alebo stránka nie je prístupná vášmu účtu.",
    de: "Die Adresse ist vielleicht vertippt, oder die Seite ist für dein Konto nicht offen.",
  },
  "basic.search.summary": {
    en: "Find a page of this box by its name.",
    sk: "Nájdite stránku tohto zariadenia podľa názvu.",
    de: "Finde eine Seite dieser Box über ihren Namen.",
  },
  "basic.search.noneBody": {
    en: "Try a shorter word. With JavaScript on, the search also looks through files, repositories and issues.",
    sk: "Skúste kratšie slovo. Súbory, repozitáre a úlohy sa hľadajú so zapnutým JavaScriptom.",
    de: "Versuch ein kürzeres Wort. Dateien, Repositorys und Issues werden mit eingeschaltetem JavaScript durchsucht.",
  },
  "basic.look.saveVeil": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "basic.look.upload": {
    en: "A picture of your own is uploaded with JavaScript on; one you uploaded stays here.",
    sk: "Vlastný obrázok sa nahráva so zapnutým JavaScriptom; ten, ktorý ste už nahrali, tu zostáva.",
    de: "Ein eigenes Bild lädst du mit eingeschaltetem JavaScript hoch; eins, das du hochgeladen hast, bleibt hier.",
  },
});
