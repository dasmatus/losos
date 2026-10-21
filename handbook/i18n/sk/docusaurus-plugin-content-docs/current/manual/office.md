---
title: Kancelárske dokumenty
sidebar_position: 4.6
slug: /manual/office
---

# Kancelárske dokumenty

LosOS Office otvára textové dokumenty, tabuľky a prezentácie v prehliadači a
ukladá ich späť na zariadenie. Na jednom dokumente môže naraz pracovať
viac ľudí a každý vidí, kde práve píšu ostatní. Editor je Collabora
Online a beží priamo na zariadení. Dokumenty ho nikdy neopustia.

Otvára a ukladá tieto typy:

| Druh | Typy |
|---|---|
| Text | `.odt`, `.docx`, `.docm`, `.dotx`, `.doc`, `.rtf`, `.txt` |
| Tabuľky | `.ods`, `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.csv` |
| Prezentácie | `.odp`, `.pptx`, `.pptm`, `.ppsx`, `.ppt` |
| Kresby | `.odg` |

Súbory PDF, kresby z Visia a súbory Pages, Numbers a Keynote od Apple
zobrazí bez zmeny. Staršie formáty Microsoftu otvorí aj uloží, no najviac
zachovajú formáty OpenDocument a novšie formáty Microsoftu.

Súbory Word, Excel a PowerPoint z Microsoft 365 zostanú tým, čím sú:
`.docx` sa uloží späť ako `.docx` aj so štýlmi a zošit si ponechá vzorce.
Makrá v súboroch `.docm`, `.xlsm` a `.pptm` sa na zariadení nikdy
nespustia.

## Zapnutie \{#turning-it-on}

Na novom zariadení je LosOS Office vypnutý, pretože potrebuje pamäť, aj keď nikto
nemá otvorený žiadny dokument. Zapnete ho v **Nastavenia → Rozšírené**
voľbou `losos.office.enable` a potom stlačte **Použiť**. Počítajte s asi
550 MB bez otvoreného dokumentu. Každý otvorený dokument pridá ďalšiu pamäť:
krátky niekoľko desiatok MB, veľký viac. Zariadenie mu nikdy nedovolí vziať
viac než 2 GB.

## Otvorenie dokumentu \{#opening-a-document}

Kliknite na názov dokumentu a otvorí sa na celú šírku stránky, pod hornou
lištou zariadenia a lištou s cestou späť a názvom dokumentu. Editor sa
riadi svetlou alebo tmavou témou zariadenia a preberá jeho farby; samotná
stránka dokumentu si nechá vzhľad, s akým bola napísaná. Dokument, ktorý smiete iba čítať, sa
otvorí so štítkom **Len na čítanie** a editor ho nedovolí zmeniť. Editor
funguje na adrese https zariadenia, tej istej, ktorú používa LosOS Login;
stránka otvorená cez http sa tam najprv sama presunie.

Editor ukladá priebežne a aj keď odídete. **Súbor → Uložiť ako** uloží
kópiu vedľa originálu v ktoromkoľvek z typov vyššie alebo ako PDF.

## Spoločná práca \{#working-together}

Keď ten istý dokument otvorí druhý človek, ste obaja v tej istej kópii.
Každý vidí kurzor a meno toho druhého a každá zmena sa na druhej obrazovke
objaví hneď, ako vznikne. Nikto nečaká, kým druhý skončí, a jedno uloženie
zachová zmeny všetkých.

## Čo zostáva súkromné \{#what-stays-private}

Editor beží na zariadení a dokument číta priamo z jeho disku. Nič sa
neposiela spoločnosti Collabora ani nikomu inému: žiadne kontroly
aktualizácií, žiadne služby na pravopis či preklad, žiadne písma z
internetu.

Otvorenie dokumentu dá vášmu prehliadaču kľúč k tomuto jednému dokumentu,
ktorý vyprší po desiatich hodinách. Nič iné editor neprijme, takže sa do
dokumentu dostanú len ľudia, ktorí ho smú otvoriť sami.

## Priečinok dokumentov \{#a-folder-of-documents}

`losos.office.folder` určuje priečinok na zariadení, ktorého dokumenty
LosOS Office upravuje sám, bez LosOS cloud. V bočnom paneli sa potom za
**Aplikácie** objaví **Dokumenty** pre každého, kto sa prihlási. Vypíše
dokumenty v priečinku aj v priečinkoch v ňom a každý otvorí v editore. Ako
ostatné stránky správy odpovedá iba vo vlastnej sieti zariadenia. Je
určený pre zariadenie, ktoré drží jeden priečinok dokumentov pre ľudí vo
svojej sieti. Inak ho nechajte prázdny.
