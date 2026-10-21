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
  Nevyžiadaná pošta a Kôš, pri každom s počtom neprečítaných správ.
- **Hľadanie** prehľadá otvorený priečinok: odosielateľov, predmety a text.
- **Napísať** otvorí novú správu. Pole **Komu** ponúka ostatných ľudí na
  zariadení. **Odpovedať**, **Odpovedať všetkým** a **Preposlať** správu
  citujú.
- **Označiť**, **Označiť ako neprečítané**, **Archivovať** a **Presunúť
  do** pracujú s otvorenou správou. **Odstrániť** ju presunie do koša a
  odstránenie z koša ju zmaže natrvalo.
- Formátovaná správa sa otvorí ako obyčajný text. **Zobraziť formátovanie**
  vykreslí rozloženie od odosielateľa v izolovanom rámci bez skriptov, bez
  formulárov a bez obrázkov zvonka zariadenia, takže odosielateľ nezistí,
  že ste správu otvorili. Správa bez obyčajného textu sa v tomto rámci
  otvorí hneď.
- Prílohy sa stiahnu do počítača. Nikdy sa neotvoria priamo na stránke.

## Čo Pošta zatiaľ nerobí \{#what-mail-does-not-do-yet}

Pošta chodí len medzi ľuďmi na tomto zariadení. Správu do inej domény
zariadenie odmietne a povie prečo, a zvonka nič nepríde. Prijímanie pošty
z internetu potrebuje verejnú adresu a port 25, ktoré domáce pripojenie
máva zriedka. Zariadenie tiež neponúka IMAP ani SMTP pre iné poštové
aplikácie: jediný prístup je Pošta v správcovských stránkach.
