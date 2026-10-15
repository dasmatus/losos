---
title: Tvorca widgetov
sidebar_position: 4.7
mdx:
  format: md
---

# Tvorca widgetov

Opíšete widget bežnými slovami a jazykový model na zariadení ho napíše.
Editor widgetov (**Pridať widget**, potom **Napísať**) má vedľa karty
**Písať** kartu **Vytvoriť s AI**. Súbory widgetu sa objavia v editore, kde
si ich môžete prečítať, vyskúšať v náhľade a upraviť. Nič sa neuloží, kým
nestlačíte Uložiť, a widget od modelu beží v rovnakom sandboxovanom rámci ako
ručne písaný (pozri [Správa](administration.md#ručne-písané-widgety)).

## Beží na zariadení

Model je Llama 3.2 od spoločnosti Meta s 3 miliardami parametrov a poskytuje
ho [Ollama](https://ollama.com) priamo na zariadení. To, čo napíšete, a
widget, ktorý chcete upraviť, zariadenie nikdy neopustia a tvorba nič
nestojí.

Zariadenie si model, asi 2 GB, stiahne raz, keď je po inštalácii prvýkrát
online. Dovtedy karta oznamuje, že sa model ešte sťahuje. Model zostáva na
šifrovanom disku, takže reštart ho znova nesťahuje.

## Čo čakať

Tvorba beží na procesore zariadenia. Na mini-PC trvá niekoľko minút a karta
ukazuje, koľko riadkov model zatiaľ napísal. Môžete ju zastaviť.

Model je malý. Najlepšie zvláda jednu jasnú vec na widget: hodiny,
odpočítavanie, disk ako pruh, zoznam aplikácií, počasie pre jedno mesto.
Požiadajte o jednu vec vetou či dvoma. Pred uložením sa na výsledok pozrite v
náhľade; niekedy widget neukáže nič alebo ukáže nesprávnu hodnotu, a vtedy
pomôže nová tvorba alebo malá ručná úprava.

## Pamäť

Kým model píše, zaberie asi 4 GiB pamäte navyše k tomu, čo zariadenie
používa tak či tak. Uvoľní ju päť minút po poslednej tvorbe. S 8 GiB pamäte
alebo viac je miesto pre oboje. Na zariadení s menšou pamäťou môže tvorba
zlyhať pre nedostatok pamäte a karta to oznámi.

**Nastavenia, Pokročilé** majú `losos.builder.enable`. Keď ho vypnete,
zariadenie model nesťahuje ani nespúšťa a karta oznámi, že tvorca je vypnutý.

## Podmienky Llamy

Llama 3.2 je licencovaná pod
[Llama 3.2 Community License](https://www.llama.com/llama3_2/license/) a jej
[Acceptable Use Policy](https://www.llama.com/llama3_2/use-policy/). Karta
uvádza „Built with Llama“ a pod tlačidlom Vytvoriť odkazuje na oba dokumenty.

## Dobré vedieť

- Tvorba pokračuje aj po zatvorení okna.
- Zariadenie spúšťa jednu tvorbu naraz.
- Ak chcete widget upraviť, nechajte ho v editore a zaškrtnite políčko, ktoré
  žiada úpravu. Model dostane všetky súbory widgetu a vráti tie, ktoré
  ponechá.
- Widget od modelu môže čítať hodnoty zariadenia ako každý widget. Zariadenie
  meniť nemôže.
