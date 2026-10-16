---
title: Aplikácie
sidebar_position: 4
slug: /manual/apps
---

# Aplikácie

## LosOS cloud \{#losos-cloud}

Vaše súbory, fotky, kalendár, kontakty, poznámky, úlohy, hudba a ostatné
aplikácie nižšie, na `http://<address>/nextcloud`. Je to Nextcloud vo
farbách a s menom zariadenia. Prihlásite sa heslom vlastníka. LosOS cloud
dokáže všetko, čo Nextcloud. Desktopový klient synchronizuje priečinok na
vašom počítači, aplikácie v telefóne nahrávajú fotky a kalendáre a kontakty
sa synchronizujú cez CalDAV a CardDAV.

Prehľad má dlaždicu pre každú aplikáciu z ponuky LosOS cloud, v rovnakom
poradí: Súbory, Nástenka, Fotky, Aktivita, Kontakty, Kalendár, Poznámky,
Záložky, Deck, Hudba, Kolektívy, Ankety, Formuláre, Tabuľky, Memories,
Správy, Úlohy a Mapy. Zariadenie ich pri každom štarte všetky zapne. Pošta
medzi nimi nie je, pretože zariadenie nemá poštový server, ktorý by mohla
čítať. Ak si Poštu aj tak nainštalujete z obchodu s aplikáciami, jej dlaždica
sa na Prehľade objaví pri ďalšom načítaní stránky. Odkazy vedú zámerne cez
`index.php`, pretože na túto cestu odpovedá každá konfigurácia.

Účty pre rodinu alebo kolegov zakladáte vnútri LosOS cloud, v časti
Používatelia, nie na správcovských stránkach. Správcovské stránky odomkne iba
účet vlastníka.

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
počítať. Federáciu úplne vypnete cez `losos.forgejo.federation.enable` na
paneli **Rozšírené**.

Federuje aj LosOS cloud: súbory a kalendáre môžete zdieľať s ľuďmi, ktorí
majú účty na iných serveroch Nextcloud, podľa ich adresy `user@server`.
`losos.nextcloud.federation.enable` to vypne.

Oba druhy federácie fungujú iba vtedy, keď zariadenie zdieľa svoj disk
(`losos.sharingMyStorage`), pretože iba vtedy je zdieľaná časť disku
odomknutá. So zdieľaním vypnutým sa zariadenie nerozpráva so žiadnym iným
serverom a panel **Rozšírené** ukazuje oba prepínače federácie ako
nedostupné.

## Panel Aplikácie \{#the-apps-pane}

