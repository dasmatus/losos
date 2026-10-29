import { defineMessages } from "./define";

/* Editing together (src/cloud/collab/) and sharing with people on other
 * boxes (src/cloud/mesh/), in LosOS Cloud. */
export default defineMessages({
  // ── the editor page ──
  "collab.open": { en: "Edit together", sk: "Upraviť spoločne", de: "Gemeinsam bearbeiten" },
  "collab.back.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "collab.back.shared": { en: "Shared with you", sk: "Zdieľané s vami", de: "Mit dir geteilt" },
  "collab.where.here": { en: "On {box}", sk: "Na {box}", de: "Auf {box}" },
  "collab.where.remote": {
    en: "Shared by {owner} from {box}",
    sk: "Zdieľa {owner} z {box}",
    de: "Geteilt von {owner} aus {box}",
  },
  "collab.status.connecting": { en: "Connecting", sk: "Pripája sa", de: "Verbindet" },
  "collab.status.live": { en: "Live", sk: "Naživo", de: "Live" },
  "collab.status.offline": { en: "Offline", sk: "Offline", de: "Offline" },
  "collab.status.apart": { en: "{box} out of reach", sk: "{box} mimo dosahu", de: "{box} nicht erreichbar" },
  "collab.status.readOnly": { en: "View only", sk: "Len na čítanie", de: "Nur ansehen" },
  "collab.hint.apart": {
    en: "{box} is out of reach. What you type stays on {here} and merges into the file when {box} is back.",
    sk: "{box} je mimo dosahu. Čo napíšete, zostane na {here} a spojí sa so súborom, keď bude {box} späť.",
    de: "{box} ist nicht erreichbar. Was du tippst, bleibt auf {here} und wird in die Datei übernommen, sobald {box} wieder da ist.",
  },
  "collab.hint.offline": {
    en: "This page lost its connection to the box. Keep it open. It reconnects by itself and sends what you typed.",
    sk: "Stránka stratila spojenie so zariadením. Nechajte ju otvorenú: pripojí sa sama a pošle, čo ste napísali.",
    de: "Diese Seite hat die Verbindung zur Box verloren. Lass sie offen: Sie verbindet sich von selbst und sendet, was du getippt hast.",
  },
  "collab.hint.readOnly": {
    en: "{owner} lets you view this file. You see their changes as they type.",
    sk: "{owner} vám tento súbor dovoľuje zobraziť. Zmeny vidíte, ako ich píše.",
    de: "{owner} lässt dich diese Datei ansehen. Du siehst Änderungen, während getippt wird.",
  },
  "collab.edited": {
    en: "Edited by {name} on {box}, {when}",
    sk: "Naposledy upravené: {name} na {box}, {when}",
    de: "Zuletzt bearbeitet von {name} auf {box}, {when}",
  },
  "collab.here": {
    en: { one: "{count} person here", other: "{count} people here" },
    sk: { one: "Tu je {count} človek", few: "Tu sú {count} ľudia", many: "Tu je {count} ľudí", other: "Tu je {count} ľudí" },
    de: { one: "{count} Person hier", other: "{count} Personen hier" },
  },
  "collab.you": { en: "{name} (you)", sk: "{name} (vy)", de: "{name} (du)" },
  "collab.share": { en: "Share", sk: "Zdieľať", de: "Teilen" },
  "collab.download": { en: "Download", sk: "Stiahnuť", de: "Herunterladen" },
  "collab.label": { en: "{name}, edited together", sk: "{name}, spoločná úprava", de: "{name}, gemeinsam bearbeitet" },
  "collab.placeholder": {
    en: "Start typing. Everyone in this file sees it as you type.",
    sk: "Začnite písať. Každý v tomto súbore to vidí hneď.",
    de: "Fang an zu tippen. Alle in dieser Datei sehen es sofort.",
  },
  "collab.loading": { en: "Opening the file", sk: "Otvára sa súbor", de: "Datei wird geöffnet" },
  "collab.error.title": {
    en: "This file cannot be edited together",
    sk: "Tento súbor sa nedá upravovať spoločne",
    de: "Diese Datei kann nicht gemeinsam bearbeitet werden",
  },
  "collab.error.notText": {
    en: "Editing together here works for text files only. Office documents open in LosOS Office.",
    sk: "Spoločne sa tu upravujú len textové súbory. Dokumenty Office sa otvárajú v LosOS Office.",
    de: "Gemeinsam bearbeitet werden hier nur Textdateien. Office-Dokumente öffnen sich in LosOS Office.",
  },
  "collab.error.tooLarge": {
    en: "The file is larger than 2 MB, too large to edit together.",
    sk: "Súbor má viac ako 2 MB, na spoločnú úpravu je príliš veľký.",
    de: "Die Datei ist größer als 2 MB und zu groß zum gemeinsamen Bearbeiten.",
  },
  "collab.error.notFound": {
    en: "The file is gone, or it is not shared with you any more.",
    sk: "Súbor už neexistuje alebo už nie je zdieľaný s vami.",
    de: "Die Datei ist weg oder nicht mehr mit dir geteilt.",
  },
  "collab.error.unreachable": {
    en: "{box} is out of reach, and this file was never opened here before. Try again when it is back.",
    sk: "{box} je mimo dosahu a tento súbor tu ešte nebol otvorený. Skúste to znova, keď bude späť.",
    de: "{box} ist nicht erreichbar, und diese Datei wurde hier noch nie geöffnet. Versuch es wieder, sobald es zurück ist.",
  },
  "collab.when.now": { en: "just now", sk: "práve teraz", de: "gerade eben" },

  // ── sharing with someone on another box ──
  "mesh.share.title": { en: "Share {name}", sk: "Zdieľať {name}", de: "{name} teilen" },
  "mesh.share.lead": {
    en: "With someone on another box. They find it under Shared with you on their box, and what either of you types shows up for both.",
    sk: "S niekým na inom zariadení. Nájde ho v časti Zdieľané s vami na svojom zariadení a čo napíše jeden z vás, uvidia obaja.",
    de: "Mit jemandem auf einer anderen Box. Die Person findet es unter Mit dir geteilt auf ihrer Box, und was einer von euch tippt, sehen beide.",
  },
  "mesh.share.to.label": { en: "Someone on another box", sk: "Niekto na inom zariadení", de: "Jemand auf einer anderen Box" },
  "mesh.share.to.placeholder": { en: "name@box", sk: "meno@zariadenie", de: "name@box" },
  "mesh.share.to.hint": {
    en: "Their LosOS Login name and their box's address, like eva@kitchenbox.local.",
    sk: "Ich meno v LosOS Login a adresa ich zariadenia, napríklad eva@kitchenbox.local.",
    de: "Ihr Name bei LosOS Login und die Adresse ihrer Box, etwa eva@kitchenbox.local.",
  },
  "mesh.share.to.bad": {
    en: "Write it as name@box, for example eva@kitchenbox.local.",
    sk: "Napíšte to ako meno@zariadenie, napríklad eva@kitchenbox.local.",
    de: "Schreib es als name@box, zum Beispiel eva@kitchenbox.local.",
  },
  "mesh.share.to.self": {
    en: "That address is on this box.",
    sk: "Táto adresa je na tomto zariadení.",
    de: "Diese Adresse ist auf dieser Box.",
  },
  "mesh.perm.edit": { en: "Can edit", sk: "Môže upravovať", de: "Kann bearbeiten" },
  "mesh.perm.read": { en: "Can view", sk: "Môže zobraziť", de: "Kann ansehen" },
  "mesh.perm.label": { en: "What they can do", sk: "Čo môže robiť", de: "Was die Person tun kann" },
  "mesh.perm.labelFor": { en: "What {who} can do", sk: "Čo môže robiť {who}", de: "Was {who} tun kann" },
  "mesh.share.submit": { en: "Share", sk: "Zdieľať", de: "Teilen" },
  "mesh.share.with": { en: "On other boxes", sk: "Na iných zariadeniach", de: "Auf anderen Boxen" },
  "mesh.share.none": {
    en: "Not shared with anyone on another box yet.",
    sk: "Zatiaľ nie je zdieľané s nikým na inom zariadení.",
    de: "Noch mit niemandem auf einer anderen Box geteilt.",
  },
  "mesh.state.unsent": {
    en: "Not delivered yet, tries again",
    sk: "Zatiaľ nedoručené, skúša sa znova",
    de: "Noch nicht zugestellt, wird erneut versucht",
  },
  "mesh.state.pending": { en: "Waiting for them to accept", sk: "Čaká na prijatie", de: "Wartet auf Annahme" },
  "mesh.state.accepted": { en: "Accepted", sk: "Prijaté", de: "Angenommen" },
  "mesh.state.declined": { en: "Declined", sk: "Odmietnuté", de: "Abgelehnt" },
  "mesh.share.remove": { en: "Stop sharing with {who}", sk: "Prestať zdieľať s {who}", de: "Nicht mehr mit {who} teilen" },
  "mesh.share.done": { en: "Shared with {who}", sk: "Zdieľané s {who}", de: "Mit {who} geteilt" },
  "mesh.share.failed": { en: "Could not share", sk: "Zdieľanie sa nepodarilo", de: "Teilen fehlgeschlagen" },
  "mesh.share.close": { en: "Done", sk: "Hotovo", de: "Fertig" },
  "mesh.share.mine": {
    en: "Your address for people on other boxes: {address}",
    sk: "Vaša adresa pre ľudí na iných zariadeniach: {address}",
    de: "Deine Adresse für Menschen auf anderen Boxen: {address}",
  },
  "mesh.share.off": {
    en: "Sharing with other boxes is off on this box. It comes on when the box shares its storage with a mesh.",
    sk: "Zdieľanie s inými zariadeniami je na tomto zariadení vypnuté. Zapne sa, keď zariadenie zdieľa úložisko so sieťou.",
    de: "Teilen mit anderen Boxen ist auf dieser Box aus. Es geht an, sobald die Box ihren Speicher mit einem Mesh teilt.",
  },

  // ── shared with you ──
  "mesh.shared.title": { en: "Shared with you", sk: "Zdieľané s vami", de: "Mit dir geteilt" },
  "mesh.shared.lead": {
    en: "Files and folders people on other boxes shared with you. They stay on their box; you open and edit them from here.",
    sk: "Súbory a priečinky, ktoré s vami zdieľajú ľudia na iných zariadeniach. Zostávajú na ich zariadení, vy ich otvárate a upravujete odtiaľto.",
    de: "Dateien und Ordner, die Menschen auf anderen Boxen mit dir geteilt haben. Sie bleiben auf deren Box; du öffnest und bearbeitest sie von hier.",
  },
  "mesh.shared.off": {
    en: "Sharing with other boxes is off",
    sk: "Zdieľanie s inými zariadeniami je vypnuté",
    de: "Teilen mit anderen Boxen ist aus",
  },
  "mesh.shared.empty.title": { en: "Nothing shared with you yet", sk: "Zatiaľ s vami nič nezdieľajú", de: "Noch nichts mit dir geteilt" },
  "mesh.shared.empty.body": {
    en: "When someone on another box shares a file with {address}, it shows up here.",
    sk: "Keď niekto na inom zariadení zdieľa súbor s {address}, objaví sa tu.",
    de: "Wenn jemand auf einer anderen Box eine Datei mit {address} teilt, erscheint sie hier.",
  },
  "mesh.shared.from": { en: "From {name}, {address}", sk: "Od {name}, {address}", de: "Von {name}, {address}" },
  "mesh.shared.accept": { en: "Accept", sk: "Prijať", de: "Annehmen" },
  "mesh.shared.decline": { en: "Decline", sk: "Odmietnuť", de: "Ablehnen" },
  "mesh.shared.new": { en: "New", sk: "Nové", de: "Neu" },
  "mesh.shared.viewOnly": { en: "View only", sk: "Len na čítanie", de: "Nur ansehen" },
  "mesh.shared.canEdit": { en: "Can edit", sk: "Môžete upravovať", de: "Bearbeitbar" },
  "mesh.shared.unsynced": {
    en: "Changes waiting for {box}",
    sk: "Zmeny čakajú na {box}",
    de: "Änderungen warten auf {box}",
  },
  "mesh.shared.declinedNote": { en: "Declined", sk: "Odmietnuté", de: "Abgelehnt" },
  "mesh.shared.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "mesh.shared.back": { en: "All shares", sk: "Všetky zdieľania", de: "Alle Freigaben" },
  "mesh.shared.emptyFolder": { en: "This folder is empty.", sk: "Tento priečinok je prázdny.", de: "Dieser Ordner ist leer." },
  "mesh.shared.failed": { en: "Could not answer the share", sk: "Na zdieľanie sa nepodarilo odpovedať", de: "Die Freigabe konnte nicht beantwortet werden" },
  "mesh.shared.accepted": { en: "{name} is in Shared with you", sk: "{name} je v časti Zdieľané s vami", de: "{name} ist unter Mit dir geteilt" },
  "mesh.error.badAddress": {
    en: "That is not an address like name@box.",
    sk: "To nie je adresa v tvare meno@zariadenie.",
    de: "Das ist keine Adresse der Form name@box.",
  },
  "mesh.error.forbidden": {
    en: "Only the person whose file it is can share it.",
    sk: "Zdieľať ho môže len ten, komu súbor patrí.",
    de: "Nur wem die Datei gehört, kann sie teilen.",
  },
  "mesh.error.unreachable": {
    en: "The other box is out of reach right now. Try again when it is back.",
    sk: "Druhé zariadenie je teraz mimo dosahu. Skúste to znova, keď bude späť.",
    de: "Die andere Box ist gerade nicht erreichbar. Versuch es wieder, sobald sie zurück ist.",
  },
  "mesh.error.keyChanged": {
    en: "The other box answered with a different key than before, so nothing was sent. Ask its owner whether they reinstalled it.",
    sk: "Druhé zariadenie odpovedalo iným kľúčom ako predtým, preto sa nič neposlalo. Spýtajte sa jeho majiteľa, či ho preinštaloval.",
    de: "Die andere Box hat mit einem anderen Schlüssel als zuvor geantwortet, deshalb wurde nichts gesendet. Frag ihre Besitzer, ob sie neu installiert wurde.",
  },
});
