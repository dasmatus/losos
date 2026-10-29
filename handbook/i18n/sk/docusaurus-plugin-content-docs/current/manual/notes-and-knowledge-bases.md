---
title: Poznámky a znalostné bázy
sidebar_position: 4.42
slug: /manual/notes-and-knowledge-bases
---

# Poznámky a znalostné bázy

**Poznámky** v [LosOS Cloud](losos-cloud) otvárajú priečinok poznámok,
súborov `.note` a `.md`, tak, ako Obsidian otvára trezor (vault): poznámky vľavo, otvorenú
poznámku v strede a vedľa nej poznámky, ktoré na ňu odkazujú. Trezor
Obsidianu synchronizovaný do LosOS Cloud sa otvorí taký, aký je, s
odkazmi, vloženými časťami, štítkami aj plátnami (canvas). Kým zariadenie
zdieľa a je v sieti zariadení (mesh), môžete priečinok poznámok zverejniť
aj ako znalostnú bázu: stránku, ktorú ľudia čítajú v prehliadači.

## Otvorte svoje poznámky \{#open-your-notes}

- **Poznámky** v bočnom paneli otvoria priečinok `Notes` vo vašich
  súboroch. Vytvorí sa pri prvom otvorení.
- V časti **Súbory** kliknite pravým tlačidlom na ľubovoľný priečinok a
  vyberte **Otvoriť ako poznámky**.
- V časti **Súbory** kliknite na súbor `.note` alebo `.md`. Otvorí sa vo
  svojom trezore:
  v najbližšom nadradenom priečinku, v ktorom je priečinok `.obsidian`,
  ako trezor hľadá Obsidian, inak v priečinku, v ktorom súbor je.

## .note a .md \{#note-and-md}

Nová poznámka je súbor `.note`. Súbor `.note` je MDX: Markdown, ktorý môže
obsahovať aj komponenty, písané ako značky HTML. Poznámka môže použiť
tieto, rovnaké ako dokumenty v LosOS Office:

| Komponent | Čo vykreslí |
| --- | --- |
| `<Callout type="tip" title="…">` | Rámček vo farbe svojho typu: `note`, `tip`, `important`, `warning` alebo `caution` |
| `<Details summary="…">` | Časť, ktorá sa otvorí kliknutím |
| `<Tabs>` s `<Tab label="…">` | Časti čítané jedna za druhou, každá pod svojím názvom |
| `<Columns>` s `<Column>` | Časti vedľa seba, na telefóne pod sebou |
| `<Figure src="…" caption="…">` | Obrázok s popisom |
| `<TableOfContents />` | Nadpisy poznámky ako odkazy |
| `<Table>`, `<Row>`, `<Cell>` | Tabuľka so zlúčenými bunkami a popisom |
| `<Mention login="…" />`, `<Date value="2026-08-14" />` | Človek a dátum |
| `<Equation tex="…" />` | Matematika na samostatnom riadku |

`<kbd>`, `<sub>`, `<sup>`, `<mark>`, `<u>`, `<s>`, `<small>`, `<abbr>` a
`<br />` fungujú ako v HTML.

Poznámka nikdy nespúšťa kód. MDX by súboru dovolilo niesť JavaScript, a
preto ho Poznámky odmietajú celý: výrazy `{…}`, riadky `import` a
`export`, `<script>`, obsluhy udalostí ako `onClick`, odkazy `javascript:`
a každú značku či komponent, ktorý nie je v tabuľke. Čo si poznámka
pýtala a nedostala, sa vypíše nad ňou aj s riadkom a zvyšok poznámky sa
vykreslí. Súbor `.note`, ktorý nie je platné MDX, napríklad s
neuzavretým komponentom, sa vykreslí ako Markdown s riadkom, na ktorom sa
pokazil.

Poznámky `.md` zostávajú Markdownom a fungujú ako doteraz. Tlačidlo
**Previesť na .note** nad poznámkou `.md` z nej urobí `.note`, ktorá sa
číta rovnako: zložené a lomené zátvorky, ktoré by MDX čítalo ako kód, sa
zapíšu ako obyčajný text a komentáre HTML sa zmenia na `%%komentáre%%`. Ak by nadpisy
alebo odkazy poznámky vyšli inak, zostane `.md` a správa povie, na ktorom
riadku. `[[Odkazy]]` menujú poznámku bez prípony, takže fungujú ďalej.
Obsidian otvára iba súbory `.md`, preto poznámky, ktoré upravujete aj v
Obsidiane, nechajte ako `.md`.

## Čítanie poznámky \{#reading-a-note}

Poznámky rozumejú obom druhom Markdownu, ktoré ľudia píšu, v súboroch
`.note` aj `.md`:

- GitHubu: tabuľky, zoznamy úloh, ~~prečiarknutie~~, poznámky pod čiarou,
  odkazy začínajúce `https://` a upozornenia ako `> [!NOTE]`.
