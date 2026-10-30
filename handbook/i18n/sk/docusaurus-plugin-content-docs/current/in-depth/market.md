---
title: Trh
sidebar_position: 4
mdx:
  format: md
---

# Trh

Voliteľný trh, na ktorom jedno zariadenie predáva voľné úložisko alebo výpočtový
výkon a iný ho kupuje. Kupujúci platia kartou cez **Stripe Connect**, ak edge
karty prijíma, alebo v USDC na Solane, Base, Arbitrum alebo OP. Edge každú
platbu drží, kým neskončí obdobie, za ktoré kupujúci zaplatil, vráti peniaze
kupujúcemu, keď zariadenie predajcu prestane dodávať, a ponecháva si 4 % na
pokrytie svojich prevádzkových nákladov. Edge sa dá nastaviť aj na príjem
bitcoinov, Monera a Zcashu, ktoré prídu predajcovi hneď, a preto sa nedajú
vrátiť.

Riadok **Trh** pod Mesh v bočnom paneli otvára panel na adrese
`/settings/market`. Zariadenie ukáže trh, len keď je v dosahu oficiálny edge
LosOS; inak panel povie, že trh na tomto zariadení nie je dostupný. Prepínač
zdieľania disku (`losos.sharingMyStorage`) je na tomto paneli a dá sa zapnúť
v oboch prípadoch, takže stránka Stroje funguje, len čo je zdieľanie zapnuté.
Trh je predvolene vypnutý, a to na troch úrovniach.

|                            | Predvolene | Prepínač                                      |
| -------------------------- | ---------- | --------------------------------------------- |
| Edge ho poskytuje          | vypnuté    | `losos.edge.market.enable`                    |
| Zariadenie môže obchodovať | vypnuté    | `losos.edge.tenants.<id>.market`              |
| Predajca môže ponúkať      | zatvorené  | Stripe hlási pripojený účet ako pripravený, alebo na edge s mincami predajca nastavil adresu pre výplaty v USDC alebo kľúč pre mince |

Zariadenie, ktoré nikdy neobchoduje, to neovplyvní. Trh je funkcia
`losos-registrar` na edge.

**Trh predáva to, čo už zdieľate, a nič iné.** Platí zariadeniu za úložisko
a výpočtový výkon, ktorými prispieva do mesh. Nie je to samostatný produkt.
Zariadenie môže ponúkať úložisko, len kým je jeho uzol zaradený do mesh, a výpočtový
výkon, len kým zároveň zdieľa výpočtový výkon (`losos.cluster.shareCompute`).
Prestaňte zdieľať a vaše ponuky okamžite zmiznú z pultu. Vrátia sa, keď
zdieľanie obnovíte, a už zaplatené objednávky to neovplyvní.

## Ako sa pohybujú peniaze {#how-the-money-moves}

Peniaze sa nikdy nedotknú zariadenia LosOS. Kupujúci platia jedným z týchto
spôsobov, podľa toho, čo edge dovolí:

