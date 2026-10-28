---
title: Kontakty
sidebar_position: 4.8
slug: /manual/contacts
---

# Kontakty

![LosOS Contacts s otvorenou Evou: Vaše kontakty a Na tomto zariadení, adresár podľa písmen a Evina karta s tlačidlami pošty, chatu, hovoru a videa nad jej adresou, telefónom, adresou v chate a adresou v sieti mesh.](@site/docs/img/contacts-sk.png)

LosOS Contacts je adresár pre každého človeka na zariadení. Položka
**Kontakty** v bočnom paneli ho otvorí na adrese `/contacts`. Váš adresár
patrí len vám. Ostatní ľudia na zariadení, ani správcovia, ho nedokážu
prečítať cez administráciu ani cez LosOS Cloud.

## Kedy sú Kontakty k dispozícii \{#when-contacts-is-there}

Kontakty uchovávajú vaše kontakty v [LosOS Cloud](losos-cloud), takže sú
k dispozícii na každom zariadení, na ktorom beží LosOS Cloud, či zariadenie
zdieľa svoje úložisko, alebo nie. [Pošta](mail) a [Chat](chat) bežia len
vtedy, keď zariadenie zdieľa so sieťou, do ktorej sa pripojilo. Keď je
zdieľanie vypnuté, stránka človeka nemá tlačidlá **Pošta**, **Chat**,
**Hovor** ani **Video** a adresár zostáva taký, aký je.

Náhradný kľúč otvorí administráciu, ale nemá adresár, preto vás Kontakty
požiadajú o prihlásenie vlastným účtom.

## Kde je adresár uložený \{#where-the-book-is-kept}

Vaše kontakty sú adresáre vášho účtu v LosOS Cloud, jeden vCard na každý
kontakt. Kontakty zobrazia ľudí zo všetkých vašich adresárov, aj z toho,
ktorý vytvoril telefón alebo Thunderbird. Kontakt, ktorý pridáte tu, sa
uloží do vášho prvého adresára **Contacts**. Keď kontakt upravíte, Kontakty
zmenia polia, ktoré zobrazujú, a zvyšok karty nechajú tak, ako ho zapísal
telefón, napríklad skupiny, zvonenia a príbuzných. Fotky zostávajú v karte
a stránka ich zobrazí ako malý štvorcový obrázok.

## Adresár \{#the-book}

- **Vaše kontakty** zobrazí ľudí, ktorých ste pridali alebo importovali,
  podľa mena, s prvým písmenom ako nadpisom. **Hľadanie** prehľadá mená,
  prezývky, organizácie, e-mailové adresy, telefónne čísla a adresy
  v chate, s diakritikou aj bez nej.
- **Na tomto boxe** zobrazí každého ďalšieho, kto má na zariadení účet,
  takže ho zastihnete bez pridávania. **Pridať do kontaktov** ho skopíruje
  do vášho adresára.
- Kliknutím na človeka otvoríte jeho stránku: e-mailové adresy, telefónne
  čísla, adresu v chate, adresu v sieti, poštové adresy, narodeniny, weby
  a poznámky. Každý riadok má tlačidlo **Kopírovať**.
- Kliknutím pravým tlačidlom na človeka v zozname dostanete tie isté akcie
  bez otvárania stránky; pozri
  [ponuku pravého tlačidla](admin-pages#the-right-click-menu).

## Ako niekoho zastihnúť \{#reaching-someone}

Stránka človeka má tlačidlo pre každý spôsob, ako ho zastihnúť:

- **Pošta** otvorí novú správu na jeho prvú adresu. Zobrazí sa, keď na
  zariadení beží [Pošta](mail) a kontakt má e-mailovú adresu.
- **Chat** otvorí vašu priamu konverzáciu s ním alebo ju začne.
- **Hovor** a **Video** otvoria túto konverzáciu a začnú
  [hovor](calls).

Chat, Hovor a Video sa zobrazia, keď na zariadení beží Chat, a potrebujú adresu kontaktu v chate, napríklad
`@eva:mattbox.local`. Každý na tomto zariadení ju má; pri kontakte ju
zadajte do poľa **Chat (Matrix ID)**.

Adresár používajú aj ostatné aplikácie. Polia **Komu** a **Kópia** v Pošte
a **Napísať niekomu** v Chate pri písaní ponúkajú vaše kontakty popri
ľuďoch na zariadení. Kto je na zariadení a zároveň vo vašom adresári, sa
zobrazí raz, ako váš kontakt.

## Pridávanie a úpravy \{#adding-and-editing}

**Nový kontakt** otvorí editor. Povinné je len meno. **Ďalšie polia**
pridajú prezývku, pozíciu, poštové adresy, weby, narodeniny, adresu v sieti
(`meno@box`, na zdieľanie súborov s niekým na inom zariadení) a poznámku.
Každú e-mailovú adresu a telefónne číslo môžete označiť ako domov, práca,
mobil a podobne. **Pridať fotku** vezme obrázok z vášho počítača.

## Import a export \{#importing-and-exporting}

**Importovať** prijme súbor `.vcf` z telefónu alebo z iného adresára: vCard
4.0, 3.0 alebo 2.1, s jedným kontaktom alebo s mnohými. Fotky v súbore sa
prenesú tiež. Kontakt, ktorý už vo vašom adresári je, sa aktualizuje,
nepridá sa druhýkrát. Rozhoduje vlastný identifikátor v súbore, ak ho má,
inak meno spolu s prvou e-mailovou adresou alebo telefónnym číslom.

**Exportovať** stiahne celý adresár ako jeden súbor `.vcf`, vo vCard 3.0
(ktorý prečíta každý telefón) alebo 4.0. Stránka človeka a ponuka pravého
tlačidla exportujú aj jeden kontakt samostatne.

## V telefóne a v počítači \{#on-your-phone-and-computer}

Telefóny a adresáre v počítači sa s tými istými adresármi synchronizujú cez
CardDAV. Nastavte ich podľa časti
[Kalendáre a kontakty](losos-cloud#calendars-and-contacts). Kontakt, ktorý
pridáte v telefóne, sa objaví v Kontaktoch, a zmena v Kontaktoch sa do
telefónu dostane pri jeho ďalšej synchronizácii. Import a export zostávajú
na jednorazový presun odinakiaľ.
