---
title: Úprava stránok
sidebar_position: 5.1
slug: /manual/pages
---

# Úprava stránok

Každá stránka administrácie je napísaná v MDX: Markdown s niekoľkými
značkami. Správca môže zmeniť, čo stránka hovorí a v akom poradí ukazuje
svoje časti, pridať vlastné tlačidlá a dať všetkým stránkam štýly. Stránka,
ktorú nikto nezmenil, vyzerá presne tak, ako ju LosOS dodáva.

Zatiaľ sa dajú meniť Prehľad a Úložisko. Ostatné obrazovky a aplikácie
pribudnú.

## Kto ich môže meniť \{#who-can-change-them}

Iba správcovia. Časť **Stránky** v **Nastavenia → Vzhľad** vidia len oni a
zariadenie zmenu od nikoho iného neprijme. Každý prihlásený vidí stránky
tak, ako ich správca nechal, v každom prehliadači, so zapnutým aj vypnutým
JavaScriptom.

## Úprava stránky \{#editing-a-page}

**Nastavenia → Vzhľad → Stránky** uvádza každú stránku s označením **Ako ju
dodáva LosOS** alebo **Zmenená**. Ceruzka vedľa nej otvorí stránku s
editorom vedľa (na telefóne pod ňou). Náhľadom je samotná stránka: čo
píšete, sa na nej hneď ukáže, a nič sa neuloží, kým nestlačíte **Uložiť**.

Stránka sa skladá z troch druhov vecí v poradí, v akom ich napíšete:

- **Text**: nadpisy, odseky, zoznamy, odkazy a ostatný Markdown.
- **Bloky stránky**: časti, ktoré kreslí LosOS, napríklad aplikácie alebo
  disk, každý na samostatnom riadku, napríklad `<Apps />`. Blok, ktorý
  vynecháte, sa nezobrazí. Editor uvádza bloky upravovanej stránky a čo
  ktorý kreslí.
- **Komponenty**: tie, ktoré môže použiť dokument LosOS (Callout, Columns,
  Steps, Badge a ďalšie), a `Button`.

Prehľad, ako ho LosOS dodáva, sú len jeho bloky:

```mdx
<BoxName />

<Applying />

<Apps />

<Glance />
```

Tá istá stránka s privítaním, tlačidlom a aplikáciami bez pásu s úložiskom
pod nimi:

```mdx
<BoxName />

## Vitajte na kancelárskom zariadení

Sú tu súbory a kód. Pred inštaláciou aplikácie sa opýtajte.

<Button href="/apps" variant="secondary">Otvoriť aplikácie</Button>

<Apps hideStorage />
```

Editor ukáže, čo je na stránke zle, s riadkom a stĺpcom, a **Uložiť** počká,
kým nie je nič. Zariadenie kontroluje každé uloženie rovnako, takže stránka,
ktorá sa nedá prečítať, sa nikdy neuloží. Na stránke nie je žiadny skript:
`import`, `export` a `{výrazy}` z MDX sú odmietnuté a kreslia sa len
uvedené značky.

## Tlačidlá \{#buttons}

`<Button href="…">Text</Button>` nakreslí jedno z vlastných tlačidiel
stránky. Adresa je cesta na tomto zariadení (`/apps`, `/settings/look`)
alebo odkaz `https:`, `http:` či `mailto:`. `variant` je `primary`
(predvolené), `secondary`, `outline`, `ghost` alebo `link`; `size` je `sm`,
`md` (predvolené) alebo `lg`. Tlačidlo je odkaz: otvorí stránku a samo nič
nezmení.

## Štýly \{#the-stylesheet}

Riadok **Štýly** otvorí editor CSS. Po uložení ich používa každá stránka a
prichádzajú po štýloch LosOS, takže vaše pravidlo vyhrá nad rovnako silným.
Každá stránka sa pomenúva na koreňovom elemente, takže
`html[data-page="overview"]` je Prehľad a `html[data-page="settings.storage"]`
je Úložisko. Text, ktorý ste napísali, je vnútri `.mdx-page-text`. Kým je
editor otvorený, každá stránka, ktorú navštívite, ukazuje štýly, ktoré
práve píšete.

## História a návrat \{#history-and-going-back}

Každé uloženie, návrat aj zmena štýlov je commit na vaše meno vo vlastnom
repozitári Git na zariadení: `/var/lib/losos/pages`, oddelene od
konfigurácie zariadenia. **História** v editore uvádza commity stránky.
**Späť na predvolenú** vráti stránku do podoby, v akej ju LosOS dodáva, a
**Odstrániť štýly** odstráni tie vaše. Aj to sú commity, takže história
zostáva.

Keď aktualizácia LosOS zmení predvolenú podobu stránky, ktorú ste zmenili,
editor to povie. Vaša stránka zostane, ako ste ju nechali; **Otvoriť novú
predvolenú podobu** ukáže tú novú, aby ste si z nej mohli vziať, čo pribudlo.
