---
title: Chat
sidebar_position: 4.6
slug: /manual/chat
---

# Chat

LosOS Chat je komunikátor na zariadení pre každého, kto má účet v LosOS
Login.
Ľudia si píšu priame správy aj v miestnostiach, posielajú si obrázky a
súbory, odpovedajú vo vláknach a reagujú na správy. Priame správy a nové
miestnosti sú šifrované od konca po koniec, takže zariadenie ukladá len to,
čo samo nevie prečítať.

Chat používa Matrix, otvorený štandard na posielanie správ, a beží priamo na
zariadení. Nič neodchádza do cudzej služby.

## Kedy je chat k dispozícii

Chat patrí k časti zariadenia, ktorú zdieľate so sieťou zariadení (mesh), a
všetko si ukladá do jej úložiska. Beží len vtedy, keď zariadenie zdieľa
svoje úložisko a je pripojené k sieti zariadení. Vtedy sa na bočnom paneli
stránok správy objaví **Chat**.

Keď zdieľanie vypnete, chat sa zastaví s ním. Položka na bočnom paneli
zmizne a adresa chatu vedie späť na stránky správy. Správy zostanú v
zdieľanom úložisku a vrátia sa, keď zdieľanie znova zapnete.

## Prihlásenie

Otvorte **Chat** na bočnom paneli alebo prejdite na
`https://<name>.local/chat/`. Chat je jedna zo [správcovských stránok](admin-pages), s rovnakou
hornou lištou a bočným panelom: konverzácie vľavo a otvorená vedľa nich.
Používa vaše prihlásenie do LosOS Login.
Samostatné heslo do chatu neexistuje.

Keď sa odhlásite z LosOS Login, odhlási sa aj chat. Keď správca niekoho
odstráni v **Nastavenia** > **Používatelia**, zariadenia tohto človeka sa z
chatu hneď odhlásia.

Adresa každého v chate je `@<prihlasovacie meno>:<name>.local`, napríklad
`@eva:mattbox.local`.

## Zabezpečené správy

Keď chat otvoríte prvýkrát, požiada vás o nastavenie zabezpečených správ.
Kliknite na **Nastaviť zabezpečené správy** a potom na **Vytvoriť kľúč na
obnovenie**. Chat ukáže kľúč na obnovenie z dvanástich skupín písmen a
číslic. Skopírujte ho alebo stiahnite, uložte ho na bezpečné miesto mimo
tohto počítača a kliknite na **Uložil(a) som ho**. Chat ho už znova neukáže.

Kľúč umožní vašim ďalším zariadeniam čítať šifrované správy. Keď chat
otvoríte v inom prehliadači alebo na inom počítači, ukáže **Odomknite toto
zariadenie na čítanie šifrovaných správ**. Kliknite naň a napíšte kľúč na
obnovenie, alebo nové zariadenie potvrďte z toho, ktoré už používate, tak,
že porovnáte emoji, ktoré ukážu obe.

Bez kľúča a bez iného zariadenia nové zariadenie neprečíta správy poslané
pred jeho prihlásením. Nikto vám ich neobnoví, ani správca zariadenia.

Tlačidlo so zámkom v hornej lište kedykoľvek otvorí
**Zabezpečené správy**. Uvádza vaše zariadenia a každé z nich môžete overiť
alebo odhlásiť.

## Písanie ľuďom

- **Priama správa:** kliknite na **Napísať niekomu** a vyberte človeka. V
  zozname je každý, kto má na zariadení účet v LosOS Login.
- **Miestnosť:** kliknite na **Nová miestnosť**, pomenujte ju a vyberte, či
  je na zariadení verejná, alebo len na pozvanie. Ľudí pozvete z hlavičky
  miestnosti.
- **Priestory** zoskupujú miestnosti. Sú to tlačidlá nad konverzáciami, vedľa
  vyhľadávania; nový vytvoríte tlačidlom **Nový priestor** a miestnosti doň
  pridáte v nastaveniach každej miestnosti.
- **Nájsť miestnosti** uvádza verejné miestnosti na zariadení.

## V konverzácii

Keď podržíte ukazovateľ nad správou, môžete na ňu reagovať, odpovedať,
odpovedať vo vlákne, upraviť ju alebo zmazať. To isté, a navyše kopírovanie
textu, ponúka kliknutie pravým tlačidlom na správu. Tlačidlo vlákien v hlavičke
miestnosti uvádza jej vlákna.

Obrázky, videá a súbory priložíte kancelárskou sponkou, vložením zo
schránky alebo pretiahnutím. V šifrovanej miestnosti sa zašifrujú skôr, ako
opustia váš prehliadač. Mikrofón nahrá hlasovú správu.

Ak má zariadenie LosOS Cloud, tlačidlo s oblakom vedľa sponky pošle súbory
priamo z vášho Cloudu. Súbor, ktorý vám niekto pošle, má na svojej karte
tlačidlo s oblakom a v ponuke pravého tlačidla **Uložiť do LosOS Cloud**:
uloží sa dešifrovaný do priečinka **Chat** vo vašom Cloude a tlačidlo ho
potom otvorí tam. Keď v LosOS Cloud pošlete súbor do chatu, chat sa spýta,
do ktorej konverzácie má ísť.

Malý rad tvárí pod správou ukazuje, kto už čítal až potiaľto.

## Iné aplikácie pre Matrix

Iné aplikácie pre Matrix, napríklad Element, sa do chatu na zariadení
neprihlásia, pretože chat nemá heslá a prihlasuje len cez LosOS Login.
Používajte stránku chatu v prehliadači na ľubovoľnom zariadení v sieti
zariadenia.

Chat komunikuje len so zariadením. Spojenie s inými servermi Matrix je
vypnuté, kým ho správca nezapne cez `losos.chat.federation.enable` v
**Nastavenia** > **Rozšírené**. Na to treba vlastnú doménu, ktorú ostatné
servery dosiahnu.