- **USDC na Solane, Base, Arbitrum One alebo OP Mainnet.** Platba ide do
  escrow: do programu `losos-escrow` na Solane, do kontraktu `LososEscrow` na
  ostatných. Edge ju overí (adresu predajcu, sumu, potvrdenia) a drží ju
  tam, kým obdobie kupujúceho neskončí. Potom ju uvoľní predajcovi
  a pokladnici platformy, ktorá si berie 4 % poplatok. Alebo celú platbu
  vráti kupujúcemu: keď je adresa predajcu nesprávna alebo je to kontrakt,
  keď objednávka už bola zaplatená, keď jej jednotky prepadli, alebo keď sa
  kúpené nepodarilo včas pripraviť. Keď bolo zariadenie predajcu príliš dlho
  offline, platbu rozdelí: kupujúci dostane späť podiel, ktorý nebol
  dodaný, a predajca si ponechá zvyšok. Ak edge nič neurobí, kupujúci si
  môže vklad z escrow vziať späť 33 dní po objednaní. Pozri
  [Platba v USDC](#paying-with-usdc).
- **Karta cez Stripe Connect.** Na edge voliteľná. Platba zostáva na
  zostatku platformy v Stripe, kým obdobie kupujúceho neskončí, potom edge
  prevedie podiel predajcu, vráti ju alebo ju rozdelí, rovnako ako pri
  escrow. Edge môže prijímať len USDC, bez platieb kartou, takže na
  prevádzku trhu netreba firemný účet u spracovateľa kariet.

Na edge, ktorý nie je nastavený aj na priame platby, sú to jediné dva
spôsoby platby. Keď sú priame platby zapnuté, kupujúci vidia aj:

- **USDC na Solane, na edge bez programu escrow.** Priamy prevod, rozdelený
  medzi predajcu a pokladnicu v transakcii samotného kupujúceho.
- **Bitcoin, Monero a Zcash.** Priamo do vlastnej peňaženky predajcu, na
  novú adresu pre každú objednávku, odvodenú z verejného kľúča predajcu:
  rozšíreného verejného kľúča alebo deskriptora pre Bitcoin, primárnej
  adresy a súkromného kľúča na prezeranie pre Monero, zjednoteného kľúča na
  prezeranie pre Zcash. Edge len sleduje a mince nikdy nedrží. 4 % poplatok
  z týchto predajov je dlh, ktorý predajca vyrovná v USDC, pozri
  [Poplatky platformy z predajov v minciach](#platform-fees-on-coin-sales).

Nič zaplatené priamo sa nedá podržať, takže kupujúci, ktorý takto platí,
nemôže dostať peniaze späť, keď je zariadenie predajcu offline.

Pri platbách kartou je edge Stripe **platforma**:

1. Predajca sa zaregistruje (onboarding). Edge mu vytvorí pripojený účet
   Stripe Express, **označí ho UUID zariadenia** (metadáta `losos_box_uuid`)
   a vráti odkaz na onboarding hostovaný Stripe. Zariadenie zaregistrované ešte
   predtým, než označovanie existovalo, sa označí, keď jeho vlastník nabudúce
   otvorí stránku.
2. Stripe oznámi edge (`account.updated`), keď účet môže prijímať prevody.
   Na edge, ktorý neprijíma mince, môže predajca niečo ponúknuť až potom.
3. Kupujúci si objedná jednotky z ponuky. Edge ich zarezervuje a vytvorí
   Stripe Checkout Session ako platbu na platformu, s objednávkou ako jej
   `transfer_group` a s účtom predajcu a poplatkom v metadátach. Predajcovi
   zatiaľ nič nepríde.
4. Stripe oznámi edge (`checkout.session.completed`), že platba prešla.
   Objednávka prejde do stavu `paid` a začne sa jej obdobie.
5. Keď sa obdobie skončí, edge z tejto platby prevedie podiel predajcu
   (`source_transaction`), vráti peniaze kupujúcemu, alebo urobí časť
   z oboch. Platforma si ponechá poplatok z toho, čo predajca dostane.

Poplatok je `feeBps` bázických bodov z hrubej sumy, zaokrúhlený polovicou
nahor. Pri predvolenej hodnote 400 zaplatí predaj za 20.00 EUR platforme 0.80
a predajcovi 19.20. Nastavená hodnota je obmedzená na 2000 (20 %).

## Zaplatené obdobia a vrátenie peňazí {#paid-periods-and-refunds}

Zaplatená objednávka kupuje obdobie: mesiac úložiska, výpočtového výkonu
alebo stroja. Edge drží peniaze kupujúceho počas tohto obdobia a sleduje
zariadenie predajcu, kým beží, takže ani jedna strana nemôže odísť
s podielom tej druhej.

- **Predajca nemôže stiahnuť ponuku.** Kým na ponuke beží niečie obdobie,
  tlačidlo Prestať predávať je sivé a uvádza, dokedy. Edge ju pred týmto
  dátumom odmietne zatvoriť.
- **Edge kontroluje, že zariadenie je dostupné.** Pri každom prechode
  reconcile sa pýta mesh, či je uzol predajcu `Ready`, a kde nemá mesh,
  ktorého by sa spýtal, použije heartbeat zariadenia. Čas, keď bol samotný
  edge nedostupný alebo príliš zaneprázdnený na kontrolu, sa predajcovi
  nikdy nepočíta.
- **Zariadenie offline vráti peniaze kupujúcemu.** Ak je zariadenie
  predajcu počas obdobia offline 24 hodín v kuse alebo spolu 48 hodín,
  obdobie sa tým skončí. Kupujúci dostane späť podiel obdobia, ktorý nebol
  dodaný, zaokrúhlený nahor na cent, a predajca dostane zaplatené za
  zvyšok, mínus poplatok z tejto časti. Zariadenie, ktoré je offline od
  začiatku, vráti celú platbu.
- **Nič nepripravené, nič nezaplatené.** Ak kúpené nie je pripravené do 23
  hodín od platby, kupujúci dostane všetko späť.
- **Predajca dostane zaplatené, keď obdobie skončí.** Skupina Zárobky na
  paneli Trh ukazuje, čo sa drží, čo je pripravené a čo je na ceste.
  Vyplatiť pošle pripravené hneď a je sivé, kým nejaké obdobie neskončí.
  Edge vypláca aj sám, deň po skončení každého obdobia, a vždy dosť skoro
  na to, aby si kupujúci v USDC nemohol vziať späť vklad za obdobie, ktoré
  bolo dodané.
- **Kupujúci nemôže zrušiť obdobie, ktoré sa práve dodáva**, a nemôže
  spustiť vrátenie peňazí. To robia len kontroly edge.

Platbu kartou, ktorú niekto napadne u svojej banky, rieši Stripe: kým je
spor otvorený, edge ju nevyplatí ani nevráti.

## Čo sa predáva {#what-is-for-sale}

| Druh      | Jednotka    | Význam                                                |
| --------- | ----------- | ----------------------------------------------------- |
| `storage` | GiB-mesiac  | Kapacita v Longhorn poole mesh                        |
| `compute` | vCPU-hodina | Plánovanie na uzle predajcu, v rámci jeho okna        |
| `vm`      | replika-mesiac | Replika virtuálneho stroja na zariadení predajcu, delené 50/50 |

Stroje majú vlastnú stránku, [Virtuálne stroje](/in-depth/virtual-machines.md), a
vlastný panel v administrácii.

Ceny sú v menšej jednotke (centoch) jednej meny na edge
(`losos.edge.market.currency`). Jedna objednávka musí mať spolu aspoň 50
menších jednotiek, čo je minimum Stripe.

## Plnenie {#fulfilment}

Zaplatená objednávka je **30-dňový nárok** (`expires_at`). Výpočtové jednotky
sa po jeho vypršaní vrátia do ponuky. Jednotky úložiska sa vrátia, až keď
claim zmizne, pretože claim drží kapacitu v Longhorne aj po skončení mesiaca.

- **Úložisko.** Edge vytvorí v mesh klastri namespace `market-<buyer id>`
  a `PersistentVolumeClaim` zakúpenej veľkosti, pomenovaný podľa objednávky
  (`ord-...`), z `losos.edge.market.storageClass` (predvolene `longhorn`).
  Beží to po každom prechode reconcile a hneď po webhooku o zaplatení. Je to
  idempotentné, keďže odpoveď `AlreadyExists` sa počíta ako hotovo, a ak
  apiserver odmietne, skúša sa to znova v každom intervale reconcile.
  Objednávka sa považuje za splnenú, až keď je claim `Bound`. Claim, ktorý
  zostal `Pending` (taká storage class neexistuje alebo chýba kapacita), sa
  kontroluje znova pri každom prechode. Predpokladá to triedu s väzbou
  `Immediate`, čo je predvolená trieda Longhornu. Trieda
  `WaitForFirstConsumer` sa nikdy neviaže, pretože claim zatiaľ nič
  nepripája. Prehľad účtu hlási naviazaný claim ako
  `volume: "<namespace>/<claim>"`. Edge si meno claimu zapíše skôr, než oň
  požiada, takže claim, ktorý sa nikdy nenaviaže, alebo taký, ktorý vznikol
  tesne pred zastavením edge, si po skončení mesiaca stále ponechá svoje
  jednotky predané.
- **Zálohy na kúpenom úložisku.** Zariadenie kupujúceho môže svoje zálohy
  držať na naviazanej objednávke úložiska (Nastavenia, Záloha). Zariadenie
  šifruje pomocou restic skôr, než čokoľvek odíde, rovnako ako pri buckete,
  a hovorí s cestou edge `/market/storage/<order>/`, ktorá obsluhuje
  protokol REST nástroja restic. Edge pri prvom použití spustí pre každú
  objednávku jeden `rest-server` vedľa claimu, z obrazu, ktorý nesie so
  sebou (`losos.edge.market.backups.enable`, predvolene zapnuté) a nikdy
  ho nesťahuje. Jediný účet servera patrí edge, s heslom vytvoreným pre
  túto objednávku, takže zariadenia mesh siete, ktoré dosiahnu jeho adresu
  v klastri, ho neotvoria. Edge túto cestu otvorí len kľúču, ktorý ukladá
  ako hash
  SHA-256. Zariadenie kupujúceho ho nastaví cez `POST /market/storage-door`
  a odvodí ho zo svojho kódu obnovy a id objednávky. Nové zariadenie
  s tým istým kódom obnovy sa preto dostane k tomu istému zväzku. Zariadenia,
  ktoré držia repliky Longhornu, aj edge vidia len šifrovaný text restic.
- **Výpočtový výkon.** Kredit vo vCPU-hodinách, uvedený pod `entitlements`.
  Zatiaľ ho nič nemeria ani podľa neho neplánuje.

:::note[Obmedzenia, ktoré sa zatiaľ nevynucujú]
Obmedzenia, o ktorých treba vedieť a z ktorých sa zatiaľ žiadne nevynucuje:

- Claim nie je pripnutý k uzlu predajcu. Longhorn rozmiestňuje repliky po
  celom poole, takže predajca dostáva zaplatené za príspevok doň, nie za
  hosťovanie práve tohto zväzku.
- Jediná cesta ku claimu je cesta pre zálohy uvedená vyššie. Nič iné ho
  nepripája.
- Nič sa nikdy nemaže. Vypršaný nárok sa prestane hlásiť, ale zväzok drží
  dáta kupujúceho a jeho odstránenie je na rozhodnutí operátora. Kým operátor
  claim nezmaže, jeho GiB zostávajú predané, takže tá istá kapacita sa nikdy
  nepredá dvakrát. Platí to aj pre claim, ktorý je stále `Pending`, keďže sa
  ešte môže naviazať. Prechod reconcile kontroluje každý vypršaný claim
  a jeho jednotky vráti do predaja, keď naň apiserver odpovie 404.
- ServiceAccount registrara získa po zapnutí trhu `get`/`create` na
  namespaces a PersistentVolumeClaims v celom klastri a pri zapnutých
  zálohách na kúpenom úložisku aj na Deployments a Services (a `create` na
  Secrets). Kubernetes RBAC
  nedokáže žiadne z nich obmedziť na mená `market-*`.
:::

## Admin UI {#admin-ui}

Riadok **Trh** je pod Mesh v bočnom paneli a otvára ho `/settings/market`.
Panel začína prepínačom zdieľania disku. Presunul sa sem z panela
Úložisko, pretože požičiavanie disku iným zariadeniam a platba zaň sú jedno
rozhodnutie. Pod ním je to, čo vlastník kúpil, s expiráciou a zväzkom,
a pult, z ktorého sa nakupuje. Na predaj slúži nastavenie výplat (adresa
USDC, kľúč pre Bitcoin alebo Monero a Stripe, ak edge karty prijíma),
dlžné poplatky platformy z predajov v minciach, formulár ponuky
a vlastníkove vlastné ponuky a predaje. Prepínač sa zobrazuje
bez ohľadu na to, či edge tomuto zariadeniu trh ponúka, pretože je to nastavenie
zariadenia, nie edge.

- Formulár ponuky ponúka len to, čo sa už zdieľa: úložisko, keď sa zariadenie pripojilo
  k mesh, výpočtový výkon, keď zároveň zdieľa výpočtový výkon. Všetko ostatné
  je sivé s uvedeným dôvodom. Edge vynucuje to isté pravidlo, panel ho len
  vysvetľuje.
- Ponuka, za ktorú niekto zaplatil, uvádza „zaplatené do“ s dátumom a jej
  tlačidlo Prestať predávať zostáva dovtedy sivé. Každá objednávka uvádza,
  kde sú jej peniaze: držané, na ceste k predajcovi, vrátené alebo
  rozdelené, s vrátenou sumou a dôvodom.
- Skupina Zárobky sa objaví, len čo sa drží nejaký predaj: čo sa drží za
  bežiace obdobia, čo je pripravené a čo je na ceste, s tlačidlom
  Vyplatiť, ktoré je sivé, kým nejaké obdobie neskončí.
- Platba kartou a onboarding do Stripe otvárajú vlastné stránky Stripe na novej
  karte. Nič v admin UI kartu nevidí. lososd a potom znova stránka overia,
  že odkaz je stránka `https://checkout.stripe.com` alebo
  `https://connect.stripe.com`, takže skompromitovaný edge nemôže poslať
  vlastníka na podvrhnutý formulár na kartu. Vlastné domény Checkout nie sú
  podporované.
- Stránky nemôžu volať edge samy, keďže CSP admin UI je `connect-src 'self'`.
  Namiesto toho to preposiela `lososd`: `GET /api/market`
  a `POST /api/market/{onboard,listings,listings/close,orders,release}`. Relay
  používa URL registrara, id appliance a proxy token, ktoré už poskytuje
  `losos.proxy.enable`. Vyžaduje https a token posiela do `curl` cez stdin,
  nie na príkazovom riadku.
- Tam, kde je trh vypnutý (žiadna proxy, edge ho má vypnutý alebo tento
  tenant nie je prihlásený), `GET /api/market` odpovie `{"available": false}`
  so stavom 200 a panel to tak aj povie. Zámerne to nie je 404. Admin UI berie
  404 ako „toto zariadenie túto routu neposkytuje“ po zvyšok relácie.

## API {#api}

Všetky routy sú na verejnom API registrara (`register.<publicDomain>`).
Overené routy berú rovnaké `appliance_id` a `token` ako `/register`.
Telá sú JSON.

| Routa                          | Overenie   | Účel                                             |
| ------------------------------ | ---------- | ------------------------------------------------ |
| `GET /market/listings`         | žiadne     | Čo sa dá teraz kúpiť. Nemenuje žiadneho predajcu |
| `POST /market/account`         | token      | Vaše ponuky, nákupy, predaje, nároky             |
| `POST /market/seller/onboard`  | token      | Spustiť alebo obnoviť onboarding do Stripe       |
| `POST /market/listings`        | token      | `kind`, `unit_price`, `capacity`                 |
| `POST /market/listings/close`  | token      | `listing_id`; zaplatené objednávky si jednotky ponechajú; 409, kým beží zaplatené obdobie |
| `POST /market/orders`          | token      | `listing_id`, `quantity`; vráti URL Checkout     |
| `POST /market/sales/release`   | token      | Vyplatí predaje, ktorých obdobia skončili; 409, kým žiadne také nie sú |
| `POST /market/fees/pay`        | token      | `tx`, `network`; započíta prevod USDC do dlžných poplatkov z mincí |
| `POST /market/webhook`         | podpis     | Udalosti Stripe                                  |

Každá routa odpovedá 503, keď je trh vypnutý. Na tokenových routách dostane
tenant bez bitu `market` odpoveď 403. Verejný prehľad ponúk a webhook
nepatria žiadnemu tenantovi, takže to sa im nikdy nestane. `POST /market/account`
vracia 100 najnovších nákupov a predajov na každej strane, živé ako prvé.
Zatvorená ponuka, z ktorej si nikto neobjednal, sa zmaže; ponuka
s objednávkami zostáva, kým zostávajú ony. Ani jedna strana sa nedozvie id
appliance tej druhej a verejný prehľad ponúk ho nikdy neukazuje.

Edge rezervuje kapacitu pri vytvorení Checkout Session a drží ju, kým Stripe
neoznámi, ako sa session skončila: zaplatená (`checkout.session.completed`)
alebo opustená (`checkout.session.expired`, posielaná, keď uplynie 31 minút
session). Udalosť `expired` sa počíta, len ak menuje session, ktorú si
objednávka zaznamenala. Ak sa Stripe nedá zastihnúť, aby session vytvoril,
edge kapacitu okamžite uvoľní, pokiaľ zaň už nedorazila platba. Ak nedorazí
ani jedna udalosť, rezervácia vyprší po trojdňovom okne opakovaní Stripe,
takže platba oneskorená výpadkom edge stále nájde svoje jednotky nepredané.
Nasledujúci prechod reconcile potom zaznamená objednávku ako `expired`.
Platba, ktorá príde ešte neskôr, sa uzná, ak sú jednotky stále voľné. Inak ju
edge zaloguje s jej session id, aby ju operátor mohol vrátiť. Dvaja kupujúci,
ktorí súperia o posledné jednotky, nemôžu session dostať obaja.

## Nastavenie pre operátora {#operator-setup}

Tieto kroky nastavujú platby kartou. Edge, ktorý prijíma len mince, ich
vynechá; ten prípad opisujú poznámky pre operátora pri zdrojovom kóde
(`docs/ci-and-releases.md`).

1. V Stripe dashboarde zapnite **Connect** na účte platformy.
2. Zapečaťte tajný kľúč (stačí aj obmedzený kľúč) pomocou `systemd-creds`,
   pričom ho čítajte zo stdin, aby sa otvorený text nikdy nedotkol disku:

   ```sh
   systemd-creds encrypt --name=stripe-secret-key - \
     /var/secrets/losos-stripe-secret-key.cred
   ```

   Cesta k blobu je `losos.edge.market.stripeSecretKeySealed` a dešifruje
   ho vždy len jednotka gate. Meno musí byť presne `stripe-secret-key`,
   pretože blob sa dešifruje len pod menom, s ktorým bol zapečatený.
3. Pridajte dva webhook endpointy, oba na
   `https://register.<publicDomain>/market/webhook`: jeden pre udalosti na
   vašom účte (`checkout.session.completed`, `checkout.session.expired`)
   a jeden, ktorý počúva **events on Connected accounts** (udalosti na
   pripojených účtoch, `account.updated`). Stripe každý podpisuje vlastným
   tajomstvom, takže zapečaťte obe tajomstvá `whsec_...`, každé na samostatnom
   riadku, pod menom `stripe-webhook-secret` do
   `losos.edge.market.webhookSecretSealed` (predvolene
   `/var/secrets/losos-stripe-webhook-secret.cred`). Edge prijme ktorékoľvek.
4. Nastavte `losos.edge.market.enable = true` a `losos.edge.market.returnUrl`,
   absolútnu http(s) URL bez prihlasovacích údajov a bez `#fragment`
   (objednávka a stav sa pripoja ako parametre dopytu).
5. Nastavte `losos.edge.tenants.<id>.market = true` pre každé zariadenie, ktoré smie
   obchodovať.

:::tip[Začnite v testovacom režime Stripe]
Začnite v Stripe **test mode** (testovací režim). Edge bol testovaný len proti
náhrade za Stripe. Tá overí, čo edge posiela a ako reaguje, no nevie povedať,
či to Stripe prijme. To rozhodne prvý beh v testovacom režime.
:::

### Testovací a ostrý režim {#test-mode-and-live-mode}

Edge prečíta režim z kľúča. Kľúče `sk_test_` a `rk_test_` spustia trh v
testovacom režime, `sk_live_` a `rk_live_` v ostrom, a žiadne zvláštne
nastavenie, ktoré by s kľúčom nesúhlasilo, neexistuje. Gate zapíše režim do
logu pri štarte a odmietne štartovať s kľúčom, z ktorého režim nevie
prečítať. Každá odpoveď trhu (`/market/listings`, `/market/account`,
onboarding a checkout) nesie `"mode": "test"` alebo `"mode": "live"`.

Každý režim má vlastnú knihu: `market.json` pre ostrý a `market-test.json`
vedľa neho pre testovací. Prechod do ostrého režimu je zapečatenie ostrého
kľúča a reštart gate. Testoví predajcovia, ponuky a objednávky zostanú v
testovacej knihe, ostrý pult začína prázdny a každý predajca prejde
onboardingom znova s ostrým účtom Stripe. Testovací kľúč vráti testovaciu
knihu, ako bola. `market.json` zapísaný predtým, ako edge režimy oddeľoval, edge
číta ako ostrú knihu. Zväzky, ktoré si vzali testovacie objednávky, zostanú v
sieti, kým ich operátor neodstráni.

Webhook secrets režim nenesú, každá udalosť Stripe áno (`livemode`).
Podpísaná udalosť z druhého režimu znamená, že zapečatený webhook secret a
kľúč sú z rôznych režimov, preto ju gate odmietne a do logu zapíše, ktorý je
ktorý. Stripe odmietnutú udalosť skúša znova tri dni, takže kým sa v tom čase
zapečatí zodpovedajúci pár, nič sa nestratí.

## Bezpečnostné vlastnosti {#safety-properties}

- Webhook je overený HMAC podpisom Stripe nad surovým telom, s päťminútovým
  oknom proti opakovaniu a vyšším limitom tela než ostatné routy.
- Stripe môže doručiť udalosť dvakrát alebo v inom poradí. Udalosť, ktorú
  edge už použil, sa podľa jej id zahodí, a `account.updated` starší než stav
  účtu, ktorý edge má, sa ignoruje, takže oneskorená udalosť nestiahne
  pripraveného predajcu z ponuky.
- Platba sa počíta, len keď sa session id, suma a mena zhodujú s tým, čo edge
  zaznamenal pre objednávku. Nezhoda sa zaloguje a nesplní sa. Ak sa edge
  zastavil skôr, než si session id zapísal, prevezme sa id z podpísanej
  udalosti, keďže objednávku v nej mohol pomenovať len gate tohto edge.
- **Registrar nikdy nedrží kľúč Stripe.** Samostatná jednotka,
  `losos-stripe-gate` (`losos-registrar stripe-gate`), je jediný proces, ktorý
  ho má. Registrar s gate komunikuje cez Unix socket
  (`/run/losos-stripe-gate/gate.sock`, 0600) a smie žiadať len o vytvorenie
  účtu, označenie účtu, kontrolu účtu, vytvorenie odkazu na onboarding,
  spustenie Checkout Session, overenie podpisu webhooku alebo vyrovnanie
  držanej platby na trhu. Gate odmietne checkout, ktorého cieľ nie je id
  `acct_...`, ktorého mena sa líši od nastavenej, ktorého poplatok presahuje
  strop 20 % alebo celú sumu, ktorého návratová URL nie je obyčajná http(s)
  URL alebo ktorého session žije dlhšie ako deň. Odmietne aj Stripe
  endpoint, ktorý nie je `https`. Pri vyrovnaní si gate platbu sám prečíta
  zo Stripe a rozdelí len platbu, ktorú vytvoril pre trh, medzi jej
  kupujúceho a predajcu uvedeného pri checkoute: nikdy nevráti viac, než sa
  zaplatilo, nič nevráti, keď už predajca dostal zaplatené, nič neurobí,
  kým je platba v spore, a každé vrátenie a každý prevod urobí najviac raz.
  Neexistuje operácia „prepošli toto do Stripe“. Skompromitovaný registrar
  preto nemôže prečítať kľúč, nemôže poslať peniaze nikam inam než na
  pripojený účet v rámci limitov poplatkov gate alebo späť kupujúcemu,
  ktorý ich zaplatil, a nemôže hýbať vlastnými peniazmi platformy. Stále
  môže žiadať o checkouty a vyrovnania, pretože to je jeho úloha. Obe
  jednotky bežia ako procesy na tom istom stroji a root na edge sa stále
  dostane k obom. Ako druhá vrstva jednotka registrara označí zapečatené
  bloby a adresár s credentials gate ako nedostupné.
- Edge nikdy neukladá tajomstvá Stripe v otvorenom texte. Sú uložené ako
  bloby `systemd-creds`, zapečatené pomocou TPM2, kde ho edge má, a inak
  kľúčom hostiteľa, a ku gate sa dostanú cez `LoadCredentialEncrypted=`.
  V otvorenom texte sú len v súkromnom credential tmpfs tejto jednotky
  a kópia `/var` nesie len šifrovaný text. Rotácia tajomstva znamená
  zapečatiť nový blob a reštartovať `losos-stripe-gate`. Chýbajúci blob gate
  preskočí, čo vypne trh (503) a nič iné. Gate je samostatná jednotka, takže
  master proxy v registrari si to ani nevšimne.
- **Účet Stripe nesie UUID zariadenia, nie jeho kód na obnovenie.** Kód na
  obnovenie je prihlasovací údaj a zostáva na zariadení. Zariadenie posiela jednosmernú
  hodnotu z neho odvodenú, prvých 16 bajtov
  `SHA-256("losos-box-id-v1:" + code)` naformátovaných ako UUID. Je stabilná
  po celú životnosť inštalácie a neprezrádza nič, z čoho by sa dal kód
  obnoviť. Registrar overí, že ide o kanonické UUID. Prehliadač ho nemôže
  zvoliť, pretože ho pridáva sám lososd.
- Súbory s tajomstvami sa pred použitím kontrolujú na tvar (`sk_`/`rk_`,
  `whsec_`).
- `market.json` má 0600 a zapisuje sa atomicky. Súbor, ktorý sa nedá
  sparsovať, zabráni spusteniu edge. Považovať ho za prázdny by znamenalo
  zabudnúť, kto za čo zaplatil.
- Chybové texty a endpointy Stripe zostávajú v žurnále; volajúci dostanú
  pevnú správu.

## Platba v USDC {#paying-with-usdc}

Edge môže sklad, výpočtový výkon a stroje účtovať v USDC namiesto Stripe.
Hardvér zostáva na Stripe. Ktoré siete prijíma, si vyberá edge:

| Sieť         | Pre testovací edge  |
| ------------ | ------------------- |
| Base         | Base Sepolia        |
| Arbitrum One | Arbitrum Sepolia    |
| OP Mainnet   | OP Sepolia          |
| Solana       | Solana Devnet       |

Na Solane platbu drží program `losos-escrow`, ak ho edge uvádza. Solana
vyrovná platbu za pár sekúnd a za zlomok centu, preto ju panel ponúkne ako
prvú. Edge bez tohto programu ponúka Solanu len vtedy, keď prijíma aj priame
platby, keďže nič zaplatené na nej by sa nedalo podržať.

Edge beží buď na ostrých, alebo na testovacích sieťach, nikdy na oboch. Na
edge, ktorý prijíma karty, sa režim riadi jeho kľúčom Stripe: ostré siete
s kľúčom `sk_live_`, testovacie s `sk_test_`. Edge, ktorý prijíma len mince,
beží v režime svojich sietí. Zmes registrar odmietne, takže testovacia platba sa
nedostane do ostrej evidencie. Každá sieť má vlastný RPC endpoint,
zapečatený cez systemd-creds, a číta ho len registrar. Nastavenie opisujú
poznámky pre operátora pri zdrojovom kóde (`docs/ci-and-releases.md`).

### Ako platiť {#how-you-pay}

1. Na Trhu vytvorte peňaženku, importujte 12 alebo 24 slov, alebo pripojte
   peňaženku v prehliadači: MetaMask, Rabby, Rainbow alebo Coinbase Wallet
   pre siete Etherea, Phantom, Solflare alebo Backpack pre Solanu. Jedna
   fráza dá adresu na každej sieti, ktorú edge prijíma, odvodenú tak ako v
   týchto peňaženkách (`m/44'/60'/0'/0/0` pre Ethereum, `m/44'/501'/0'/0'`
   pre Solanu), takže frázu odtiaľto do nich importujete a naopak. Slová sa
   v prehliadači zašifrujú. Zariadenie ich nedostane.
2. Panel ukáže USDC peňaženky na každej sieti a vedľa mince, ktorou sa na
   nej platia poplatky: ETH, na Solane SOL.
3. Objednajte ponuku. Nie je stránka Stripe. Panel ukáže sumu v USDC a
   spýta sa, na ktorej sieti platiť. Ako prvé ponúkne escrow na Solane,
   pokiaľ peňaženka nemá USDC len na inej sieti; potom sieť, kde má
   peňaženka dosť, s najnižším poplatkom.
4. Zaplaťte. Na sieti Etherea peňaženka schváli USDC pre `LososEscrow` a
   zavolá `pay`. Zmluva platbu podrží a zapíše si predajcu, poplatok a
   lehotu na vrátenie; predajcovi zatiaľ nič nepríde. Na Solane jedna
   transakcia zavolá `pay` programu escrow, ktorý pre objednávku otvorí
   vklad, presunie doň USDC a zapíše tie isté tri údaje. Vklad drží z
   peňaženky asi 0,0043 SOL nájomného, ktoré sa jej vrátia, keď sa vklad
   uzavrie. Bez programu escrow jedna transakcia pošle poplatok platformy do
   treasury a zvyšok predajcovi a v memo uvedie objednávku; prvá platba
   predajcovi alebo treasury im aj otvorí účet USDC, čo stojí trochu SOL.
5. Edge prečíta transakciu cez svoj endpoint pre danú sieť a objednávku
   označí ako zaplatenú. Jedna transakcia zaplatí len jednu objednávku.
   Objednávka potom uvádza svoju sieť a odkaz na transakciu v prehliadači
   blokov tej siete, pre kupujúceho aj predajcu.
6. Na sieti Etherea edge overí adresu predajcu na tej sieti. Ak tam USDC
   prijať nemôže, edge vráti celú platbu kupujúcemu a objednávka sa
   nevybaví. Inak, a na Solane vždy, vklad zostane v escrow, kým obdobie
   beží, a edge ho vyrovná tak, ako opisuje
   [Zaplatené obdobia a vrátenie peňazí](#paid-periods-and-refunds):
   poplatok do treasury a zvyšok predajcovi, všetko späť kupujúcemu, alebo
   rozdelenie. Objednávka ukáže, čo z toho sa stalo.

Kým platbu na sieti Etherea započíta, edge overí, že RPC odpovedá za reťazec
tej siete, že `LososEscrow` sťahuje USDC od Circle pre tento reťazec, platí
nastavenej treasury a prijíma pokyny od vlastného kľúča edge, a že nad
platbou je už ďalší blok. `LososEscrow` si vezme najviac polovicu platby, čo
je pevné delenie pri strojoch; presný poplatok a lehotu na vrátenie každej
objednávky kontroluje edge.

Adresa predajcu USDC prijať môže, keď na tej sieti nemá kód zmluvy, alebo má
len delegáciu podľa EIP-7702, takže zostáva účtom, ku ktorému niekto má
kľúč. Že ho niekto má, ukázal už podpis výplatnej adresy. Treasury, zmluva
USDC, escrow a nulová adresa sa nepočítajú nikdy. Vklad sa dá uvoľniť len
predajcovi a treasury zapísaným pri platbe, vrátiť kupujúcemu, alebo medzi
nich rozdeliť (`settle`); nikam inam ho poslať nič nevie. Ak je vklad 33 dní
od zadania objednávky stále držaný, kupujúci si ho môže zo zmluvy vziať späť
sám. Edge vypláca aspoň dva dni predtým.

### Dva z troch kľúčov

Držaná platba sa pohne, len keď sa zhodnú dva z troch kľúčov: kľúč edge,
kľúč zariadenia kupujúceho a kľúč zariadenia predajcu. Peňaženka pri platbe
uvedie kľúče oboch zariadení z faktúry a edge platbu započíta, len keď sú to
kľúče, ktoré mu tie dve zariadenia dali. O tom, kedy sa platba uvoľní,
vráti alebo rozdelí, rozhoduje edge ako doteraz, a potom čaká, kým s tým
jedno zariadenie súhlasí:

- uvoľnenie potrebuje zariadenie predajcu, ktoré súhlasí len s uvoľnením
  vlastného predaja;
- vrátenie alebo rozdelenie potrebuje zariadenie kupujúceho, ktoré súhlasí
  len s vrátením alebo rozdelením vlastného nákupu, alebo s vrátením celej
  platby, ktorú nevzala žiadna objednávka.

Každé zariadenie pred podpisom overí rozhodnutie voči vlastnému účtu a
druhým kľúčom je transakcia samotného edge. Ukradnutý alebo vynútený kľúč
edge teda sám nepohne ničím a edge nemôže platbu ani zadržať navždy, lebo
kupujúci si ju po lehote stále vezme späť.

Kľúč zariadenia je poverenie `market-escrow-key`, vytvorené pri prvom
štarte a zapečatené ako ostatné tajomstvá zariadenia. Zariadenie nikdy
neopustí: lososd z neho odvodí jeden kľúč pre siete Ethereum a jeden pre
Solanu, oba zaregistruje na edge a pri každej novej faktúre overí, že ako
kľúč kupujúceho uvádza ten jeho. Zariadenie sa edge zhruba raz za minútu
pýta na rozhodnutia, ktoré naň čakajú, takže uvoľnenie prebehne do minúty
alebo dvoch od rozhodnutia edge.

Čo to nepokrýva:

- Edge spolu s predajcom stále môžu uvoľniť platbu za obdobie, ktoré sa
  nedodalo, a edge spolu s kupujúcim vrátiť platbu za obdobie, ktoré sa
  dodalo. Výšku rozdelenia určuje edge a zariadenie kupujúceho ju prijme.
- Zariadenie kupujúceho nevie overiť kľúč predajcu, ktorý k nemu prichádza
  cez edge. Edge, ktorý by tam dal vlastný kľúč, by mohol platbu uvoľniť
  zapísanému predajcovi aj bez zariadenia predajcu; peniaze aj tak idú len
  tomuto predajcovi.
- Ostatné platobné údaje novej objednávky, adresa predajcu a sumy, tiež
  prichádzajú cez edge. Dva z troch chránia už držané platby, nie prvú,
  ktorú si vypýta edge v cudzích rukách.

Na Solane edge overí, že endpoint hlási genesis hash siete a že konfigurácia
programu escrow pre treasury uvádza USDC od Circle a vlastný kľúč edge. Potom
prečíta vklad, ktorý potvrdená transakcia vytvorila, a započíta ho, len keď
patrí k tejto konfigurácii a uvádza objednávku, predajcu a presnú sumu,
poplatok a lehotu na vrátenie. Program drží tie isté pravidlá ako
`LososEscrow`: poplatok najviac polovica, uvoľnenie len zapísanému predajcovi
a treasury, vrátenie len kupujúcemu a vlastné vrátenie kupujúceho až po
lehote. Vklad sa pri vyplatení uzavrie a jeho nájomné sa vráti kupujúcemu.
Malý poplatok za vlastné transakcie platí edge v SOL a predajcovi a treasury
otvorí účty USDC, ak ich nemajú. Bez programu escrow edge overí, že
transakcia prešla a je potvrdená, že jej memo uvádza objednávku a že USDC
predajcu a treasury narástli presne o ich podiely.

Objednávka drží svoje jednotky štvrť hodiny. Panel v posledných dvoch
minútach novú platbu nezačne a transakciu odoslanej platby si pamätá, kým ju
edge nezapočíta, takže opätovná kontrola nikdy nezaplatí dvakrát. Platba,
ktorá príde po uplynutí tejto doby, platí, kým sú jednotky voľné. Ak sa
medzitým predali alebo objednávku už zaplatila iná platba, edge platbu z
escrow vráti a panel to oznámi. Priama platba na Solane, bez programu
escrow, zostane predajcovi a vrátenie peňazí sa musí vyriešiť mimo trhu.

:::tip[Testovacie mince sú z faucetov]
Pre testovací edge je testovacie USDC z
[faucet Circle](https://faucet.circle.com), ktorý obsluhuje všetky štyri.
Testovacie ETH dá faucet každej siete, testovacie SOL faucet Solany. Nič z
toho nemá hodnotu.
:::

### Kúpa USDC za eurá {#buying-usdc-with-euros}

Peňaženka s adresou na sieti Base si tam môže kúpiť USDC za eurá na
[Binance](https://www.binance.com), s vaším vlastným účtom Binance. Panel Trh
ukáže tento riadok len vtedy, keď je edge partnerom Binance Connect a
vyrovnáva platby na hlavnej sieti Base. Binance predáva len USDC na sieti
Base, takže Bitcoin a Monero sa naďalej platia z vašej vlastnej peňaženky.

1. Zvoľte Kúpiť. Panel uvedie, že USDC predáva Binance, nie LosOS, a uvedie
   Podmienky používania, Podmienky produktov, Oznámenie o ochrane súkromia a
   Upozornenie na riziká od Binance. Každý dokument sa otvorí na novej karte
   a políčko na ich prijatie zostane zamknuté, kým neotvoríte všetky štyri.
   Potom zadajte sumu od 1 do 100 000 eur. Prehliadač si zapamätá, ktorú
   verziu podmienok ste prijali a kedy.
2. Vytvorte objednávku. Zariadenie pošle edge len sumu a adresu peňaženky.
   Edge objednávku podpíše svojím partnerským kľúčom a Binance odpovie
   odkazom na svoju platobnú stránku.
3. Otvorte Binance. Tam sa prihlásite, Binance overí vašu totožnosť, ukáže
   kurz a poplatok a prijme platbu. LosOS vašu totožnosť ani platobné údaje
   nikdy nevidí.
4. Kým je panel otvorený, každých 15 sekúnd sa edge pýta na objednávku a po
   opätovnom načítaní ju sleduje ďalej až sedem dní. Oznámi, keď Binance
   prijíma platbu, keď posiela USDC a keď ich poslal, s odkazom na
   transakciu na Basescan. Ak si Binance USDC namiesto toho nechá na vašom
   účte Binance alebo nákup neprejde, panel to tiež oznámi.

Binance rozhoduje, v ktorých krajinách predáva. V čase písania (október 2026)
nemá Binance licenciu podľa nariadenia EÚ MiCA a nové nákupy od obyvateľov EÚ
neprijíma, takže kupujúceho v EÚ stránka Binance odmietne.

Nákup je medzi vami a Binance: vrátenie peňazí alebo spor riešite s Binance,
nie s edge.

### Mená ENS a posielanie USDC {#ens-names-and-sending-usdc}

Panel vie z peňaženky poslať USDC na adresu na ktorejkoľvek sieti edge. Ak má
edge endpoint pre ENS, panel ukáže pri adrese Etherea aj jej primárne meno a
pri posielaní na sieti Etherea prijme meno namiesto adresy. Mená sa hľadajú
na Ethereum mainnete, pre testovací edge na Sepolii. Nájdu sa len mená so záznamami na
reťazci: stránka správy nedosiahne nič okrem zariadenia, takže mená
obsluhované cez gateway mimo reťazca sa nehľadajú.

### Ako dostávať platby {#how-you-get-paid}

Podpíšte peňaženkou, ktorá má USDC prijímať. Adresa Etherea berie predaje
zaplatené na Base, Arbitrum a OP; prídu na ňu, keď obdobie kupujúceho skončí a
edge platbu z escrow uvoľní. To isté kliknutie podpíše aj adresu Solany,
ktorá berie predaje zaplatené na Solane rovnako, alebo priamo na edge bez
programu escrow. Edge overí každý podpis a uloží len adresy. Predajca, ktorý má
nastavenú len jednu z nich, sa ponúka iba kupujúcim na tom druhu siete. Tú
istú frázu importujete do MetaMask, Rabby, Rainbow, Trust Wallet alebo
Phantom, keď peniaze chcete minúť. Pre tieto predaje nie je žiadny pripojený
účet Stripe.

:::warning[Peňaženka vytvorená tu zostane v jednom prehliadači]
Peňaženka vytvorená na Trhu žije len v tom prehliadači, preto sa panel pred
ponukou spýta, kam majú ísť jej predaje. Podpíšte ňou ako vyššie, alebo
zvoľte „Použiť adresu inej peňaženky“: vložte adresu peňaženky, ktorú držíte
inde, podpíšte ňou správu, ktorú panel ukáže, a vložte podpis. Edge ho overí
rovnako.
:::

## Platba v bitcoinoch {#paying-with-bitcoin}

Edge, ktorý vyrovnáva v USDC a prijíma priame platby, môže za objednávky
úložiska, výpočtov a strojov prijímať aj bitcoiny. Beží na ňom vlastný orezaný
uzol Bitcoinu: na mainnete pre ostrý edge, na testnet4, signete alebo regteste
pre testovací. Registrár odmietne každé iné spárovanie, takže ostrý edge nikdy
nezapočíta testovacie mince. Uzol drží peňaženku len na sledovanie a žiadny
súkromný kľúč.

### Predaj za bitcoiny {#selling-for-bitcoin}

Na Trhu vložte do riadka Bitcoinu rozšírený verejný kľúč svojej peňaženky:
zpub, xpub alebo ypub, ktorý exportuje Sparrow, Electrum či BlueWallet (na
testovacom edgi vpub, tpub alebo upub), alebo deskriptor výstupu s rozsahom.
Kľúč vie robiť adresy, ale nevie míňať, a súkromný kľúč edge odmietne. Každá
objednávka potom dostane z tohto kľúča prijímaciu adresu, ktorú žiadna iná
objednávka nepoužila, takže mince idú rovno do vašej peňaženky a zobrazia sa
v nej ako každá iná platba. Použite účet, ktorý držíte len na tieto predaje:
všetko, čo príde na jeho adresu potom, ako ju dostala objednávka, sa počíta
k tej objednávke. Peňaženky sa pozerajú asi 20 nepoužitých adries
dopredu; ak za sebou ostane nezaplatených viac ako 20 objednávok, zvýšte v
peňaženke limit medzery (gap limit), aby ste videli platby po nich.

Poplatok platformy sa z platby v bitcoinoch nestrháva. Celá suma príde
predajcovi a edge vedie priebežný súčet poplatkov z týchto predajov, ktorý
predajca dlhuje platforme a vidí ho na paneli (ako sa vyrovnáva, opisuje
[Poplatky platformy z predajov v minciach](#platform-fees-on-coin-sales)).
Predajca s kľúčom Bitcoinu a bez adresy USDC môže
ponúkať aj tak; kupujúci potom platia len v bitcoinoch.

### Platba v bitcoinoch pre kupujúceho {#paying-in-bitcoin}

Pri objednávke zvoľte pod „Zaplatiť cez“ Bitcoin. Panel ukáže sumu v BTC,
adresu, QR kód a odkaz, ktorý otvorí peňaženku v počítači. Zaplaťte z
ľubovoľnej peňaženky; nič netreba hlásiť späť. Uzol edgu vidí, že platba
prišla, a panel ju sleduje od zachytenia po potvrdenie.

Suma v BTC je ponuka z cenových zdrojov, ktoré sa medzi sebou zhodujú, platná
štvrť hodiny. Objednávku do 50 $ započíta jedno potvrdenie, do 500 $ dve, do
5 000 $ tri a nad to šesť. Pri nedoplatku panel povie, koľko ešte chýba, a
čo prevýši sumu, ostáva predajcovi. Platba poslaná po uplynutí ponuky sa
porovná s cenou v tej chvíli: do 1 % sa objednávka dokončí, inak panel
požiada o rozdiel. Objednávka potom ako sieť uvádza Bitcoin a odkazuje na
transakciu na mempool.space.

## Platba v Monere {#paying-with-monero}

Edge, ktorý vyrovnáva v USDC a prijíma priame platby, môže za tie isté
objednávky prijímať aj Monero. Beží na ňom vlastný orezaný uzol Monera
a vedľa neho služba peňaženiek: na mainnete pre ostrý edge, na stagenete
alebo testnete pre testovací. Registrár odmietne každé iné spárovanie aj
adresu vytvorenú pre inú sieť, než na akej je uzol.

### Predaj za Monero {#selling-for-monero}

Na Trhu vložte do riadka Monera dve veci: hlavnú adresu peňaženky, tú, ktorá
sa začína na 4 (pre testovací edge na 5 alebo 9), a jej súkromný kľúč na
prezeranie (view key). Feather, Cake Wallet aj oficiálne peňaženky ho
ukazujú vedľa seedu, medzi kľúčmi peňaženky. Kľúč na prezeranie dovolí edgu
vidieť, čo prichádza na vaše adresy, a nič viac: míňať nevie. Zariadenie
dvojicu pred odoslaním overí a kľúč na míňanie (spend key) odmietne menom,
takže omylom vložený kľúč na míňanie zariadenie nikdy neopustí. Seed nikdy
nevkladajte.

Edge pre vás drží peňaženku len na prezeranie a každej objednávke dá ďalšiu
podadresu prvého účtu vašej peňaženky, takže mince idú rovno do vašej
peňaženky. Vaša peňaženka ich nájde tiež: pozerá sa dopredu aj na
podadresy, ktoré ešte neukázala. Ak za sebou ostane nezaplatených viac ako
asi 200 objednávok, zvýšte v peňaženke dosah hľadania podadries
(subaddress lookahead), aby ste videli platby po nich.

Rovnako ako pri bitcoinoch predajca dlží poplatok platformy zvlášť a
celá platba príde predajcovi.

### Platba v Monere pre kupujúceho {#paying-in-monero}

Pri objednávke zvoľte pod „Zaplatiť cez“ Monero. Panel ukáže sumu v XMR,
podadresu, QR kód a odkaz, ktorý otvorí peňaženku. Zaplaťte z ľubovoľnej
peňaženky. Objednávku do 5 000 $ započíta desať potvrdení a nad to dvadsať;
nedoplatok, preplatok aj oneskorená platba sa riešia ako pri bitcoinoch.
Platba uzamknutá na neskoršie bloky sa nepočíta. Monero skrýva, kto komu čo
zaplatil, takže zaplatená objednávka uvádza Monero ako sieť bez odkazu na
prehliadač blokov.

## Platba v Zcashi {#paying-with-zcash}

Edge, ktorý vyrovnáva v USDC a prijíma priame platby, môže za tie isté
objednávky prijímať aj tienený Zcash. Nebeží na ňom žiadny uzol Zcashu. Pýta
si kompaktné bloky od verejných serverov lightwalletd, predvolene od štyroch
serverov na mainnete od rôznych prevádzkovateľov, a dešifruje ich sám, takže
váš kľúč na prezeranie edge nikdy neopustí. Platba sa počíta, až keď ju
servery dvoch rôznych prevádzkovateľov ukážu v bloku s rovnakým hašom; server,
ktorý ukáže niečo, čo nikto iný neukáže, je prehlasovaný. Ostrý edge vyrovnáva
len na mainnete a nepýta sa servera, ktorý hlási inú sieť; testnet a regtest
sú pre testovací edge.

### Predaj za Zcash {#selling-for-zcash}

Na Trhu vložte do riadka Zcashu zjednotený kľúč na prezeranie vašej
peňaženky: úplný, ktorý sa začína na `uview1`, alebo lepšie kľúč pre
prichádzajúce platby, ktorý sa začína na `uivk1` a vidí len to, čo prichádza,
nič z toho, čo posielate vy. Kľúč potrebuje časť Orchard, ktorú má každá
peňaženka vytvorená od roku 2022. Edge si z nej nechá len kľúč pre
prichádzajúce platby, ktorý vidí, čo prichádza, a míňať nevie. Zariadenie aj
edge odmietnu kľúč na míňanie aj seed menom, takže omylom vložený kľúč
zariadenie nikdy neopustí. Panel ukáže späť prvú adresu vašej peňaženky,
aby ste videli, do ktorej peňaženky objednávky platia; samotný kľúč už
nikdy neukáže.

Každá objednávka dostane novú tienenú adresu vašej peňaženky: zjednotenú
adresu len s časťou Orchard s indexom diverzifikátora vysoko nad tými,
ktoré rozdáva vaša peňaženka sama. Vaša peňaženka platby na ňu nájde sama,
lebo dešifruje všetko, čo príde na ktorúkoľvek adresu jej kľúča. Od NU6.3
platba na adresu Orchard pristane v poole Ironwood, ktorý používa tie isté
kľúče; edge číta oba.

Rovnako ako pri bitcoinoch predajca dlží poplatok platformy zvlášť a
celá platba príde predajcovi.

### Platba v Zcashi pre kupujúceho {#paying-in-zcash}

Pri objednávke zvoľte pod „Zaplatiť cez“ Zcash. Panel ukáže sumu v ZEC,
adresu, QR kód a platobný odkaz `zcash:` (ZIP 321), ktorý otvorí peňaženku.
Zaplaťte z peňaženky, ktorá vie posielať na zjednotenú adresu. Počítajú sa
len platby v blokoch po tom, čo objednávka adresu dostala, a platba čakajúca
v mempoole sa ukáže ako videná, keď ju ukážu dvaja prevádzkovatelia.
Objednávku do 50 $ započítajú tri potvrdenia, do 500 $ šesť, do 5 000 $
desať a nad to 24; nedoplatok, preplatok aj oneskorená platba sa riešia ako
pri bitcoinoch. Platba je tienená, takže zaplatená objednávka uvádza Zcash
ako sieť bez odkazu na prehliadač blokov.

## Poplatky platformy z predajov v minciach {#platform-fees-on-coin-sales}

Platba v bitcoinoch, Monere alebo Zcashi príde do peňaženky predajcu celá,
takže 4 % platformy z týchto predajov sú dlh, ktorý edge vedie voči
predajcovi. Panel Trh ho ukazuje vo vlastnom riadku pod kľúčmi mincí spolu
s limitmi nižšie. Vyrovnáva sa v USDC, dvoma spôsobmi.

**Z predajov v USDC.** Kým predajca dlží poplatky z mincí, edge zvýši
poplatok z jeho ďalších objednávok v USDC o dlžnú sumu, na sieťach Etherea
aj na Solane. Kupujúci platí stále uvedenú cenu. Menší je len podiel
predajcu a predaj na paneli uvádza, koľko z neho išlo na dlžné poplatky.
Poplatok nikdy neprekročí polovicu ceny objednávky, čo je najviac, koľko si
escrow kontrakt vezme. Dlh klesne až vtedy, keď peniaze prídu do pokladnice:
keď escrow platbu uvoľní, alebo keď sa potvrdí priama platba na Solane.
Objednávka, ktorá sa vráti alebo prepadne, nevyberie nič. Keď je naraz
otvorených viac objednávok v USDC, každá si vezme len to, čo si ešte
nevzali ostatné. Ak predaj vyberie viac, než sa ešte dlží, lebo dlh medzitým
vyrovnala priama platba, zvyšok zostane ako kredit a najprv sa použije na
poplatky z ďalších predajov v minciach.

**Priamo.** Riadok poplatkov má tlačidlo, ktoré pošle dlžnú sumu v USDC
z peňaženky na paneli do pokladnice platformy, na ktorejkoľvek sieti
Etherea, na ktorej edge vyrovnáva, a odovzdá transakciu edge. Edge ju
prečíta zo svojho vlastného uzla a započíta ju, keď má toľko potvrdení ako
platba objednávky. Započíta len prevod USDC do pokladnice z adresy predajcu
pre výplaty, každú transakciu raz a nikdy viac, než sa dlží. Čo sa zaplatí
nad dlh, sa nevracia. Na Solane sa poplatky neplatia.

**Kedy sa platby v minciach zastavia.** Kým predajca dlží viac, než je limit
edge, teda 10 USD, ak jeho operátor nenastavil iný, alebo dlží akúkoľvek
sumu dlhšie ako 30 dní, kupujúci mu nemôžu platiť v bitcoinoch, Monere ani
Zcashi. Nová objednávka ponúkne len USDC a nová platba v minciach sa odmietne
s dôvodom. Platba v minciach, ktorá už prebieha, sa sleduje do konca. USDC
funguje ďalej a každým predajom spláca dlh, takže platby v minciach sa vrátia,
keď je dlh vyrovnaný. U predajcu, ktorý prijíma len mince, sa dovtedy
objednávať nedá.

## Čo nie je pokryté {#not-covered}

- **Spory a vrátenia peňazí, ktoré neurobil edge.** Spor pri karte sa rieši
  v Stripe dashboarde. Vrátenie peňazí z akéhokoľvek iného dôvodu potom, ako
  predajca dostal zaplatené, je na rozhodnutí operátora, s
  `reverse_transfer`, aby sa vrátil aj podiel predajcu.
- **Dôkaz, že úložisko je v poriadku.** Zariadenie sa počíta ako online,
  kým je jeho uzol v mesh `Ready`. Edge nekontroluje, či zväzok kupujúceho
  odpovedá.
- **Vynucovanie nároku.** Plnenie vytvorí claim; nebráni kupujúcemu použiť
  viac, než kúpil, nemeria výpočtový výkon a pri expirácii neodoberá prístup.
- **Dane, fakturácia a overovanie predajcov** nad rámec toho, čo robí
  onboarding Stripe. Prevádzka trhoviska prináša právne povinnosti, ktoré
  závisia od toho, kde pôsobíte. Toto je experiment, nie rada.
