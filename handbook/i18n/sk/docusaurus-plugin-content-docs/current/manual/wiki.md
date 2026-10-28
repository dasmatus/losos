---
title: LosOS Wiki
sidebar_position: 4.65
slug: /manual/wiki
---

# LosOS Wiki

LosOS Wiki uchováva dokumentáciu tak ako tímová wiki: priestory so
stránkami v strome, písané v prehliadači, a každý priestor zverejnený aj
ako samostatná stránka. **Wiki** v bočnom paneli ju otvorí na adrese
`/wikis`.

Je to aj zoznam všetkých wiki na boxe. Jeden zoznam ukazuje každú wiki na
boxe, ktorú smiete čítať:

- **Priestory**: wiki repozitárov v LosOS Git, ktoré sa píšu tu.
- **Znalostné bázy**: priečinky poznámok zverejnené z
  [Poznámok v LosOS Cloud](notes-and-knowledge-bases). Píšu sa v
  Poznámkach; zoznam odkazuje na ich stránky.
- **Vstavané**: táto príručka.

Hľadanie nad zoznamom prejde stránky všetkých priestorov a znalostných báz
naraz a tie isté slová ponúkne aj vyhľadávaniu v príručke.

![Zoznam wiki v LosOS Wiki: priestory boxu, znalostná báza a príručka, nad nimi jedno hľadanie.](@site/docs/img/wiki-directory-sk.png)

## Priestory \{#spaces}

Priestor je wiki repozitára v [LosOS Git](losos-git). Jeho stránky sú
súbory v Markdowne vo vlastnom repozitári Git vedľa kódu,
`<repozitár>.wiki.git`, v rovnakom usporiadaní ako vo Forgejo a na GitHube.
Box, ktorý prešiel z Forgejo, tam svoje wiki už má a zobrazia sa ako
priestory bez akéhokoľvek importu.

Kto smie čo, sa riadi repozitárom:

| Repozitár je | Kto priestor číta | Kto doň píše |
| --- | --- | --- |
| Verejný | Ktokoľvek s adresou | Ľudia, ktorí doň smú posielať zmeny |
| Pre ľudí na boxe | Každý prihlásený na boxe | Ľudia, ktorí doň smú posielať zmeny |
| Súkromný | Jeho vlastník a spolupracovníci | Ľudia, ktorí doň smú posielať zmeny |

**Nový priestor** vytvorí repozitár s daným názvom a zvolenou viditeľnosťou
a napíše jeho prvú stránku. Ak chcete dať wiki existujúcemu repozitáru,
otvorte ho v LosOS Git a v jeho ponuke zvoľte **Wiki**.

## Stránky \{#pages}

![Priestor otvorený na domovskej stránke, vľavo strom stránok.](@site/docs/img/wiki-space-sk.png)

- Každá stránka je jeden súbor. „Getting started“ je `Getting-started.md`.
- Stránka pod inou stránkou je súbor v priečinku: **Nová stránka pod touto**
  na stránke `Guides` vytvorí `Guides/<názov>.md`. Strom vľavo sleduje
  priečinky.
- Ako prvá sa otvorí `Home`. Ak chýba, otvorí sa prvá stránka.
- `_Sidebar.md` a `_Footer.md` lemujú každú stránku vedľa nich, z boku a
  zdola. Priečinok môže mať vlastné.
- `[[Stránka]]` a `[[Stránka|text]]` odkazujú na stránku toho istého
  priestoru, rovnako ako obyčajný odkaz na `Stránka` alebo `Stránka.md`. V
  priečinku sa odkazy počítajú od toho priečinka.
- Obrázky uložené v priestore (PNG, JPEG, GIF, WebP alebo AVIF) sa na
  stránke zobrazia: `![Záhony](beds.png)`. Obrázky z iných webov zostanú
  odkazmi.

**Upraviť** otvorí stránku v editore Markdownu s náhľadom. Každé uloženie
je commit v histórii priestoru s vaším menom a nepovinnou poznámkou, čo sa
zmenilo; **Zmeny** pod stránkou ich vypíšu. Ak stránku medzitým uložil
niekto iný, uloženie sa odmietne a váš text zostane v editore, takže
stránku môžete otvoriť znova a doplniť ho. **Odstrániť stránku** odstráni
súbor tiež commitom, takže text zostane v histórii.

## S Gitom \{#with-git}

Každý priestor je repozitár Git. Naklonujte ho adresou, ktorá je pod
stromom stránok:

```sh
git clone https://<box>/git/<vlastník>/<repozitár>.wiki.git
```

Na push treba tie isté práva a ten istý token ako na push do samotného
repozitára (pozri [LosOS Git](losos-git)). Prvý push na adresu wiki
repozitára priestor vytvorí. Stránky napísané v prehliadači a stránky
poslané Gitom sú tie isté súbory.

## Zverejnená stránka \{#the-published-site}

![Priestor garden ako zverejnená stránka.](@site/docs/img/wiki-site-sk.png)

Každý priestor je aj stránka na adrese
`https://<box>/wiki/<vlastník>/<repozitár>/`, podobne ako stránky projektu
na GitHub Pages: strom stránok, stránka a hľadanie v priestore, bez
možnosti niečo zmeniť. Je otvorená každému, kto smie priestor čítať, takže
verejný priestor sa dá čítať bez prihlásenia, aj cez edge. Kto smie písať,
vidí **Upraviť**, ktoré stránku otvorí v LosOS Wiki.
`https://<box>/wiki/` vypíše všetky priestory, ktoré smiete čítať.

Keď LosOS Git drží svoje stránky za LosOS Login
(`losos.git.site.requireSignIn`), sú za ním aj zverejnené stránky.
