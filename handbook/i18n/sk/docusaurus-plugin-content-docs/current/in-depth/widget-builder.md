---
title: Vytvoriť s AI
sidebar_position: 4.7
mdx:
  format: md
---

# Vytvoriť s AI

Opíšete widget alebo aplikáciu bežnými slovami a jazykový model na
zariadení ich napíše. Kartu **Vytvoriť s AI** majú dve miesta:

- editor widgetov (**Pridať widget**, potom **Napísať**), vedľa karty
  **Písať**;
- **Vaše aplikácie** na stránke Aplikácie, pod **Vytvoriť aplikáciu**.

To, čo model napíše, sa objaví v editore a v náhľade, kde si to môžete
prečítať, použiť a upraviť. Nič sa neuloží, kým nestlačíte Uložiť. Widget od
modelu beží v rovnakom sandboxovanom rámci ako ručne písaný (pozri
[Správa](administration.md#widgets-written-by-hand)) a aplikácia beží vo
vlastnom sandboxovanom rámci vo vašom prehliadači, nikdy na zariadení.

## Ako tvorba prebieha {#how-a-build-goes}

Tvorba je krátky cyklus, ktorý spúšťa zariadenie a karta ho ukazuje krok za
krokom:

1. **Písanie.** Model napíše widget alebo aplikáciu.
2. **Kontrola.** Zariadenie prečíta kód a hľadá chyby, ktoré malý model robí
   najčastejšie: meno, ktoré sa používa, ale nikde nevzniká, meno použité
   skôr než riadok, ktorý ho vytvára, skript, ktorý hľadá prvok, aký widget
   nemá, kód, ktorý sa nedá preložiť, alebo príklad z pokynov poslaný späť
   namiesto toho, o čo ste žiadali.
3. **Vyskúšanie** (len aplikácie). Náhľad vo vašom prehliadači spustí
   aplikáciu tak, ako by to urobil človek: vyplní každé pole, stlačí prvé
   tlačidlo a pozrie sa, čo si aplikácia ponechala. Keď si nenechala nič a
   má zoznam na výber, vyberie inú položku a stlačí znova. Zapíše si aj
   všetko, čo aplikácia vyhodí pri štarte alebo po stlačení.
4. **Oprava.** Keď kontrola alebo vyskúšanie niečo našli, zariadenie to
   modelu povie slovami a model napíše celé dielo znova. Aplikácia dostane
   najviac tri opravy, widget jednu.

Každý krok ukazuje, čo našiel. Tvorbu môžete zastaviť v ktoromkoľvek kroku.
Keď cyklus skončí, do editora sa dostane najlepšia verzia: tá, ktorá sa dá
preložiť a ostalo v nej najmenej problémov. Ak nejaké ostali, karta ich
vypíše, aby ste vedeli, na čo sa v náhľade pozrieť.

Vyskúšanie potrebuje otvorenú stránku. Ak okno zatvoríte, tvorba pokračuje
a kroky uvedú, že aplikácia nebola vyskúšaná.

## Beží na zariadení {#it-runs-on-the-box}

Model je Qwen2.5-Coder 1.5B Instruct od tímu Qwen v Alibaba Cloud a
poskytuje ho [Ollama](https://ollama.com) priamo na zariadení. To, čo
napíšete, a widget či aplikácia, ktoré chcete upraviť, zariadenie nikdy
neopustia a tvorba nič nestojí.

Zariadenie si model, asi 1 GB, stiahne raz, keď je po inštalácii prvýkrát
online. Dovtedy karta oznamuje, že sa model ešte sťahuje. Model zostáva na
šifrovanom disku, takže reštart ho znova nesťahuje.

### Prečo taký malý model {#why-a-model-this-small}

Model je vybraný pre najmenšie zariadenie, na ktorom LosOS beží: 4 GiB
pamäte, dve jadrá procesora, žiadna grafická karta a popri tom bežia LosOS
cloud a LosOS Git. Väčší model by sa tam nezmestil alebo by písal jednu
aplikáciu takmer hodinu.

Malý model robí viac chýb a na to je cyklus. Väčšinu toho, čo pokazí,
zachytí kontrola alebo vyskúšanie a opraví sa v ďalšom kole.

Na zariadení, ktorému zostáva 8 GiB pamäte alebo viac, píše väčší
Qwen2.5-Coder lepšie aplikácie hneď na prvý raz. V **Nastavenia, Pokročilé**
nastavte `losos.builder.model` na `qwen2.5-coder:7b`. Kým píše, potrebuje
asi 5 GiB a trvá niekoľkonásobne dlhšie.

## Čo čakať {#what-to-expect}

Tvorba beží na procesore zariadenia. Na dvojjadrovom mini-PC trvá aplikácia
DURATION_APP_SK, widget DURATION_WIDGET_SK a každá oprava zhruba toľko
isto. Karta ukazuje, koľko riadkov model zatiaľ napísal.

:::tip[Žiadajte jednu vec]
Model je malý. Najlepšie zvláda jednu jasnú vec: nákupný zoznam, poznámky,
sledovanie návykov, hodiny, disk ako pruh. Vetou či dvoma povedzte, čo do
nej patrí. Potom výsledok pred uložením vyskúšajte v náhľade. Keď niečo
nesedí, vytvorte ho znova, požiadajte o úpravu alebo upravte kód ručne.
:::

## Pamäť {#memory}

Kým model píše, zaberie asi 1,5 GiB pamäte navyše k tomu, čo zariadenie
používa tak či tak. Uvoľní ju päť minút po poslednej tvorbe. Ak zariadeniu
pamäť chýba, tvorba môže zlyhať a karta to oznámi.

:::note[Vypnutie tvorby]
**Nastavenia, Pokročilé** majú `losos.builder.enable`. Keď ho vypnete,
zariadenie model nesťahuje ani nespúšťa a karta oznámi, že tvorba je
vypnutá.
:::

## Podmienky Qwenu {#qwens-terms}

Qwen2.5-Coder 1.5B Instruct je licencovaný pod
[Apache License 2.0](https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/blob/main/LICENSE).
Karta uvádza model pod tlačidlom Vytvoriť a odkazuje na jeho licenciu a
stránku.

## Dobré vedieť {#good-to-know}

- Tvorba pokračuje aj po zatvorení okna, len bez vyskúšania.
- Zariadenie spúšťa jednu tvorbu naraz.
- Ak chcete widget alebo aplikáciu upraviť, nechajte ich v editore a
  zaškrtnite políčko, ktoré žiada úpravu. Model dostane to, čo tam je, a
  vráti celé dielo upravené.
- Widget od modelu môže čítať hodnoty zariadenia ako každý widget.
  Aplikácia si cez stránku ukladá údaje na zariadení. Ani jedno nemôže
  zariadenie meniť.
