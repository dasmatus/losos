---
title: Marktplatz (geplant)
sidebar_position: 4
mdx:
  format: md
---

# Marktplatz (geplant)

Ein optionaler Marktplatz, auf dem eine Box freien Speicher oder Rechenleistung
verkauft und eine andere sie kauft. Die Zahlung läuft über **Stripe Connect**,
und die Edge behält 4 % ein, um ihre Betriebskosten zu decken.

**Er ist noch nicht geöffnet.** Der Code ist durchgehend fertig: die
`/market/*`-Routen des Registrars, das Stripe-Gate, das Relay in lososd und der
Bereich in der Admin-UI. Eine Stripe-Connect-Plattform braucht jedoch ein
eingetragenes Unternehmen dahinter, und das gibt es noch nicht. Bis dahin zeigt
die Admin-UI den Tab **Markt** ausgegraut mit einem „soon(TM)“-Badge. Niemand
kann ihn anklicken oder über die Adresse erreichen, und nichts auf einer Box
fragt den Marktplatz irgendetwas. Der Schalter für die Festplattenfreigabe
(`losos.sharingMyStorage`) liegt in diesem Bereich, also öffnet sich die
Freigabe der Festplatte für das Mesh zusammen mit dem Marktplatz, und die UI
kann sie vorher nicht einschalten. Der Rest dieser Seite beschreibt, wie der
Marktplatz funktioniert, sobald er öffnet. Auch dann ist er standardmäßig aus,
und zwar auf drei Ebenen.

|                                  | Standard     | Schalter                                         |
| -------------------------------- | ------------ | ------------------------------------------------ |
| Die Edge bietet ihn an           | aus          | `losos.edge.market.enable`                       |
| Eine Box darf handeln            | aus          | `losos.edge.tenants.<id>.market`                 |
| Ein Verkäufer darf anbieten      | geschlossen  | Stripe meldet das verbundene Konto als bereit    |

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

## Wie das Geld fließt

Die Edge ist die Stripe-**Plattform**. Geld berührt nie eine LosOS-Box.

1. Ein Verkäufer durchläuft das Onboarding. Die Edge legt für ihn ein
   verbundenes Stripe-Express-Konto an, **versieht es mit der UUID der Box**
   (Metadaten `losos_box_uuid`) und gibt den von Stripe gehosteten
   Onboarding-Link zurück. Eine Box, die das Onboarding durchlaufen hat, bevor
   es die Kennzeichnung gab, wird gekennzeichnet, wenn ihr Besitzer die Seite
   das nächste Mal öffnet.
2. Stripe teilt der Edge mit (`account.updated`), wann das Konto Überweisungen
   empfangen kann. Erst dann kann der Verkäufer etwas anbieten.
3. Ein Käufer bestellt Einheiten eines Angebots. Die Edge reserviert sie und
   erstellt eine Stripe Checkout Session als **Destination Charge**. Der
   Käufer zahlt an die Plattform, `transfer_data[destination]` leitet den
   Verkaufserlös an das Konto des Verkäufers weiter, und
   `application_fee_amount` bleibt bei der Plattform.
4. Stripe teilt der Edge mit (`checkout.session.completed`), dass die Zahlung
   eingegangen ist. Die Bestellung wird `paid`.

Die Gebühr beträgt `feeBps` Basispunkte des Bruttobetrags, kaufmännisch
gerundet. Beim Standardwert 400 bringt ein Verkauf über 20.00 EUR der
Plattform 0.80 und dem Verkäufer 19.20. Der konfigurierte Wert ist auf 2000
(20 %) begrenzt.

## Was verkauft wird

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

## Erfüllung

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
- **Rechenleistung.** Ein Guthaben in vCPU-Stunden, aufgeführt unter
  `entitlements`. Noch misst nichts dagegen und plant nichts danach.

Grenzen, die du kennen solltest und von denen noch keine durchgesetzt wird:

- Der Claim ist nicht an den Node des Verkäufers gebunden. Longhorn verteilt
  Replikate über den Pool, also wird der Verkäufer für seinen Beitrag dazu
  bezahlt, nicht dafür, genau dieses Volume zu hosten.
- Der Käufer hat keinen Zugriffsweg zum Claim; er existiert im Cluster, damit
  ein Workload ihn einhängen kann.
- Es wird nie etwas gelöscht. Ein abgelaufener Anspruch wird nicht mehr
  gemeldet, aber das Volume enthält die Daten des Käufers, und das Entfernen
  entscheidet der Betreiber. Bis der Betreiber den Claim löscht, bleiben
  seine GiB verkauft, sodass dieselbe Kapazität nie zweimal verkauft wird. Das
  gilt auch für einen Claim, der noch `Pending` ist, da er noch binden kann.
  Der Reconcile-Durchlauf prüft jeden abgelaufenen Claim und gibt seine
  Einheiten wieder in den Verkauf, sobald der Apiserver für ihn 404 antwortet.
