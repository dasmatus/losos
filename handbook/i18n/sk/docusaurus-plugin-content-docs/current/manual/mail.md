---
title: Pošta
sidebar_position: 4.7
slug: /manual/mail
---

# Pošta

![LosOS Mail: priečinky, doručená pošta s tromi správami a hore Šifrovanie, Filtre a Napísať.](@site/docs/img/mail-inbox-sk.png)

![Správa od Evy otvorená vedľa doručenej pošty, nad ňou tlačidlá na odpoveď, preposlanie, presun a zmazanie.](@site/docs/img/mail-message-sk.png)

LosOS Mail je poštový klient pre ľudí na zariadení. Každý, kto je
prihlásený cez LosOS Login, má schránku a položka **Pošta** v bočnom paneli
ju otvorí na adrese `/mail`. Poštový server je Stalwart 1.0. Beží na
zdieľanej strane zariadenia pod účtom `shared`, schránky uchováva
v zdieľanom dátovom priečinku a odpovedá len na samotnom zariadení: nič
v sieti sa k nemu nedostane.

## Kedy je Pošta k dispozícii \{#when-mail-is-there}

Pošta existuje, len kým zariadenie zdieľa svoje úložisko so sieťou, do
ktorej sa pripojilo (pozri [režimy úložiska](../types/storage-modes)). Keď
je zdieľanie vypnuté alebo zariadenie nie je v sieti:

- v bočnom paneli nie je **Pošta** a `/mail` sa neotvorí;
- poštový server je zastavený;
- schránky zostávajú nedotknuté na šifrovanom disku zariadenia.

Keď zariadenie znova zdieľa so svojou sieťou, Pošta sa vráti s tou istou
poštou.

## Vaša adresa \{#your-address}

Vaša adresa je vaše meno v LosOS Login v poštovej doméne zariadenia,
napríklad `matus@mattbox.home.arpa`. Zariadenie vytvorí schránku, keď prvý
raz otvoríte Poštu. Samostatné heslo k pošte neexistuje: Pošta je otvorená,
kým ste prihlásení. Náhradný kľúč otvorí správcovské stránky, ale nemá
schránku, takže Pošta vás požiada o prihlásenie vlastným účtom.

## Čítanie a písanie \{#reading-and-writing}

- **Priečinky**: Doručená pošta, Koncepty, Odoslaná pošta, Archív,
  Nevyžiadaná pošta a Kôš, vedľa nich vaše vlastné priečinky a pri každom
  počet neprečítaných správ. Na širokej obrazovke sú ako strom naľavo od
  zoznamu, na užšej ako riadok nad ním.
- **Hľadanie** prehľadá otvorený priečinok: odosielateľov, predmety a text.
- **Napísať** otvorí novú správu. Polia **Komu** a **Kópia** ponúkajú vaše
  [kontakty](contacts) a ostatných ľudí na zariadení. **Odpovedať**, **Odpovedať všetkým** a **Preposlať** správu
  citujú.
- **Označiť**, **Označiť ako neprečítané**, **Archivovať** a **Presunúť
  do** pracujú s otvorenou správou. **Odstrániť** ju presunie do koša a
  odstránenie z koša ju zmaže natrvalo.
- Formátovaná správa sa otvorí ako obyčajný text. **Zobraziť formátovanie**
  vykreslí rozloženie od odosielateľa v izolovanom rámci bez skriptov, bez
  formulárov a bez obrázkov zvonka zariadenia, takže odosielateľ nezistí,
  že ste správu otvorili. Správa bez obyčajného textu sa v tomto rámci
  otvorí hneď.
