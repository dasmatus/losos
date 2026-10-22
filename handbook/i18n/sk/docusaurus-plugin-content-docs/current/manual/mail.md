---
title: Pošta
sidebar_position: 4.7
slug: /manual/mail
---

# Pošta

LosOS Mail je poštový klient pre ľudí na zariadení. Každý, kto je
prihlásený cez LosOS Login, má schránku a položka **Pošta** v bočnom paneli
ju otvorí na adrese `/mail`. Poštový server je Stalwart. Beží na zdieľanej
strane zariadenia pod účtom `shared` a schránky uchováva v zdieľanom
dátovom priečinku.

## Kedy je Pošta k dispozícii \{#when-mail-is-there}

Pošta existuje, len kým zariadenie zdieľa svoje úložisko so sieťou, do
ktorej sa pripojilo (pozri [režimy úložiska](../types/storage-modes)). Keď
je zdieľanie vypnuté alebo zariadenie nie je v sieti:

- v bočnom paneli nie je **Pošta** a `/mail` sa neotvorí;
- poštový server je zastavený;
- schránky zostávajú na disku v zdieľanom priečinku, ktorý zariadenie počas
  vypnutého zdieľania nedokáže prečítať.

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

## Čo Pošta zatiaľ nerobí \{#what-mail-does-not-do-yet}

Pošta chodí len medzi ľuďmi na tomto zariadení. Správu do inej domény
zariadenie odmietne a povie prečo, a zvonka nič nepríde. Prijímanie pošty
z internetu potrebuje verejnú adresu a port 25, ktoré domáce pripojenie
máva zriedka. Zariadenie tiež neponúka IMAP ani SMTP pre iné poštové
aplikácie: jediný prístup je Pošta v správcovských stránkach.
