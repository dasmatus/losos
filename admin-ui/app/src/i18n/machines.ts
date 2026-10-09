import { defineMessages } from "./define";

/* The Machines pane (screens/settings/pane-machines.tsx): virtual machines
 * on the mesh, sold by the replica through the market. Words it shares with
 * the market pane (payouts, the payment tab) stay under panes.market.*. */

export default defineMessages({
  "machines.loading": {
    en: "Asking the edge what it runs…",
    sk: "Zisťujem, čo okrajový server spúšťa…",
    de: "Frage den Edge, was er betreibt…",
  },
  "machines.failed": {
    en: "The machines could not be loaded.",
    sk: "Virtuálne stroje sa nepodarilo načítať.",
    de: "Die Maschinen konnten nicht geladen werden.",
  },
  "machines.refresh": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "machines.actionFailedTitle": {
    en: "That did not go through",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },
  "machines.actionFailed": {
    en: "The edge did not answer. Try again in a moment.",
    sk: "Okrajový server neodpovedal. Skúste to o chvíľu znova.",
    de: "Der Edge hat nicht geantwortet. Versuch es gleich noch einmal.",
  },

  // ── Not offered ─────────────────────────────────────────────────────────
  "machines.unavailable.notSharing.title": {
    en: "Machines are for boxes that share their disk",
    sk: "Virtuálne stroje sú pre zariadenia, ktoré zdieľajú svoj disk",
    de: "Maschinen gibt es für Boxen, die ihre Festplatte teilen",
  },
  "machines.unavailable.notSharing.detail": {
    en: "This box does not share its disk with the mesh, so it can neither rent virtual machines nor host them.",
    sk: "Toto zariadenie nezdieľa svoj disk so sieťou mesh, takže si virtuálne stroje nemôže prenajať ani ich hostiť.",
    de: "Diese Box teilt ihre Festplatte nicht mit dem Mesh. Sie kann deshalb weder Maschinen mieten noch welche betreiben.",
  },
  "machines.unavailable.notSharing.planned": {
    en: "Disk sharing opens together with the market.",
    sk: "Zdieľanie disku sa otvorí spolu s trhom.",
    de: "Die Festplattenfreigabe öffnet zusammen mit dem Markt.",
  },
  "machines.unavailable.notSharing.open": {
    en: "Share the disk",
    sk: "Zdieľať disk",
    de: "Festplatte teilen",
  },
  "machines.unavailable.noOfficialEdge.title": {
    en: "No LosOS edge in reach",
    sk: "Žiadny okrajový server LosOS v dosahu",
    de: "Kein LosOS-Edge erreichbar",
  },
  "machines.unavailable.noOfficialEdge.detail": {
    en: "Machines are sold only through an edge that LosOS runs, and none of the edges this box can reach is one.",
    sk: "Virtuálne stroje sa predávajú len cez okrajový server, ktorý prevádzkuje LosOS, a žiadny z dosiahnuteľných ním nie je.",
    de: "Maschinen werden nur über einen Edge verkauft, den LosOS betreibt, und keiner der erreichbaren Edges ist einer.",
  },
  "machines.unavailable.notOffered.title": {
    en: "This edge runs no machines",
    sk: "Tento okrajový server nespúšťa virtuálne stroje",
    de: "Dieser Edge betreibt keine Maschinen",
  },
  "machines.unavailable.notOffered.detail": {
    en: "The edge this box uses does not run virtual machines on its mesh.",
    sk: "Okrajový server, ktorý toto zariadenie používa, nespúšťa vo svojej sieti mesh virtuálne stroje.",
    de: "Der Edge dieser Box betreibt auf seinem Mesh keine virtuellen Maschinen.",
  },

  // ── Start a machine ─────────────────────────────────────────────────────
  "machines.start.title": { en: "Start a machine", sk: "Spustiť stroj", de: "Maschine starten" },
  "machines.start.image": { en: "System", sk: "Systém", de: "System" },
  "machines.start.shape": {
    en: "{cpu} vCPU, {memory} GiB memory, {disk} GiB disk per replica",
    sk: "{cpu} vCPU, {memory} GiB pamäte, {disk} GiB disku na repliku",
    de: "{cpu} vCPU, {memory} GiB Arbeitsspeicher, {disk} GiB Festplatte pro Replikat",
  },
  "machines.start.offered": { en: "Ready to start", sk: "Pripravené na spustenie", de: "Startbereit" },
  "machines.start.own": { en: "Your images", sk: "Vaše obrazy", de: "Deine Images" },
  "machines.start.name": { en: "Name", sk: "Názov", de: "Name" },
  "machines.start.namePlaceholder": { en: "web server", sk: "webový server", de: "Webserver" },
  "machines.start.nameInvalid": {
    en: "Give it a name of up to 40 characters.",
    sk: "Zadajte názov s najviac 40 znakmi.",
    de: "Gib einen Namen mit höchstens 40 Zeichen ein.",
  },
  "machines.start.host": { en: "Runs on", sk: "Beží na", de: "Läuft auf" },
  "machines.start.noHosts": {
    en: "No box is offering to host machines right now.",
    sk: "Momentálne žiadne zariadenie neponúka hosťovanie strojov.",
    de: "Gerade bietet keine Box an, Maschinen zu betreiben.",
  },
  "machines.start.offer": {
    en: { one: "{price} per replica-month, {count} free", other: "{price} per replica-month, {count} free" },
    sk: {
      one: "{price} za repliku na mesiac, {count} voľná",
      few: "{price} za repliku na mesiac, {count} voľné",
      many: "{price} za repliku na mesiac, {count} voľných",
      other: "{price} za repliku na mesiac, {count} voľných",
    },
    de: { one: "{price} pro Replikat-Monat, {count} frei", other: "{price} pro Replikat-Monat, {count} frei" },
  },
  "machines.start.replicas": { en: "Replicas", sk: "Repliky", de: "Replikate" },
  "machines.start.replicasDetail": {
    en: "Copies of the machine, each with its own disk. Up to {max}.",
    sk: "Kópie stroja, každá s vlastným diskom. Najviac {max}.",
    de: "Kopien der Maschine, jede mit eigener Festplatte. Bis zu {max}.",
  },
  "machines.start.fewer": { en: "One replica fewer", sk: "O repliku menej", de: "Ein Replikat weniger" },
  "machines.start.more": { en: "One replica more", sk: "O repliku viac", de: "Ein Replikat mehr" },
  "machines.start.userData": {
    en: "Cloud-init (optional)",
    sk: "Cloud-init (nepovinné)",
    de: "Cloud-init (optional)",
  },
  "machines.start.userDataDetail": {
    en: "A #cloud-config document or a script the machine runs on its first start, such as your SSH key.",
    sk: "Dokument #cloud-config alebo skript, ktorý stroj spustí pri prvom štarte, napríklad s vaším kľúčom SSH.",
    de: "Ein #cloud-config-Dokument oder Skript, das die Maschine beim ersten Start ausführt, etwa mit deinem SSH-Schlüssel.",
  },
  "machines.start.total": {
    en: {
      one: "{total} a month for {count} replica",
      other: "{total} a month for {count} replicas",
    },
    sk: {
      one: "{total} mesačne za {count} repliku",
      few: "{total} mesačne za {count} repliky",
      many: "{total} mesačne za {count} replík",
      other: "{total} mesačne za {count} replík",
    },
    de: {
      one: "{total} im Monat für {count} Replikat",
      other: "{total} im Monat für {count} Replikate",
    },
  },
  "machines.start.noPrice": {
    en: "No price until a box offers to host",
    sk: "Bez ceny, kým žiadne zariadenie neponúkne hosťovanie",
    de: "Kein Preis, solange keine Box Hosting anbietet",
  },
  "machines.start.split": {
    en: "{percent}% goes to the owner of the box that runs it, {ours}% to LosOS.",
    sk: "{percent} % dostane vlastník zariadenia, na ktorom beží, {ours} % LosOS.",
    de: "{percent} % bekommt, wem die Box gehört, auf der sie läuft, {ours} % bekommt LosOS.",
  },
  "machines.start.button": { en: "Pay and start", sk: "Zaplatiť a spustiť", de: "Bezahlen und starten" },
  "machines.start.paying": {
    en: "Pay on Stripe's page in the other tab. The replicas start once the payment arrives; this page shows them as they come up.",
    sk: "Zaplaťte na stránke Stripe v druhej karte. Repliky sa spustia po prijatí platby a táto stránka ich ukáže, ako budú nabiehať.",
    de: "Bezahle auf der Seite von Stripe im anderen Tab. Die Replikate starten, sobald die Zahlung da ist, und diese Seite zeigt sie, wie sie hochfahren.",
  },
  "machines.start.caption": {
    en: "Each replica runs for a month on the box you pick and is stopped when the month runs out. Its disk is kept.",
    sk: "Každá replika beží mesiac na zariadení, ktoré vyberiete, a po uplynutí mesiaca sa zastaví. Jej disk zostane.",
    de: "Jedes Replikat läuft einen Monat auf der gewählten Box und wird danach angehalten. Seine Festplatte bleibt erhalten.",
  },
  "machines.start.installerOnly": {
    en: "Not offered yet, because they ship as installers only: {systems}.",
    sk: "Zatiaľ nie sú v ponuke, pretože existujú len ako inštalátory: {systems}.",
    de: "Noch nicht im Angebot, weil es sie nur als Installer gibt: {systems}.",
  },

  // ── This box's machines ─────────────────────────────────────────────────
  "machines.mine.title": { en: "Your machines", sk: "Vaše stroje", de: "Deine Maschinen" },
  "machines.mine.replicas": {
    en: { one: "{count} replica", other: "{count} replicas" },
    sk: { one: "{count} replika", few: "{count} repliky", many: "{count} replík", other: "{count} replík" },
    de: { one: "{count} Replikat", other: "{count} Replikate" },
  },
  "machines.mine.replicaList": { en: "Replicas", sk: "Repliky", de: "Replikate" },
  "machines.mine.awaitingPayment": { en: "Awaiting payment", sk: "Čaká na platbu", de: "Wartet auf Zahlung" },
  "machines.mine.lapsed": {
    en: "Stopped, the month ran out",
    sk: "Zastavený, mesiac uplynul",
    de: "Angehalten, der Monat ist um",
  },
  "machines.mine.until": { en: "Runs until {date}", sk: "Beží do {date}", de: "Läuft bis {date}" },
  "machines.mine.caption": {
    en: "The replicas share one address on the edge when it publishes machines.",
    sk: "Ak okrajový server zverejňuje stroje, repliky majú na ňom spoločnú adresu.",
    de: "Wenn der Edge Maschinen veröffentlicht, teilen sich die Replikate dort eine Adresse.",
  },
  "machines.replica.running": { en: "running", sk: "beží", de: "läuft" },
  "machines.replica.starting": { en: "starting", sk: "štartuje", de: "startet" },
  "machines.replica.preparing": { en: "copying its disk", sk: "kopíruje disk", de: "kopiert die Festplatte" },
  "machines.replica.paused": { en: "paused", sk: "pozastavený", de: "pausiert" },
  "machines.replica.stopped": { en: "stopped", sk: "zastavený", de: "angehalten" },
  "machines.replica.failed": { en: "failed", sk: "zlyhal", de: "fehlgeschlagen" },

  // ── This box's images ───────────────────────────────────────────────────
  "machines.images.title": { en: "Your own images", sk: "Vlastné obrazy", de: "Eigene Images" },
  "machines.images.file": { en: "QCOW2 file", sk: "Súbor QCOW2", de: "QCOW2-Datei" },
  "machines.images.name": { en: "Name of the image", sk: "Názov obrazu", de: "Name des Images" },
  "machines.images.efi": {
    en: "Boots with UEFI",
    sk: "Štartuje cez UEFI",
    de: "Startet mit UEFI",
  },
  "machines.images.upload": { en: "Upload", sk: "Nahrať", de: "Hochladen" },
  "machines.images.pickFirst": {
    en: "Pick a QCOW2 file first.",
    sk: "Najprv vyberte súbor QCOW2.",
    de: "Wähle zuerst eine QCOW2-Datei.",
  },
  "machines.images.tooBig": {
    en: "That file is larger than the edge takes ({limit}).",
    sk: "Súbor je väčší, než okrajový server prijme ({limit}).",
    de: "Die Datei ist größer, als der Edge annimmt ({limit}).",
  },
  "machines.images.sending": {
    en: "Sending {name} to the edge, {percent}%",
    sk: "Posielam {name} na okrajový server, {percent} %",
    de: "Sende {name} an den Edge, {percent} %",
  },
  "machines.images.sendingLabel": { en: "Upload", sk: "Nahrávanie", de: "Hochladen" },
  "machines.images.stored": {
    en: "{size} on the edge, needs a {disk} GiB disk",
    sk: "{size} na okrajovom serveri, potrebuje disk {disk} GiB",
    de: "{size} auf dem Edge, braucht {disk} GiB Festplatte",
  },
  "machines.images.notStored": {
    en: "Not uploaded yet",
    sk: "Zatiaľ nenahraný",
    de: "Noch nicht hochgeladen",
  },
  "machines.images.remove": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },
  "machines.images.storedTitle": { en: "Image uploaded", sk: "Obraz nahraný", de: "Image hochgeladen" },
  "machines.images.storedBody": {
    en: "{name} can now be started like any other system.",
    sk: "{name} sa teraz dá spustiť ako ktorýkoľvek iný systém.",
    de: "{name} lässt sich jetzt starten wie jedes andere System.",
  },
  "machines.images.removedTitle": { en: "Image removed", sk: "Obraz odstránený", de: "Image entfernt" },
  "machines.images.caption": {
    en: "QCOW2 only, up to {limit}. The file is kept on the edge, and only your own machines can read it.",
    sk: "Len QCOW2, najviac {limit}. Súbor zostáva na okrajovom serveri a čítať ho môžu len vaše vlastné stroje.",
    de: "Nur QCOW2, bis zu {limit}. Die Datei bleibt auf dem Edge, und nur deine eigenen Maschinen können sie lesen.",
  },

  // ── Hosting machines ────────────────────────────────────────────────────
  "machines.host.title": {
    en: "Host machines on this box",
    sk: "Hosťovať stroje na tomto zariadení",
    de: "Maschinen auf dieser Box betreiben",
  },
  "machines.host.notHosting": {
    en: "This box can host machines once it has joined the mesh and shares its disk there.",
    sk: "Toto zariadenie môže hosťovať stroje, keď sa pripojí k sieti mesh a zdieľa v nej svoj disk.",
    de: "Diese Box kann Maschinen betreiben, sobald sie dem Mesh beigetreten ist und dort ihre Festplatte teilt.",
  },
  "machines.host.price": {
    en: "Price per replica-month ({currency})",
    sk: "Cena za repliku na mesiac ({currency})",
    de: "Preis pro Replikat-Monat ({currency})",
  },
  "machines.host.capacity": {
    en: "Replicas you will run",
    sk: "Počet replík, ktoré spustíte",
    de: "Replikate, die du betreibst",
  },
  "machines.host.capacityInvalid": {
    en: "Offer between 1 and 50 replicas.",
    sk: "Ponúknite 1 až 50 replík.",
    de: "Biete zwischen 1 und 50 Replikate an.",
  },
  "machines.host.list": { en: "Offer", sk: "Ponúknuť", de: "Anbieten" },
  "machines.host.offer": {
    en: "Machines · {price} per replica-month",
    sk: "Stroje · {price} za repliku na mesiac",
    de: "Maschinen · {price} pro Replikat-Monat",
  },
  "machines.host.free": {
    en: "{count} of {capacity} free",
    sk: "Voľné: {count} z {capacity}",
    de: "{count} von {capacity} frei",
  },
  "machines.host.listedTitle": { en: "Offer is up", sk: "Ponuka je zverejnená", de: "Angebot steht" },
  "machines.host.listedBody": {
    en: "Other boxes can now start machines here.",
    sk: "Iné zariadenia tu teraz môžu spúšťať stroje.",
    de: "Andere Boxen können jetzt hier Maschinen starten.",
  },
  "machines.host.closedTitle": { en: "Offer closed", sk: "Ponuka uzavretá", de: "Angebot geschlossen" },
  "machines.host.caption": {
    en: "You receive {percent}% of every machine sale and LosOS keeps {ours}%. Machines that are running keep running until their month is over.",
    sk: "Z každého predaja stroja dostanete {percent} % a LosOS si ponechá {ours} %. Bežiace stroje bežia, kým im neuplynie mesiac.",
    de: "Du bekommst {percent} % jedes Maschinenverkaufs, LosOS behält {ours} %. Laufende Maschinen laufen weiter, bis ihr Monat um ist.",
  },
});
