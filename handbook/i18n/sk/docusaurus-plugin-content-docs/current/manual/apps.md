---
title: Aplikácie
sidebar_position: 4
slug: /manual/apps
---

# Aplikácie

## LosOS Cloud \{#losos-cloud}

Vaše súbory, fotky, kalendáre, kontakty a tabule, na
`https://<address>/cloud/`. Každý, kto sa môže prihlásiť cez LosOS Login
([Prihlásenie](sign-in-and-spare-key)), má vlastné. Klient Nextcloud pre
počítač a aplikácie pre telefón sa s ním synchronizujú a kalendárové
aplikácie sa k nemu dostanú cez CalDAV a CardDAV. Podrobnosti sú v
[LosOS Cloud](losos-cloud). Na Prehľade ho otvoria dlaždice **Súbory** a
**Fotky**.

## LosOS Git \{#losos-git}

Vaše repozitáre, na `http://<address>/git/<účet>/<repozitár>.git`. LosOS
Git je vlastný Git server zariadenia. Človek, ktorého pozná LosOS Login, v
ňom má účet od prvej návštevy, pod tým istým menom. Počítač klonuje a
pushuje s vlastným tokenom, ktorý si git pýta ako heslo: vyrobí ho
`losos git setup` ([Terminálový klient](terminal)). Vlastná konfigurácia
zariadenia je súkromný repozitár `notshared/losos-config`
([Správa](../in-depth/administration)). Adresy na klonovanie sa riadia
adresou, cez ktorú ste sa k zariadeniu dostali. Ak zariadenie nie je na kód, vypnite LosOS Git na paneli
**Aplikácie**.

## Panel Aplikácie \{#the-apps-pane}

