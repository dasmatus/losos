---
title: Kalendár
sidebar_position: 4.85
slug: /manual/calendar
---

# Kalendár

![Týždeň v LosOS Calendar: vľavo vaše kalendáre a dva odbery, rozvrh hodín a daňový termín z odberov a Review the box, udalosť s videohovorom v LosOS Chat.](@site/docs/img/calendar-week-sk.png)

LosOS Calendar uchováva vaše udalosti a kalendáre, ktoré sledujete. Položka
**Kalendár** v bočnom paneli ho otvorí na adrese `/calendar`. Vaše kalendáre
patria len vám. Ostatní ľudia na zariadení, ani správcovia, ich nedokážu
prečítať.

## Kedy je Kalendár k dispozícii \{#when-calendar-is-there}

Kalendár uchováva udalosti v [LosOS Cloud](losos-cloud), takže je
k dispozícii na každom zariadení, na ktorom beží LosOS Cloud, či zariadenie
zdieľa svoje úložisko, alebo nie. Pozvánky cez [Poštu](mail) a hovory
v [Chate](chat) potrebujú tieto aplikácie, ktoré bežia len vtedy, keď
zariadenie zdieľa so sieťou, do ktorej sa pripojilo. Bez nich udalosť nemá
hovor ani pozvánku a všetko ostatné zostáva rovnaké.

Náhradný kľúč otvorí administráciu, ale nemá kalendár, preto vás Kalendár
požiada o prihlásenie vlastným účtom.

## Zobrazenia \{#the-views}

- **Týždeň** ukáže dni od pondelka do nedele po hodinách, celodenné
  udalosti nad nimi a čiaru v aktuálnom čase. Dvojklikom na voľnú hodinu
  v nej začnete novú udalosť.
- **Mesiac** ukáže šesť týždňov. Deň, ktorý má viac udalostí, než sa
  zmestí, ukáže, koľko ich je navyše; kliknutím na deň otvoríte jeho týždeň.
- **Zoznam** ukáže najbližších šesť týždňov ako zoznam, deň po dni.

Tlačidlo **Dnes** a šípky vedľa neho prechádzajú týždňami alebo mesiacmi.

Stĺpec vľavo zobrazuje **Vaše kalendáre** a **Odbery**. Kliknutím na farbu
kalendára skryjete jeho udalosti alebo ich znova zobrazíte. Stránka si
pamätá, ktoré ste skryli. Tlačidlo **…** vedľa kalendára otvorí jeho
nastavenia.

Časy sa zobrazujú v časovom pásme vášho prehliadača. Udalosť vytvorená
v inom pásme, napríklad pozvánka zo zahraničia, si ponechá svoje a zobrazí
sa v správnom čase vo vašom.

## Udalosti \{#events}

**Nová udalosť** otvorí editor. Povinný je len názov. Udalosť môže:

- trvať celý deň alebo niekoľko dní;
- opakovať sa každý deň, týždeň, mesiac alebo rok, vo vybrané dni v týždni
  a do posledného dňa. Opakovanie vytvorené v telefóne, ktoré editor
  nevie zobraziť, napríklad „posledný piatok v mesiaci“, zostane také, aké
  bolo, kým nezvolíte iné;
- mať **Hostí**, ktorých pri písaní vyberiete zo svojich
  [kontaktov](contacts) a z ľudí na zariadení;
- byť **Videohovorom** v [Chate](chat), šifrovaným end-to-end;
- mať **Pripomienku** pred začiatkom;
- mať miesto a poznámky.

Kliknutím na udalosť ju otvoríte. Udalosť ukáže svojich hostí a či každý
z nich príde, s tlačidlom **Chat** pre každého hosťa, ktorý má adresu
v chate. Hovor má tlačidlo **Pripojiť sa k hovoru**, ktoré ho otvorí
v Chate. **Odstrániť** sa pri opakujúcej sa udalosti spýta, či odstrániť
len ten deň, alebo všetky dni.

Keď telefón zmenil udalosť počas toho, ako ste ju upravovali, uloženie to
oznámi a zmenu z telefónu neprepíše. Zatvorte editor a otvorte udalosť
znova, aby ste videli, čo sa zmenilo.

## Pozvánky \{#invitations}

