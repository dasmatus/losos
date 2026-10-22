---
title: Kontrola s AI v LosOS Git
sidebar_position: 4.6
slug: /manual/ai-review
---

# Kontrola s AI v LosOS Git

LosOS Reviewer číta pull requesty repozitára v LosOS Git a nechá k nim
kontrolu, podobne ako to robí kontrolný bot na hostovanej forge. Beží na
zariadení s tým istým jazykovým modelom ako Vytvoriť s AI, takže kód
zariadenie neopustí a kontrola nič nestojí.

V každom repozitári je vypnutý, kým ho niekto nezapne.

## Zapnutie \{#turning-it-on}

Otvorte repozitár v LosOS Git, potom **Nastavenia**, a zapnite **Kontrolu
s AI**. Prepínač vidia len administrátori repozitára.

Prepínač sa nedá zapnúť, kým je na zariadení vypnuté Vytvoriť s AI
(`losos.builder.enable`), pretože toto nastavenie inštaluje model. Nové
zariadenie si model stiahne raz, keď je prvýkrát online. Kým sťahovanie
neskončí, kontroly čakajú vo fronte a pull request to uvádza.

## Čo robí \{#what-it-does}

Keď sa pull request otvorí alebo znova otvorí, LosOS Reviewer skontroluje
celý. Keď doň pribudnú nové commity, skontroluje to, čo tieto commity
zmenili od jeho poslednej kontroly. Pri každom behu zverejní jednu
kontrolu pod účtom **LosOS Reviewer** (`losos-reviewer`), označeným ako
bot:

- prehľad toho, čo pull request robí;
- tabuľku zmenených súborov, pri každom jednu vetu o tom, čo sa zmenilo;
- komentáre k jednotlivým riadkom, kde našiel problém. Keď ho opraví jeden
  zmenený riadok, komentár nesie tento riadok ako návrh.

Iba komentuje. Nikdy neschvaľuje, nežiada zmeny, nepushuje ani nezlučuje.

Na jeho komentáre odpovedáte a vyriešite ich ako komentáre kohokoľvek
iného. Ak chcete, aby sa pozrel znova, vyberte **LosOS Reviewer** v časti
**Kontrolóri** na pull requeste, aj keď ho už raz skontroloval. Môže to
urobiť každý, kto smie na pull requeste žiadať o kontrolu.

Kým pracuje, kontroly pull requestu ukazujú riadok **LosOS Reviewer** s
jeho postupom, napríklad „Reviewing: 2 of 5 files done“, a po skončení
počet komentárov.

## Čo dokáže nájsť \{#what-it-can-find}

Každý zmenený súbor sa číta po častiach s najviac 80 riadkami. Pri každej
časti sa stanú dve veci:

- Zariadenie skontroluje nové riadky na krátky zoznam chýb, ktorými si
  môže byť isté: SQL poskladané z hodnôt, premenné shellu bez úvodzoviek v
  príkazoch, ktoré menia súbory, kľúče a tokeny zapísané v kóde, spoločný
  predvolený zoznam v Pythone, rozsah o krok za koncom v Ruste, `==` v
  JavaScripte a TypeScripte, efekt v Reacte bez zoznamu závislostí a názov
  súboru, z ktorého sa už neodstraňujú priečinky.
- Model dostane otázku, či nové riadky obsahujú chybu. Komentár modelu
  zostane, len ak menuje riadok, ktorý pull request pridáva, a cituje kód
  na tomto riadku. Všetko ostatné, čo povie, sa zahodí.

Koľko model pridá, závisí od jeho veľkosti. Predvolený model,
Qwen2.5-Coder 1.5B, sa zmestí do zariadenia so 4 GiB pamäte. Zmeny
popisuje dobre, ale chybu sám nájde zriedka, takže na takom zariadení
pochádza väčšina komentárov z kontrol zariadenia. Qwen2.5-Coder 7B nájde
aj vlastné chyby a potrebuje asi 8 GiB. Ak ho chcete použiť, nastavte
`losos.builder.model` na `qwen2.5-coder:7b` v paneli Rozšírené. Potom ho
použije aj Vytvoriť s AI.

Kontrola nie je dôkaz, že kód je správny. Pred zlúčením si kód prečítajte
sami.

## Čo vynechá \{#what-it-leaves-out}

- Súbory, ktoré píšu nástroje a nikto ich nečíta riadok po riadku:
  lock súbory, minifikovaný a generovaný kód, obrázky a iné súbory, ktoré
  nie sú text.
- Zmazané súbory a riadky, ktoré sú len odstránené.
- Všetko za prvými 24 časťami veľkého pull requestu. Tabuľka uvádza, ktoré
  súbory vynechal.

## Zariadenie zostane použiteľné \{#the-box-stays-usable}

Kontroly bežia po jednej, otázka po otázke, v poradí, v akom o ne niekto
požiadal. Nový push nahradí nedokončenú kontrolu toho istého pull
requestu a vypnutie prepínača zahodí kontroly, ktoré ešte čakajú. Na dvoch
jadrách trvá jedna časť asi 10 až 20 sekúnd, takže pull request s
niekoľkými súbormi je skontrolovaný za pár minút. Vytváranie vo Vytvoriť s
AI a kontrola zdieľajú model a každé počká na aktuálnu otázku toho
druhého.

LosOS Git sa dostane len na adresy na samotnom zariadení. Kontrolór
nepoužíva zdieľané počítače siete, pretože by tak poslal váš kód na
zariadenia iných ľudí.
