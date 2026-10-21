---
title: Webová stránka edge
sidebar_position: 6.5
mdx:
  format: md
---

# Webová stránka edge

Firemný edge má pre ľudí, ktorí ho spravujú, webovú stránku na `/manage/`.
Ukazuje zariadenia, ktoré edge smeruje, a na každom zariadení, ktorého
vlastník to dovolí, mení nastavenia LosOS Cloud a LosOS Git a inštaluje či
odstraňuje aplikácie. Oficiálne edge LosOS túto stránku nemajú a zariadenie
odmietne, aby ho niektorý z nich spravoval.

Stránka je nástroj firmy, nie vlastníka. Vlastník má na všetko ostatné
naďalej administračné stránky svojho zariadenia a na zariadení rozhoduje, či
ho edge smie spravovať.

## Zapnutie

Na edge nastavte `losos.edge.manage.enable`. Počítač s
[edge bránou](edge-federation.md) ho má zapnuté.

Edge si pri prvom spustení vytvorí kľúč na správu: 64 znakov, `0` až `9` a
`a` až `f`, uložený v `/var/lib/losos-registrar/manage-key`. Stránku môže
používať každý, kto kľúč má, preto ho držte spolu s ostatnými
administrátorskými heslami firmy. Na počítači s edge bránou ho
`losos-edge manage-key` na konzole vypíše spolu s adresou stránky. Ak chcete
kľúč zmeniť, súbor zmažte a registrar reštartujte. Edge vytvorí nový kľúč a
starý prestane platiť.

Stránka odpovedá na adrese edge v lokálnej sieti, na tej, podľa ktorej ho
zariadenia nachádzajú, napríklad `http://<edge>.local:8443/manage/`.

## Prihlásenie

Stránka si vypýta kľúč na správu a overí ho na edge, až potom pokračuje.
Kľúč drží len v danej karte prehliadača, takže zatvorením karty sa
odhlásite. **Odhlásiť sa** hore urobí to isté hneď.

Ak edge odpovie, že kľúč nepozná, kľúč je preklepnutý alebo zmenený. Ak
odpovie, že nespravuje zariadenia, je `losos.edge.manage.enable` vypnuté.

## Zoznam zariadení

![Zoznam zariadení na edge: dve spravované tu, jedno, ktorého vlastník správu nezapol.](@site/docs/img/edge-manage-boxes-sk.png)

**Zariadenia** ukazuje každé zariadenie, ktoré edge smeruje, podľa mena a s
časom, kedy sa edge naposledy ozvalo. Zoznam sa obnovuje každých 10 sekúnd.

- **Spravované tu** znamená, že vlastník zariadenia zapol na stránke Mesh
  zariadenia **Nechať firemný okrajový server spravovať toto zariadenie**.
  **Spravovať** zariadenie otvorí.
- **Nezapnuté** znamená, že vlastník to nezapol. Edge zariadenie smeruje,
  ale meniť ho nemôže. Ak to firma potrebuje, opýtajte sa vlastníka.

Zariadenie pri každom heartbeate povie, či to vlastník dovoľuje, takže zmena
na zariadení sa tu ukáže do jedného heartbeatu. Zariadenie, ktoré heartbeaty
prestane posielať, zo zoznamu zmizne.

## Stránka zariadenia

![Stránka zariadenia: nastavenia LosOS Cloud a LosOS Git, nainštalované aplikácie a hľadanie aplikácií.](@site/docs/img/edge-manage-box-sk.png)

Stránka zariadenia má tri časti, tie isté ako panel Aplikácie na samotnom
zariadení:

- **Nastavenia LosOS Cloud a LosOS Git.** Predvolený jazyk, oblasť
  telefónnych čísel, ako dlho sa držia zmazané súbory a staré verzie,
  predvolená kvóta, verejné odkazy s heslom a platnosťou, a v LosOS Git
  prihlásenie, súkromie, úvodná stránka a nastavenia pushu. Je tu aj
  federácia s inými zariadeniami. Nastavenie, ktoré zariadenie pevne určilo
  vo vlastnej konfigurácii, sa neukáže.
- **Nainštalované aplikácie.** Čo na zariadení beží, s tlačidlom na
  odstránenie každej.
- **Hľadanie aplikácií.** Hľadanie v obchode s aplikáciami, s tlačidlom na
  inštaláciu pri každom výsledku.

Zmenené nastavenia sa zbierajú v lište dole. **Použiť** ich pošle
zariadeniu, ktoré ich skontroluje, uloží do histórie svojej konfigurácie a
prebuduje sa, presne ako keď zmenu použije vlastník na zariadení. Lišta
ukazuje prebudovanie, kým neskončí.

## Čo edge nemôže

Každé volanie zo stránky ide do zariadenia jeho vlastným tunelom a
rozhoduje zariadenie. Odpovie len vtedy, keď volanie nesie token tunela
zariadenia a prepínač vlastníka menuje práve tento edge. Potom edge pustí k
nastaveniam aplikácií a k čítaniu toho, čo číta každá stránka po prihlásení,
k ničomu inému.

Edge preto nemôže:

- otvoriť ničie súbory, v LosOS Cloud, LosOS Git ani nikde inde;
- pridávať, meniť ani odstraňovať ľudí, ani obnovovať heslá;
- meniť úložisko, zálohy, sieť, zdieľanie ani aktualizácie;
- zapnúť prepínač, ani ho držať zapnutý, keď ho vlastník vypne.

Upravená stránka na edge sa ďalej nedostane, lebo zariadenie kontroluje
každé volanie samo. Prepínač patrí edge, ktorý zariadenie používalo, keď ho
vlastník zapol. Ak zariadenie neskôr prejde na iný edge, ten ho nezdedí. Keď
vlastník prepínač vypne, ďalšie volanie edge zariadenie odmietne a po
najbližšom heartbeate sa tu ukáže ako **Nezapnuté**. Otvorená stránka
zariadenia v tej chvíli oznámi, že zariadenie edge odmietlo.
