---
title: Hovory
sidebar_position: 11
slug: /manual/calls
---

# Hovory

Ľudia na zariadení si môžu z LosOS chatu volať, hlasom alebo s videom, v
priamej správe alebo v miestnosti s najviac ôsmimi ľuďmi. Hovor ide cez
zariadenie a zariadenie ho nevidí ani nepočuje. Každý snímok obrazu a zvuku
zašifruje prehliadač, ktorý ho posiela, a dešifrujú ho až prehliadače,
ktoré ho prijímajú.

## Kedy sú hovory k dispozícii \{#when-calls-are-there}

Hovory patria k chatu, takže potrebujú to isté čo chat: zariadenie zdieľa
svoje úložisko a pripojilo sa k sieti mesh. Potrebujú aj
**Nastavenia → Rozšírené → `losos.calls.enable`**, ktoré je zapnuté, kým ho
nevypnete. Ak je čokoľvek z toho vypnuté, miestnosti nemajú tlačidlá na
volanie a zariadenie pre hovory nespúšťa nič. Keď niečo z toho znova
zapnete, hovory sa vrátia po ďalšom Použiť.

## Ako začať hovor \{#starting-a-call}

Otvorte miestnosť alebo priamu správu. V hlavičke sú dve tlačidlá,
**Hlasový hovor** a **Videohovor**. Prehliadač sa raz opýta na mikrofón, a
na kameru, keď začínate s videom.

Každému v miestnosti sa v rohu stránky chatu ukáže karta, ktorá zvoní
minútu. **Prijať** pripojí tak, ako sa hovor začal. Pri
videohovore **Prijať bez videa** pripojí len s mikrofónom. **Odmietnuť**
kartu len skryje.

Časová os miestnosti ukáže, že sa začal hovor. Kým hovor trvá, riadok má
tlačidlo **Pripojiť sa**, takže sa môže pripojiť aj ten, kto zvonenie
zmeškal alebo z hovoru odišiel. Keď odíde posledný človek, časová os ukáže,
že sa hovor skončil.

## Počas hovoru \{#during-a-call}

Každý človek má svoju dlaždicu. Dlaždica ukazuje jeho kameru, alebo jeho
obrázok, kým má kameru vypnutú. Tri tlačidlá dole vypínajú a zapínajú váš
mikrofón a kameru a ukončia vašu účasť v hovore. Vypnutie kamery ju
zastaví, takže zhasne jej svetlo, a kým ju znova nezapnete, neposiela sa
nič.

Štítok **Šifrované end-to-end** hore ukazuje, koľko snímok váš prehliadač
poslal zašifrovaných a koľko prijal a otvoril.

## Čo vidí zariadenie \{#what-the-box-sees}

Zariadenie každý snímok posunie ostatným v hovore bez toho, aby ho otvorilo.
Vie, kto je v ktorom hovore a kedy, aké veľké sú snímky a ako často
prichádzajú. Obraz ani zvuk nikdy nemá.

Prehliadač každého človeka si pre hovor vytvorí vlastný kľúč. Prehliadače
si kľúče odovzdávajú v šifrovaných správach chatu medzi zariadeniami, ktoré
zariadenie LosOS prenáša, ale nevie ich prečítať. Kľúče sa počas hovoru
menia:

- Keď sa niekto pripojí, všetci prejdú na ďalší kľúč a nový človek dostane
  ten. Neotvorí nič, čo sa poslalo, kým neprišiel.
- Keď niekto odíde, všetci si vytvoria nový náhodný kľúč a dajú ho len
  tým, ktorí zostali. Ten, kto odišiel, neotvorí nič, čo sa pošle potom.
- Každých päť minút všetci aj tak prejdú na ďalší kľúč.

Kľúče sú len v pamäti prehliadača a s koncom hovoru zmiznú. Nič sa
neukladá, takže kľúč, ktorý by sa našiel neskôr, z hovoru neotvorí nič.

Jednu hranicu je dobré poznať. Prehliadač posiela svoj kľúč zariadeniam,
ktoré pre každého človeka v hovore uvádza server chatu na zariadení LosOS.
Niekto, kto by ovládol softvér zariadenia, by mohol uviesť aj vlastné
zariadenie. Overte si navzájom zariadenia v chate (**Zabezpečené správy**,
potom **Overiť**), aby ste vedeli, že zariadenie naozaj patrí tomu druhému.

## Prehliadače \{#browsers}

Aby sa prehliadač mohol pripojiť k hovoru, musí vedieť šifrovať snímky.
Aktuálny Firefox, Chrome, Edge a Safari to vedia. Prehliadač, ktorý to
nevie, neukáže tlačidlá na volanie a nikdy nepošle snímok bez šifrovania.

## Kde hovory fungujú \{#where-calls-work}

Hovory fungujú medzi počítačmi, ktoré sa k zariadeniu dostanú v jeho
vlastnej sieti. Zariadenie prijíma médiá na porte UDP 7882
(**Nastavenia → Rozšírené → `losos.calls.udpPort`**), ktorý je na jeho
firewalle otvorený, kým sú hovory zapnuté. Hovory zvonka tejto siete zatiaľ
nefungujú.
