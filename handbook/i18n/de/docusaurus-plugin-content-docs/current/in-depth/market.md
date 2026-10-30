---
title: Marktplatz
sidebar_position: 4
mdx:
  format: md
---

# Marktplatz

Ein optionaler Marktplatz, auf dem eine Box freien Speicher oder Rechenleistung
verkauft und eine andere sie kauft. Käufer zahlen per Karte über **Stripe
Connect**, wo die Edge Karten annimmt, oder in USDC auf Solana, Base,
Arbitrum oder OP. Die Edge hält jede Zahlung zurück, bis der Zeitraum endet, für den der
Käufer bezahlt hat, erstattet dem Käufer, wenn die Box des Verkäufers nicht
mehr liefert, und behält 4 % ein, um ihre Betriebskosten zu decken. Eine Edge
lässt sich auch so einstellen, dass sie Bitcoin, Monero und Zcash annimmt;
diese erreichen den Verkäufer sofort und lassen sich deshalb nicht
erstatten.

Die Zeile **Markt** unter Mesh in der Seitenleiste öffnet den Bereich, unter
`/settings/market`. Eine Box zeigt den Marktplatz nur, wenn eine offizielle
LosOS-Edge erreichbar ist; sonst sagt der Bereich, dass der Markt auf dieser
Box nicht verfügbar ist. Der Schalter für die Festplattenfreigabe
(`losos.sharingMyStorage`) liegt in diesem Bereich und lässt sich in beiden
Fällen einschalten, sodass die Seite Maschinen funktioniert, sobald die
Freigabe an ist. Der Marktplatz ist standardmäßig aus, und zwar auf drei
Ebenen.

|                                  | Standard     | Schalter                                         |
| -------------------------------- | ------------ | ------------------------------------------------ |
| Die Edge bietet ihn an           | aus          | `losos.edge.market.enable`                       |
| Eine Box darf handeln            | aus          | `losos.edge.tenants.<id>.market`                 |
| Ein Verkäufer darf anbieten      | geschlossen  | Stripe meldet das verbundene Konto als bereit, oder auf einer Edge mit Coins hat der Verkäufer eine USDC-Auszahlungsadresse oder einen Coin-Schlüssel hinterlegt |

Eine Box, die nie handelt, ist nicht betroffen. Der Marktplatz ist eine
Funktion des `losos-registrar` der Edge.

**Der Marktplatz verkauft, was du bereits teilst, und sonst nichts.** Er
bezahlt eine Box für den Speicher und die Rechenleistung, die sie zum Mesh
beiträgt. Er ist kein eigenständiges Produkt. Eine Box kann Speicher nur
anbieten, solange ihr Node im Mesh eingetragen ist, und Rechenleistung nur,
solange sie zusätzlich Rechenleistung teilt (`losos.cluster.shareCompute`).
Hörst du auf zu teilen, verschwinden deine Angebote sofort aus dem Regal. Sie
kehren zurück, wenn du wieder teilst, und bereits bezahlte Bestellungen sind
nicht betroffen.

## Wie das Geld fließt {#how-the-money-moves}

Geld berührt nie eine LosOS-Box. Käufer zahlen auf eine dieser Arten, je
nachdem, was die Edge erlaubt:

- **USDC auf Solana, Base, Arbitrum One oder OP Mainnet.** Die Zahlung geht
  in einen Escrow: das Programm `losos-escrow` auf Solana, den Vertrag
  `LososEscrow` auf den anderen. Die Edge prüft sie (die Adresse des Verkäufers, den
  Betrag, die Bestätigungen) und hält sie dort, bis der Zeitraum des Käufers
  endet. Dann gibt sie sie an den Verkäufer und an die Kasse der Plattform
  frei, die die Gebühr von 4 % erhält. Oder sie zahlt die ganze Zahlung an
  den Käufer zurück: wenn die Adresse des Verkäufers falsch oder ein Vertrag
  ist, wenn die Bestellung schon bezahlt war, wenn ihre Einheiten verfallen
  sind oder wenn sich das Gekaufte nicht rechtzeitig einrichten ließ. War die
  Box des Verkäufers zu lange offline, teilt sie die Zahlung auf: Der Käufer
  bekommt den Anteil zurück, der nicht geliefert wurde, und der Verkäufer
  behält den Rest. Handelt die Edge nicht, kann der Käufer die Einzahlung 33
  Tage nach der Bestellung selbst aus dem Escrow zurückholen. Siehe
  [Mit USDC zahlen](#paying-with-usdc).
- **Karte, über Stripe Connect.** Auf einer Edge optional. Die Zahlung
  bleibt auf dem Stripe-Guthaben der Plattform, bis der Zeitraum des Käufers
  endet. Dann überweist die Edge den Anteil des Verkäufers, erstattet sie
  oder teilt sie auf, genauso wie beim Escrow. Eine Edge kann nur USDC
  annehmen, ohne Kartenzahlungen, sodass der Betrieb des Marktplatzes kein
  Geschäftskonto bei einem Kartenabwickler braucht.

Das sind die einzigen Zahlungswege auf einer Edge, die nicht zusätzlich auf
direkte Zahlungen eingestellt ist. Mit direkten Zahlungen sehen Käufer
außerdem:

- **USDC auf Solana, auf einer Edge ohne das Escrow-Programm.** Eine direkte
  Überweisung, in der eigenen Transaktion des Käufers zwischen Verkäufer und
  Kasse aufgeteilt.
- **Bitcoin, Monero und Zcash.** Direkt in die eigene Wallet des Verkäufers,
  an eine neue Adresse für jede Bestellung, abgeleitet aus dem öffentlichen
  Schlüssel des Verkäufers: einem erweiterten öffentlichen Schlüssel oder
  Deskriptor für Bitcoin, der Hauptadresse und dem privaten View-Key für
  Monero, dem Unified Viewing Key für Zcash. Die Edge schaut nur zu und hält
  nie Coins. Die Gebühr von 4 % auf diese Verkäufe ist eine Schuld, die der
  Verkäufer in USDC begleicht, siehe
  [Plattformgebühren aus Coin-Verkäufen](#platform-fees-on-coin-sales).

Was direkt gezahlt wird, lässt sich nicht zurückhalten. Ein Käufer, der so
zahlt, kann daher kein Geld zurückbekommen, wenn die Box des Verkäufers
offline geht.

Für Kartenzahlungen ist die Edge die Stripe-**Plattform**:

1. Ein Verkäufer durchläuft das Onboarding. Die Edge legt für ihn ein
   verbundenes Stripe-Express-Konto an, **versieht es mit der UUID der Box**
   (Metadaten `losos_box_uuid`) und gibt den von Stripe gehosteten
   Onboarding-Link zurück. Eine Box, die das Onboarding durchlaufen hat, bevor
   es die Kennzeichnung gab, wird gekennzeichnet, wenn ihr Besitzer die Seite
   das nächste Mal öffnet.
2. Stripe teilt der Edge mit (`account.updated`), wann das Konto Überweisungen
   empfangen kann. Auf einer Edge ohne Coins kann der Verkäufer erst dann
   etwas anbieten.
3. Ein Käufer bestellt Einheiten eines Angebots. Die Edge reserviert sie und
   erstellt eine Stripe Checkout Session als Zahlung an die Plattform, mit
   der Bestellung als `transfer_group` und dem Konto des Verkäufers und der
   Gebühr in ihren Metadaten. Beim Verkäufer kommt noch nichts an.
4. Stripe teilt der Edge mit (`checkout.session.completed`), dass die Zahlung
   eingegangen ist. Die Bestellung wird `paid`, und ihr Zeitraum beginnt.
5. Ist der Zeitraum vorbei, überweist die Edge den Anteil des Verkäufers aus
   dieser Zahlung (`source_transaction`), erstattet dem Käufer oder tut von
   beidem einen Teil. Die Plattform behält die Gebühr auf das, was der
   Verkäufer ausgezahlt bekommt.

Die Gebühr beträgt `feeBps` Basispunkte des Bruttobetrags, kaufmännisch
gerundet. Beim Standardwert 400 bringt ein Verkauf über 20.00 EUR der
Plattform 0.80 und dem Verkäufer 19.20. Der konfigurierte Wert ist auf 2000
(20 %) begrenzt.

## Bezahlte Zeiträume und Erstattungen {#paid-periods-and-refunds}

Eine bezahlte Bestellung kauft einen Zeitraum: einen Monat Speicher,
Rechenleistung oder eine Maschine. Die Edge hält das Geld des Käufers für
diesen Zeitraum zurück und beobachtet die Box des Verkäufers, solange er
läuft, sodass keine Seite mit dem Anteil der anderen davongehen kann.

- **Der Verkäufer kann das Angebot nicht zurückziehen.** Solange auf einem
  Angebot der Zeitraum eines Käufers läuft, ist Nicht mehr verkaufen
  ausgegraut und sagt, bis wann. Die Edge lehnt es ab, das Angebot vorher zu
  schließen.
- **Die Edge prüft, ob die Box da ist.** Sie fragt das Mesh bei jedem
  Reconcile-Durchlauf, ob der Node des Verkäufers `Ready` ist, und nimmt den
  Heartbeat der Box, wo sie kein Mesh fragen kann. Zeit, in der die Edge
  selbst ausgefallen oder zu beschäftigt zum Nachsehen war, zählt nie gegen
  den Verkäufer.
- **Ist die Box offline, bekommt der Käufer Geld zurück.** Ist die Box des
  Verkäufers während des Zeitraums 24 Stunden am Stück oder insgesamt 48
  Stunden offline, endet der Zeitraum dort. Der Käufer bekommt den Anteil des
  Zeitraums zurück, der nicht geliefert wurde, auf den Cent aufgerundet, und
  der Verkäufer wird für den Rest bezahlt, abzüglich der Gebühr auf diesen
  Teil. Ist die Box von Anfang an offline, wird die ganze Zahlung erstattet.
- **Nichts eingerichtet, nichts bezahlt.** Ist das Gekaufte nicht innerhalb
  von 23 Stunden nach der Zahlung eingerichtet, bekommt der Käufer alles
  zurück.
- **Der Verkäufer wird bezahlt, wenn der Zeitraum endet.** Die Gruppe
  Einnahmen im Markt-Bereich zeigt, was gehalten wird, was bereit ist und was
  unterwegs ist. Auszahlen schickt sofort, was bereit ist, und ist
  ausgegraut, bis ein Zeitraum endet. Die Edge zahlt auch von selbst aus,
  einen Tag nach dem Ende jedes Zeitraums, und immer früh genug, dass ein
  USDC-Käufer keine Einzahlung für einen gelieferten Zeitraum zurückholen
  kann.
- **Der Käufer kann einen Zeitraum, der geliefert wird, nicht kündigen** und
  keine Erstattung anstoßen. Das tun nur die Prüfungen der Edge.

Eine Kartenzahlung, die jemand bei seiner Bank anficht, bleibt Stripe
überlassen: Die Edge zahlt sie weder aus noch erstattet sie, solange der
Streitfall offen ist.

## Was verkauft wird {#what-is-for-sale}

| Art       | Einheit      | Bedeutung                                              |
| --------- | ------------ | ------------------------------------------------------ |
| `storage` | GiB-Monat    | Kapazität im Longhorn-Pool des Mesh                    |
| `compute` | vCPU-Stunde  | Scheduling auf dem Node des Verkäufers, in seinem Fenster |
| `vm`      | Replikat-Monat | Ein Replikat einer virtuellen Maschine auf der Box des Verkäufers, 50/50 geteilt |

Maschinen haben eine eigene Seite, [Virtuelle Maschinen](/in-depth/virtual-machines.md),
und einen eigenen Bereich in der Admin-Oberfläche.

Preise sind in der kleinsten Einheit (Cent) einer Währung pro Edge angegeben
(`losos.edge.market.currency`). Eine einzelne Bestellung muss insgesamt
mindestens 50 kleinste Einheiten betragen, das Minimum von Stripe.

## Erfüllung {#fulfilment}

Eine bezahlte Bestellung ist ein **Anspruch für 30 Tage** (`expires_at`).
Recheneinheiten gehen nach Ablauf an das Angebot zurück. Speichereinheiten
kehren erst zurück, wenn der Claim verschwunden ist, denn der Claim belegt
nach Monatsende weiterhin Longhorn-Kapazität.

- **Speicher.** Die Edge legt im Mesh-Cluster den Namespace
  `market-<buyer id>` an sowie einen `PersistentVolumeClaim` in der gekauften
  Größe, benannt nach der Bestellung (`ord-...`), aus
  `losos.edge.market.storageClass` (Standard `longhorn`). Das läuft nach
  jedem Reconcile-Durchlauf und direkt nach einem Webhook über eine Zahlung.
  Es ist idempotent, da eine `AlreadyExists`-Antwort als erledigt zählt, und
  wird in jedem Reconcile-Intervall wiederholt, wenn der Apiserver ablehnt.
  Die Bestellung gilt erst als erfüllt, wenn der Claim `Bound` ist. Ein Claim,
  der `Pending` bleibt (keine solche Storage Class oder keine Kapazität), wird
  bei jedem Durchlauf erneut geprüft. Das setzt eine Klasse mit
  `Immediate`-Binding voraus, was die Standardklasse von Longhorn ist. Eine
  `WaitForFirstConsumer`-Klasse bindet nie, weil noch nichts den Claim
  einhängt. Die Kontoansicht meldet einen gebundenen Claim als
  `volume: "<namespace>/<claim>"`. Die Edge notiert den Namen des Claims,
  bevor sie ihn anfordert, sodass ein Claim, der nie bindet, oder einer, der
  kurz vor dem Anhalten der Edge entstand, seine Einheiten auch nach
  Monatsende verkauft hält.
- **Backups auf gekauftem Speicher.** Die Box des Käufers kann ihre Backups
  auf einer gebundenen Speicherbestellung ablegen (Einstellungen, Backup).
  Die Box verschlüsselt mit restic, bevor etwas sie verlässt, wie bei einem
  Bucket, und spricht mit `/market/storage/<order>/` auf der Edge, das
  restics REST-Protokoll bedient. Die Edge startet beim ersten Zugriff einen
  `rest-server` pro Bestellung neben dem Claim, aus einem Image, das sie
  mitbringt (`losos.edge.market.backups.enable`, standardmäßig an) und nie
  zieht. Das einzige Konto des Servers ist das der Edge, mit einem Passwort
  nur für diese Bestellung, also öffnen ihn die Mesh-Boxen nicht, die seine
  Cluster-Adresse erreichen. Die Edge öffnet diesen Pfad nur für einen
  Schlüssel, den sie als
  SHA-256-Hash speichert. Die Box des Käufers setzt ihn mit
  `POST /market/storage-door` und leitet ihn aus ihrem
  Wiederherstellungscode und der Bestell-ID ab. Eine neue Box mit demselben
  Wiederherstellungscode erreicht also dasselbe Volume. Die Boxen mit
  Longhorns Replikaten und die Edge sehen nur restics Chiffretext.
- **Rechenleistung.** Ein Guthaben in vCPU-Stunden, aufgeführt unter
  `entitlements`. Noch misst nichts dagegen und plant nichts danach.

:::note[Noch nicht durchgesetzte Grenzen]
Grenzen, die du kennen solltest und von denen noch keine durchgesetzt wird:

- Der Claim ist nicht an den Node des Verkäufers gebunden. Longhorn verteilt
  Replikate über den Pool, also wird der Verkäufer für seinen Beitrag dazu
  bezahlt, nicht dafür, genau dieses Volume zu hosten.
- Der einzige Weg zum Claim ist der Backup-Pfad oben. Nichts anderes hängt
  ihn ein.
- Es wird nie etwas gelöscht. Ein abgelaufener Anspruch wird nicht mehr
  gemeldet, aber das Volume enthält die Daten des Käufers, und das Entfernen
  entscheidet der Betreiber. Bis der Betreiber den Claim löscht, bleiben
  seine GiB verkauft, sodass dieselbe Kapazität nie zweimal verkauft wird. Das
  gilt auch für einen Claim, der noch `Pending` ist, da er noch binden kann.
  Der Reconcile-Durchlauf prüft jeden abgelaufenen Claim und gibt seine
  Einheiten wieder in den Verkauf, sobald der Apiserver für ihn 404 antwortet.
- Der ServiceAccount des Registrars erhält clusterweit `get`/`create` auf
  Namespaces und PersistentVolumeClaims, wenn der Marktplatz aktiviert ist,
  und auf Deployments und Services (und `create` auf Secrets), wenn Backups
  auf gekauftem Speicher an sind. Kubernetes-RBAC kann keines davon auf `market-*`-Namen einschränken.
:::

## Admin-UI {#admin-ui}

Die Zeile **Markt** liegt unter Mesh in der Seitenleiste, und
`/settings/market` öffnet sie. Der Bereich beginnt mit dem Schalter für die
Festplattenfreigabe. Er ist aus dem Bereich Speicher hierher gewandert, weil das Verleihen
von Festplattenplatz an andere Boxen und die Bezahlung dafür eine einzige
Entscheidung sind. Darunter folgt, was der Besitzer gekauft hat, mit Ablauf
und Volume, und das Regal zum Einkaufen. Für den Verkauf gibt es die
Einrichtung der Auszahlungen (eine USDC-Adresse, ein Bitcoin- oder
Monero-Schlüssel und Stripe, wo die Edge Karten annimmt), die offenen
Plattformgebühren aus Coin-Verkäufen, ein Angebotsformular und die eigenen
Angebote und Verkäufe des Besitzers. Der Schalter wird angezeigt, ob die Edge
dieser Box den Marktplatz anbietet oder nicht, denn er ist eine Einstellung
der Box, nicht der Edge.

- Das Angebotsformular bietet nur an, was bereits geteilt wird: Speicher,
  sobald die Box dem Mesh beigetreten ist, Rechenleistung, sobald sie
  zusätzlich Rechenleistung teilt. Alles andere ist ausgegraut, mit Angabe
  des Grundes. Die Edge setzt dieselbe Regel durch, der Bereich erklärt sie
  nur.
- Ein Angebot, für das jemand bezahlt hat, zeigt „bezahlt bis“ mit dem
  Datum, und seine Schaltfläche Nicht mehr verkaufen bleibt bis dahin
  ausgegraut. Jede Bestellung sagt, wo ihr Geld steht: gehalten, unterwegs
  zum Verkäufer, erstattet oder aufgeteilt, mit dem erstatteten Betrag und
  dem Grund.
- Die Gruppe Einnahmen erscheint, sobald ein Verkauf gehalten wird: was für
  laufende Zeiträume gehalten wird, was bereit ist und was unterwegs ist, mit
  einer Schaltfläche Auszahlen, die ausgegraut ist, bis ein Zeitraum endet.
- Kartenzahlung und Stripe-Onboarding öffnen die eigenen Seiten von Stripe in einem
  neuen Tab. Nichts in der Admin-UI sieht eine Karte. lososd und danach noch
  einmal die Seite prüfen, dass der Link eine Seite unter
  `https://checkout.stripe.com` oder `https://connect.stripe.com` ist, sodass
  eine kompromittierte Edge den Besitzer nicht auf ein nachgemachtes
  Kartenformular schicken kann. Eigene Checkout-Domains werden nicht
  unterstützt.
- Die Seiten können die Edge nicht selbst aufrufen, da die CSP der Admin-UI
  `connect-src 'self'` ist. Stattdessen leitet `lososd` weiter:
  `GET /api/market` und
  `POST /api/market/{onboard,listings,listings/close,orders,release}`. Das Relay
  verwendet die URL des Registrars, die Appliance-ID und das Proxy-Token, die
  `losos.proxy.enable` bereits bereitstellt. Es verlangt https und übergibt
  das Token an `curl` über stdin statt auf der Kommandozeile.
- Wo der Marktplatz aus ist (kein Proxy, die Edge hat ihn deaktiviert oder
  dieser Tenant ist nicht freigeschaltet), antwortet `GET /api/market` mit
  `{"available": false}` und Status 200, und der Bereich sagt das auch. Das
  ist absichtlich kein 404. Die Admin-UI behandelt ein 404 für den Rest der
  Sitzung als „diese Box bedient die Route nicht“.

## API {#api}

Alle Routen liegen auf der öffentlichen API des Registrars
(`register.<publicDomain>`). Authentifizierte Routen nehmen dieselben
`appliance_id` und `token` wie `/register`. Bodies sind JSON.

| Route                          | Auth       | Zweck                                                |
| ------------------------------ | ---------- | ---------------------------------------------------- |
| `GET /market/listings`         | keine      | Was jetzt gekauft werden kann. Nennt keinen Verkäufer |
| `POST /market/account`         | Token      | Deine Angebote, Käufe, Verkäufe, Ansprüche           |
| `POST /market/seller/onboard`  | Token      | Stripe-Onboarding starten oder fortsetzen            |
| `POST /market/listings`        | Token      | `kind`, `unit_price`, `capacity`                     |
| `POST /market/listings/close`  | Token      | `listing_id`; bezahlte Bestellungen behalten ihre Einheiten; 409, solange ein bezahlter Zeitraum läuft |
| `POST /market/orders`          | Token      | `listing_id`, `quantity`; liefert eine Checkout-URL  |
| `POST /market/sales/release`   | Token      | Zahlt die Verkäufe aus, deren Zeiträume vorbei sind; 409, solange es keine gibt |
| `POST /market/fees/pay`        | Token      | `tx`, `network`; rechnet eine USDC-Überweisung auf offene Coin-Gebühren an |
| `POST /market/webhook`         | Signatur   | Stripe-Ereignisse                                    |

Jede Route antwortet mit 503, wenn der Marktplatz aus ist. Auf den
Token-Routen erhält ein Tenant ohne das `market`-Bit 403. Die öffentliche
Angebotsansicht und der Webhook gehören keinem Tenant, also passiert ihnen das
nie. `POST /market/account` liefert die 100 jüngsten Käufe und Verkäufe auf
jeder Seite, laufende zuerst. Ein geschlossenes Angebot, bei dem niemand
bestellt hat, wird gelöscht; eines mit Bestellungen bleibt, solange diese
bestehen. Keine Partei erfährt die Appliance-ID der anderen, und die
öffentliche Angebotsansicht zeigt nie eine.

Die Edge reserviert Kapazität, wenn sie die Checkout Session erstellt, und
hält sie, bis Stripe meldet, wie die Session endete: bezahlt
(`checkout.session.completed`) oder abgebrochen (`checkout.session.expired`,
gesendet, wenn die 31 Minuten der Session ablaufen). Ein `expired`-Ereignis
zählt nur, wenn es die Session nennt, die die Bestellung festgehalten hat. Ist
Stripe zum Erstellen der Session nicht erreichbar, gibt die Edge die Kapazität
sofort frei, sofern nicht schon eine Zahlung dafür eingegangen ist. Kommt
keines der beiden Ereignisse je an, verfällt die Reservierung nach dem
dreitägigen Wiederholungsfenster von Stripe, sodass eine durch einen
Edge-Ausfall verzögerte Zahlung ihre Einheiten noch unverkauft vorfindet. Der
nächste Reconcile-Durchlauf verbucht die Bestellung dann als `expired`. Eine
Zahlung, die noch später eintrifft, wird anerkannt, wenn die Einheiten noch
frei sind. Andernfalls protokolliert die Edge sie mit ihrer Session-ID, damit
der Betreiber sie erstatten kann. Zwei Käufer, die um die letzten Einheiten
konkurrieren, können nicht beide eine Session bekommen.

## Einrichtung für Betreiber {#operator-setup}

Diese Schritte richten Kartenzahlungen ein. Eine Edge, die nur Coins annimmt,
überspringt sie; diesen Fall beschreiben die Betreiberhinweise beim Quellcode
(`docs/ci-and-releases.md`).

1. Aktiviere im Stripe-Dashboard **Connect** auf dem Plattformkonto.
2. Versiegle den geheimen Schlüssel (ein eingeschränkter Schlüssel
   genügt) mit `systemd-creds` und lies ihn dabei von stdin, damit der
   Klartext nie die Festplatte berührt:

   ```sh
   systemd-creds encrypt --name=stripe-secret-key - \
     /var/secrets/losos-stripe-secret-key.cred
   ```

   Der Pfad des Blobs ist `losos.edge.market.stripeSecretKeySealed`, und nur
   die Gate-Unit entschlüsselt ihn jemals. Der Name muss genau
   `stripe-secret-key` lauten, denn ein Blob lässt sich nur unter dem Namen
   entschlüsseln, mit dem er versiegelt wurde.
3. Leg zwei Webhook-Endpunkte an, beide unter
   `https://register.<publicDomain>/market/webhook`: einen für Ereignisse auf
   deinem Konto (`checkout.session.completed`, `checkout.session.expired`) und
   einen, der auf **events on Connected accounts** (Ereignisse auf verbundenen
   Konten, `account.updated`) hört. Stripe signiert jeden mit einem eigenen
   Secret, also versiegle beide `whsec_...`-Secrets, eines pro Zeile,
   unter dem Namen `stripe-webhook-secret` in
   `losos.edge.market.webhookSecretSealed` (Standard
   `/var/secrets/losos-stripe-webhook-secret.cred`). Die Edge akzeptiert
   jedes der beiden.
4. Setze `losos.edge.market.enable = true` und
   `losos.edge.market.returnUrl`, eine absolute http(s)-URL ohne Zugangsdaten
   und ohne `#fragment` (Bestellung und Status werden als Query-Parameter
   angehängt).
5. Setze `losos.edge.tenants.<id>.market = true` für jede Box, die
   handeln darf.

:::tip[Im Stripe-Testmodus beginnen]
Beginne im Stripe-**test mode** (Testmodus). Die Edge wurde bisher nur
gegen einen Ersatz für Stripe getestet. Der prüft, was die Edge sendet und wie
sie reagiert, kann aber nicht sagen, ob Stripe es akzeptiert. Das klärt der
erste Lauf im Testmodus.
:::

### Testmodus und Live-Modus {#test-mode-and-live-mode}

Die Edge liest den Modus am Schlüssel ab. Schlüssel mit `sk_test_` und
`rk_test_` betreiben den Marktplatz im Testmodus, `sk_live_` und `rk_live_`
im Live-Modus, und es gibt keine eigene Einstellung, die dem Schlüssel
widersprechen könnte. Das Gate schreibt den Modus beim Start ins Log und
startet nicht mit einem Schlüssel, dessen Modus es nicht lesen kann. Jede
Antwort des Marktplatzes (`/market/listings`, `/market/account`,
Onboarding und Checkout) trägt `"mode": "test"` oder `"mode": "live"`.

Jeder Modus führt sein eigenes Buch: `market.json` für live und
`market-test.json` daneben für test. Live gehen heißt, einen Live-Schlüssel
zu versiegeln und das Gate neu zu starten. Verkäufer, Angebote und
Bestellungen aus dem Test bleiben im Testbuch, das Live-Regal beginnt leer,
und jeder Verkäufer durchläuft das Onboarding mit einem Live-Konto bei Stripe
neu. Ein Testschlüssel bringt das Testbuch zurück, wie es war. Eine
`market.json` von vor der Trennung liest die Edge als Live-Buch. Volumes, die
Testbestellungen belegt haben, bleiben im Mesh, bis der Betreiber sie
entfernt.

Webhook-Secrets tragen keinen Modus, jedes Stripe-Ereignis schon
(`livemode`). Ein signiertes Ereignis aus dem anderen Modus heißt, dass
versiegeltes Webhook-Secret und Schlüssel aus verschiedenen Modi stammen.
Das Gate lehnt es ab und schreibt ins Log, welches welches ist. Stripe
wiederholt ein abgelehntes Ereignis drei Tage lang; wird das passende Paar in
dieser Zeit versiegelt, geht nichts verloren.

## Sicherheitseigenschaften {#safety-properties}

- Der Webhook wird über die HMAC-Signatur von Stripe über den rohen Body
  authentifiziert, mit einem Replay-Fenster von fünf Minuten und einer höheren
  Body-Obergrenze als bei anderen Routen.
- Stripe kann ein Ereignis doppelt oder in anderer Reihenfolge zustellen. Ein
  Ereignis, das die Edge schon angewandt hat, wird anhand seiner ID
  verworfen, und ein `account.updated`, das älter ist als der Kontostand der
  Edge, wird ignoriert. So nimmt ein verspätetes Ereignis einen bereiten
  Verkäufer nicht wieder aus dem Angebot.
- Eine Zahlung zählt nur, wenn Session-ID, Betrag und Währung dem entsprechen,
  was die Edge für die Bestellung festgehalten hat. Eine Abweichung wird
  protokolliert und nicht erfüllt. Hat die Edge angehalten, bevor sie die
  Session-ID notiert hat, wird die ID aus dem signierten Ereignis übernommen,
  da nur das Gate dieser Edge die Bestellung darin benannt haben kann.
- **Der Registrar hält nie den Stripe-Schlüssel.** Eine eigene Unit,
  `losos-stripe-gate` (`losos-registrar stripe-gate`), ist der einzige Prozess,
  der ihn hat. Der Registrar spricht mit dem Gate über einen Unix-Socket
  (`/run/losos-stripe-gate/gate.sock`, 0600) und darf nur darum bitten, ein
  Konto anzulegen, ein Konto zu kennzeichnen, ein Konto zu prüfen, einen
  Onboarding-Link zu erzeugen, eine Checkout Session zu starten, eine
  Webhook-Signatur zu verifizieren oder eine gehaltene Zahlung des
  Marktplatzes abzurechnen. Das Gate lehnt einen Checkout ab, dessen Ziel
  keine `acct_...`-ID ist, dessen Währung von der konfigurierten abweicht,
  dessen Gebühr die Obergrenze von 20 % oder den ganzen Betrag übersteigt,
  dessen Rücksprung-URL keine einfache http(s)-URL ist oder dessen
  Session-Lebensdauer mehr als einen Tag beträgt. Es lehnt auch einen
  Stripe-Endpunkt ab, der nicht `https` ist. Zum Abrechnen liest das Gate die
  Zahlung selbst bei Stripe nach und teilt nur eine Zahlung auf, die es für
  den Marktplatz erstellt hat, zwischen ihrem Käufer und dem beim Checkout
  genannten Verkäufer: nie mehr zurück, als gezahlt wurde, nichts zurück,
  sobald der Verkäufer bezahlt ist, nichts, solange die Zahlung angefochten
  ist, und jede Erstattung und Überweisung höchstens einmal. Es gibt keine
  Operation „leite das an Stripe weiter“. Ein kompromittierter Registrar kann
  daher den Schlüssel nicht lesen, kein Geld irgendwohin schicken außer an
  ein verbundenes Konto innerhalb der Gebührengrenzen des Gates oder zurück an
  den Käufer, der es gezahlt hat, und das eigene Geld der Plattform nicht
  bewegen. Er kann weiterhin Checkouts und Abrechnungen anfordern, denn das
  ist seine Aufgabe. Beide Units laufen als Prozesse auf
  derselben Maschine, und root auf der Edge kommt weiterhin an beide heran.
  Als zweite Schicht markiert die Unit des Registrars die versiegelten Blobs
  und das Credential-Verzeichnis des Gates als unzugänglich.
- Die Edge speichert die Stripe-Secrets nie im Klartext. Sie liegen als
  `systemd-creds`-Blobs vor, versiegelt mit TPM2, wo die Edge eines hat, und
  sonst mit dem Host-Schlüssel, und erreichen das Gate über
  `LoadCredentialEncrypted=`. Im Klartext existieren sie nur im privaten
  Credential-tmpfs dieser Unit, und eine Kopie von `/var` enthält nur
  Chiffretext. Ein Secret zu rotieren heißt, einen neuen Blob zu versiegeln
  und `losos-stripe-gate` neu zu starten. Ein fehlender Blob überspringt das
  Gate, was den Marktplatz abschaltet (503) und sonst nichts. Das Gate ist
  eine eigene Unit, also bemerkt der Master-Proxy im Registrar davon nichts.
- **Das Stripe-Konto trägt die UUID der Box, nicht ihren
  Wiederherstellungscode.** Der Wiederherstellungscode ist ein Zugangsnachweis
  und bleibt auf der Box. Die Box sendet einen daraus abgeleiteten
  Einwegwert, die ersten 16 Bytes von `SHA-256("losos-box-id-v1:" + code)`,
  formatiert als UUID. Er bleibt über die gesamte Lebensdauer der Installation
  gleich und verrät nichts, woraus sich der Code rekonstruieren ließe. Der
  Registrar prüft, dass es eine kanonische UUID ist. Der Browser kann sie nicht
  wählen, weil lososd sie selbst hinzufügt.
- Secret-Dateien werden vor der Verwendung auf ihre Form geprüft
  (`sk_`/`rk_`, `whsec_`).
- `market.json` hat 0600 und wird atomar geschrieben. Eine Datei, die sich
  nicht parsen lässt, verhindert den Start der Edge. Sie als leer zu behandeln
  hieße zu vergessen, wer wofür bezahlt hat.
- Fehlertexte und Endpunkte von Stripe bleiben im Journal; Aufrufer erhalten
  eine feste Meldung.

## Mit USDC zahlen {#paying-with-usdc}

Ein Edge kann Speicher, Rechenleistung und Maschinen in USDC abrechnen statt
über Stripe. Hardware bleibt bei Stripe. Welche Netze er annimmt, wählt der
Edge:

| Netz         | Für einen Test-Edge |
| ------------ | ------------------- |
| Base         | Base Sepolia        |
| Arbitrum One | Arbitrum Sepolia    |
| OP Mainnet   | OP Sepolia          |
| Solana       | Solana Devnet       |

Auf Solana hält das Programm `losos-escrow` die Zahlung, wo der Edge eines
nennt. Solana rechnet in Sekunden und für einen Bruchteil eines Cents ab,
deshalb bietet der Bereich es zuerst an. Ein Edge ohne das Programm bietet
Solana nur an, wenn er auch direkte Zahlungen annimmt, da sich dort nichts
zurückhalten ließe.

Ein Edge läuft auf Live-Netzen oder auf Testnetzen, nie auf beiden. Bei
einem Edge, der Karten annimmt, folgt der Modus seinem Stripe-Schlüssel:
Live-Netze mit einem `sk_live_`-Schlüssel, Testnetze mit `sk_test_`. Ein Edge,
der nur Coins annimmt, läuft im Modus seiner Netze. Eine Mischung lehnt der Registrar ab, damit keine
Testzahlung im Live-Buch landet. Jedes Netz hat einen eigenen RPC-Endpunkt,
mit systemd-creds versiegelt und nur vom Registrar gelesen. Die Einrichtung
beschreiben die Betreiberhinweise beim Quellcode (`docs/ci-and-releases.md`).

### So zahlst du {#how-you-pay}

1. Im Markt eine Wallet anlegen, eine 12- oder 24-Wort-Phrase importieren
   oder eine Wallet im Browser verbinden: MetaMask, Rabby, Rainbow oder
   Coinbase Wallet für die Ethereum-Netze, Phantom, Solflare oder Backpack
   für Solana. Eine Phrase ergibt eine Adresse auf jedem Netz, das der Edge
   annimmt, abgeleitet wie in diesen Wallets (`m/44'/60'/0'/0/0` für
   Ethereum, `m/44'/501'/0'/0'` für Solana), also lässt sich eine hier
   erzeugte Phrase dort importieren und umgekehrt. Die Phrase wird im
   Browser verschlüsselt. Das Gerät bekommt sie nie.
2. Der Bereich zeigt das USDC der Wallet auf jedem Netz, daneben die Münze,
   mit der dort Gebühren bezahlt werden: ETH, auf Solana SOL.
3. Aus dem Angebot bestellen. Es gibt keine Stripe-Seite. Der Bereich zeigt
   den Betrag in USDC und fragt, auf welchem Netz gezahlt wird. Zuerst bietet
   er den Escrow auf Solana an, außer die Wallet hat das USDC nur auf einem
   anderen Netz; dann das Netz, auf dem die Wallet genug hat, mit der
   niedrigsten Gebühr.
4. Zahlen. Auf einem Ethereum-Netz genehmigt die Wallet USDC für
   `LososEscrow` und ruft `pay` auf. Der Vertrag hält die Zahlung und
   vermerkt Verkäufer, Gebühr und eine Rückgabefrist; beim Verkäufer kommt
   noch nichts an. Auf Solana ruft eine Transaktion `pay` des
   Escrow-Programms auf, das für die Bestellung eine Einlage eröffnet, das
   USDC hineinlegt und dieselben drei Angaben vermerkt. Die Einlage hält etwa
   0,0043 SOL Miete aus der Wallet, die zu ihr zurückgehen, wenn die Einlage
   geschlossen wird. Ohne das Escrow-Programm schickt eine Transaktion die
   Plattformgebühr an die Treasury und den Rest an den Verkäufer und nennt
   die Bestellung in einem Memo; die erste Zahlung an einen Verkäufer oder
   die Treasury eröffnet auch deren USDC-Konto, was etwas SOL kostet.
5. Der Edge liest die Transaktion über seinen Endpunkt für dieses Netz und
   markiert die Bestellung als bezahlt. Eine Transaktion bezahlt nur eine
   Bestellung. Die Bestellung nennt dann ihr Netz und verlinkt die
   Transaktion im Block-Explorer dieses Netzes, für Käufer und Verkäufer.
6. Auf einem Ethereum-Netz prüft der Edge danach die Adresse des Verkäufers
   in diesem Netz. Kann sie dort kein USDC empfangen, zahlt der Edge die
   ganze Zahlung an den Käufer zurück, und die Bestellung wird nicht
   ausgeführt. Sonst, und auf Solana immer, bleibt die Einzahlung im Escrow,
   solange der Zeitraum läuft, und der Edge rechnet sie ab, wie
   [Bezahlte Zeiträume und Erstattungen](#paid-periods-and-refunds)
   beschreibt: die Gebühr an die Treasury und den Rest an den Verkäufer,
   alles zurück an den Käufer oder eine Aufteilung. Die Bestellung zeigt,
   was davon geschah.

Bevor er eine Zahlung auf einem Ethereum-Netz zählt, prüft der Edge, dass der
RPC-Endpunkt für die Chain dieses Netzes antwortet, dass `LososEscrow`
Circles USDC dieser Chain einzieht, an die eingestellte Treasury zahlt und
Anweisungen vom eigenen Schlüssel des Edge annimmt, und dass über der Zahlung
schon ein Block liegt. `LososEscrow` nimmt höchstens die Hälfte einer
Zahlung, die feste Teilung bei Maschinen; die genaue Gebühr und Rückgabefrist
jeder Bestellung prüft der Edge.

Eine Verkäuferadresse kann USDC empfangen, wenn sie in diesem Netz keinen
Vertragscode hat oder nur eine Delegation nach EIP-7702, sodass sie ein Konto
bleibt, zu dem jemand den Schlüssel hat. Dass jemand ihn hat, hat schon die
Signatur der Auszahlungsadresse gezeigt. Die Treasury, der USDC-Vertrag, das
Escrow und die Nulladresse zählen nie. Eine Einzahlung kann nur an
Verkäufer und Treasury freigegeben werden, die bei der Zahlung vermerkt
wurden, an den Käufer zurückgehen oder zwischen beiden aufgeteilt werden
(`settle`); anderswohin schicken kann sie nichts. Wird eine Einzahlung 33
Tage nach der Bestellung noch gehalten, holt der Käufer sie direkt aus dem
Vertrag zurück. Der Edge zahlt mindestens zwei Tage vorher aus.

### Zwei von drei Schlüsseln

Eine gehaltene Zahlung bewegt sich nur, wenn zwei von drei Schlüsseln
zustimmen: der des Edge, der der Box des Käufers und der der Box des
Verkäufers. Die Wallet nennt beim Bezahlen die Schlüssel beider Boxen aus
der Rechnung, und der Edge zählt die Zahlung nur, wenn es die Schlüssel
sind, die ihm die beiden Boxen gegeben haben. Wann eine Zahlung freigegeben,
erstattet oder aufgeteilt wird, entscheidet wie bisher der Edge, und dann
wartet er, bis eine Box zustimmt:

- eine Freigabe braucht die Box des Verkäufers, die nur der Freigabe eines
  eigenen Verkaufs zustimmt;
- eine Erstattung oder Aufteilung braucht die Box des Käufers, die nur der
  Erstattung oder Aufteilung eines eigenen Kaufs zustimmt, oder der
  vollständigen Rückgabe einer Zahlung, die keine Bestellung genommen hat.

Jede Box prüft die Entscheidung vor dem Signieren gegen ihr eigenes Konto,
und der zweite Schlüssel ist die Transaktion des Edge selbst. Ein
gestohlener oder erzwungener Edge-Schlüssel allein bewegt also nichts, und
der Edge kann eine Zahlung auch nicht für immer festhalten, weil der Käufer
sie nach der Frist weiterhin zurückholt.

Der Schlüssel der Box ist das Credential `market-escrow-key`, beim ersten
Start erzeugt und versiegelt wie die anderen Geheimnisse der Box. Er
verlässt die Box nie: lososd leitet daraus einen Schlüssel für die
Ethereum-Netze und einen für Solana ab, meldet beide beim Edge an und
prüft bei jeder neuen Rechnung, dass sie seinen eigenen als den des Käufers
nennt. Eine Box fragt den Edge etwa einmal pro Minute nach Entscheidungen,
die auf sie warten, sodass eine Freigabe ein bis zwei Minuten nach der
Entscheidung des Edge ankommt.

Was das nicht abdeckt:

- Edge und Verkäufer zusammen können weiterhin eine Zahlung für einen
  Zeitraum freigeben, der nie geliefert wurde, und Edge und Käufer zusammen
  eine für einen gelieferten erstatten. Die Höhe einer Aufteilung bestimmt
  der Edge, und die Box des Käufers nimmt sie an.
- Die Box des Käufers kann den Schlüssel des Verkäufers nicht prüfen, der
  über den Edge zu ihr kommt. Ein Edge, der dort seinen eigenen Schlüssel
  einsetzt, könnte eine Zahlung ohne die Box des Verkäufers an den
  vermerkten Verkäufer freigeben; das Geld geht trotzdem nur an diesen
  Verkäufer.
- Die übrigen Zahlungsdaten einer neuen Bestellung, die Adresse des
  Verkäufers und die Beträge, kommen ebenfalls über den Edge. Zwei von drei
  schützt bereits gehaltene Zahlungen, nicht die erste, die ein Edge in
  fremden Händen verlangt.

Auf Solana prüft der Edge, dass der Endpunkt den Genesis-Hash des Netzes
meldet und dass die Konfiguration des Escrow-Programms für die Treasury
Circles USDC und den eigenen Schlüssel des Edge nennt. Dann liest er die
Einlage, die die bestätigte Transaktion angelegt hat, und zählt sie nur, wenn
sie zu dieser Konfiguration gehört und die Bestellung, den Verkäufer und den
genauen Betrag, die Gebühr und die Rückgabefrist nennt. Das Programm hält
dieselben Regeln wie `LososEscrow`: höchstens die Hälfte als Gebühr, eine
Freigabe nur an den vermerkten Verkäufer und die Treasury, eine Erstattung
nur an den Käufer und die eigene Erstattung des Käufers erst nach der Frist.
Eine Einlage wird bei der Auszahlung geschlossen, und ihre Miete geht an den
Käufer zurück. Die kleine Gebühr seiner eigenen Transaktionen zahlt der Edge
in SOL, und er eröffnet die USDC-Konten von Verkäufer und Treasury, wenn sie
keine haben. Ohne das Escrow-Programm prüft er, dass die Transaktion
gelungen und bestätigt ist, dass ihr Memo die Bestellung nennt und dass das
USDC von Verkäufer und Treasury um genau ihre Anteile gewachsen ist.

Eine Bestellung hält ihre Einheiten eine Viertelstunde. In den letzten zwei
Minuten startet der Bereich keine Zahlung mehr, und er behält die Transaktion
einer gesendeten Zahlung, bis der Edge sie gezählt hat, sodass erneutes
Prüfen nie doppelt zahlt. Eine Zahlung nach Ablauf dieser Zeit gilt noch,
solange die Einheiten frei sind. Wurden sie inzwischen verkauft oder hat eine
andere Zahlung die Bestellung schon bezahlt, zahlt der Edge die Zahlung aus
dem Escrow zurück, und der Bereich sagt es. Eine direkte Zahlung auf Solana,
ohne das Escrow-Programm, bleibt beim Verkäufer, und die Rückzahlung muss
außerhalb des Markts geregelt werden.

:::tip[Testmünzen kommen aus Faucets]
Für einen Test-Edge kommt Test-USDC aus
[Circles Faucet](https://faucet.circle.com), der alle vier bedient. Test-ETH
gibt der Faucet jedes Netzes, Test-SOL der Solana-Faucet. Nichts davon hat
einen Wert.
:::

### USDC mit Euro kaufen {#buying-usdc-with-euros}

Eine Wallet mit einer Adresse auf Base kann dort bei
[Binance](https://www.binance.com) USDC mit Euro kaufen, mit deinem eigenen
Binance-Konto. Der Markt-Bereich zeigt die Zeile nur, wenn der Edge
Binance-Connect-Partner ist und auf dem Base-Mainnet abrechnet. Binance
verkauft nur USDC auf Base, Bitcoin und Monero zahlst du also weiter aus
einer eigenen Wallet.

1. Wähle Kaufen. Der Bereich sagt, dass Binance die USDC verkauft, nicht
   LosOS, und listet die Nutzungsbedingungen, Produktbedingungen, den
   Datenschutzhinweis und die Risikowarnung von Binance. Jedes Dokument
   öffnet sich in einem neuen Tab, und das Kästchen zum Akzeptieren bleibt
   gesperrt, bis du alle vier geöffnet hast. Dann gib einen Betrag von 1 bis
   100.000 Euro ein. Der Browser merkt sich, welche Fassung der Bedingungen
   du wann akzeptiert hast.
2. Leg die Bestellung an. Die Box schickt dem Edge nur den Betrag und die
   Adresse der Wallet. Der Edge signiert die Bestellung mit seinem
   Partnerschlüssel, und Binance antwortet mit einem Link zu seiner
   Bezahlseite.
3. Öffne Binance. Dort meldest du dich an, Binance prüft deine Identität,
   zeigt Kurs und Gebühr und nimmt die Zahlung. LosOS sieht deine Identität
   und Zahlungsdaten nie.
4. Solange er offen ist, fragt der Bereich den Edge alle 15 Sekunden nach
   der Bestellung, und nach einem Neuladen verfolgt er sie bis zu sieben Tage
   weiter. Er meldet, wenn Binance die Zahlung entgegennimmt, wenn es die
   USDC schickt und wenn sie geschickt sind, mit einem Link zur Transaktion
   auf Basescan. Behält Binance die USDC stattdessen in deinem Binance-Konto
   oder klappt der Kauf nicht, sagt der Bereich auch das.

Binance entscheidet, in welche Länder es verkauft. Zum Zeitpunkt dieses
Textes (Oktober 2026) hat Binance keine Lizenz nach der EU-Verordnung MiCA
und nimmt keine neuen Käufe von Einwohnern der EU an, ein Käufer in der EU
wird also auf der Seite von Binance abgelehnt.

Der Kauf ist eine Sache zwischen dir und Binance: Erstattungen und
Streitfälle gehen an Binance, nicht an den Edge.

### ENS-Namen und USDC senden {#ens-names-and-sending-usdc}

Der Bereich kann USDC aus der Wallet an eine Adresse auf jedem Netz des Edge
senden. Hat der Edge einen ENS-Endpunkt, zeigt der Bereich neben einer
Ethereum-Adresse auch ihren primären Namen und nimmt beim Senden auf einem
Ethereum-Netz einen Namen statt einer Adresse an. Namen werden im Ethereum
Mainnet nachgeschlagen, für einen Test-Edge auf Sepolia. Gefunden werden nur Namen mit Einträgen auf
der Chain: die Verwaltungsseite erreicht nichts außer dem Gerät, also werden
Namen über ein Gateway außerhalb der Chain nicht nachgeschlagen.

### So bekommst du Geld {#how-you-get-paid}

Mit der Wallet signieren, die USDC empfangen soll. Die Ethereum-Adresse nimmt
Verkäufe auf Base, Arbitrum und OP; sie kommen an, sobald der Zeitraum des
Käufers endet und der Edge die Zahlung aus dem Escrow freigibt. Derselbe
Klick signiert eine Solana-Adresse, die Verkäufe auf Solana auf dieselbe Weise
nimmt, oder direkt auf einem Edge ohne das Escrow-Programm. Der Edge prüft jede
Signatur, speichert nur die Adressen. Ein Verkäufer mit nur einer der beiden
wird nur Käufern auf dieser Art von Netz angeboten.
Dieselbe Phrase lässt sich in MetaMask, Rabby, Rainbow, Trust Wallet oder
Phantom importieren, wenn du das Geld ausgeben willst. Für diese Verkäufe gibt
es kein verbundenes Stripe-Konto.

:::warning[Eine hier angelegte Wallet bleibt in einem Browser]
Eine im Markt angelegte Wallet lebt nur in diesem Browser, deshalb fragt der
Bereich vor dem Anbieten, wohin ihre Verkäufe gehen sollen. Mit ihr wie oben
signieren, oder „Adresse einer anderen Wallet nehmen“ wählen: die Adresse
einer Wallet einfügen, die du anderswo hältst, mit dieser Wallet die Nachricht
signieren, die der Bereich zeigt, und die Signatur einfügen. Der Edge prüft sie
genauso.
:::

## Mit Bitcoin zahlen {#paying-with-bitcoin}

Ein Edge, der in USDC abrechnet und direkte Zahlungen annimmt, kann für
Speicher-, Rechen- und Maschinenbestellungen auch Bitcoin annehmen. Er betreibt dafür einen eigenen
geprunten Bitcoin-Knoten: auf Mainnet für einen Live-Edge, auf testnet4,
Signet oder Regtest für einen Test-Edge. Der Registrar lehnt jede andere
Paarung ab, sodass ein Live-Edge nie Testmünzen zählt. Der Knoten hält eine
reine Beobachtungs-Wallet und keinen privaten Schlüssel.

### Für Bitcoin verkaufen {#selling-for-bitcoin}

Im Markt den erweiterten öffentlichen Schlüssel deiner Wallet in die
Bitcoin-Zeile einfügen: den zpub, xpub oder ypub, den Sparrow, Electrum oder
BlueWallet exportieren (auf einem Test-Edge einen vpub, tpub oder upub),
oder einen Output-Deskriptor mit Bereich. Der Schlüssel kann Adressen
erzeugen, aber nichts ausgeben, und einen privaten Schlüssel lehnt der Edge
ab. Jede Bestellung bekommt dann aus diesem Schlüssel eine Empfangsadresse,
die keine andere Bestellung benutzt hat, sodass die Münzen direkt in deine
Wallet gehen und dort wie jede andere Zahlung erscheinen. Nimm ein Konto nur
für diese Verkäufe: alles, was an eine Adresse geht, nachdem eine Bestellung
sie bekommen hat, zählt für diese Bestellung. Wallets schauen
etwa 20 unbenutzte Adressen voraus; bleiben mehr als 20 Bestellungen
hintereinander unbezahlt, das Gap-Limit in der Wallet erhöhen, um die
Zahlungen danach zu sehen.

Die Plattformgebühr wird nicht von einer Bitcoin-Zahlung abgezogen. Der
ganze Betrag geht an den Verkäufer, und der Edge führt eine laufende Summe
der Gebühr auf diese Verkäufe, die der Verkäufer der Plattform schuldet und
im Bereich sieht (wie sie beglichen wird, steht unter
[Plattformgebühren aus Coin-Verkäufen](#platform-fees-on-coin-sales)). Ein
Verkäufer mit Bitcoin-Schlüssel und ohne USDC-Adresse kann
trotzdem anbieten; Käufer zahlen dann nur in Bitcoin.

### In Bitcoin bezahlen {#paying-in-bitcoin}

Bei der Bestellung unter „Zahlen mit“ Bitcoin wählen. Der Bereich zeigt den
Betrag in BTC, die Adresse, einen QR-Code und einen Link, der eine Wallet
auf dem Computer öffnet. Mit einer beliebigen Wallet zahlen; zurückmelden
muss man nichts. Der Knoten des Edge sieht die Zahlung ankommen, und der
Bereich verfolgt sie von gesehen bis bestätigt.

Der BTC-Betrag ist ein Angebot aus Preisquellen, die übereinstimmen, gültig
für eine Viertelstunde. Eine Bestätigung zählt eine Bestellung bis 50 $,
zwei bis 500 $, drei bis 5.000 $ und sechs darüber. Eine zu kleine Zahlung
sagt, wie viel noch fehlt, und was über den Betrag hinausgeht, bleibt beim
Verkäufer. Eine Zahlung nach Ablauf des Angebots wird am Preis in diesem
Moment gemessen: innerhalb von 1 % wird die Bestellung abgeschlossen, sonst
fragt der Bereich nach der Differenz. Die Bestellung nennt dann Bitcoin als
Netz und verlinkt die Transaktion auf mempool.space.

## Mit Monero zahlen {#paying-with-monero}

Ein Edge, der in USDC abrechnet und direkte Zahlungen annimmt, kann für
dieselben Bestellungen auch Monero annehmen. Er betreibt dafür einen eigenen geprunten Monero-Knoten und daneben
einen Wallet-Dienst: auf Mainnet für einen Live-Edge, auf Stagenet oder
Testnet für einen Test-Edge. Der Registrar lehnt jede andere Paarung ab und
ebenso eine Adresse, die für ein anderes Netz gemacht ist als das des Knotens.

### Für Monero verkaufen {#selling-for-monero}

Im Markt zwei Dinge in die Monero-Zeile einfügen: die Hauptadresse deiner
Wallet, die mit 4 beginnt (für einen Test-Edge mit 5 oder 9), und ihren
privaten View-Key. Feather, Cake Wallet und die offiziellen Wallets zeigen
ihn neben dem Seed, bei den Schlüsseln der Wallet. Mit dem View-Key sieht der
Edge, was an deine Adressen geht, und sonst nichts: ausgeben kann er damit
nicht. Das Gerät prüft das Paar vor dem Senden und lehnt den Spend-Key mit
Namen ab, sodass ein versehentlich eingefügter Spend-Key das Gerät nie
verlässt. Den Seed nie einfügen.

Der Edge hält für dich eine reine View-Wallet und gibt jeder Bestellung die
nächste Subadresse des ersten Kontos deiner Wallet, sodass die Münzen direkt
in deine Wallet gehen. Deine Wallet findet sie auch: Sie schaut auch nach
Subadressen voraus, die sie noch nicht angezeigt hat. Bleiben mehr als etwa
200 Bestellungen hintereinander unbezahlt, die Subadress-Vorschau
(subaddress lookahead) in der Wallet erhöhen, um die Zahlungen danach zu
sehen.

Wie bei Bitcoin schuldet der Verkäufer die Plattformgebühr separat,
und die ganze Zahlung geht an den Verkäufer.

### In Monero bezahlen {#paying-in-monero}

Bei der Bestellung unter „Zahlen mit“ Monero wählen. Der Bereich zeigt den
Betrag in XMR, die Subadresse, einen QR-Code und einen Link, der eine Wallet
öffnet. Mit einer beliebigen Wallet zahlen. Zehn Bestätigungen zählen eine
Bestellung bis 5.000 $, zwanzig darüber; eine zu kleine, eine zu große und
eine verspätete Zahlung werden wie bei Bitcoin behandelt. Eine Zahlung, die
für spätere Blöcke gesperrt ist, zählt nicht. Monero verbirgt, wer wem was
gezahlt hat, daher nennt eine bezahlte Bestellung Monero als Netz ohne Link
zu einem Block-Explorer.

## Mit Zcash zahlen {#paying-with-zcash}

Ein Edge, der in USDC abrechnet und direkte Zahlungen annimmt, kann für
dieselben Bestellungen auch abgeschirmtes Zcash annehmen. Er betreibt keinen Zcash-Knoten. Er fragt
öffentliche lightwalletd-Server nach kompakten Blöcken, standardmäßig vier
Mainnet-Server verschiedener Betreiber, und entschlüsselt sie selbst, sodass
dein Viewing Key den Edge nie verlässt. Eine Zahlung zählt erst, wenn Server
zweier verschiedener Betreiber sie in einem Block mit demselben Hash zeigen;
ein Server, der etwas zeigt, das sonst keiner zeigt, wird überstimmt. Ein
Live-Edge rechnet nur auf Mainnet ab und fragt keinen Server, der ein anderes
Netz meldet; Testnet und Regtest sind für einen Test-Edge.

### Für Zcash verkaufen {#selling-for-zcash}

Im Markt den Unified Viewing Key deiner Wallet in die Zcash-Zeile einfügen:
den vollen, der mit `uview1` beginnt, oder besser den eingehenden, der mit
`uivk1` beginnt und nur sieht, was ankommt, nichts von dem, was du sendest.
Der Schlüssel braucht einen Orchard-Teil, den jede seit 2022 angelegte Wallet
hat. Der Edge behält nur den Incoming Viewing Key dieses Teils, der sieht,
was ankommt, und nichts ausgeben kann. Gerät und Edge lehnen einen Spending
Key und eine Seed-Phrase mit Namen ab, sodass ein versehentlich eingefügter
Schlüssel das Gerät nie verlässt. Der Bereich zeigt die erste Adresse deiner
Wallet an, damit du siehst, an welche Wallet Bestellungen gehen; den
Schlüssel selbst zeigt er nie wieder.

Jede Bestellung bekommt eine neue abgeschirmte Adresse deiner Wallet: eine
reine Orchard-Unified-Adresse mit einem Diversifier-Index weit über denen,
die deine Wallet selbst vergibt. Deine Wallet findet Zahlungen daran von
selbst, weil sie alles entschlüsselt, was an eine Adresse ihres Schlüssels
geht. Seit NU6.3 landet eine Zahlung an eine Orchard-Adresse im
Ironwood-Pool, der dieselben Schlüssel nutzt; der Edge liest beide.

Wie bei Bitcoin schuldet der Verkäufer die Plattformgebühr separat,
und die ganze Zahlung geht an den Verkäufer.

### In Zcash bezahlen {#paying-in-zcash}

Bei der Bestellung unter „Zahlen mit“ Zcash wählen. Der Bereich zeigt den
Betrag in ZEC, die Adresse, einen QR-Code und einen `zcash:`-Zahlungslink
(ZIP 321), der eine Wallet öffnet. Mit einer Wallet zahlen, die an eine
Unified-Adresse senden kann. Es zählen nur Zahlungen in Blöcken nach dem
Zeitpunkt, an dem die Bestellung die Adresse bekam, und eine Zahlung im
Mempool gilt als gesehen, sobald zwei Betreiber sie zeigen. Drei
Bestätigungen zählen eine Bestellung bis 50 $, sechs bis 500 $, zehn bis
5.000 $ und 24 darüber; eine zu kleine, eine zu große und eine verspätete
Zahlung werden wie bei Bitcoin behandelt. Die Zahlung ist abgeschirmt, daher
nennt eine bezahlte Bestellung Zcash als Netz ohne Link zu einem
Block-Explorer.

## Plattformgebühren aus Coin-Verkäufen {#platform-fees-on-coin-sales}

Eine Zahlung in Bitcoin, Monero oder Zcash erreicht die Wallet des
Verkäufers vollständig, daher sind die 4 % der Plattform an diesen
Verkäufen eine Schuld, die die Edge für den Verkäufer führt. Der Bereich
Marktplatz zeigt sie in einer eigenen Zeile unter den Coin-Schlüsseln, mit
den Grenzen weiter unten. Beglichen wird sie in USDC, auf zwei Wegen.

**Aus USDC-Verkäufen.** Solange ein Verkäufer Coin-Gebühren schuldet, erhöht
die Edge die Gebühr seiner nächsten USDC-Bestellungen um den offenen Betrag,
auf den Ethereum-Netzen wie auf Solana. Der Käufer zahlt weiterhin den
angegebenen Preis. Nur der Anteil des Verkäufers ist kleiner, und der
Verkauf im Bereich nennt, wie viel davon auf offene Gebühren ging. Die
Gebühr übersteigt nie die Hälfte des Bestellpreises, das Höchste, was der
Escrow-Vertrag nimmt. Die Schuld sinkt erst, wenn das Geld die Kasse
erreicht: wenn der Escrow die Zahlung freigibt oder eine direkte Zahlung auf
Solana bestätigt ist. Eine Bestellung, die erstattet wird oder verfällt, zieht
nichts ein. Sind mehrere USDC-Bestellungen gleichzeitig offen, nimmt jede
nur, was die anderen nicht schon genommen haben. Zieht ein Verkauf mehr ein,
als noch offen ist, weil eine direkte Zahlung die Schuld inzwischen beglichen
hat, bleibt der Rest als Guthaben und zahlt zuerst die Gebühren der nächsten
Coin-Verkäufe.

**Direkt.** Die Gebührenzeile hat eine Schaltfläche, die den offenen Betrag
in USDC aus der Wallet im Bereich an die Kasse der Plattform sendet, auf
jedem Ethereum-Netz, auf dem die Edge abrechnet, und die Transaktion der
Edge übergibt. Die Edge liest sie von ihrem eigenen Knoten und rechnet sie
an, sobald sie so viele Bestätigungen hat wie eine Bestellzahlung. Sie
rechnet nur eine USDC-Überweisung an die Kasse von der Auszahlungsadresse
des Verkäufers an, jede Transaktion einmal und nie mehr als offen ist. Was
über die Schuld hinaus gezahlt wird, wird nicht erstattet. Auf Solana werden
keine Gebühren bezahlt.

**Wann Coin-Zahlungen stoppen.** Solange ein Verkäufer mehr als die Grenze
der Edge schuldet, 10 US-Dollar, sofern ihr Betreiber keine andere gesetzt
hat, oder seit mehr als 30 Tagen irgendeinen Betrag schuldet, können Käufer
ihn nicht in Bitcoin, Monero oder Zcash bezahlen. Eine neue Bestellung
bietet nur USDC an, und eine neue Coin-Zahlung wird mit dem Grund abgelehnt.
Eine Coin-Zahlung, die schon läuft, wird bis zum Ende verfolgt. USDC
funktioniert weiter und zahlt mit jedem Verkauf auf die Schuld ein, sodass
Coin-Zahlungen zurückkommen, sobald sie beglichen ist. Bei einem Verkäufer,
der nur Coins annimmt, kann bis dahin nicht bestellt werden.

## Nicht abgedeckt {#not-covered}

- **Streitfälle und Erstattungen, die nicht die Edge vorgenommen hat.** Ein
  Streitfall bei einer Karte wird im Stripe-Dashboard bearbeitet. Eine
  Erstattung aus einem anderen Grund, nachdem der Verkäufer bezahlt wurde,
  entscheidet der Betreiber, mit `reverse_transfer`, damit auch der Anteil
  des Verkäufers zurückkommt.
- **Nachweis, dass der Speicher intakt ist.** Eine Box gilt als online,
  solange ihr Node im Mesh `Ready` ist. Die Edge prüft nicht, ob das Volume
  des Käufers antwortet.
- **Durchsetzung des Anspruchs.** Die Erfüllung legt den Claim an; sie hindert
  einen Käufer nicht daran, mehr zu nutzen, als er gekauft hat, misst keine
  Rechenleistung und entzieht bei Ablauf keinen Zugriff.
- **Steuern, Rechnungsstellung und Verkäuferprüfung** über das hinaus, was das
  Onboarding von Stripe leistet. Der Betrieb eines Marktplatzes bringt
  rechtliche Pflichten mit sich, die davon abhängen, wo du tätig bist. Dies
  ist ein Experiment, keine Beratung.