- Obsidianu: `[[odkazy]]` na iné poznámky, aj `[[Poznámka|zobrazený
  text]]`, `[[Poznámka#Nadpis]]` a `[[Poznámka#^blok]]`; `![[vloženie]]`
  poznámky, jej časti alebo obrázka; bloky callout, aj zbaliteľné
  (`> [!tip]-`); `#štítky` a `#vnorené/štítky`; `==zvýraznenie==`;
  `%%komentáre%%`, ktoré sa nezobrazujú; matematiku medzi `$` a `$$`; a
  vlastnosti na začiatku poznámky.

Bloky kódu sa farbia podľa jazyka. Diagramy Mermaid zostávajú kódom: na
ich vykreslenie treba štýly, ktoré stránky zariadenia nepovoľujú.

Odkaz na poznámku, ktorá ešte neexistuje, je bledší. Kliknutím ju
vytvoríte. Keď zaškrtnete úlohu, poznámka sa uloží so zaškrtnutým
políčkom.

## Vedľa poznámky \{#beside-a-note}

- **Lokálny graf**: poznámka a poznámky, na ktoré odkazuje alebo ktoré
  odkazujú na ňu, jeden až tri odkazy ďaleko. Kliknutím na bod tú
  poznámku otvoríte.
- **Odkazujú sem**: poznámky, ktoré odkazujú na túto.
- **Odkazuje na** a **Ešte nenapísané**: kam táto poznámka odkazuje.
- **Osnova**: jej nadpisy. Kliknutím na nadpis naň skočíte.

## Úpravy, myšlienková mapa a graf \{#edit-mind-map-and-graph}

- **Upraviť** otvorí poznámku v editore, do ktorého ľudia píšu spolu
  (pozrite [Zdieľanie a spoločné úpravy](sharing-and-editing-together)),
  a na širokej obrazovke je poznámka vykreslená vedľa textu. Napíšte
  `[[` a vyberte poznámku, na ktorú chcete odkázať. Keď označíte slová, v
  editore alebo pri čítaní, objaví sa nad nimi
  [lišta s akciami](admin-pages#the-bar-over-a-selection).
- **Myšlienková mapa** nakreslí nadpisy poznámky a zoznamy pod nimi ako
  vetvy. Kliknutím na vetvu nadpisu si prečítate tú časť.
- Tlačidlo **Graf všetkých poznámok** nad poznámkami ukáže každú poznámku
  v priečinku a každý odkaz medzi nimi. Prepínače pridajú štítky, prílohy,
  odkazy na ešte nenapísané poznámky a poznámky, na ktoré nič neodkazuje.
  Pole filtruje podľa názvu.

Vyhľadávacie pole nad poznámkami nájde slová v názvoch aj v texte.
**Štítky** zobrazia všetky štítky; kliknutím na štítok uvidíte jeho
poznámky.

## Zverejnite priečinok ako znalostnú bázu \{#publish-a-folder-as-a-knowledge-base}

Znalostné bázy potrebujú, aby zariadenie zdieľalo a bolo v sieti
zariadení, rovnako ako chat a pošta. Bez oboch položka v ponuke chýba a
stránky sú vypnuté.

1. V časti **Súbory** kliknite pravým tlačidlom na priečinok a vyberte
   **Zverejniť ako znalostnú bázu**.
2. Zadajte názov a vyberte, kto ju môže čítať:
   - **Ktokoľvek s odkazom** číta bez prihlásenia, všade, kde je
     zariadenie dostupné: v jeho sieti a cez jeho edge, ak ho má.
   - **Ľudia na tomto zariadení** sa najprv prihlásia cez LosOS Login.
3. Stlačte **Zverejniť**. Okno ukáže adresu stránky, napríklad
   `https://<name>.local/kb/kitchen-garden/`.

Stránka nasleduje priečinok: poznámka, ktorú zmeníte alebo pridáte, sa tam
ukáže hneď. Každá poznámka má vlastnú adresu, napríklad
`/kb/kitchen-garden/Tomatoes`. Čitatelia dostanú tie isté odkazy, vloženia,
vyhľadávanie, graf aj myšlienkovú mapu a nič, čím by poznámky zmenili.

Na stránke nie sú:

- poznámky, ktorých vlastnosti hovoria `publish: false` alebo
  `private: true`;
- priečinky, ktorých názov začína bodkou, medzi nimi `.obsidian`;
- nič mimo priečinka.

Ak chcete zmeniť názov alebo to, kto ju môže čítať, kliknite na priečinok
znova pravým tlačidlom a vyberte **Znalostná báza**. **Zrušiť
zverejnenie** stránku zruší; poznámky zostanú vo vašich súboroch.

Ak zariadenie prestane zdieľať alebo opustí sieť zariadení, žiadna stránka
neodpovedá. Vrátia sa také, aké boli, keď zariadenie znova zdieľa v sieti
zariadení.
