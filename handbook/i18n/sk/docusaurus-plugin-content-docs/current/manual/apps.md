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

Vaše repozitáre, na `http://<address>/forgejo/`. Je to Forgejo v tých istých
farbách. Pri prvom použití sa v ňom zaregistrujte, vedie si vlastné účty.
Adresy na klonovanie používajú meno zariadenia, takže počítač, ktorý
nepreloží `.local`, by mal použiť adresu. Ak zariadenie nie je na kód,
vypnite LosOS Git na paneli **Aplikácie**.

### Federácia \{#federation}

LosOS Git sa cez ActivityPub rozpráva s inými servermi Forgejo. Forgejo to
stále označuje za experimentálne. Hviezdičky, ktoré vašim repozitárom dajú
ľudia na inom serveri, sa započítajú aj tu. Ľudia na iných serveroch môžu
sledovať účet na vašom zariadení a vidieť, čo robí. Zariadenie odpovedá aj
na adrese, cez ktorú fediverse hľadá servery, `/.well-known/nodeinfo`. To
všetko siaha tak ďaleko ako samotné zariadenie. Cez [edge](../types/official-edge)
je to internet. Na zariadení, ktoré je iba vo vašej lokálnej sieti, sú to
ostatné zariadenia v tej sieti. Zariadenie nikdy nezverejňuje, koľko má
účtov ani aké sú aktívne.

Ak sa chcete zapojiť, vypíšte na stránke repozitára
**Nastavenia → Federácia** ostatné servery, ktorých hviezdičky sa majú
počítať. Federáciu úplne vypnete prepínačom **Federovať s inými servermi
Forgejo** v sekcii LosOS Git na paneli **Aplikácie**.

:::note[Federácia potrebuje zdieľanie disku]
Federácia funguje iba vtedy, keď zariadenie zdieľa svoj disk
(`losos.sharingMyStorage`), pretože iba vtedy je zdieľaná časť disku
odomknutá. So zdieľaním vypnutým sa zariadenie nerozpráva so žiadnym iným
serverom a panel **Aplikácie** ukazuje prepínač federácie sivý.
:::

## Panel Aplikácie \{#the-apps-pane}

Pri každej aplikácii panel ukazuje, či je zapnutá, v akom
[režime](../types/app-modes) beží, a odkaz. Pod nimi majú LosOS Cloud aj
LosOS Git vlastnú rozbaľovaciu sekciu, zavretú, kým ju neotvoríte. Pod tým
**Nájsť ďalšie** hľadá
Helm charty v [Artifact Hub](https://artifacthub.io). Výsledky sťahuje
zariadenie, takže vyhľadávanie potrebuje internet. Každý riadok uvádza, kto
chart zverejnil. Nikto na strane LosOS žiadny z nich neskontroloval.

![Panel Aplikácie s jednou nainštalovanou aplikáciou nad Nájsť ďalšie a výsledkami vyhľadávania v Artifact Hub, každý s tým, kto ho zverejnil, a tlačidlom Inštalovať alebo Zmeniť.](@site/docs/img/apps-find-more-sk.png)

## Nastavenia pre všetkých \{#settings-for-everyone}

V sekciách LosOS Cloud a LosOS Git má panel Aplikácie nastavenia, ktoré
platia pre každý účet v danej aplikácii.

Pre LosOS Cloud: koľko dní kôš drží vymazaný súbor. Prázdne pole ho drží
30 dní.

Pre LosOS Git, po jeho prepínači federácie ([Federácia](#federation)): či niečo vidia iba prihlásené účty, čo návštevník uvidí ako
prvé, či nový repozitár začína ako súkromný, verejný alebo ako posledný
repozitár účtu, či je e-mailová adresa nového účtu skrytá a či push do
repozitára, ktorý ešte neexistuje, ho vytvorí.

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
  až 30099, a dostane sa k nemu iba vaša lokálna sieť.

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

:::note[Beh ako shared potrebuje režim záťaže]
Aplikácia `notshared` sa nainštaluje na každom zariadení. `shared` potrebuje
vlastný klaster zariadenia, ktorý beží iba vtedy, keď je Kód nastavený na
**Oddelene** ([režim záťaže](../types/app-modes)), a
zariadenie musí zdieľať svoj disk. Inštalovanie vypnete nastavením
`losos.apps.enable` na false na paneli **Rozšírené**.
:::

## Aplikácie vytvorené pre zariadenie \{#apps-made-for-the-box}

Aplikácie môžu prísť aj ako súbor Compose vo
[formáte aplikácií LosOS](../in-depth/app-format), ktorý napíšete vy alebo
ktorý za vás pošle nejaký program. Bežia ako
`notshared`, v **Nainštalovaných aplikáciách** majú riadok „vytvorená pre
toto zariadenie“ a majú **Otvoriť** a **Odstrániť**, ale nie **Zmeniť**,
pretože nemajú vlastnosti.
