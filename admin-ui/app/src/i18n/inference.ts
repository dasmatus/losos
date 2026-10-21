/* The Market pane's AI models: buying answers from models that run on the
 * memory boxes lend, and lending this box's own (ai-models.tsx). */

import { defineMessages } from "./define";

export default defineMessages({
  "panes.market.ai.title": {
    en: "AI models",
    sk: "Modely AI",
    de: "KI-Modelle",
  },
  "panes.market.ai.loading": {
    en: "Looking at the models…",
    sk: "Pozeráme sa na modely…",
    de: "Die Modelle werden geladen …",
  },
  "panes.market.ai.failed": {
    en: "The models did not load",
    sk: "Modely sa nenačítali",
    de: "Die Modelle wurden nicht geladen",
  },
  "panes.market.ai.none": {
    en: "This edge runs no AI models.",
    sk: "Tento okrajový server nespúšťa žiadne modely AI.",
    de: "Dieser Edge betreibt keine KI-Modelle.",
  },
  "panes.market.ai.network": {
    en: "Pay on",
    sk: "Platiť v sieti",
    de: "Bezahlen auf",
  },
  "panes.market.ai.price": {
    en: "{in} in, {out} out per million tokens",
    sk: "{in} za vstup, {out} za výstup, za milión tokenov",
    de: "{in} Eingabe, {out} Ausgabe pro Million Tokens",
  },
  "panes.market.ai.memory": {
    en: "Needs {need}; boxes lend {lent} now",
    sk: "Potrebuje {need}; zariadenia teraz požičiavajú {lent}",
    de: "Braucht {need}; Boxen verleihen gerade {lent}",
  },
  "panes.market.ai.memoryLabel": {
    en: "Memory lent against what the model needs",
    sk: "Požičaná pamäť oproti tomu, čo model potrebuje",
    de: "Verliehener Speicher im Vergleich zum Bedarf des Modells",
  },
  "panes.market.ai.gib": {
    en: "{count} GiB",
    sk: "{count} GiB",
    de: "{count} GiB",
  },
  "panes.market.ai.state.ready": {
    en: { one: "Running on {count} box", other: "Running on {count} boxes" },
    sk: { one: "Beží na {count} zariadení", few: "Beží na {count} zariadeniach", many: "Beží na {count} zariadeniach", other: "Beží na {count} zariadeniach" },
    de: { one: "Läuft auf {count} Box", other: "Läuft auf {count} Boxen" },
  },
  "panes.market.ai.state.loading": {
    en: "Loading onto its boxes",
    sk: "Načítava sa do zariadení",
    de: "Wird auf die Boxen geladen",
  },
  "panes.market.ai.state.down": {
    en: "A box is restarting",
    sk: "Jedno zo zariadení sa reštartuje",
    de: "Eine Box startet neu",
  },
  "panes.market.ai.state.short": {
    en: "Waiting for more memory",
    sk: "Čaká na viac pamäte",
    de: "Wartet auf mehr Speicher",
  },
  "panes.market.ai.state.idle": {
    en: "Starts when you ask",
    sk: "Spustí sa, keď oň požiadate",
    de: "Startet, wenn du es brauchst",
  },
  "panes.market.ai.start": {
    en: "Start",
    sk: "Spustiť",
    de: "Starten",
  },
  "panes.market.ai.cancel": {
    en: "Cancel",
    sk: "Zrušiť",
    de: "Abbrechen",
  },
  "panes.market.ai.limit": {
    en: "Most it may spend in an hour (USDC)",
    sk: "Najviac, koľko smie minúť za hodinu (USDC)",
    de: "Höchstens pro Stunde (USDC)",
  },
  "panes.market.ai.topUp": {
    en: "Add to your balance (USDC)",
    sk: "Pridať do zostatku (USDC)",
    de: "Zum Guthaben hinzufügen (USDC)",
  },
  "panes.market.ai.held": {
    en: "Your balance in the escrow: {amount}",
    sk: "Váš zostatok v úschove: {amount}",
    de: "Dein Guthaben im Treuhandkonto: {amount}",
  },
  "panes.market.ai.payFrom": {
    en: "Pay from",
    sk: "Platiť z",
    de: "Bezahlen mit",
  },
  "panes.market.ai.payFrom.local": {
    en: "The wallet on this box",
    sk: "Peňaženka na tomto zariadení",
    de: "Die Wallet auf dieser Box",
  },
  "panes.market.ai.payFrom.injected": {
    en: "A browser wallet",
    sk: "Peňaženka v prehliadači",
    de: "Eine Browser-Wallet",
  },
  "panes.market.ai.password": {
    en: "Wallet password",
    sk: "Heslo peňaženky",
    de: "Wallet-Passwort",
  },
  "panes.market.ai.noWallet": {
    en: "Set up the USDC wallet below first, or connect a browser wallet.",
    sk: "Najprv si nižšie nastavte peňaženku USDC alebo pripojte peňaženku v prehliadači.",
    de: "Richte zuerst unten die USDC-Wallet ein oder verbinde eine Browser-Wallet.",
  },
  "panes.market.ai.limitInvalid": {
    en: "Allow between 0.01 and 100 USDC an hour.",
    sk: "Povoľte od 0,01 do 100 USDC za hodinu.",
    de: "Erlaube zwischen 0,01 und 100 USDC pro Stunde.",
  },
  "panes.market.ai.topUpInvalid": {
    en: "Enter an amount of USDC, such as 5 or 2.50.",
    sk: "Zadajte sumu v USDC, napríklad 5 alebo 2.50.",
    de: "Gib einen Betrag in USDC ein, etwa 5 oder 2.50.",
  },
  "panes.market.ai.topUpNeeded": {
    en: "The balance is empty: add something to it to start.",
    sk: "Zostatok je prázdny: na spustenie doň niečo pridajte.",
    de: "Das Guthaben ist leer: füge etwas hinzu, um zu starten.",
  },
  "panes.market.ai.step.quote": {
    en: "Asking the edge for a price…",
    sk: "Pýtame sa okrajového servera na cenu…",
    de: "Der Edge wird nach dem Preis gefragt …",
  },
  "panes.market.ai.step.wallet": {
    en: "Approve in your wallet. Each lending box gets a tap of its own.",
    sk: "Potvrďte v peňaženke. Každé požičiavajúce zariadenie dostane vlastný kohútik.",
    de: "Bestätige in deiner Wallet. Jede verleihende Box bekommt einen eigenen Zapfhahn.",
  },
  "panes.market.ai.step.confirm": {
    en: "Checking the escrow…",
    sk: "Kontrolujeme úschovu…",
    de: "Das Treuhandkonto wird geprüft …",
  },
  "panes.market.ai.startedTitle": {
    en: "{model} is running for you",
    sk: "{model} pre vás beží",
    de: "{model} läuft für dich",
  },
  "panes.market.ai.startedBody": {
    en: "You pay per token, at most {limit} an hour. Stop it whenever you like.",
    sk: "Platíte za tokeny, najviac {limit} za hodinu. Zastaviť ho môžete kedykoľvek.",
    de: "Du zahlst pro Token, höchstens {limit} pro Stunde. Du kannst es jederzeit stoppen.",
  },
  "panes.market.ai.failedTitle": {
    en: "That did not work",
    sk: "Nepodarilo sa to",
    de: "Das hat nicht geklappt",
  },
  "panes.market.ai.failedBody": {
    en: "The edge gave no reason. Try again in a moment.",
    sk: "Edge neuviedol dôvod. Skúste to o chvíľu znova.",
    de: "Die Edge hat keinen Grund genannt. Versuch es gleich noch einmal.",
  },
  "panes.market.ai.caption": {
    en: "Each model runs split across the spare memory of several boxes on the mesh. The boxes running a model can read its prompts and answers. You pay per token from a balance you keep in the escrow, never more in an hour than you allow, and the edge keeps {percent}% of it.",
    sk: "Každý model beží rozdelený vo voľnej pamäti viacerých zariadení v sieti mesh. Zariadenia, na ktorých model beží, môžu čítať jeho otázky aj odpovede. Platíte za tokeny zo zostatku, ktorý máte v úschove, nikdy viac za hodinu, než povolíte, a okrajový server si z toho ponechá {percent} %.",
    de: "Jedes Modell läuft aufgeteilt im freien Speicher mehrerer Boxen im Mesh. Die Boxen, auf denen ein Modell läuft, können seine Eingaben und Antworten lesen. Du zahlst pro Token aus einem Guthaben im Treuhandkonto, nie mehr pro Stunde, als du erlaubst, und die Edge behält {percent} % davon.",
  },
  "panes.market.ai.meters": {
    en: "Running for you",
    sk: "Beží pre vás",
    de: "Läuft für dich",
  },
  "panes.market.ai.meter.quoted": {
    en: "Waiting for the wallet",
    sk: "Čaká na peňaženku",
    de: "Wartet auf die Wallet",
  },
  "panes.market.ai.meter.open": {
    en: "Open",
    sk: "Otvorené",
    de: "Offen",
  },
  "panes.market.ai.meter.ending": {
    en: "Stopping",
    sk: "Zastavuje sa",
    de: "Wird gestoppt",
  },
  "panes.market.ai.meter.ended": {
    en: "Stopped",
    sk: "Zastavené",
    de: "Gestoppt",
  },
  "panes.market.ai.meter.lapsed": {
    en: "Lapsed",
    sk: "Vypršalo",
    de: "Abgelaufen",
  },
  "panes.market.ai.meter.detail": {
    en: "{spent} spent on {tokens} tokens · at most {limit} an hour",
    sk: "{spent} minuté za {tokens} tokenov · najviac {limit} za hodinu",
    de: "{spent} für {tokens} Tokens ausgegeben · höchstens {limit} pro Stunde",
  },
  "panes.market.ai.stop": {
    en: "Stop",
    sk: "Zastaviť",
    de: "Stoppen",
  },
  "panes.market.ai.stoppedTitle": {
    en: "Stopped",
    sk: "Zastavené",
    de: "Gestoppt",
  },
  "panes.market.ai.stoppedBody": {
    en: "The edge closes the taps; nothing more is billed.",
    sk: "Okrajový server zatvorí kohútiky; nič ďalšie sa neúčtuje.",
    de: "Der Edge schließt die Zapfhähne; es wird nichts mehr berechnet.",
  },
  "panes.market.ai.checkAgain": {
    en: "Check again",
    sk: "Skontrolovať znova",
    de: "Erneut prüfen",
  },
  "panes.market.ai.balance": {
    en: "Your balance",
    sk: "Váš zostatok",
    de: "Dein Guthaben",
  },
  "panes.market.ai.balance.on": {
    en: "On {chain}",
    sk: "V sieti {chain}",
    de: "Auf {chain}",
  },
  "panes.market.ai.balance.runway": {
    en: "Lasts about {time} at the limits you set",
    sk: "Pri vašich limitoch vydrží asi {time}",
    de: "Reicht bei deinen Grenzen etwa {time}",
  },
  "panes.market.ai.balance.low": {
    en: "Less than a day left at the limits you set: about {time}",
    sk: "Pri vašich limitoch zostáva menej ako deň: asi {time}",
    de: "Bei deinen Grenzen bleibt weniger als ein Tag: etwa {time}",
  },
  "panes.market.ai.balance.idle": {
    en: "Nothing is running on it",
    sk: "Nič z neho nebeží",
    de: "Darauf läuft gerade nichts",
  },
  "panes.market.ai.time.minutes": {
    en: { one: "{count} minute", other: "{count} minutes" },
    sk: { one: "{count} minútu", few: "{count} minúty", many: "{count} minút", other: "{count} minút" },
    de: { one: "{count} Minute", other: "{count} Minuten" },
  },
  "panes.market.ai.time.hours": {
    en: { one: "{count} hour", other: "{count} hours" },
    sk: { one: "{count} hodinu", few: "{count} hodiny", many: "{count} hodín", other: "{count} hodín" },
    de: { one: "{count} Stunde", other: "{count} Stunden" },
  },
  "panes.market.ai.time.days": {
    en: { one: "{count} day", other: "{count} days" },
    sk: { one: "{count} deň", few: "{count} dni", many: "{count} dní", other: "{count} dní" },
    de: { one: "{count} Tag", other: "{count} Tage" },
  },
  "panes.market.ai.withdraw.ask": {
    en: "Ask for it back",
    sk: "Požiadať o vrátenie",
    de: "Zurückfordern",
  },
  "panes.market.ai.withdraw.asked": {
    en: "{amount} asked back; you can take it from {when}",
    sk: "O vrátenie {amount} ste požiadali; vybrať si ich môžete od {when}",
    de: "{amount} zurückgefordert; abholen kannst du es ab {when}",
  },
  "panes.market.ai.withdraw.ready": {
    en: "{amount} can be taken back now",
    sk: "{amount} si teraz môžete vybrať",
    de: "{amount} kannst du jetzt abholen",
  },
  "panes.market.ai.withdraw.take": {
    en: "Take it back",
    sk: "Vybrať",
    de: "Abholen",
  },
  "panes.market.ai.withdraw.caption": {
    en: "Money in the escrow comes back to your wallet {hours} hours after you ask for it, so the boxes are always paid for what they already answered. Ask and take back with the wallet that paid.",
    sk: "Peniaze z úschovy sa vrátia do vašej peňaženky {hours} hodín po tom, ako o ne požiadate, takže zariadenia dostanú zaplatené za všetko, čo už zodpovedali. Žiadajte a vyberajte tou peňaženkou, ktorá platila.",
    de: "Geld im Treuhandkonto kommt {hours} Stunden nach deiner Anfrage in deine Wallet zurück, damit die Boxen für alles bezahlt werden, was sie schon beantwortet haben. Fordere es mit der Wallet zurück, die bezahlt hat.",
  },
  "panes.market.ai.withdraw.wrongWallet": {
    en: "This balance belongs to {wallet}, not to the wallet chosen to pay from.",
    sk: "Tento zostatok patrí peňaženke {wallet}, nie peňaženke zvolenej na platenie.",
    de: "Dieses Guthaben gehört {wallet}, nicht der Wallet, mit der bezahlt werden soll.",
  },
  "panes.market.ai.withdraw.askedTitle": {
    en: "Asked for it back",
    sk: "Požiadali ste o vrátenie",
    de: "Zurückgefordert",
  },
  "panes.market.ai.withdraw.askedBody": {
    en: "You can take it back from {when}.",
    sk: "Vybrať si ho môžete od {when}.",
    de: "Abholen kannst du es ab {when}.",
  },
  "panes.market.ai.withdraw.takenTitle": {
    en: "Back in your wallet",
    sk: "Späť vo vašej peňaženke",
    de: "Zurück in deiner Wallet",
  },
  "panes.market.ai.chat.title": {
    en: "Try it",
    sk: "Vyskúšajte",
    de: "Ausprobieren",
  },
  "panes.market.ai.chat.model": {
    en: "Model",
    sk: "Model",
    de: "Modell",
  },
  "panes.market.ai.chat.prompt": {
    en: "Ask something",
    sk: "Na niečo sa opýtajte",
    de: "Frag etwas",
  },
  "panes.market.ai.chat.maxTokens": {
    en: "Longest answer, in tokens",
    sk: "Najdlhšia odpoveď v tokenoch",
    de: "Längste Antwort in Tokens",
  },
  "panes.market.ai.chat.send": {
    en: "Send",
    sk: "Odoslať",
    de: "Senden",
  },
  "panes.market.ai.chat.waiting": {
    en: "Waiting for the answer. A large model can take a few minutes.",
    sk: "Čakáme na odpoveď. Veľkému modelu to môže trvať niekoľko minút.",
    de: "Die Antwort kommt noch. Ein großes Modell kann ein paar Minuten brauchen.",
  },
  "panes.market.ai.chat.cost": {
    en: "This answer cost {cost}: {in} tokens in, {out} out.",
    sk: "Táto odpoveď stála {cost}: {in} tokenov na vstupe, {out} na výstupe.",
    de: "Diese Antwort hat {cost} gekostet: {in} Tokens Eingabe, {out} Ausgabe.",
  },
  "panes.market.ai.chat.notReady": {
    en: "The model answers once it is loaded on its boxes.",
    sk: "Model odpovie, keď sa načíta do svojich zariadení.",
    de: "Das Modell antwortet, sobald es auf seine Boxen geladen ist.",
  },

  // ── Lending this box's memory ─────────────────────────────────────────
  "panes.market.lend.title": {
    en: "Spare memory",
    sk: "Voľná pamäť",
    de: "Freier Speicher",
  },
  "panes.market.lend.switch": {
    en: "Lend memory to AI models",
    sk: "Požičiavať pamäť modelom AI",
    de: "Speicher an KI-Modelle verleihen",
  },
  "panes.market.lend.amount": {
    en: "Memory to lend (GiB)",
    sk: "Pamäť na požičanie (GiB)",
    de: "Zu verleihender Speicher (GiB)",
  },
  "panes.market.lend.amountInvalid": {
    en: "Lend a whole number of GiB, from 1 to 256.",
    sk: "Požičajte celý počet GiB, od 1 do 256.",
    de: "Verleihe eine ganze Zahl GiB, von 1 bis 256.",
  },
  "panes.market.lend.paid": {
    en: "Paid to this box",
    sk: "Vyplatené tomuto zariadeniu",
    de: "An diese Box gezahlt",
  },
  "panes.market.lend.owed": {
    en: "Used and not yet paid",
    sk: "Využité a ešte nezaplatené",
    de: "Genutzt und noch nicht bezahlt",
  },
  "panes.market.lend.serving": {
    en: "Holding part of",
    sk: "Drží časť modelu",
    de: "Hält einen Teil von",
  },
  "panes.market.lend.servingNone": {
    en: "No model right now",
    sk: "Teraz žiadny model",
    de: "Gerade kein Modell",
  },
  "panes.market.lend.caption": {
    en: "Memory this box is not using holds part of a large model for people who pay per token. The lending runs as the shared user, apart from your own files, and never takes more than the amount above; if it would, it stops and nothing else on the box is touched. Payments go to this box's USDC payout address.",
    sk: "Pamäť, ktorú toto zariadenie nepoužíva, drží časť veľkého modelu pre ľudí, ktorí platia za tokeny. Požičiavanie beží ako používateľ shared, oddelene od vašich súborov, a nikdy nezaberie viac než sumu vyššie; ak by malo, zastaví sa a nič iné na zariadení sa nezmení. Platby idú na výplatnú adresu USDC tohto zariadenia.",
    de: "Speicher, den diese Box nicht nutzt, hält einen Teil eines großen Modells für Leute, die pro Token zahlen. Das Verleihen läuft als Benutzer shared, getrennt von deinen eigenen Dateien, und nimmt nie mehr als die Menge oben; würde es das, hält es an und nichts anderes auf der Box wird berührt. Zahlungen gehen an die USDC-Auszahlungsadresse dieser Box.",
  },
});