- Prílohy sa stiahnu do počítača a nikdy sa neotvoria priamo na stránke.
  Čo ďalej s nimi môžete robiť, opisuje
  [Súbory a LosOS Cloud](#files-and-losos-cloud).
- Keď je odosielateľ na tomto zariadení a zariadenie má Chat, tlačidlá
  **Chat** a **Zavolať** v paneli správy s ním začnú rozhovor.
- Kliknite pravým tlačidlom na správu v zozname a odpovedzte na ňu,
  označte, archivujte alebo presuňte ju bez otvorenia; pozrite
  [ponuku pravého tlačidla](admin-pages#the-right-click-menu).

## Vlastné priečinky \{#your-own-folders}

Vytvárajte si vlastné priečinky vedľa priečinkov Pošty aj v nich, do
ľubovoľnej hĺbky.

- **Nový priečinok** (ikona priečinka hore nad stromom) sa spýta na názov
  a na to, kam ho dať. Názov nemôže obsahovať lomku a dva priečinky na
  rovnakom mieste nemôžu mať rovnaký názov.
- Kliknite pravým tlačidlom na priečinok a otvorte ho, vytvorte v ňom
  priečinok, premenujte ho, presuňte do iného priečinka alebo na najvyššiu
  úroveň, alebo ho odstráňte. Priečinky Pošty sa dajú len otvoriť a
  vytvoriť v nich priečinok.
- Pretiahnite správu na priečinok a presuniete ju tam. Pretiahnite svoj
  priečinok na iný a vložíte ho doň, alebo na prázdne miesto pod stromom a
  dostanete ho na najvyššiu úroveň.
- Pri odstránení priečinka sa jeho správy najprv presunú do koša, takže
  omylom sa nič nestratí. Priečinok, v ktorom sú ďalšie priečinky, sa
  neodstráni, kým ich nepresuniete alebo neodstránite.

## Filtre \{#filters}

Filtre triedia novú poštu hneď, ako príde. Uchováva a spúšťa ich poštový
server, takže fungujú, aj keď je Pošta zatvorená a nikto nie je
prihlásený.

- **Filtre** vedľa tlačidla **Napísať** ich zobrazia v poradí, v akom sa
  spúšťajú. O správe rozhodne prvý filter, ktorý sa s ňou zhoduje.
- Filter sa pozrie, od koho správa je, komu je (Komu alebo Kópia) alebo na
  jej predmet, a hľadá v nich text. Na veľkosti písmen nezáleží. Môže
  žiadať, aby platili všetky podmienky, alebo stačí ktorákoľvek z nich.
- Potom správu presunie do priečinka (aj do koša, a tak filter maže),
  označí ju ako prečítanú, označí ju, alebo urobí viac z toho naraz.
- Prepínač pri filtri ho vypne bez toho, aby sa stratil, a šípky menia
  poradie.
- Kliknite pravým tlačidlom na správu a vyberte **Filtrovať podobnú
  poštu…**, čím začnete filter pre jej odosielateľa.
- Filtre pracujú s poštou, ktorá príde po ich uložení; pošta, ktorá už je
  v priečinku, tam zostane. Odstránenie priečinka vypne filter, ktorý doň
  len presúval poštu.

## Súbory a LosOS Cloud \{#files-and-losos-cloud}

- **Uložiť do LosOS Cloud** vedľa prílohy ju uloží do priečinka **Mail**
  vo vašom LosOS Cloud. **Otvoriť** ju potom otvorí v aplikácii, ktorá ju
  upravuje: Markdown v Poznámkach, iné textové súbory v spoločnom editore.
- Pri písaní pridáte súbory tlačidlami **Priložiť z tohto počítača** a
  **Priložiť z LosOS Cloud**, spolu najviac 20 MB. **Preposlať** ponechá
  prílohy správy.
- V LosOS Cloud kliknite pravým tlačidlom na súbor a vyberte **Poslať
  e-mailom**. Otvorí sa nová správa so súborom v prílohe.

## Opýtať sa AI zariadenia \{#ask-the-boxs-ai}

Tlačidlom **Zhrnúť** v paneli správy sa opýtate vlastného jazykového modelu
zariadenia na otvorenú správu. Model beží na zariadení, takže správa aj
odpoveď zostávajú na ňom, a číta len správu, na ktorú ste sa pýtali.
Odpovedá na jednu otázku naraz, na malom zariadení môže odpoveď trvať minútu
či dve. Text v správe, ktorý AI prikazuje niečo urobiť, sa berie ako časť
správy, nie ako vaša požiadavka.

Otázky na poštu, súbory a poznámky idú modelu Gemma 3 1B, ktorý je určený na
text. **Vytvoriť s AI** má vlastný model na kód. Zariadenie si každý model
stiahne raz, keď je prvýkrát online, a v pamäti drží naraz len jeden.
Správca môže zvoliť iný model v **Nastavenia**, **Rozšírené**
(`assistant.model`). Gemma is provided under and subject to the Gemma Terms
of Use found at ai.google.dev/gemma/terms.

## Šifrovanie \{#encryption}

Pošta vie šifrovať a podpisovať správy cez OpenPGP, štandard, ktorý čítajú
GnuPG, Thunderbird a väčšina poštových programov.

**Váš kľúč.** **Šifrovanie** vedľa tlačidla **Napísať** vytvorí kľúč pre
vašu adresu alebo importuje kľúč, ktorý už máte. Zvolíte si k nemu heslo;
nie je to vaše prihlasovacie heslo. Kľúč sa vytvorí a odomkne vo vašom
prehliadači. Zariadenie uchováva súkromný kľúč len zamknutý vaším heslom a
ešte raz ho zapečatí do svojho TPM alebo vlastným kľúčom, ako aj svoje
ostatné tajomstvá (pozri [prihlasovacie údaje](../reference/credentials)),
takže bez hesla vašu šifrovanú poštu na zariadení nikto neprečíta, ani
správca. To isté okno ukáže odtlačok, stiahne zálohu (stále zamknutú vaším
heslom) a verejný kľúč, kľúč znova zamkne a odstráni ho. Odstránený kľúč
neprečíta poštu poslanú preň, kým neimportujete zálohu.

![Okno Šifrovanie s kľúčom: odtlačok, adresa a dátum a tlačidlá na zamknutie, zálohu a odstránenie kľúča.](@site/docs/img/mail-pgp-key.png)

**Odomknutie.** Kľúč zostane odomknutý v karte, kde ste ho odomkli, kým ho
nezamknete alebo kartu nezavriete. Pošta si heslo vypýta, keď ho potrebuje:
na otvorenie šifrovanej správy alebo na podpísanie.

**Písanie.** Pod správou zapnete alebo vypnete **Šifrovať** a **Podpísať**.
Kým píšete adresu, Pošta hľadá kľúč toho človeka a ukáže, odkiaľ je:

- niekto na tomto zariadení: kľúč, ktorý si tu vytvoril;
- vaše kontakty: kľúč uložený pri kontakte;
- predošlá pošta: kľúč, ktorý ten človek poslal v hlavičke Autocrypt,
  naučený, keď ste jeho poštu otvorili;
- jeho poštová doména: Web Key Directory (WKD) domény;
- keys.openpgp.org, keď doména žiadny nemá. Správca ho môže nasmerovať na
  iný server kľúčov alebo vypnúť cez `losos.mail.keyserver`.

Keď má kľúč každý príjemca, **Šifrovať** sa zapne samo. Ak ho niekto nemá,
Pošta povie kto, a správa sa neodošle, kým šifrovanie nevypnete alebo tohto
príjemcu nevynecháte. Šifrovaná pošta je zašifrovaná aj pre váš vlastný
kľúč, takže svoju kópiu v Odoslanej pošte prečítate. Predmet šifrovaný nie
je. Pošta sa posiela ako PGP/MIME; **Vložené, pre staršie poštové programy**
dá šifrovaný text priamo do tela správy, čo chráni text, ale nie prílohy.

![Nová správa pre Evu: Evin kľúč sa našiel na tomto zariadení a Šifrovať a Podpísať sú zapnuté.](@site/docs/img/mail-pgp-compose.png)

**Čítanie.** Šifrovaná správa má v zozname zámok. Keď ju otvoríte, dešifruje
sa vo vašom prehliadači a riadok pod hlavičkou povie, ako prišla: šifrovaná,
podpísaná a kým, podpísaná kľúčom, ktorý zariadenie nepozná, alebo s
podpisom, ktorý nesedí. Správa podpísaná niekým iným než jej odosielateľom
to povie. Dešifrovaná pošta sa zobrazí ako text, nikdy vo formátovaní od
odosielateľa, a jej prílohy sa sťahujú ako všetky ostatné.

![Eva číta správu: v zozname je zámok a poznámka hovorí, že ju zašifroval a podpísal Matus, s Matusovým odtlačkom.](@site/docs/img/mail-pgp-opened.png)

**Váš verejný kľúč pre ostatných.** Zariadenie zverejňuje verejný kľúč
každého vo svojej Web Key Directory na `/.well-known/openpgpkey/`, takže ho
poštové programy inde nájdu podľa vašej adresy. Každá správa, ktorú pošlete,
ho nesie aj v hlavičke Autocrypt.

## Prechod na Stalwart 1.0 \{#moving-to-stalwart-10}

Zariadenie, ktoré malo poštu už pred touto verziou, ju pri prvom spustení
po aktualizácii samo skopíruje na nový server: priečinky, správy, adresy
odosielateľa a filtre každého človeka. Pošta počas kopírovania funguje
a pošta, ktorá medzitým príde, zostane. Ak sa zariadenie v polovici zastaví,
kopírovanie pokračuje pri ďalšom spustení a nič nezdvojí. Staré schránky
zostanú vedľa nových v priečinku `db-0.15` a nič sa z nich nezmaže.

## Zálohy \{#backups}

[Zálohy](backup-and-erase) obsahujú aj Poštu. Poštový server sa na pár
sekúnd, kým zariadenie skopíruje schránky, zastaví a potom sa znova spustí.
V tých sekundách vás Pošta požiada, aby ste to o chvíľu skúsili znova.

## Čo Pošta zatiaľ nerobí \{#what-mail-does-not-do-yet}

Pošta chodí len medzi ľuďmi na tomto zariadení. Správu do inej domény
zariadenie odmietne a povie prečo, a zvonka nič nepríde. Prijímanie pošty
z internetu potrebuje verejnú adresu a port 25, ktoré domáce pripojenie
máva zriedka. Zariadenie tiež neponúka IMAP ani SMTP pre iné poštové
aplikácie: jediný prístup je Pošta v správcovských stránkach.