- Der ServiceAccount des Registrars erhält clusterweit `get`/`create` auf
  Namespaces und PersistentVolumeClaims, wenn der Marktplatz aktiviert ist.
  Kubernetes-RBAC kann keines von beiden auf `market-*`-Namen einschränken.

## Admin-UI

Derzeit ist die Zeile **Einstellungen, Markt** ausgegraut und mit „soon(TM)“
beschriftet. Sie ist eine deaktivierte Schaltfläche, die die Tastatur
überspringt, und `/settings/market` öffnet stattdessen den Standardbereich.
Zum Öffnen genügt ein Flag, `planned` an der Zeile in
`admin-ui/app/src/screens/settings/panes.ts`, plus der passende Schalter in
`admin-ui/app/tests/app.browser.mjs`, der die Browser-Prüfungen des Bereichs
bis dahin zurückhält.

Einmal geöffnet, beginnt der Bereich mit dem Schalter für die
Festplattenfreigabe. Er ist aus dem Bereich Speicher hierher gewandert, weil das Verleihen
von Festplattenplatz an andere Boxen und die Bezahlung dafür eine einzige
Entscheidung sind. Darunter folgt, was der Besitzer gekauft hat, mit Ablauf
und Volume, und das Regal zum Einkaufen. Für den Verkauf gibt es die
Einrichtung der Stripe-Auszahlungen, ein Angebotsformular und die eigenen
Angebote und Verkäufe des Besitzers. Der Schalter wird angezeigt, ob die Edge
dieser Box den Marktplatz anbietet oder nicht, denn er ist eine Einstellung
der Box, nicht der Edge.

- Das Angebotsformular bietet nur an, was bereits geteilt wird: Speicher,
  sobald die Box dem Mesh beigetreten ist, Rechenleistung, sobald sie
  zusätzlich Rechenleistung teilt. Alles andere ist ausgegraut, mit Angabe
  des Grundes. Die Edge setzt dieselbe Regel durch, der Bereich erklärt sie
  nur.
- Zahlung und Stripe-Onboarding öffnen die eigenen Seiten von Stripe in einem
  neuen Tab. Nichts in der Admin-UI sieht eine Karte. lososd und danach noch
  einmal die Seite prüfen, dass der Link eine Seite unter
  `https://checkout.stripe.com` oder `https://connect.stripe.com` ist, sodass
  eine kompromittierte Edge den Besitzer nicht auf ein nachgemachtes
  Kartenformular schicken kann. Eigene Checkout-Domains werden nicht
  unterstützt.
- Die Seiten können die Edge nicht selbst aufrufen, da die CSP der Admin-UI
  `connect-src 'self'` ist. Stattdessen leitet `lososd` weiter:
  `GET /api/market` und
  `POST /api/market/{onboard,listings,listings/close,orders}`. Das Relay
  verwendet die URL des Registrars, die Appliance-ID und das Proxy-Token, die
  `losos.proxy.enable` bereits bereitstellt. Es verlangt https und übergibt
  das Token an `curl` über stdin statt auf der Kommandozeile.
- Wo der Marktplatz aus ist (kein Proxy, die Edge hat ihn deaktiviert oder
  dieser Tenant ist nicht freigeschaltet), antwortet `GET /api/market` mit
  `{"available": false}` und Status 200, und der Bereich sagt das auch. Das
  ist absichtlich kein 404. Die Admin-UI behandelt ein 404 für den Rest der
  Sitzung als „diese Box bedient die Route nicht“.

## API

Alle Routen liegen auf der öffentlichen API des Registrars
(`register.<publicDomain>`). Authentifizierte Routen nehmen dieselben
`appliance_id` und `token` wie `/register`. Bodies sind JSON.

| Route                          | Auth       | Zweck                                                |
| ------------------------------ | ---------- | ---------------------------------------------------- |
| `GET /market/listings`         | keine      | Was jetzt gekauft werden kann. Nennt keinen Verkäufer |
| `POST /market/account`         | Token      | Deine Angebote, Käufe, Verkäufe, Ansprüche           |
| `POST /market/seller/onboard`  | Token      | Stripe-Onboarding starten oder fortsetzen            |
| `POST /market/listings`        | Token      | `kind`, `unit_price`, `capacity`                     |
| `POST /market/listings/close`  | Token      | `listing_id`; bezahlte Bestellungen behalten ihre Einheiten |
| `POST /market/orders`          | Token      | `listing_id`, `quantity`; liefert eine Checkout-URL  |
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

## Einrichtung für Betreiber

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

Beginne im Stripe-**test mode** (Testmodus). Die Edge wurde bisher nur
gegen einen Ersatz für Stripe getestet. Der prüft, was die Edge sendet und wie
sie reagiert, kann aber nicht sagen, ob Stripe es akzeptiert. Das klärt der
erste Lauf im Testmodus.

