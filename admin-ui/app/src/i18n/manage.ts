import { defineMessages } from "./define";

/* The company edge's manage page (src/manage/), served by the edge at
 * /manage/: its key, the boxes it routes, and one box's LosOS Cloud, LosOS
 * Git and apps. The rows of a box reuse the Apps pane's own messages. */

export default defineMessages({
  "manage.title": { en: "LosOS edge", sk: "LosOS edge", de: "LosOS-Edge" },
  "manage.signOut": { en: "Sign out", sk: "Odhlásiť sa", de: "Abmelden" },

  // ── The key ───────────────────────────────────────────────────────────
  "manage.key.title": {
    en: "Manage the boxes behind this edge",
    sk: "Spravujte zariadenia za týmto edge",
    de: "Boxen hinter dieser Edge verwalten",
  },
  "manage.key.summary": {
    en: "Paste the edge's manage key. The edge shows it on its own screen.",
    sk: "Vložte kľúč na správu z edge. Edge ho ukazuje na vlastnej obrazovke.",
    de: "Fügen Sie den Verwaltungsschlüssel der Edge ein. Die Edge zeigt ihn auf ihrem eigenen Bildschirm.",
  },
  "manage.key.label": { en: "Manage key", sk: "Kľúč na správu", de: "Verwaltungsschlüssel" },
  "manage.key.submit": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "manage.key.shape": {
    en: "The key is 64 characters, 0 to 9 and a to f.",
    sk: "Kľúč má 64 znakov, 0 až 9 a a až f.",
    de: "Der Schlüssel hat 64 Zeichen, 0 bis 9 und a bis f.",
  },
  "manage.key.wrong": {
    en: "This edge does not know that key.",
    sk: "Tento edge taký kľúč nepozná.",
    de: "Diese Edge kennt diesen Schlüssel nicht.",
  },
  "manage.key.off": {
    en: "This edge does not manage boxes. Turn on losos.edge.manage.enable on the edge.",
    sk: "Tento edge nespravuje zariadenia. Na edge zapnite losos.edge.manage.enable.",
    de: "Diese Edge verwaltet keine Boxen. Schalten Sie auf der Edge losos.edge.manage.enable ein.",
  },
  "manage.key.failed": {
    en: "The edge did not answer: {message}",
    sk: "Edge neodpovedal: {message}",
    de: "Die Edge hat nicht geantwortet: {message}",
  },

  // ── The gateway itself ────────────────────────────────────────────────
  "manage.gateway.loading": {
    en: "Loading this gateway…",
    sk: "Načítavam túto bránu…",
    de: "Lade dieses Gateway…",
  },
  "manage.gateway.title": { en: "This gateway", sk: "Táto brána", de: "Dieses Gateway" },
  "manage.gateway.name": { en: "Name", sk: "Názov", de: "Name" },
  "manage.gateway.noShell": {
    en: "The gateway has no console login and no SSH. You set it up here.",
    sk: "Brána nemá prihlásenie na konzole ani SSH. Nastavujete ju tu.",
    de: "Das Gateway hat keine Konsolenanmeldung und kein SSH. Sie richten es hier ein.",
  },
  "manage.gateway.owner": { en: "Owner", sk: "Vlastník", de: "Besitzer" },
  "manage.gateway.ownerDetail": {
    en: "OpenPGP key {fingerprint}",
    sk: "Kľúč OpenPGP {fingerprint}",
    de: "OpenPGP-Schlüssel {fingerprint}",
  },
  "manage.gateway.unclaimed": {
    en: "Boxes trust this gateway once its owner has signed its settings.",
    sk: "Zariadenia tejto bráne dôverujú, až keď jej vlastník podpíše jej nastavenia.",
    de: "Boxen vertrauen diesem Gateway, sobald sein Besitzer die Einstellungen signiert hat.",
  },
  "manage.gateway.noOwner": { en: "No owner yet", sk: "Zatiaľ bez vlastníka", de: "Noch kein Besitzer" },
  "manage.gateway.edgeKey": { en: "Edge key", sk: "Kľúč edge", de: "Edge-Schlüssel" },
  "manage.gateway.edgeKeyDetail": {
    en: "Name it in the statement you sign.",
    sk: "Uveďte ho vo vyhlásení, ktoré podpisujete.",
    de: "Nennen Sie ihn in der Erklärung, die Sie signieren.",
  },
  "manage.gateway.claim": { en: "Claim this gateway", sk: "Prevziať túto bránu", de: "Dieses Gateway übernehmen" },
  "manage.gateway.change": { en: "Change the settings", sk: "Zmeniť nastavenia", de: "Einstellungen ändern" },
  "manage.gateway.claimDetail": {
    en: "Paste the settings file and your detached OpenPGP signature over it. The gateway restarts to measure them.",
    sk: "Vložte súbor s nastaveniami a k nemu oddelený podpis OpenPGP. Brána sa reštartuje, aby ich zmerala.",
    de: "Fügen Sie die Einstellungsdatei und Ihre abgetrennte OpenPGP-Signatur dazu ein. Das Gateway startet neu, um sie zu messen.",
  },
  "manage.gateway.claimButton": { en: "Claim", sk: "Prevziať", de: "Übernehmen" },
  "manage.gateway.changeButton": { en: "Change", sk: "Zmeniť", de: "Ändern" },
  "manage.gateway.settingsLabel": { en: "Settings file", sk: "Súbor s nastaveniami", de: "Einstellungsdatei" },
  "manage.gateway.signatureLabel": { en: "Signature", sk: "Podpis", de: "Signatur" },
  "manage.gateway.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "manage.gateway.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "manage.gateway.accepted": {
    en: "Settings accepted",
    sk: "Nastavenia prijaté",
    de: "Einstellungen angenommen",
  },
  "manage.gateway.acceptedDetail": {
    en: "The gateway restarts to measure them. Reload this page in a minute.",
    sk: "Brána sa reštartuje, aby ich zmerala. O minútu túto stránku znova načítajte.",
    de: "Das Gateway startet neu, um sie zu messen. Laden Sie diese Seite in einer Minute neu.",
  },
  "manage.gateway.notOwner": {
    en: "The signature is not from this gateway's owner.",
    sk: "Podpis nie je od vlastníka tejto brány.",
    de: "Die Signatur stammt nicht vom Besitzer dieses Gateways.",
  },
  "manage.gateway.refused": {
    en: "The gateway refused the settings: {message}",
    sk: "Brána nastavenia odmietla: {message}",
    de: "Das Gateway hat die Einstellungen abgelehnt: {message}",
  },

  // ── Its hub ───────────────────────────────────────────────────────────
  "manage.uplink.title": { en: "Hub", sk: "Hub", de: "Hub" },
  "manage.uplink.none": {
    en: "Not relaying",
    sk: "Bez prenosu",
    de: "Keine Weiterleitung",
  },
  "manage.uplink.noneDetail": {
    en: "The boxes here are reachable on this network only.",
    sk: "Zariadenia sú dostupné len v tejto sieti.",
    de: "Die Boxen hier sind nur in diesem Netz erreichbar.",
  },
  "manage.uplink.via": {
    en: "{id} via {registrar}",
    sk: "{id} cez {registrar}",
    de: "{id} über {registrar}",
  },
  "manage.uplink.relaying": {
    en: { one: "One box relayed", other: "{count} boxes relayed" },
    sk: { one: "Prenáša sa jedno zariadenie", few: "Prenášajú sa {count} zariadenia", other: "Prenáša sa {count} zariadení" },
    de: { one: "Eine Box weitergeleitet", other: "{count} Boxen weitergeleitet" },
  },
  "manage.uplink.set": { en: "Set up", sk: "Nastaviť", de: "Einrichten" },
  "manage.uplink.change": { en: "Change", sk: "Zmeniť", de: "Ändern" },
  "manage.uplink.clear": { en: "Stop relaying", sk: "Zastaviť prenos", de: "Weiterleitung beenden" },
  "manage.uplink.caption": {
    en: "To reach these boxes from anywhere, enter the tenant row the hub's operator gave you.",
    sk: "Aby boli tieto zariadenia dostupné odkiaľkoľvek, zadajte riadok nájomcu od operátora hubu.",
    de: "Damit diese Boxen von überall erreichbar sind, geben Sie die Mandantenzeile ein, die Ihnen der Betreiber des Hubs gegeben hat.",
  },
  "manage.uplink.id": { en: "Tenant id", sk: "ID nájomcu", de: "Mandanten-ID" },
  "manage.uplink.registrar": { en: "Registrar URL", sk: "URL registrátora", de: "Registrar-URL" },
  "manage.uplink.endpoint": { en: "Tunnel endpoint", sk: "Koncový bod tunela", de: "Tunnel-Endpunkt" },
  "manage.uplink.token": { en: "Token", sk: "Token", de: "Token" },
  "manage.uplink.bootstrap": {
    en: "Bootstrap token (optional)",
    sk: "Úvodný token (nepovinný)",
    de: "Bootstrap-Token (optional)",
  },
  "manage.uplink.saved": { en: "Hub saved", sk: "Hub uložený", de: "Hub gespeichert" },
  "manage.uplink.savedDetail": {
    en: "The boxes here are relayed within a minute.",
    sk: "Zariadenia sa začnú prenášať do minúty.",
    de: "Die Boxen hier werden innerhalb einer Minute weitergeleitet.",
  },
  "manage.uplink.cleared": {
    en: "Stopped relaying. The boxes are reachable on this network only.",
    sk: "Prenos zastavený. Zariadenia sú dostupné len v tejto sieti.",
    de: "Weiterleitung beendet. Die Boxen sind nur in diesem Netz erreichbar.",
  },
  "manage.uplink.failed": {
    en: "The hub could not be changed",
    sk: "Hub sa nepodarilo zmeniť",
    de: "Der Hub konnte nicht geändert werden",
  },

  // ── The secondary cluster ─────────────────────────────────────────────
  "manage.cluster.title": { en: "Secondary cluster", sk: "Sekundárny klaster", de: "Sekundärer Cluster" },
  "manage.cluster.caption": {
    en: "Gateways on one network that serve the same boxes. When one goes away, its boxes move to another without enrolling again.",
    sk: "Brány v jednej sieti, ktoré obsluhujú tie isté zariadenia. Keď jedna vypadne, jej zariadenia prejdú na inú bez novej registrácie.",
    de: "Gateways in einem Netz, die dieselben Boxen bedienen. Fällt eines aus, wechseln seine Boxen ohne neue Aufnahme zu einem anderen.",
  },
  "manage.cluster.none": { en: "In no cluster", sk: "V žiadnom klastri", de: "In keinem Cluster" },
  "manage.cluster.noneDetail": {
    en: "Boxes here hold secondary links to this gateway alone.",
    sk: "Zariadenia tu majú sekundárne spojenie iba s touto bránou.",
    de: "Boxen hier halten sekundäre Verbindungen nur zu diesem Gateway.",
  },
  "manage.cluster.start": { en: "Start a cluster", sk: "Založiť klaster", de: "Cluster gründen" },
  "manage.cluster.join": { en: "Join a cluster", sk: "Pridať sa ku klastru", de: "Cluster beitreten" },
  "manage.cluster.leave": { en: "Leave", sk: "Opustiť", de: "Verlassen" },
  "manage.cluster.in": { en: "Cluster {name}", sk: "Klaster {name}", de: "Cluster {name}" },
  "manage.cluster.member": {
    en: "This gateway is {member} in it.",
    sk: "Táto brána je v ňom {member}.",
    de: "Dieses Gateway heißt darin {member}.",
  },
  "manage.cluster.addAnother": {
    en: "To add another gateway on this network, open its page, choose Join a cluster and enter:",
    sk: "Ak chcete pridať ďalšiu bránu v tejto sieti, otvorte jej stránku, zvoľte Pridať sa ku klastru a zadajte:",
    de: "Um ein weiteres Gateway in diesem Netz hinzuzufügen, öffnen Sie seine Seite, wählen Sie Cluster beitreten und geben Sie ein:",
  },
  "manage.cluster.name": { en: "Name", sk: "Názov", de: "Name" },
  "manage.cluster.secret": { en: "Secret", sk: "Tajomstvo", de: "Geheimnis" },
  "manage.cluster.address": { en: "Gateway address", sk: "Adresa brány", de: "Gateway-Adresse" },
  "manage.cluster.started": { en: "Cluster {name} started", sk: "Klaster {name} založený", de: "Cluster {name} gegründet" },
  "manage.cluster.joined": { en: "Joined cluster {name}", sk: "Pridané ku klastru {name}", de: "Cluster {name} beigetreten" },
  "manage.cluster.syncs": {
    en: "The gateways sync on their next pass, within a minute.",
    sk: "Brány sa zosynchronizujú pri ďalšom prechode, do minúty.",
    de: "Die Gateways gleichen sich beim nächsten Durchlauf ab, innerhalb einer Minute.",
  },
  "manage.cluster.left": { en: "Left the cluster", sk: "Klaster opustený", de: "Cluster verlassen" },
  "manage.cluster.leftDetail": {
    en: "The boxes here stay on this gateway. The others no longer fail over to it.",
    sk: "Zariadenia tu zostávajú na tejto bráne. Ostatné na ňu už neprechádzajú.",
    de: "Die Boxen hier bleiben an diesem Gateway. Die anderen weichen nicht mehr darauf aus.",
  },
  "manage.cluster.failed": {
    en: "Could not change the cluster",
    sk: "Klaster sa nepodarilo zmeniť",
    de: "Der Cluster konnte nicht geändert werden",
  },

  // ── The main cluster ──────────────────────────────────────────────────
  "manage.main.title": { en: "Main cluster", sk: "Hlavný klaster", de: "Hauptcluster" },
  "manage.main.caption": {
    en: "The gateways that relay through the same hub, each on a primary link.",
    sk: "Brány, ktoré sú prepojené cez ten istý hub, každá na primárnom spojení.",
    de: "Die Gateways, die über denselben Hub weiterleiten, jedes über eine primäre Verbindung.",
  },
  "manage.main.inCluster": { en: "Cluster {cluster}", sk: "Klaster {cluster}", de: "Cluster {cluster}" },
  "manage.main.boxes": { en: "{count} boxes", sk: "Zariadenia: {count}", de: "{count} Boxen" },

  // ── Boxes that enrolled here ──────────────────────────────────────────
  "manage.enrolled.title": {
    en: "Enrolled on first contact",
    sk: "Zaregistrované pri prvom kontakte",
    de: "Beim ersten Kontakt aufgenommen",
  },
  "manage.enrolled.caption": {
    en: "Forget a box that left. If it comes back, it enrols again.",
    sk: "Zabudnite zariadenie, ktoré odišlo. Ak sa vráti, zaregistruje sa znova.",
    de: "Vergessen Sie eine Box, die gegangen ist. Kommt sie zurück, wird sie wieder aufgenommen.",
  },
  "manage.enrolled.forget": { en: "Forget", sk: "Zabudnúť", de: "Vergessen" },
  "manage.enrolled.forgot": { en: "Forgot {id}", sk: "{id} zabudnuté", de: "{id} vergessen" },
  "manage.enrolled.forgotDetail": {
    en: "It enrols again on its next contact.",
    sk: "Pri ďalšom kontakte sa zaregistruje znova.",
    de: "Beim nächsten Kontakt wird sie wieder aufgenommen.",
  },
  "manage.enrolled.failed": {
    en: "Could not forget {id}",
    sk: "{id} sa nepodarilo zabudnúť",
    de: "{id} konnte nicht vergessen werden",
  },

  // ── The boxes ─────────────────────────────────────────────────────────
  "manage.boxes.title": { en: "Boxes", sk: "Zariadenia", de: "Boxen" },
  "manage.boxes.summary": {
    en: "The boxes this edge routes. You can manage a box once its owner switches it on, on the box's Mesh page.",
    sk: "Zariadenia, ktoré tento edge smeruje. Zariadenie môžete spravovať, keď to jeho vlastník zapne na stránke Mesh zariadenia.",
    de: "Die Boxen, die diese Edge weiterleitet. Sie können eine Box verwalten, sobald ihr Besitzer das auf der Mesh-Seite der Box einschaltet.",
  },
  "manage.boxes.loading": {
    en: "Loading the boxes…",
    sk: "Načítavam zariadenia…",
    de: "Lade die Boxen…",
  },
  "manage.boxes.failed": {
    en: "Could not load the boxes: {message}",
    sk: "Zariadenia sa nepodarilo načítať: {message}",
    de: "Die Boxen konnten nicht geladen werden: {message}",
  },
  "manage.boxes.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "manage.boxes.empty": {
    en: "No box has joined this edge yet.",
    sk: "K tomuto edge sa zatiaľ nepripojilo žiadne zariadenie.",
    de: "Noch keine Box hat sich mit dieser Edge verbunden.",
  },
  "manage.boxes.seen": {
    en: "Last heard from {ago}",
    sk: "Naposledy ozvalo {ago}",
    de: "Zuletzt gemeldet {ago}",
  },
  "manage.boxes.justNow": { en: "just now", sk: "práve teraz", de: "gerade eben" },
  "manage.boxes.minutes": {
    en: { one: "a minute ago", other: "{count} minutes ago" },
    sk: { one: "pred minútou", few: "pred {count} minútami", other: "pred {count} minútami" },
    de: { one: "vor einer Minute", other: "vor {count} Minuten" },
  },
  "manage.boxes.managed": { en: "Managed here", sk: "Spravované tu", de: "Hier verwaltet" },
  "manage.boxes.notManaged": {
    en: "Not switched on",
    sk: "Nezapnuté",
    de: "Nicht eingeschaltet",
  },
  "manage.boxes.open": { en: "Manage", sk: "Spravovať", de: "Verwalten" },
  "manage.boxes.notManagedDetail": {
    en: "Its owner has not let this edge manage it.",
    sk: "Vlastník tomuto edge nedovolil spravovať ho.",
    de: "Ihr Besitzer hat dieser Edge die Verwaltung nicht erlaubt.",
  },

  // ── One box ───────────────────────────────────────────────────────────
  "manage.box.back": { en: "All boxes", sk: "Všetky zariadenia", de: "Alle Boxen" },
  "manage.box.summary": {
    en: "LosOS Cloud, LosOS Git and the apps on this box. Everything else stays with its owner.",
    sk: "LosOS Cloud, LosOS Git a aplikácie na tomto zariadení. Všetko ostatné ostáva jeho vlastníkovi.",
    de: "LosOS Cloud, LosOS Git und die Apps auf dieser Box. Alles andere bleibt bei ihrem Besitzer.",
  },
  "manage.box.refused": {
    en: "The box refused this edge. Its owner may have switched management off.",
    sk: "Zariadenie tento edge odmietlo. Vlastník mohol správu vypnúť.",
    de: "Die Box hat diese Edge abgelehnt. Ihr Besitzer hat die Verwaltung vielleicht ausgeschaltet.",
  },

  // ── Its settings ──────────────────────────────────────────────────────
  "manage.site.loading": {
    en: "Loading the box's settings…",
    sk: "Načítavam nastavenia zariadenia…",
    de: "Lade die Einstellungen der Box…",
  },
  "manage.site.failed": {
    en: "Could not load the box's settings: {message}",
    sk: "Nastavenia zariadenia sa nepodarilo načítať: {message}",
    de: "Die Einstellungen der Box konnten nicht geladen werden: {message}",
  },
  "manage.site.none": {
    en: "This box has no settings of LosOS Cloud or LosOS Git to change from here.",
    sk: "Toto zariadenie nemá nastavenia LosOS Cloud ani LosOS Git, ktoré by sa dali zmeniť odtiaľto.",
    de: "Diese Box hat keine Einstellungen von LosOS Cloud oder LosOS Git, die sich von hier ändern lassen.",
  },
  "manage.site.applying": {
    en: "Applying the box's new settings",
    sk: "Používam nové nastavenia zariadenia",
    de: "Wende die neuen Einstellungen der Box an",
  },
  "manage.site.didNotStart": {
    en: "The box did not take the change",
    sk: "Zariadenie zmenu neprijalo",
    de: "Die Box hat die Änderung nicht angenommen",
  },
  "manage.site.done": {
    en: "The box runs its new settings",
    sk: "Zariadenie beží s novými nastaveniami",
    de: "Die Box läuft mit den neuen Einstellungen",
  },
  "manage.site.buildFailed": {
    en: "The box could not apply the change",
    sk: "Zariadenie nedokázalo zmenu použiť",
    de: "Die Box konnte die Änderung nicht anwenden",
  },
});