Pri každej aplikácii panel ukazuje, či je zapnutá, v akom
[režime](../types/app-modes) beží, a odkaz. Pod tým **Nájsť ďalšie** hľadá
Helm charty v [Artifact Hub](https://artifacthub.io). Výsledky sťahuje
zariadenie, takže vyhľadávanie potrebuje internet. Každý riadok uvádza, kto
chart zverejnil. Nikto na strane LosOS žiadny z nich neskontroloval.

![Panel Aplikácie s jednou nainštalovanou aplikáciou nad Nájsť ďalšie a výsledkami vyhľadávania v Artifact Hub, každý s tým, kto ho zverejnil, a tlačidlom Inštalovať alebo Zmeniť.](@site/docs/img/apps-find-more.png)

## Nastavenia pre všetkých \{#settings-for-everyone}

Pod dvoma aplikáciami má panel Aplikácie nastavenia, ktoré platia pre každý
účet v LosOS cloud a LosOS Git. Inak by ste ich našli na vlastných
administrátorských stránkach každej aplikácie.

Pre LosOS cloud: jazyk prihlasovacej stránky a účtov, ktoré si žiadny
nevybrali, krajina, do ktorej patrí telefónne číslo bez predvoľby krajiny,
či sa súbory dajú zdieľať verejným odkazom, či každý odkaz potrebuje heslo,
po koľkých dňoch odkazy prestanú fungovať, koľko miesta dostane nový účet a
ako dlho sa uchováva kôš a staré verzie. Prázdne pole nechá voľbu na LosOS
cloud.

Pre LosOS Git: či niečo vidia iba prihlásené účty, čo návštevník uvidí ako
prvé, či nový repozitár začína ako súkromný, verejný alebo ako posledný
repozitár účtu, či je e-mailová adresa nového účtu skrytá a či push do
repozitára, ktorý ešte neexistuje, ho vytvorí.

![Panel Aplikácie s nastaveniami pre všetkých v LosOS cloud a LosOS Git.](@site/docs/img/apps-site-settings.png)

Ako každé iné nastavenie ich použijete tlačidlom **Použiť**. Zariadenie ich
nastaví znova pri každom štarte aplikácie, takže vlastné administrátorské
stránky aplikácií tieto hodnoty ukazujú, ale zmeniť ich nedokážu. Vlastné
nastavenia každého účtu, zoznam účtov a všetko, čo na paneli nie je,
zostáva v aplikácii.

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

![Inštalačný dialóg: názov, voľba medzi notshared a shared so sivým shared a vlastnosti chartu zbalené podľa sekcií.](@site/docs/img/apps-install.png)

Druhý krok sa ešte raz opýta. Hovorí, že zariadenie stiahne a spustí kód,
ktorý napísal vydavateľ, ako používateľ, ktorého ste vybrali, bez
administrátorských práv. Kým tam nestlačíte **Inštalovať**, zariadenie nič
nedostane.

Zariadenie potom nainštaluje chart do vlastného klastra a upraví ho tak, aby
bežal rovnako ako LosOS cloud a LosOS Git.

- Každá časť aplikácie beží ako vybraný používateľ, nikdy ako root, bez
  ďalších oprávnení a bez prístupu ku klastru.
- Úložisko, o ktoré chart žiada, sa stane priečinkami pod
  `/home/<user>/data/apps/<name>`.
- Aplikácia dostane jeden vlastný port z `losos.apps.ports`, predvolene 30000
  až 30099, a dostane sa k nemu iba vaša lokálna sieť.

Chart, ktorý potrebuje viac, zariadenie odmietne a povie prečo. Typickými
dôvodmi sú administrátorské práva, port pod 1024, port, ktorý už používa
iná aplikácia, alebo systémový priečinok. Aplikácia zložená z viacerých
služieb, ktoré sa navzájom volajú menom, sa nemusí spustiť, pretože vlastný
klaster zariadenia nemá vnútornú sieť.

**Nainštalované aplikácie**, nad vyhľadávaním, vypisuje každú aplikáciu s jej
stavom. **Otvoriť** prejde na port aplikácie na adrese, ktorú práve
používate. **Zmeniť** otvorí ten istý dialóg s aktuálnymi vlastnosťami
aplikácie. **Odstrániť** sa najprv opýta a potom aplikáciu zo zariadenia
odstráni. Jej priečinok s dátami zostane, takže inštalácia pod tým istým
názvom dáta znova nájde.

Vyhľadávanie ponúka inštaláciu iba vtedy, keď sú Súbory alebo Kód nastavené
na **Oddelene**, čo je [režim záťaže](../types/app-modes). Iba vtedy beží
vlastný klaster zariadenia. Inštalovanie vypnete nastavením
`losos.apps.enable` na false na paneli **Rozšírené**.

## Dôveryhodné adresy \{#trusted-addresses}

LosOS cloud odpovedá na meno zariadenia a na tú adresu, na ktorú prišla
požiadavka. Zariadenie túto adresu posiela spolu s každou požiadavkou. LosOS
cloud nikdy neodpovedá na zástupný znak. Preto zariadenie, na ktoré
pristupujete cez IP adresu, neukáže stránku o nedôveryhodnej doméne. Ak ju
niekedy uvidíte, [Nedôveryhodná doména](../troubleshooting/app-tile-404#access-through-untrusted-domain)
vysvetľuje prečo.