Keď uložíte udalosť s novými hosťami, Kalendár ponúkne, že každému z nich
pošle pozvánku. Pozvánka je obyčajná správa s prílohou `.ics`, takže ju
prečíta každá kalendárová aplikácia, na zariadení aj mimo neho. **Poslať
pozvánky** pri udalosti ju pošle znova a **Napísať hosťom** otvorí správu
všetkým z nich.

Pozvánka, ktorá vám príde do Pošty, ukáže nad správou, o čo ide, s tlačidlami
**Áno**, **Možno** a **Nie**. Vaša odpoveď vloží udalosť do vášho kalendára
a pošle ju e-mailom tomu, kto vás pozval. Keď k nemu odpoveď dorazí, jeho
kópia udalosti ju ukáže hneď, ako správu otvorí. Zrušenie funguje rovnako:
jeho otvorenie označí udalosť vo vašom kalendári ako zrušenú.

## Pripomienky \{#reminders}

Pripomienka sa zobrazí na tej stránke administrácie, ktorú máte práve
otvorenú: názov udalosti, kedy začína a **Pripojiť sa k hovoru** pri hovore
alebo **Otvoriť** pri čomkoľvek inom. Ak ste v prehliadači povolili
upozornenia, zobrazí sa aj ako upozornenie, kým je karta na pozadí.
Pripomienka sa zobrazí, len keď je niekde otvorená stránka zariadenia.

## Odbery \{#subscriptions}

**Odoberať** sleduje kalendár, ktorý zverejňuje niekto iný, napríklad
zoznam daňových termínov alebo rozvrh z EduPage. Vložte jeho adresu, ktorá
začína `webcal://`, `https://` alebo `http://`, zvoľte, ako často ho čítať,
a farbu. Kalendár ho raz prečíta a až potom si ho ponechá.

- Odber sa číta znova sám, každú hodinu, šesť hodín, deň alebo týždeň,
  podľa vašej voľby. **Načítať teraz** v jeho nastaveniach ho prečíta hneď.
  Udalosti, ktoré zverejňovateľ pridal, zmenil alebo odstránil, sa zmenia
  s ním.
- Odber je len na čítanie. Jeho udalosti sa nedajú upraviť tu ani
  v telefóne; mení ich len zverejňovateľ.
- Zariadenie číta adresu v samostatnej službe, ktorá nemá prístup k vašim
  súborom a nedostane sa k službám, ktoré počúvajú len vnútri zariadenia.
- Keď sa čítanie nepodarí, nastavenia kalendára povedia prečo a kedy
  naposledy fungovalo. Udalosti z posledného úspešného čítania zostanú.

Zariadenie, na ktorom bežal Nextcloud, si odbery ponechá: presun do LosOS
Cloud prenesie každý z nich ako odber.

## Import \{#importing}

**Importovať** vloží súbor `.ics`, napríklad taký, ktorý exportovala
kalendárová aplikácia, do jedného z vašich kalendárov alebo do nového,
pomenovaného podľa súboru. Udalosť, ktorá už v kalendári je, sa aktualizuje
a nepridá sa druhýkrát.

## Zverejnenie kalendára \{#publishing-a-calendar}

**Zverejniť ako odber** v nastaveniach kalendára mu dá adresu, ktorú si
môže odoberať každá kalendárová aplikácia, napríklad
`webcal://mattbox.local/cloud/feed/….ics`. Kalendár prečíta každý, kto má
adresu, bez prihlásenia, preto ju dajte len ľuďom, ktorí ho majú vidieť.
Vypnutím zverejnenia adresa prestane fungovať. Opätovné zapnutie vytvorí
novú adresu.

## V telefóne a počítači \{#on-your-phone-and-computer}

Telefóny a kalendárové aplikácie v počítači sa synchronizujú s tými istými
kalendármi cez CalDAV. Nastavte ich ako v časti
[Kalendáre a kontakty](losos-cloud#calendars-and-contacts). Udalosť, ktorú
pridáte v telefóne, sa zobrazí v Kalendári, a udalosť, ktorú zmeníte
v Kalendári, sa do telefónu dostane pri jeho najbližšej synchronizácii.
Odbery sa synchronizujú do telefónu tiež, len na čítanie.
