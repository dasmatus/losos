---
title: Kancelárske dokumenty
sidebar_position: 4.6
slug: /manual/office
---

# Kancelárske dokumenty

![Dokumenty, zoznam LosOS Office: dokumenty v priečinku LosOS Office na zariadení, s tým, kedy sa každý zmenil, a jeho veľkosťou.](@site/docs/img/office-sk.png)

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

V LosOS Cloud kliknite na názov dokumentu. Otvorí sa v LosOS Office na celú
šírku stránky, pod hornou lištou zariadenia, na adrese `/cloud/office/`
s číslom súboru, takže si ju môžete uložiť medzi záložky alebo poslať
niekomu, kto súbor tiež smie otvoriť.

Nad dokumentom sú dve lišty, nakreslené ako zvyšok LosOS. Na prvej je cesta
späť, názov dokumentu a či sú vaše zmeny už uložené v zariadení, ľudia,
ktorí sú v dokumente s vami, priblíženie, **Stiahnuť ako PDF** a
**Uložiť**. Na druhej je to, po čom siahate najčastejšie: späť a znova,
štýl odseku a veľkosť písma, tučné, kurzíva, podčiarknuté, zoznamy,
zarovnanie, odkazy, tabuľky a komentáre. Tabuľka má namiesto toho formáty
čísel, zoraďovanie a nové riadky a stĺpce, prezentácia nové snímky a
**Prezentovať**. Všetko ostatné je v ponuke po kliknutí pravým tlačidlom v
dokumente.

Lišty sa riadia svetlou alebo tmavou témou zariadenia. Strany dokumentu
zostávajú v oboch biele, tak ako sa vytlačia. Dokument, ktorý smiete iba
čítať, sa otvorí so štítkom **Len na čítanie** a bez lišty formátovania.
Editor funguje na adrese https zariadenia, tej istej, ktorú používa LosOS
Login; stránka otvorená cez http sa tam najprv sama presunie.

Editor ukladá priebežne a aj keď odídete, a **Uložiť** zapíše vaše zmeny
hneď. V LosOS Cloud každé uloženie zachová predchádzajúci obsah ako verziu
súboru a aplikácie na synchronizáciu vo vašich počítačoch si vezmú nový.

![Textový dokument otvorený v LosOS Office: nad ním lišty zariadenia s cestou späť, názvom, priblížením, stiahnutím ako PDF a uložením, pod nimi lišta formátovania.](@site/docs/img/office-document-sk.png)

## Spoločná práca \{#working-together}

Keď ten istý dokument otvorí druhý človek, ste obaja v tej istej kópii.
Každý vidí kurzor a meno toho druhého a každá zmena sa na druhej obrazovke
objaví hneď, ako vznikne. Nikto nečaká, kým druhý skončí, a jedno uloženie
zachová zmeny všetkých.

## Dokumenty a prezentácie v Markdowne \{#markdown-documents-and-presentations}

**Nový dokument** a **Nová prezentácia** v lište súborov LosOS Cloud
vytvoria v priečinku, v ktorom ste, súbor Markdown s názvom *Dokument bez
názvu* alebo *Prezentácia bez názvu* a otvoria ho v LosOS Office. Názov
zmeníte kliknutím naň hore. Súbory Markdown sú vlastné LosOS, takže sa
otvoria, aj keď Collabora nie je zapnutá, a každý súbor `.md` ponúka v
ponuke pravého tlačidla **Otvoriť v LosOS Office**.

Lišta nad textom má tlačidlo na všetko, čo Markdown vie zapísať, v podobe,
ktorú čítajú GitHub aj Obsidian: nadpisy, tučné písmo, kurzívu,
prečiarknutie, zvýraznenie, kód, dolný a horný index, klávesy, skryté
komentáre, odrážkové, číslované a úlohové zoznamy, citácie, bloky kódu,
vzorce v riadku aj samostatne, tabuľky, zbalené časti, vodorovné čiary,
upozornenia, odkazy, obrázky, odkazy na poznámky a vložené poznámky,
štítky, ciele odkazov na blok, poznámky pod čiarou a vlastnosti. Najprv
označte text a tlačidlo ho naformátuje; ďalším stlačením formát zmizne.
Fungujú aj **Ctrl B**, **Ctrl I**, **Ctrl K** a **Ctrl E**.
Nad označeným textom sa objaví aj malá lišta s najčastejšími formátmi,
odkazmi, zmienkami a **Opýtať sa AI**; pozrite
[lištu nad výberom](admin-pages#the-bar-over-a-selection).

Dokument sa píše vľavo a číta vpravo, vykreslený tak ako v Poznámkach.
**Písať** a **Čítať** dajú jednej strane celú šírku. Odkazy na iné poznámky
a obrázky sa hľadajú v tom istom priečinku alebo v trezore Obsidianu, v
ktorom súbor je. Písanie je spoločné ako v každom textovom súbore LosOS
Cloud: všetci v súbore píšu naraz a box uloží text chvíľu po tom, ako
písanie prestane.

Prezentácia je súbor Markdown s `slides: true` vo vlastnostiach. Každý
riadok `---` začína novú snímku. Každá snímka má rovnaké rozloženie: text v
strede, vycentrovaný, a obrázky vpravo. Snímky sú vypísané vľavo a snímka,
v ktorej je kurzor, sa vpravo ukazuje tak, ako bude vyzerať. **Prezentovať**
ju prehrá na celú obrazovku; šípky, medzerník a Page Down ňou posúvajú a
Escape ju ukončí. Tlačidlo PDF vytlačí dokument ako strany a prezentáciu po
jednej snímke na stranu.

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
LosOS Office upravuje sám, namiesto dokumentov v LosOS Cloud. V bočnom paneli sa potom za
**Aplikácie** objaví **Dokumenty** pre každého, kto sa prihlási. Vypíše
dokumenty v priečinku aj v priečinkoch v ňom a každý otvorí v editore. Ako
ostatné stránky správy odpovedá iba vo vlastnej sieti zariadenia. Je
určený pre zariadenie, ktoré drží jeden priečinok dokumentov pre ľudí vo
svojej sieti. Inak ho nechajte prázdny.