Pri každej aplikácii panel ukazuje, či je zapnutá, a odkaz. Pod nimi majú
LosOS Cloud aj LosOS Git vlastnú rozbaľovaciu sekciu, zavretú, kým ju
neotvoríte. Pod tým **Nájsť ďalšie** hľadá
Helm charty v [Artifact Hub](https://artifacthub.io). Výsledky sťahuje
zariadenie, takže vyhľadávanie potrebuje internet. Každý riadok uvádza, kto
chart zverejnil. Nikto na strane LosOS žiadny z nich neskontroloval.

![Panel Aplikácie s jednou nainštalovanou aplikáciou nad Nájsť ďalšie a výsledkami vyhľadávania v Artifact Hub, každý s tým, kto ho zverejnil, a tlačidlom Inštalovať alebo Zmeniť.](@site/docs/img/apps-find-more-sk.png)

## Nastavenia pre všetkých \{#settings-for-everyone}

V sekciách LosOS Cloud a LosOS Git má panel Aplikácie nastavenia, ktoré
platia pre každý účet v danej aplikácii.

Pre LosOS Cloud: koľko dní kôš drží vymazaný súbor. Prázdne pole ho drží
30 dní.

Pre LosOS Git: či niečo vidia iba prihlásené účty, či nový repozitár
začína ako súkromný, verejný alebo ako posledný repozitár účtu a či push
do repozitára, ktorý ešte neexistuje, ho vytvorí.

![Panel Aplikácie s nastaveniami pre všetkých v LosOS Cloud a LosOS Git.](@site/docs/img/apps-site-settings-sk.png)

Ako každé iné nastavenie ich použijete tlačidlom **Použiť**. Zariadenie ich
nastaví znova pri každom štarte aplikácie, takže zmena urobená vnútri
aplikácie nevydrží. Vlastné nastavenia každého účtu zostávajú v aplikácii.

## Inštalácia aplikácie z vyhľadávania \{#installing-an-app-from-the-search}

Riadok, ktorý zariadenie dokáže stiahnuť, má tlačidlo **Inštalovať**.
Otvorí dialóg v dvoch krokoch.

Prvý krok prečíta chart a ukáže tri veci.

- **Názov na tomto zariadení.** Začína ako názov chartu. Pomenúva aj
  priečinok aplikácie, takže ho neskôr zmeniť nemôžete.
- **Spustiť ako.** `notshared` alebo `shared`, dvaja používatelia
  zariadenia. Aplikácia `notshared` drží svoje dáta na strane disku, ktorá
  patrí vlastníkovi, vedľa vašich súborov. Aplikácia `shared` ich drží na
  strane, ktorú zariadenie požičiava do mesh. Kým zariadenie nezdieľa svoj
  disk, dialóg má `shared` sivé.
- **Vlastnosti.** Každé nastavenie zo súboru hodnôt chartu s jeho
  predvolenou hodnotou, zbalené podľa sekcií. Keď chart dodáva schému, každé
  nastavenie dostane aj svoj opis a možnosti. **Ako text** namiesto toho
  ukáže na úpravu celý súbor hodnôt. Vlastnosť zmenená vo formulári má
  prednosť pred tým istým riadkom v texte.

![Inštalačný dialóg: názov, voľba medzi notshared a shared so sivým shared a vlastnosti chartu zbalené podľa sekcií.](@site/docs/img/apps-install-sk.png)

Druhý krok sa ešte raz opýta. Hovorí, že zariadenie stiahne a spustí kód,
ktorý napísal vydavateľ, ako používateľ, ktorého ste vybrali, bez
administrátorských práv. Kým tam nestlačíte **Inštalovať**, zariadenie nič
nedostane.

Čo aplikáciu spúšťa, závisí od používateľa. Aplikácia `notshared` beží
na Podmane bez roota, priamo ako `notshared`, a nepotrebuje žiadny klaster.
Aplikácia `shared` beží vo vlastnom klastri zariadenia, pretože to je časť
zariadenia, ktorej sa dotýka sieť. V oboch prípadoch zariadenie chart
upraví tak, aby bežal rovnako ako LosOS Git.

- Každá časť aplikácie beží ako vybraný používateľ, nikdy ako root, bez
  ďalších oprávnení a bez prístupu ku klastru.
- Úložisko, o ktoré chart žiada, sa stane priečinkami pod
  `/home/<user>/data/apps/<name>`.
- Aplikácia dostane jeden vlastný port z `losos.apps.ports`, predvolene 30000
  až 30099, a dostane sa k nemu iba vaša lokálna sieť. Port sa otvorí, len
  čo aplikácia beží, a zatvorí sa, keď sa zastaví.

:::note[Charty, ktoré zariadenie odmietne]
Chart, ktorý potrebuje viac, zariadenie odmietne a povie prečo. Typickými
dôvodmi sú administrátorské práva, port pod 1024, port, ktorý už používa
iná aplikácia, alebo systémový priečinok. Služby aplikácie `notshared` sa
navzájom nájdu menom priamo na zariadení, takže musia počúvať na rôznych
portoch. Aplikácia `shared` zložená z viacerých služieb, ktoré sa navzájom
volajú menom, sa nemusí spustiť, pretože vlastný klaster zariadenia nemá
vnútornú sieť. Plánovaná úloha (CronJob) beží iba ako `shared`.
:::

**Nainštalované aplikácie**, nad vyhľadávaním, vypisuje každú aplikáciu s jej
stavom. **Otvoriť** prejde na port aplikácie na adrese, ktorú práve
používate. **Zmeniť** otvorí ten istý dialóg s aktuálnymi vlastnosťami
aplikácie. **Odstrániť** sa najprv opýta a potom aplikáciu zo zariadenia
odstráni. Jej priečinok s dátami zostane, takže inštalácia pod tým istým
názvom dáta znova nájde.

:::note[Beh ako shared potrebuje mesh]
Aplikácia `notshared` sa nainštaluje na každom zariadení. `shared` potrebuje
vlastný klaster zariadenia, ktorý beží iba vtedy, keď zariadenie zdieľa
disk s mesh ([Kde bežia aplikácie](../types/app-modes)). Inštalovanie vypnete nastavením
`losos.apps.enable` na false na paneli **Rozšírené**.
:::

## Štatistiky \{#statistics}

**Nastavenia › Štatistiky** ukazujú, ako často niekto otvoril vaše
nainštalované aplikácie. Vidíte žiadosti za poslednú hodinu, minútu po
minúte, a koľko z nich skončilo chybou. Vidíte aj to, koľko zariadenie
odoslalo a ako dlho trvali najpomalšie odpovede. Každá aplikácia je
uvedená so svojím portom. Počítadlá začínajú odznova, keď sa zariadenie
reštartuje.

Widget **Štatistiky** ukáže poslednú hodinu na Prehľade. Pridáte ho cez
**Pridať widget**. Kliknutím naň otvoríte celú stránku.

## Vaše aplikácie \{#your-apps}

Malé vlastné aplikácie, nákupný zoznam, rozpis domácich prác, zoznam kníh,
ktoré opíšete a AI na zariadení napíše
([Vytvoriť s AI](../in-depth/widget-builder)). Sú pod **Vaše aplikácie** na
stránke Aplikácie. **Vytvoriť aplikáciu** otvorí editor: aplikáciu opíšte na
karte **Vytvoriť s AI** alebo napíšte jej kód na karte **Kód**. Náhľad vedľa
je skutočná aplikácia, takže ju môžete používať ešte pred uložením. Čo do
nej zadáte, zabudne; uložená aplikácia si údaje drží na zariadení.

Každá uložená aplikácia má vlastnú stránku. Aplikácia je jeden komponent
React zložený z komponentov LosOS, ktorý zariadenie pri uložení skontroluje
a preloží. Beží len vo vašom prehliadači, v sandboxovanom rámci bez prístupu
k zariadeniu okrem vlastných údajov. **Upraviť** ju znova otvorí v editore a
**Odstrániť** ju zo zariadenia zmaže so všetkým, čo sa do nej zadalo.

Každá aplikácia má aj vlastný súkromný repozitár v LosOS Git, pomenovaný
podľa nej (`app-chores` pre aplikáciu s názvom Chores), a každé uloženie je
v ňom commit. **Zdroj** na stránke aplikácie ho otvorí. Naklonujte ho,
upravte `app.jsx` a pošlite zmenu (push): do minúty ju zariadenie
skontroluje tak ako pri uložení a aplikácia ju spustí. Ak sa zmena nedá
preložiť, aplikácia zostane taká, aká bola, jej stránka povie prečo a
zariadenie pridá commit, ktorý aplikáciu vráti, takže to uvidíte aj pri
ďalšom `git pull`. Súbory, ktoré do repozitára pridáte, v ňom zostanú, a
údaje aplikácie v ňom nie sú. Keď aplikáciu odstránite, repozitár v LosOS
Git zostane; ak ho nechcete, zmažte ho tam. Keď je LosOS Git vypnutý,
verzie si drží zariadenie samo.

## Aplikácie vytvorené pre zariadenie \{#apps-made-for-the-box}

Aplikácie môžu prísť aj ako súbor Compose vo
[formáte aplikácií LosOS](../in-depth/app-format), ktorý napíšete vy alebo
ktorý za vás pošle nejaký program. Bežia ako
`notshared`, v **Nainštalovaných aplikáciách** majú riadok „vytvorená pre
toto zariadenie“ a majú **Otvoriť** a **Odstrániť**, ale nie **Zmeniť**,
pretože nemajú vlastnosti.