Die Schritte 2 und 3 können auch aus GitHub-Actions-Secrets kommen statt aus
einer Shell auf der Edge; siehe [Schlüssel aus GitHub Actions](#keys-from-github-actions).

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

### Schlüssel aus GitHub Actions {#keys-from-github-actions}

Der Workflow `edge-credentials` (`.github/workflows/edge-credentials.yml`)
versiegelt die Schlüssel aus den Actions-Secrets des Repositorys, damit
niemand sie auf der Edge einfügt.

1. Erzeuge einen SSH-Schlüssel für den Workflow
   (`ssh-keygen -t ed25519 -N "" -f edge-credentials`) und setze
   `losos.edge.credentials.deployKey` auf seine öffentliche Hälfte. Die Edge
   lässt diesen Schlüssel als root nur `losos-seal-credential` ausführen:
   keine Shell, kein Forwarding, kein Terminal.
2. Lege die Actions-Secrets an:

   | Secret                  | Inhalt                                                     |
   | ----------------------- | ---------------------------------------------------------- |
   | `STRIPE_KEY`            | geheimer oder eingeschränkter Stripe-Schlüssel, test/live  |
   | `STRIPE_WEBHOOK_SECRET` | optional: beide `whsec_`-Secrets, eines pro Zeile          |
   | `CLAUDE_KEY`            | der Claude-API-Schlüssel (`sk-ant-...`)                    |
   | `EDGE_SSH_KEY`          | die private Hälfte des Schlüssels aus Schritt 1            |
   | `EDGE_SSH_HOST`         | die Adresse der Edge (Secret oder Variable)                |
   | `EDGE_SSH_KNOWN_HOSTS`  | die Zeile der Edge aus `ssh-keyscan` (Secret oder Variable) |
   | `EDGE_SSH_PORT`         | optional, sonst 22                                         |

3. Starte **edge-credentials** auf `main` im Tab Actions. Er gibt aus, ob der
   Stripe-Schlüssel ein Test- oder ein Live-Schlüssel ist, nie den Schlüssel
   selbst, und reicht jedes gesetzte Secret an `losos-seal-credential`
   weiter. Das prüft die Form, versiegelt es mit `systemd-creds` unter seinem
   Namen und startet das Gate neu. Einen Schlüssel tauschen heißt, das Secret
   zu ändern und den Workflow erneut zu starten.

Der Claude-Schlüssel wird als `claude-api-key` nach
`/var/secrets/losos-claude-api-key.cred` versiegelt
(`losos.edge.credentials.secrets.claude-api-key.sealed`). Der Registrar liest ihn
mit `LoadCredentialEncrypted=claude-api-key:<dieser Pfad>`, und ein neu
versiegelter Schlüssel startet `losos-registrar.service` neu.

## Sicherheitseigenschaften

- Der Webhook wird über die HMAC-Signatur von Stripe über den rohen Body
  authentifiziert, mit einem Replay-Fenster von fünf Minuten und einer höheren
  Body-Obergrenze als bei anderen Routen.
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
  Onboarding-Link zu erzeugen, eine Checkout Session zu starten oder eine
  Webhook-Signatur zu verifizieren. Das Gate lehnt einen Checkout ab, dessen
  Ziel keine `acct_...`-ID ist, dessen Währung von der konfigurierten
  abweicht, dessen Gebühr die Obergrenze von 20 % oder den ganzen Betrag
  übersteigt, dessen Rücksprung-URL keine einfache http(s)-URL ist oder
  dessen Session-Lebensdauer mehr als einen Tag beträgt. Es lehnt auch einen
  Stripe-Endpunkt ab, der nicht `https` ist. Es gibt keine Operation „leite
  das an Stripe weiter“. Ein kompromittierter Registrar kann daher den
  Schlüssel nicht lesen, kein Geld irgendwohin schicken außer an ein
  verbundenes Konto innerhalb der Gebührengrenzen des Gates und keine
  Erstattungen oder Auszahlungen auslösen. Er kann weiterhin Checkouts
  anfordern, denn das ist seine Aufgabe. Beide Units laufen als Prozesse auf
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

## Nicht abgedeckt

- **Erstattungen und Streitfälle.** Bearbeite diese im Stripe-Dashboard
  mit `reverse_transfer` und `refund_application_fee`, damit sowohl der Anteil
  des Verkäufers als auch der Anteil der Plattform zurückfließen. Der
  Marktplatz bildet keines von beiden ab.
- **Durchsetzung des Anspruchs.** Die Erfüllung legt den Claim an; sie hindert
  einen Käufer nicht daran, mehr zu nutzen, als er gekauft hat, misst keine
  Rechenleistung und entzieht bei Ablauf keinen Zugriff.
- **Steuern, Rechnungsstellung und Verkäuferprüfung** über das hinaus, was das
  Onboarding von Stripe leistet. Der Betrieb eines Marktplatzes bringt
  rechtliche Pflichten mit sich, die davon abhängen, wo du tätig bist. Dies
  ist ein Experiment, keine Beratung.
