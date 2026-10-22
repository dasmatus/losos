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

**Čítanie.** Šifrovaná správa má v zozname zámok. Keď ju otvoríte, dešifruje
sa vo vašom prehliadači a riadok pod hlavičkou povie, ako prišla: šifrovaná,
podpísaná a kým, podpísaná kľúčom, ktorý zariadenie nepozná, alebo s
podpisom, ktorý nesedí. Správa podpísaná niekým iným než jej odosielateľom
to povie. Dešifrovaná pošta sa zobrazí ako text, nikdy vo formátovaní od
odosielateľa, a jej prílohy sa sťahujú ako všetky ostatné.

**Váš verejný kľúč pre ostatných.** Zariadenie zverejňuje verejný kľúč
každého vo svojej Web Key Directory na `/.well-known/openpgpkey/`, takže ho
poštové programy inde nájdu podľa vašej adresy. Každá správa, ktorú pošlete,
ho nesie aj v hlavičke Autocrypt.

## Čo Pošta zatiaľ nerobí \{#what-mail-does-not-do-yet}

Pošta chodí len medzi ľuďmi na tomto zariadení. Správu do inej domény
zariadenie odmietne a povie prečo, a zvonka nič nepríde. Prijímanie pošty
z internetu potrebuje verejnú adresu a port 25, ktoré domáce pripojenie
máva zriedka. Zariadenie tiež neponúka IMAP ani SMTP pre iné poštové
aplikácie: jediný prístup je Pošta v správcovských stránkach.
